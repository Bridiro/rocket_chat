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
use rsa::{
    pkcs1::EncodeRsaPublicKey, pkcs8::LineEnding, Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey,
};
use sha2::{Digest, Sha512};
use std::sync::Mutex;

struct KeyPair {
    pub pub_key: RsaPublicKey,
    pub priv_key: RsaPrivateKey,
}

impl Clone for KeyPair {
    fn clone(&self) -> Self {
        KeyPair {
            pub_key: self.pub_key.clone(),
            priv_key: self.priv_key.clone(),
        }
    }
}

struct AppState {
    keys: Mutex<Option<KeyPair>>,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct User {
    #[field(validate = len(..20))]
    username: String,
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
                let result = diesel::insert_into(rooms_users)
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

        let result = diesel::insert_into(rooms)
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
        let result2 = diesel::insert_into(rooms_users)
            .values((
                rocket_chat::schema::rooms_users::room_name.eq(room.room),
                rocket_chat::schema::rooms_users::user.eq(room.user),
            ))
            .execute(connection);
        if result == Ok(1) && result2 == Ok(1) {
            format!("GRANTED")
        } else {
            format!("REJECTED")
        }
    } else {
        format!("PROBLEM WITH DATABASE")
    }
}

#[post("/remove-room", data = "<form>")]
fn remove_room(form: Form<Room>) -> String {
    use rocket_chat::schema::rooms_users::dsl::*;
    let connection = &mut rocket_chat::establish_connection();

    let room = form.clone().room;
    let for_user = form.into_inner().user;

    let deleted_room_user = diesel::delete(
        rocket_chat::schema::rooms_users::table
            .filter(room_name.eq(&room))
            .filter(user.eq(for_user)),
    )
    .execute(connection);

    println!("deleted_rooms_users: {:?}", deleted_room_user);

    if deleted_room_user == Ok(1) {
        if let Ok(room_user) = rooms_users
            .filter(room_name.eq(&room))
            .select(RoomUserDB::as_select())
            .load(connection)
        {
            if room_user.len() > 0 {
                return format!("GRANTED");
            } else if room_user.len() == 0 {
                if let Ok(_deleted) = diesel::delete(
                    rocket_chat::schema::rooms::table
                        .filter(rocket_chat::schema::rooms::room_name.eq(room)),
                )
                .execute(connection)
                {
                    return format!("GRANTED");
                } else {
                    return format!("DB ERROR");
                }
            }
        }
    }

    format!("REJECTED")
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

        Json(pub_rooms)
    } else {
        let default: Vec<PubRoom> = vec![PubRoom::new("lobby".to_string(), false)];
        Json(default)
    }
}

#[post("/login", data = "<form>")]
fn login(form: Form<User>) -> Json<Vec<PubRoom>> {
    let userform = form.into_inner();
    use rocket_chat::schema::rooms_users::dsl::*;
    use rocket_chat::schema::users::dsl::*;
    let connection = &mut rocket_chat::establish_connection();

    if let Ok(result) = users
        .limit(1)
        .filter(username.eq(&userform.username))
        .select(passwd)
        .load::<String>(connection)
    {
        if result.len() > 0 && hash_password(userform.password) == result[0] {
            if let Ok(rooms_users_db) = rooms_users
                .filter(rocket_chat::schema::rooms_users::user.eq(userform.username))
                .select(RoomUserDB::as_select())
                .load(connection)
            {
                let pub_rooms = rooms_users_db
                    .iter()
                    .map(|room_user| PubRoom::new(room_user.room_name.clone(), false))
                    .collect::<Vec<PubRoom>>();

                Json(pub_rooms)
            } else {
                let empty: Vec<PubRoom> = Vec::new();
                Json(empty)
            }
        } else {
            let empty: Vec<PubRoom> = Vec::new();
            Json(empty)
        }
    } else {
        let empty: Vec<PubRoom> = Vec::new();
        Json(empty)
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
            passwd.eq(userr.password),
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

#[derive(Debug, Clone, FromForm, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Encoding {
    x: String,
}

#[post("/encoding", data = "<form>")]
fn encoding(form: Form<Encoding>) -> String {
    let val = form.into_inner();
    println!("{}", &val.x);
    val.x
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

// Hash a passed password with SHA-512 and returns a String
fn hash_password(password: String) -> String {
    let mut hasher = Sha512::new();
    hasher.update(password);
    let result = hasher.finalize();
    String::from_utf8_lossy(&result).to_string()
}

fn generate_key_pair() -> KeyPair {
    let bits = 2048;
    let mut rng = rand::thread_rng();
    let priv_key: RsaPrivateKey =
        RsaPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
    let pub_key: RsaPublicKey = RsaPublicKey::from(&priv_key);
    KeyPair { pub_key, priv_key }
}

fn get_keys(state: &State<AppState>) -> Option<KeyPair> {
    let data = state.keys.lock().unwrap();
    data.clone()
}

fn get_rsa_pub_key_from_state(state: &State<AppState>) -> RsaPublicKey {
    get_keys(state).unwrap().pub_key
}

fn get_rsa_priv_key_from_state(state: &State<AppState>) -> RsaPrivateKey {
    get_keys(state).unwrap().priv_key
}

#[get("/rsa-pub-key")]
fn get_rsa_pub_key(state: &State<AppState>) -> String {
    get_rsa_pub_key_from_state(state)
        .to_pkcs1_pem(LineEnding::default())
        .unwrap()
}

#[get("/rsa")]
fn rsa_prova(state: &State<AppState>) -> String {
    let mut rng = rand::thread_rng();

    let data = b"hello world";
    let enc_data = get_rsa_pub_key_from_state(state)
        .encrypt(&mut rng, Pkcs1v15Encrypt, &data[..])
        .expect("failed to encrypt");
    assert_ne!(&data[..], &enc_data[..]);

    let dec_data = get_rsa_priv_key_from_state(state)
        .decrypt(Pkcs1v15Encrypt, &enc_data)
        .expect("failed to decrypt");
    String::from_utf8(dec_data).unwrap()
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .manage(channel::<Message>(1024).0)
        .manage(AppState {
            keys: Mutex::new(Some(generate_key_pair())),
        })
        .mount(
            "/",
            routes![
                post,
                add_room,
                remove_room,
                search_rooms,
                login,
                signup,
                encoding,
                get_rsa_pub_key,
                rsa_prova,
                events
            ],
        )
        .mount("/", FileServer::from(relative!("static")))
}
