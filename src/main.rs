#[macro_use]
extern crate rocket;

use rocket::{
    form::Form,
    fs::{relative, FileServer},
    response::stream::{Event, EventStream},
    serde::{json::Json, Deserialize, Serialize},
    tokio::select,
    tokio::sync::broadcast::{channel, error::RecvError, Sender},
    Shutdown, State,
};
use sha2::{Digest, Sha512};

#[derive(Debug, Clone, FromForm, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Message {
    #[field(validate = len(..30))]
    pub room: String,
    #[field(validate = len(..20))]
    pub username: String,
    pub message: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct Room {
    pub room: String,
    pub password: String,
    pub require_password: bool,
    pub hidden: bool,
}

impl Room {
    fn new(room: String, password: String, require_password: bool, hidden: bool) -> Room {
        Room {
            room: room,
            password: password,
            require_password: require_password,
            hidden: hidden,
        }
    }
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct PubRoom {
    pub room: String,
    pub require_password: bool,
}

impl PubRoom {
    fn new(room: String, require_password: bool) -> PubRoom {
        PubRoom {
            room: room,
            require_password: require_password,
        }
    }
}

#[post("/message", data = "<form>")]
fn post(form: Form<Message>, queue: &State<Sender<Message>>) {
    let _res = queue.send(form.into_inner());
}

#[post("/add-room", data = "<form>")]
fn add_room(form: Form<Room>) -> String {
    let room = form.into_inner();
    let rooms = vec![
        Room::new(String::from("pazzi"), String::from(""), false, false),
        Room::new(
            String::from("maniaci"),
            String::from("12345678"),
            true,
            false,
        ),
        Room::new(
            String::from("drogati"),
            String::from("12345678"),
            true,
            false,
        ),
        Room::new(String::from("adhd"), String::from(""), false, false),
        Room::new(String::from("dromedari"), String::from(""), false, true),
        Room::new(
            String::from("pastrengo"),
            String::from("12345678"),
            true,
            false,
        ),
    ];

    let mut contained = false;
    for r in rooms {
        if r.room == room.room {
            contained = true;
        }
    }

    if contained {
        let valid = if room.require_password {
            confront_password(hash_password("12345678".to_string()), room.password.clone())
        } else {
            true
        };
        if valid {
            // TODO: gain access to room
            format!("GRANTED")
        } else {
            // TODO: reject access to room
            format!("REJECTED")
        }
    } else {
        // TODO: add room to db
        format!("GRANTED")
    }
}

#[post("/search-rooms")]
fn search_rooms() -> Json<Vec<PubRoom>> {
    let rooms = vec![
        Room::new(String::from("pazzi"), String::from(""), false, false),
        Room::new(
            String::from("maniaci"),
            String::from("12345678"),
            true,
            false,
        ),
        Room::new(
            String::from("drogati"),
            String::from("12345678"),
            true,
            false,
        ),
        Room::new(String::from("adhd"), String::from(""), false, false),
        Room::new(String::from("dromedari"), String::from(""), false, true),
        Room::new(
            String::from("pastrengo"),
            String::from("12345678"),
            true,
            false,
        ),
    ];

    let pub_rooms = rooms
        .iter()
        .map(|room| PubRoom::new(room.room.clone(), room.require_password))
        .collect::<Vec<PubRoom>>();

    Json(pub_rooms)
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

// Confront a password hash and a password, returns a bool
fn confront_password(hash: String, password: String) -> bool {
    let mut hasher = Sha512::new();
    hasher.update(password);
    let result = hasher.finalize();
    String::from_utf8_lossy(&result).to_string() == hash
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .manage(channel::<Message>(1024).0)
        .mount("/", routes![post, add_room, search_rooms, events])
        .mount("/", FileServer::from(relative!("static")))
}
