CREATE TABLE
    rooms (
        room_name VARCHAR(30) NOT NULL,
        passwd VARCHAR(30) DEFAULT NULL,
        require_password BOOLEAN NOT NULL,
        hidden_room BOOLEAN NOT NULL,
        PRIMARY KEY (room_name),
        UNIQUE (room_name)
    );