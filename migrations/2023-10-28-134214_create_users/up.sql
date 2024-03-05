CREATE TABLE
    users (
        username VARCHAR(20),
        passwd TEXT NOT NULL,
        salt TEXT NOT NULL,
        PRIMARY KEY (username)
    );