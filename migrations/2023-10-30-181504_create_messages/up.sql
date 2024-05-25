CREATE TABLE
    messages (
        message_id INT AUTO_INCREMENT,
        room_id INT NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
        user_id INT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
        content TEXT NOT NULL,
        message_time DATETIME DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (message_id)
    );