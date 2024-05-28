CREATE TABLE
    directs (
        id INT AUTO_INCREMENT,
        user1_id INT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
        user2_id INT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
        aes_key TEXT NOT NULL,
        PRIMARY KEY (id)
    )