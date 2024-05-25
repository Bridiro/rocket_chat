CREATE TABLE
    rooms_users (
        room_id INT REFERENCES rooms (id) ON DELETE CASCADE,
        user_id INT REFERENCES users (id) ON DELETE CASCADE,
        PRIMARY KEY (room_id, user_id)
    );