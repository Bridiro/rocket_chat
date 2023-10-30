CREATE TABLE
    users (
        username VARCHAR(20) NOT NULL,
        passwd TEXT NOT NULL,
        PRIMARY KEY (username),
        UNIQUE (username)
    );