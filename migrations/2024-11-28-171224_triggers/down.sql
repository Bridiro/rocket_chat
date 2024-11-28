-- This file should undo anything in `up.sql`
DROP TRIGGER IF EXISTS after_insert_users_trigger;

DROP TRIGGER IF EXISTS trigger_delete_empty_rooms;