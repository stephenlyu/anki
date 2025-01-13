CREATE TABLE user (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email TEXT NOT NULL,
    name TEXT,
    password TEXT NOT NULL,
    UNIQUE (email)
);
