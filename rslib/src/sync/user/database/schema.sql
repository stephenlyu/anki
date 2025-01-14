BEGIN exclusive;
CREATE TABLE IF NOT EXISTS user (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email TEXT NOT NULL,
    name TEXT,
    password TEXT NOT NULL,
    UNIQUE (email)
);
pragma user_version = 3;
COMMIT;
