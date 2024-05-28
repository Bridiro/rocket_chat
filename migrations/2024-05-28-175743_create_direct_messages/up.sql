CREATE TABLE
    direct_messages (
        id INT AUTO_INCREMENT,
        chat_id INT NOT NULL REFERENCES directs (id) ON DELETE CASCADE,
        sender_id INT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
        message TEXT NOT NULL,
        message_time DATETIME DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (id)
    );