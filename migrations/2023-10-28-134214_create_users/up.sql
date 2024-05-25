CREATE TABLE
    users (
        id INT AUTO_INCREMENT,
        full_name VARCHAR(100) NOT NULL,
        surname VARCHAR(100) NOT NULL,
        email VARCHAR(100) NOT NULL UNIQUE,
        username VARCHAR(20) NOT NULL UNIQUE,
        passwd TEXT NOT NULL,
        salt TEXT NOT NULL,
        email_verified BOOLEAN NOT NULL DEFAULT FALSE,
        PRIMARY KEY (id)
    );