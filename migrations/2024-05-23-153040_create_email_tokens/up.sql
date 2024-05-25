CREATE TABLE
    email_tokens (
        user_id INT REFERENCES users (id) ON DELETE CASCADE,
        token TEXT NOT NULL,
        PRIMARY KEY (user_id)
    );