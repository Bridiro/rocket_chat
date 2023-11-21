CREATE TABLE
    messages (
        message_id INT AUTO_INCREMENT,
        room_name VARCHAR(30) NOT NULL REFERENCES rooms (room_name) ON DELETE CASCADE,
        username VARCHAR(20) NOT NULL REFERENCES users (username) ON DELETE CASCADE,
        content TEXT NOT NULL,
        message_time DATETIME DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (message_id)
    );