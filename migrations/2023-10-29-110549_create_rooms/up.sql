CREATE TABLE
    rooms (
        id INT AUTO_INCREMENT,
        room_name VARCHAR(30) NOT NULL,
        passwd TEXT DEFAULT NULL,
        require_password BOOLEAN NOT NULL,
        hidden_room BOOLEAN NOT NULL,
        aes_key TEXT NOT NULL,
        salt TEXT NOT NULL,
        PRIMARY KEY (id)
    );