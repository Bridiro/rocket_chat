# Web Server for a chat in Rust

It's based on the Rocket web framework for Rust.  
Right now the DB stores users, rooms and messages. Messages are encrypted using **AES256** and keys shared using **RSA** (currently 2048 bits key byt it will be 4096 in production).  
Now I have to implement the ability to send images, video and audios.

To currently use the app you have to had installed Rust and all the necessary dependencies, and you need to create a **.env** file
containing the path to your database, something like this:

    DATABASE_URL=mysql://user:password@127.0.0.1:3306/rocket_chat_db

One important thing to remember is that you have to implement this **trigger** in order for the DB to remove the unused rooms.

    DELIMITER //

    CREATE TRIGGER trigger_delete_empty_rooms AFTER DELETE ON rooms_users
    FOR EACH ROW
    BEGIN
        DECLARE count_users INT;

        IF OLD.room_id != 1 THEN
            SELECT COUNT(*) INTO count_users FROM rooms_users WHERE room_id = OLD.room_id;

            IF count_users = 0 THEN
                DELETE FROM rooms WHERE id = OLD.room_id;
            END IF;
        END IF;
    END;

    //

    DELIMITER ;

and

    DELIMITER //

    CREATE TRIGGER after_insert_users_trigger
    AFTER INSERT ON users
    FOR EACH ROW
    BEGIN
        INSERT INTO rooms_users (room_id, user_id)
        VALUES (1, NEW.id);
    END;
    //

    DELIMITER ;

Run this in your **SQL** terminal.

After this just run the command

    cargo build --release

to get the executable in the folder **target/release** or run

    cargo watch -x run

to continue building the project everytime you save a file in the editor.
