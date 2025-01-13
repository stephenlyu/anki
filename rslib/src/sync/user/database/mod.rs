use std::path::Path;

use crate::error;
use rusqlite::params;
use rusqlite::{Connection, Result, Row};

use md5::compute;

fn calculate_md5(password: &str) -> String {
    let result = compute(password);
    format!("{:x}", result)
}

#[derive(Debug, PartialEq, Eq)]
pub struct User {
    pub id: u64,
    pub email: String,
    pub name: Option<String>,
    pub password: Option<String>,
}

impl User {
    fn from_row(row: &Row) -> error::Result<Self, rusqlite::Error> {
        Ok(Self {
            id: row.get(0)?,
            email: row.get(1)?,
            name: row.get(2)?,
            password: None,
        })
    }
}

pub struct UserDatabase {
    db: Connection,
}

impl UserDatabase {
    pub fn new(path: &Path) -> Result<Self> {
        Ok(Self {
            db: open_or_create_db(path)?,
        })
    }

    pub fn verify_user(&self, email: &str, password: &str) -> Result<Option<User>> {
        let hashed_password = calculate_md5(password);
        self.db
            .prepare_cached(include_str!("verify_user.sql"))?
            .query_row(params![email, hashed_password], User::from_row)
            .map(|user| Some(user))
            .or_else(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(e),
            })
    }

    pub fn add_user(&self, user: &User) -> Result<()> {
        let mut hashed_password: Option<String> = None;
        if let Some(ref password) = user.password {
            hashed_password = Some(calculate_md5(password));
        }
        self
            .db
            .prepare_cached(include_str!("add_user.sql"))?
            .execute(params![user.email, user.name, hashed_password])?;
        Ok(())
    }
}

fn open_or_create_db(path: &Path) -> Result<Connection> {
    let db = Connection::open(path)?;
    db.busy_timeout(std::time::Duration::from_secs(0))?;
    db.pragma_update(None, "locking_mode", "exclusive")?;
    db.pragma_update(None, "journal_mode", "wal")?;
    db.execute_batch(include_str!("schema.sql"))?;
    Ok(db)
}

#[cfg(test)]
mod test {
    use super::{User, UserDatabase};
    use std::fs::{create_dir_all, exists, remove_file};
    use std::path::PathBuf;

    fn error_to_string<T>(err: T) -> String
    where
        T: ToString,
    {
        err.to_string()
    }

    #[test]
    fn test() -> Result<(), String> {
        let dir = PathBuf::from("./tmp/db");
        create_dir_all(dir.as_path()).map_err(error_to_string)?;
        let user_db = dir.join("user.db");
        let found = exists(user_db.as_path()).map_err(error_to_string)?;
        if found {
            remove_file(user_db.as_path()).map_err(error_to_string)?;
        }

        let db = UserDatabase::new(user_db.as_path()).map_err(error_to_string)?;

        let user = User {
            id: 0,
            name: None,
            email: "abc@gmai.com".to_string(),
            password: Some("123456".to_string()),
        };
        db.add_user(&user).map_err(error_to_string)?;

        let new_user = db
            .verify_user("abc@gmai.com", "123456")
            .map_err(error_to_string)?;
        println!("{:#?}", new_user);

        Ok(())
    }
}
