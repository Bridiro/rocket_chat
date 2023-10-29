CREATE TABLE
    users (
        username VARCHAR(20) NOT NULL,
        passwd VARCHAR(30) NOT NULL,
        PRIMARY KEY (username),
        UNIQUE (username)
    );