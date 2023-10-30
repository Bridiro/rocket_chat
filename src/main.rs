#[macro_use]
extern crate rocket;

use diesel::prelude::*;
use rocket::{
    form::Form,
    fs::{relative, FileServer},
    response::stream::{Event, EventStream},
    serde::{json::Json, Deserialize, Serialize},
    tokio::select,
    tokio::sync::broadcast::{channel, error::RecvError, Sender},
    Shutdown, State,
};
use rocket_chat::models::*;
use sha2::{Digest, Sha512};

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct User {
    #[field(validate = len(..20))]
    username: String,
    #[field(validate = len(..30))]
    password: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Message {
    #[field(validate = len(..30))]
    room: String,
    #[field(validate = len(..20))]
    username: String,
    message: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct Room {
    room: String,
    password: Option<String>,
    require_password: bool,
    hidden: bool,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct PubRoom {
    room: String,
    require_password: bool,
    hidden: bool,
}

impl PubRoom {
    fn new(room: String, require_password: bool, hidden: bool) -> PubRoom {
        PubRoom {
            room: room,
            require_password: require_password,
            hidden: hidden,
        }
    }
}

#[post("/message", data = "<form>")]
fn post(form: Form<Message>, queue: &State<Sender<Message>>) {
    let _res = queue.send(form.into_inner());
}

#[post("/add-room", data = "<form>")]
fn add_room(form: Form<Room>) -> String {
    use diesel::insert_into;
    use rocket_chat::schema::rooms::dsl::*;
    let connection = &mut rocket_chat::establish_connection();

    let room = form.into_inner();

    if let Ok(roomsdb) = rooms.select(RoomDB::as_select()).load(connection) {
        for r in roomsdb {
            if r.room_name == room.room {
                if (room.require_password
                    && r.passwd == Some(hash_password(room.password.unwrap())))
                    || !room.require_password
                {
                    return format!("GRANTED");
                } else {
                    return format!("REJECTED");
                }
            }
        }

        let result = insert_into(rooms)
            .values((
                room_name.eq(room.room),
                rocket_chat::schema::rooms::passwd.eq(hash_password(room.password.unwrap())),
                require_password.eq(room.require_password),
                hidden_room.eq(room.hidden),
            ))
            .execute(connection);
        if result == Ok(1) {
            format!("GRANTED")
        } else {
            println!("NON INSERITO");
            format!("REJECTED")
        }
    } else {
        format!("PROBLEM WITH DATABASE")
    }
}

#[post("/search-rooms")]
fn search_rooms() -> Json<Vec<PubRoom>> {
    use rocket_chat::schema::rooms::dsl::*;
    let connection = &mut rocket_chat::establish_connection();

    if let Ok(roomsdb) = rooms.select(RoomDB::as_select()).load(connection) {
        println!("Displaying {} rooms", roomsdb.len());
        for room in &roomsdb {
            println!("{}", room.room_name);
            println!("{:?}", room.passwd);
            println!("{}", room.require_password);
            println!("{}", room.hidden_room);
            println!("-------------------------------");
        }

        let pub_rooms = roomsdb
            .iter()
            .map(|room| {
                PubRoom::new(
                    room.room_name.clone(),
                    room.require_password,
                    room.hidden_room,
                )
            })
            .collect::<Vec<PubRoom>>();

        Json(pub_rooms)
    } else {
        let default: Vec<PubRoom> = vec![PubRoom::new("lobby".to_string(), false, false)];
        Json(default)
    }
}

#[post("/login", data = "<form>")]
fn login(form: Form<User>) -> String {
    let user = form.into_inner();
    use rocket_chat::schema::users::dsl::*;
    let connection = &mut rocket_chat::establish_connection();

    if let Ok(result) = users
        .limit(1)
        .filter(username.eq(user.username))
        .select(passwd)
        .load::<String>(connection)
    {
        if result.len() > 0 && hash_password(user.password) == result[0] {
            format!("GRANTED")
        } else {
            format!("REJECTED")
        }
    } else {
        format!("REJECTED")
    }
}

#[post("/signup", data = "<form>")]
fn signup(form: Form<User>) -> String {
    use diesel::insert_into;
    use rocket_chat::schema::users::dsl::*;

    let user = form.into_inner();
    let connection = &mut rocket_chat::establish_connection();

    let result = insert_into(users)
        .values((
            username.eq(user.username),
            passwd.eq(hash_password(user.password)),
        ))
        .execute(connection);
    if result == Ok(1) {
        format!("GRANTED")
    } else {
        format!("REJECTED")
    }
}

#[get("/events")]
async fn events(queue: &State<Sender<Message>>, mut end: Shutdown) -> EventStream![] {
    let mut rx = queue.subscribe();

    EventStream! {
        loop {
            let msg = select! {
                msg = rx.recv() => match msg {
                    Ok(msg) => msg,
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(_)) => continue,
                },
                _ = &mut end => break,
            };

            yield Event::json(&msg)
        }
    }
}

// Hash a passed password with SHA-512 and returns a String Option
fn hash_password(password: String) -> String {
    let mut hasher = Sha512::new();
    hasher.update(password);
    let result = hasher.finalize();
    String::from_utf8_lossy(&result).to_string()
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .manage(channel::<Message>(1024).0)
        .mount(
            "/",
            routes![post, add_room, search_rooms, login, signup, events],
        )
        .mount("/", FileServer::from(relative!("static")))
}
