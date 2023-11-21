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
    user: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct PubRoom {
    room: String,
    require_password: bool,
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
    use diesel::insert_into;
    use rocket_chat::schema::rooms::dsl::*;
    use rocket_chat::schema::rooms_users::dsl::*;
    let connection = &mut rocket_chat::establish_connection();

    let room = form.into_inner();

    if let Ok(roomsdb) = rooms
        .filter(rocket_chat::schema::rooms::room_name.eq(&room.room))
        .select(RoomDB::as_select())
        .load(connection)
    {
        for r in roomsdb {
            if (room.require_password && r.passwd == Some(hash_password(room.password.unwrap())))
                || !room.require_password
            {
                let result = insert_into(rooms_users)
                    .values((
                        rocket_chat::schema::rooms_users::room_name.eq(room.room),
                        rocket_chat::schema::rooms_users::user.eq(room.user),
                    ))
                    .execute(connection);
                if result == Ok(1) {
                    return format!("GRANTED");
                } else {
                    return format!("REJECTED");
                }
            } else {
                return format!("REJECTED");
            }
        }

        println!("Passw stanza: {:?}", &room.password);

        let result = insert_into(rooms)
            .values((
                rocket_chat::schema::rooms::room_name.eq(&room.room),
                rocket_chat::schema::rooms::passwd.eq(
                    if room.password != Some("null".to_string()) {
                        Some(hash_password(room.password.unwrap()))
                    } else {
                        None
                    },
                ),
                rocket_chat::schema::rooms::require_password.eq(room.require_password),
                rocket_chat::schema::rooms::hidden_room.eq(room.hidden),
            ))
            .execute(connection);
        let result2 = insert_into(rooms_users)
            .values((
                rocket_chat::schema::rooms_users::room_name.eq(room.room),
                rocket_chat::schema::rooms_users::user.eq(room.user),
            ))
            .execute(connection);
        if result == Ok(1) && result2 == Ok(1) {
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

    if let Ok(roomsdb) = rooms
        .filter(hidden_room.eq(false))
        .select(RoomDB::as_select())
        .load(connection)
    {
        let pub_rooms = roomsdb
            .iter()
            .map(|room| PubRoom::new(room.room_name.clone(), room.require_password))
            .collect::<Vec<PubRoom>>();

        println!("pub rooms: {:?}", &pub_rooms);
        Json(pub_rooms)
    } else {
        let default: Vec<PubRoom> = vec![PubRoom::new("lobby".to_string(), false)];
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
    use rocket_chat::schema::rooms_users::dsl::*;
    use rocket_chat::schema::users::dsl::*;

    let userr = form.into_inner();
    let connection = &mut rocket_chat::establish_connection();

    let result = insert_into(users)
        .values((
            username.eq(userr.username.clone()),
            passwd.eq(hash_password(userr.password)),
        ))
        .execute(connection);
    let result2 = insert_into(rooms_users)
        .values((
            rocket_chat::schema::rooms_users::room_name.eq("lobby"),
            user.eq(userr.username),
        ))
        .execute(connection);
    if result == Ok(1) && result2 == Ok(1) {
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
