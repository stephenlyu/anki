// Copyright: Ankitects Pty Ltd and contributors
// License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

mod handlers;
mod logging;
mod media_manager;
mod routes;
mod user;

use std::collections::HashMap;
use std::error::Error;
use std::future::Future;
use std::future::IntoFuture;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;

use anki_io::create_dir_all;
use axum::extract::DefaultBodyLimit;
use axum::routing::get;
use axum::routing::post;
use axum::Router;
use axum::{extract::State, http::StatusCode, Json};
use axum_client_ip::SecureClientIpSource;
use snafu::ResultExt;
use snafu::Whatever;
use tokio::net::TcpListener;
use tracing::Span;
use validator::validate_email;

use crate::error;
use crate::media::files::sha1_of_data;
use crate::sync::error::HttpResult;
use crate::sync::error::OrHttpErr;
use crate::sync::http_server::logging::with_logging_layer;
use crate::sync::http_server::media_manager::ServerMediaManager;
use crate::sync::http_server::routes::collection_sync_router;
use crate::sync::http_server::routes::health_check_handler;
use crate::sync::http_server::routes::media_sync_router;
use crate::sync::http_server::user::User;
use crate::sync::login::HostKeyRequest;
use crate::sync::login::HostKeyResponse;
use crate::sync::login::RegisterRequest;
use crate::sync::login::RegisterResponse;
use crate::sync::request::SyncRequest;
use crate::sync::request::MAXIMUM_SYNC_PAYLOAD_BYTES;
use crate::sync::response::SyncResponse;
use crate::sync::user::database::{User as Account, UserDatabase};

pub struct SimpleServer {
    state: Mutex<SimpleServerInner>,
}

pub struct SimpleServerInner {
    base_folder: PathBuf,
    /// hkey->user
    users: HashMap<String, User>,
    user_db: UserDatabase,
}

#[derive(serde::Deserialize, Debug)]
pub struct SyncServerConfig {
    #[serde(default = "default_host")]
    pub host: IpAddr,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_base", rename = "base")]
    pub base_folder: PathBuf,
    #[serde(default = "default_ip_header")]
    pub ip_header: SecureClientIpSource,
}

fn default_host() -> IpAddr {
    "0.0.0.0".parse().unwrap()
}

fn default_port() -> u16 {
    8080
}

fn default_base() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| panic!("Unable to determine home folder; please set SYNC_BASE"))
        .join(".syncserver")
}

pub fn default_ip_header() -> SecureClientIpSource {
    SecureClientIpSource::ConnectInfo
}

impl SimpleServerInner {
    fn new_from_env(base_folder: &Path) -> error::Result<Self, Whatever> {
        create_dir_all(base_folder).whatever_context("new_from_env")?;
        let users: HashMap<String, User> = Default::default();
        let user_db_path = base_folder.to_path_buf().join("user.db");

        let user_db = UserDatabase::new(&user_db_path.as_path()).whatever_context("new user db")?;

        Ok(Self {
            base_folder: base_folder.to_path_buf(),
            users: users,
            user_db: user_db,
        })
    }

    fn create_account(&self, account: &Account) -> error::Result<(), Whatever> {
        self.user_db
            .add_user(account)
            .whatever_context("create account")
    }

    fn load_account_if(
        &self,
        name: &str,
        password: &str,
    ) -> error::Result<Option<Account>, Whatever> {
        self.user_db
            .verify_user(name, password)
            .whatever_context("verify user")
    }

    fn is_user_exists(&self, key: &str) -> bool {
        self.users.contains_key(key)
    }

    fn create_user(&mut self, name: &str, hkey: &str) -> error::Result<(), Whatever> {
        let folder = self.base_folder.join(name);
        create_dir_all(&folder).whatever_context("creating SYNC_BASE")?;
        let media = ServerMediaManager::new(&folder).whatever_context("opening media")?;
        self.users.entry(hkey.to_string()).or_insert(User {
            name: name.into(),
            col: None,
            sync_state: None,
            media,
            folder,
        });
        Ok(())
    }
}

// This is not what AnkiWeb does, but should suffice for this use case.
fn derive_hkey(user_and_pass: &str) -> String {
    hex::encode(sha1_of_data(user_and_pass.as_bytes()))
}

#[axum::debug_handler]
async fn register_handler(
    State(server): State<Arc<SimpleServer>>,
    Json(payload): Json<RegisterRequest>,
) -> (StatusCode, Json<RegisterResponse>) {
    let email = payload.email.trim();
    let name = payload.name.trim();
    let password = payload.password.trim();
    if password == "" {
        let response = RegisterResponse {
            status: 400,
            message: Some("empty_password".to_string()),
        };
        return (StatusCode::BAD_REQUEST, Json(response));
    }
    if !validate_email(email) {
        let response = RegisterResponse {
            status: 400,
            message: Some("bad_email".to_string()),
        };
        return (StatusCode::BAD_REQUEST, Json(response));
    }

    let state = server.state.lock().unwrap();
    let account = Account {
        id: 0,
        email: email.to_string(),
        name: if name == "" {
            None
        } else {
            Some(name.to_string())
        },
        password: Some(password.to_string()),
    };
    let ret = state.create_account(&account);

    match ret {
        Ok(_) => {
            let response = RegisterResponse {
                status: 200,
                message: Some("success".to_string()),
            };
            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            if let Some(source) = e.source() {
                if source.to_string() == "UNIQUE constraint failed: user.email" {
                    let response = RegisterResponse {
                        status: 400,
                        message: Some("account_exists".to_string()),
                    };
                    return (StatusCode::BAD_REQUEST, Json(response));
                }
            }
            
            let response = RegisterResponse {
                status: 500,
                message: Some(e.to_string()),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
        }
    }

    // 返回成功响应
}

impl SimpleServer {
    pub(in crate::sync) async fn with_authenticated_user<F, I, O>(
        &self,
        req: SyncRequest<I>,
        op: F,
    ) -> HttpResult<O>
    where
        F: FnOnce(&mut User, SyncRequest<I>) -> HttpResult<O>,
    {
        let mut state = self.state.lock().unwrap();
        let user = state
            .users
            .get_mut(&req.sync_key)
            .or_forbidden("invalid hkey")?;
        Span::current().record("uid", &user.name);
        Span::current().record("client", &req.client_version);
        Span::current().record("session", &req.session_key);
        println!("111111111111");
        op(user, req)
    }

    pub(in crate::sync) fn get_host_key(
        &self,
        request: HostKeyRequest,
    ) -> HttpResult<SyncResponse<HostKeyResponse>> {
        let mut state = self.state.lock().unwrap();

        let result = state.load_account_if(&request.username, &request.password);
        match result {
            Ok(opt_user) => {
                if let Some(_) = opt_user {
                    let name = &request.username;
                    let password = &request.password;
                    let val = format!("{}:{}", name, password);
                    let key = derive_hkey(&val);
                    if !state.is_user_exists(&key) {
                        let ret = state.create_user(name, &key);
                        match ret {
                            Ok(_) => SyncResponse::try_from_obj(HostKeyResponse { key }),
                            Err(_) => None.or_internal_err("create user fail"),
                        }
                    } else {
                        SyncResponse::try_from_obj(HostKeyResponse { key })
                    }
                } else {
                    None.or_forbidden("invalid user/pass in get_host_key")
                }
            }
            Err(_) => None.or_internal_err("load user fail"),
        }
    }
    pub fn is_running() -> bool {
        let config = envy::prefixed("SYNC_")
            .from_env::<SyncServerConfig>()
            .unwrap();
        std::net::TcpStream::connect(format!("{}:{}", config.host, config.port)).is_ok()
    }
    pub fn new(base_folder: &Path) -> error::Result<Self, Whatever> {
        let inner = SimpleServerInner::new_from_env(base_folder)?;
        Ok(SimpleServer {
            state: Mutex::new(inner),
        })
    }

    pub async fn make_server(
        config: SyncServerConfig,
    ) -> error::Result<(SocketAddr, ServerFuture), Whatever> {
        let server = Arc::new(
            SimpleServer::new(&config.base_folder).whatever_context("unable to create server")?,
        );
        let address = &format!("{}:{}", config.host, config.port);
        let listener = TcpListener::bind(address)
            .await
            .with_whatever_context(|_| format!("couldn't bind to {address}"))?;
        let addr = listener.local_addr().unwrap();
        let server = with_logging_layer(
            Router::new()
                .nest("/sync", collection_sync_router())
                .nest("/msync", media_sync_router())
                .route("/health", get(health_check_handler))
                .route("/register", post(register_handler))
                .with_state(server)
                .layer(DefaultBodyLimit::max(*MAXIMUM_SYNC_PAYLOAD_BYTES))
                .layer(config.ip_header.into_extension()),
        );
        let future = axum::serve(
            listener,
            server.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .into_future();
        tracing::info!(%addr, "listening");
        Ok((addr, Box::pin(future)))
    }

    #[snafu::report]
    #[tokio::main]
    pub async fn run() -> error::Result<(), Whatever> {
        let config = envy::prefixed("SYNC_")
            .from_env::<SyncServerConfig>()
            .whatever_context("reading SYNC_* env vars")?;
        println!("{:#?}", config);
        let (_addr, server_fut) = SimpleServer::make_server(config).await?;
        server_fut.await.whatever_context("await server")?;
        Ok(())
    }
}

pub type ServerFuture = Pin<Box<dyn Future<Output = error::Result<(), std::io::Error>> + Send>>;
