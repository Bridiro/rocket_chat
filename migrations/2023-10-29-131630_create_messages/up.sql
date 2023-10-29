CREATE TABLE
    messages (
        message_id INTEGER NOT NULL AUTO_INCREMENT,
        room_name VARCHAR(30) NOT NULL REFERENCES rooms (room_name),
        username VARCHAR(20) NOT NULL REFERENCES users (username),
        content TEXT NOT NULL,
        message_time DATETIME DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (message_id),
        UNIQUE (room_name),
        UNIQUE (username)
    );