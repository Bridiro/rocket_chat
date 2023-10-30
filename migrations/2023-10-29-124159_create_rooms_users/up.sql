CREATE TABLE
    rooms_users (
        room_name VARCHAR(30) NOT NULL REFERENCES rooms (room_name) ON DELETE CASCADE,
        user VARCHAR(20) NOT NULL REFERENCES users (username) ON DELETE CASCADE,
        PRIMARY KEY (room_name, user)
    );