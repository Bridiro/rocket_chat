CREATE TABLE
    rooms (
        room_name VARCHAR(30),
        passwd TEXT DEFAULT NULL,
        require_password BOOLEAN NOT NULL,
        hidden_room BOOLEAN NOT NULL,
        PRIMARY KEY (room_name)
    );