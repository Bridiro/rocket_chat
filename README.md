# Web Server for a chat in Rust

It's based on the Rocket web framework for Rust.  
Right now the DB stores users, rooms and messages. Messages are encrypted using **AES256** and keys shared using **RSA** (currently 2048 bits key byt it will be 4096 in production).  
Now I have to implement the ability to send images, video and audios.

To currently use the app you have to had installed Rust and all the necessary dependencies, and you need to create a **.env** file
containing the path to your database, something like this:

    DATABASE_URL=mysql://user:password@127.0.0.1:3306/rocket_chat_db

One important thing to remember is that you have to have **diesel_rs** installed. You can do this by running:

    cargo install diesel_cli --no-default-features --features mysql

After ensuring having that installed, you have to run:

    diesel setup

and then:

    diesel migration run

After this just run the command

    cargo build --release

to get the executable in the folder **target/release** or run

    cargo watch -x run

to continue building the project everytime you save a file in the editor (need watch_rs installed).
