# Client-Server chat in Rust

It's based on the Rocket web framework for Rust.<br>
I want to implement secure rooms with a database to store messages, photos and passwords for rooms.<br>
I have to implement a user system so chats and rooms can be restored on login.<br>
I want to permit anonimity too, so to have the possibility to have rooms that do not save messages in the Db.<br>
<br>
To currently use the app you have to had installed Rust and all the necessary dependencies, and you need to create a ".env" file<br>
containing the path to your database, something like this: DATABASE_URL=mysql://user:password@127.0.0.1:3306/rocket_chat_db
