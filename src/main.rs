#[macro_use]
extern crate rocket;

use base64::prelude::*;
use diesel::prelude::*;
use rand::Rng;
use rocket::{
    form::Form,
    fs::{relative, FileServer},
    http::Status,
    response::{
        status,
        stream::{Event, EventStream},
    },
    serde::{json::Json, Deserialize, Serialize},
    tokio::{
        select,
        sync::broadcast::{channel, error::RecvError, Sender},
    },
    Shutdown, State,
};
use rocket_chat::models::*;
use rsa::{
    pkcs1::EncodeRsaPublicKey,
    pkcs8::{DecodePublicKey, LineEnding},
    Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey,
};
use sha2::{Digest, Sha512};
use std::{error::Error, sync::Mutex};

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
    rsa_key: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct Message {
    #[field(validate = len(..30))]
    room: String,
    #[field(validate = len(..20))]
    username: String,
    message: String,
}

impl Message {
    fn new(room: String, username: String, message: String) -> Message {
        Message {
            room: room,
            username: username,
            message: message,
        }
    }
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct Room {
    room: String,
    password: Option<String>,
    require_password: bool,
    hidden: bool,
    user: String,
    rsa_client_key: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct PubRoom {
    room: String,
    require_password: bool,
    key: String,
    messages: Vec<Message>,
}

impl PubRoom {
    fn new(room: String, require_password: bool, key: String, messages: Vec<Message>) -> PubRoom {
        PubRoom {
            room: room,
            require_password: require_password,
            key: key,
            messages: messages,
        }
    }
}

#[post("/message", data = "<form>")]
fn post(
    form: Form<Message>,
    queue: &State<Sender<Message>>,
) -> Result<(), status::Custom<&'static str>> {
    use diesel::insert_into;

    let connection = &mut rocket_chat::establish_connection();
    let message = form.into_inner();

    if let Ok(_) = insert_into(rocket_chat::schema::messages::dsl::messages)
        .values((
            rocket_chat::schema::messages::room_name.eq(&message.room),
            rocket_chat::schema::messages::username.eq(&message.username),
            rocket_chat::schema::messages::content.eq(&message.message),
        ))
        .execute(connection)
    {
        let _res = queue.send(message);
        Ok(())
    } else {
        Err(status::Custom(
            Status::InternalServerError,
            "error inserting message",
        ))
    }
}

#[post("/add-room", data = "<form>")]
fn add_room(
    form: Form<Room>,
    state: &State<AppState>,
) -> Result<String, status::Custom<&'static str>> {
    use rocket_chat::schema::rooms::dsl::*;
    use rocket_chat::schema::rooms_users::dsl::*;
    let connection = &mut rocket_chat::establish_connection();

    let room = form.into_inner();

    if let Ok(roomsdb) = rooms
        .filter(rocket_chat::schema::rooms::room_name.eq(&room.room))
        .select(RoomDB::as_select())
        .load(connection)
    {
        // Stanza già esistente
        for r in roomsdb {
            if (room.require_password
                && r.passwd == Some(hash_password(decrypt_rsa(room.password.unwrap(), state))))
                || !r.require_password
            {
                let result = diesel::insert_into(rooms_users)
                    .values((
                        rocket_chat::schema::rooms_users::room_name.eq(room.room),
                        rocket_chat::schema::rooms_users::user.eq(room.user),
                    ))
                    .execute(connection);
                if result == Ok(1) {
                    match encrypt_rsa(r.aes_key, room.rsa_client_key) {
                        Ok(enc) => return Ok(enc),
                        Err(_) => {
                            return Err(status::Custom(Status::InternalServerError, "RSA error"));
                        }
                    }
                } else {
                    return Err(status::Custom(
                        Status::InternalServerError,
                        "Database error",
                    ));
                }
            } else {
                return Err(status::Custom(Status::Unauthorized, "Wrong password"));
            }
        }

        // Stanza da creare
        let key = generate_aes256_key();
        let insert_room = diesel::insert_into(rooms)
            .values((
                rocket_chat::schema::rooms::room_name.eq(&room.room),
                rocket_chat::schema::rooms::passwd.eq(
                    if room.password != Some("null".to_string()) {
                        Some(hash_password(decrypt_rsa(room.password.unwrap(), state)))
                    } else {
                        None
                    },
                ),
                rocket_chat::schema::rooms::require_password.eq(room.require_password),
                rocket_chat::schema::rooms::hidden_room.eq(room.hidden),
                rocket_chat::schema::rooms::aes_key.eq(&key),
            ))
            .execute(connection);
        let insert_room_user = diesel::insert_into(rooms_users)
            .values((
                rocket_chat::schema::rooms_users::room_name.eq(room.room),
                rocket_chat::schema::rooms_users::user.eq(room.user),
            ))
            .execute(connection);
        if insert_room == Ok(1) && insert_room_user == Ok(1) {
            match encrypt_rsa(key, room.rsa_client_key) {
                Ok(enc) => Ok(enc),
                Err(_) => {
                    return Err(status::Custom(Status::InternalServerError, "RSA error"));
                }
            }
        } else {
            Err(status::Custom(
                Status::InternalServerError,
                "Database error",
            ))
        }
    } else {
        Err(status::Custom(
            Status::InternalServerError,
            "Database error",
        ))
    }
}

#[post("/remove-room", data = "<form>")]
fn remove_room(form: Form<Room>) -> Result<(), status::Custom<&'static str>> {
    let connection = &mut rocket_chat::establish_connection();

    let room = form.clone().room;
    let for_user = form.into_inner().user;

    if let Ok(_) = diesel::delete(
        rocket_chat::schema::rooms_users::table
            .filter(rocket_chat::schema::rooms_users::room_name.eq(&room))
            .filter(rocket_chat::schema::rooms_users::user.eq(for_user)),
    )
    .execute(connection)
    {
        Ok(())
    } else {
        Err(status::Custom(Status::Unauthorized, "can't"))
    }
}

#[post("/search-rooms")]
fn search_rooms() -> Result<Json<Vec<PubRoom>>, status::Custom<&'static str>> {
    use rocket_chat::schema::rooms::dsl::*;
    let connection = &mut rocket_chat::establish_connection();

    if let Ok(roomsdb) = rooms
        .filter(hidden_room.eq(false))
        .select(RoomDB::as_select())
        .load(connection)
    {
        let pub_rooms = roomsdb
            .iter()
            .map(|room| {
                PubRoom::new(
                    room.room_name.clone(),
                    room.require_password,
                    "".to_string(),
                    Vec::<Message>::new(),
                )
            })
            .collect::<Vec<PubRoom>>();

        Ok(Json(pub_rooms))
    } else {
        Err(status::Custom(
            Status::InternalServerError,
            "Database error",
        ))
    }
}

#[post("/login", data = "<form>")]
fn login(form: Form<User>, state: &State<AppState>) -> Json<Vec<PubRoom>> {
    use rocket_chat::schema::users::dsl::*;

    let userform = form.into_inner();
    let passw = decrypt_rsa(userform.password, state);
    let rsa_key = userform.rsa_key;
    let connection = &mut rocket_chat::establish_connection();

    if let Ok(result) = users
        .limit(1)
        .filter(username.eq(&userform.username))
        .select(passwd)
        .load::<String>(connection)
    {
        if result.len() > 0 && hash_password(passw) == result[0] {
            if let Ok(room_with_roomuser) = rocket_chat::schema::rooms::table
                .inner_join(rocket_chat::schema::rooms_users::table)
                .filter(rocket_chat::schema::rooms_users::user.eq(userform.username))
                .select((RoomDB::as_select(), RoomUserDB::as_select()))
                .load::<(RoomDB, RoomUserDB)>(connection)
            {
                let mut pub_rooms: Vec<PubRoom> = Vec::new();
                for (room, room_user) in room_with_roomuser {
                    if let Ok(messages_for_room) = MessageDB::belonging_to(&room)
                        .select(MessageDB::as_select())
                        .load(connection)
                    {
                        pub_rooms.push(PubRoom::new(
                            room_user.room_name,
                            false,
                            encrypt_rsa(room.aes_key, rsa_key.clone()).unwrap(),
                            messages_for_room
                                .iter()
                                .map(|mdb| {
                                    Message::new(
                                        mdb.room_name.clone(),
                                        mdb.username.clone(),
                                        mdb.content.clone(),
                                    )
                                })
                                .collect::<Vec<Message>>(),
                        ))
                    }
                }

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
fn signup(
    form: Form<User>,
    state: &State<AppState>,
) -> Result<&'static str, status::Custom<&'static str>> {
    use rocket_chat::schema::rooms_users::dsl::*;
    use rocket_chat::schema::users::dsl::*;

    let userr = form.into_inner();
    let passw = decrypt_rsa(userr.password, state);
    let connection = &mut rocket_chat::establish_connection();

    let result = diesel::insert_into(users)
        .values((
            rocket_chat::schema::users::username.eq(userr.username.clone()),
            rocket_chat::schema::users::passwd.eq(hash_password(passw)),
        ))
        .execute(connection);
    let result2 = diesel::insert_into(rooms_users)
        .values((
            rocket_chat::schema::rooms_users::room_name.eq("lobby"),
            rocket_chat::schema::rooms_users::user.eq(userr.username),
        ))
        .execute(connection);
    if result == Ok(1) && result2 == Ok(1) {
        Ok("GRANTED")
    } else {
        Err(status::Custom(Status::Unauthorized, "Not authorized"))
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

// Hash a passed password with SHA-512 and returns a String
fn hash_password(password: String) -> String {
    let mut hasher = Sha512::new();
    hasher.update(password);
    let result = hasher.finalize();
    String::from_utf8_lossy(&result).to_string()
}

// Decode a base64 encoded string with RSA
fn decrypt_rsa(encoded: String, state: &State<AppState>) -> String {
    let enc_data = BASE64_STANDARD.decode(encoded).unwrap();

    match get_rsa_priv_key_from_state(state).decrypt(Pkcs1v15Encrypt, &enc_data) {
        Ok(dec_data) => {
            if let Ok(dec_str) = String::from_utf8(dec_data) {
                dec_str
            } else {
                String::from("ERROR DECODING")
            }
        }
        Err(err) => {
            format!("ERROR DECRYPTING: {}", err)
        }
    }
}

// Encrypt a message to a base64 string with RSA
fn encrypt_rsa(message: String, public_key_pem: String) -> Result<String, Box<dyn Error>> {
    let public_key = RsaPublicKey::from_public_key_pem(&public_key_pem)
        .map_err(|e| format!("Errore durante la lettura della chiave pubblica PEM: {}", e))?;

    let message_bytes = message.as_bytes();

    let mut rng = rand::thread_rng();
    let encrypted_message = public_key
        .encrypt(&mut rng, Pkcs1v15Encrypt, message_bytes)
        .map_err(|e| format!("Errore durante la crittografia del messaggio: {}", e))?;

    let encrypted_message_base64 = BASE64_STANDARD.encode(encrypted_message);
    Ok(encrypted_message_base64)
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

fn generate_aes256_key() -> String {
    let mut key = [0u8; 32];
    match rand::thread_rng().try_fill(&mut key) {
        Ok(_) => BASE64_STANDARD.encode(key),
        Err(e) => format!("Errore nel generare la chiave: {}", e),
    }
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
                get_rsa_pub_key,
                events
            ],
        )
        .mount("/", FileServer::from(relative!("static")))
}
