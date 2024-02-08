SELECT * FROM rooms_users 
INNER JOIN rooms ON rooms_users.room_name = rooms.room_name;