CREATE TABLE
    rooms_users (
        room_name VARCHAR(30) NOT NULL REFERENCES rooms (room_name),
        user VARCHAR(20) NOT NULL REFERENCES users (username),
        PRIMARY KEY (room_name, user)
    );