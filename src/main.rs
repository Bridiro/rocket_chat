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

#[derive(Debug, Clone, FromForm, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Room {
    pub room: String,
    pub password: String,
}

#[post("/message", data = "<form>")]
fn post(form: Form<Message>, queue: &State<Sender<Message>>) {
    let _res = queue.send(form.into_inner());
}

#[post("/add-room", data = "<form>")]
fn add_room(form: Form<Room>) -> String {
    match hash_password("12345678".to_string()) {
        Some(hashed) => {
            let valid = confront_password(hashed, form.into_inner().password);
            if valid {
                format!("Password valid")
            } else {
                format!("Wrong password")
            }
        }
        _ => {
            format!("Unable to process password")
        }
    }
}

#[post("/search-rooms", data = "<name>")]
fn search_rooms(name: Form<String>) -> Json<Vec<String>> {
    let name = name.into_inner();
    let rooms = vec![
        String::from("pazzi"),
        String::from("maniaci"),
        String::from("drogati"),
        String::from("adhd"),
        String::from("dromedari"),
        String::from("pastrengo"),
    ];

    let mut matched: Vec<String> = Vec::new();
    for room in rooms {
        if room.len() >= name.len() && room[..name.len()] == name {
            matched.push(room);
        }
    }

    Json(matched)
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
fn hash_password(password: String) -> Option<String> {
    let mut hasher = Sha512::new();
    hasher.update(password);
    let result = hasher.finalize();
    match String::from_utf8_lossy(&result).to_string() {
        hashed => Some(hashed),
    }
}

// Confront a password hash and a password, returns a bool
fn confront_password(hash: String, password: String) -> bool {
    let mut hasher = Sha512::new();
    hasher.update(password);
    let result = hasher.finalize();
    match String::from_utf8_lossy(&result).to_string() {
        hashed => hash == hashed,
    }
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .manage(channel::<Message>(1024).0)
        .mount("/", routes![post, add_room, search_rooms, events])
        .mount("/", FileServer::from(relative!("static")))
}
