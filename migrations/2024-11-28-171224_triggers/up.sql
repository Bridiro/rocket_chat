-- Your SQL goes here
CREATE TRIGGER trigger_delete_empty_rooms AFTER DELETE ON rooms_users FOR EACH ROW BEGIN DECLARE count_users INT;

IF OLD.room_id != 1 THEN
SELECT
    COUNT(*) INTO count_users
FROM
    rooms_users
WHERE
    room_id = OLD.room_id;

IF count_users = 0 THEN
DELETE FROM rooms
WHERE
    id = OLD.room_id;

END IF;

END IF;

END;

CREATE TRIGGER after_insert_users_trigger AFTER INSERT ON users FOR EACH ROW BEGIN
INSERT INTO
    rooms_users (room_id, user_id)
VALUES
    (1, NEW.id);

END;