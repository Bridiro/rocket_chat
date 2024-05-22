#[macro_use]
extern crate rocket;

use base64::prelude::*;
use diesel::prelude::*;
use rand::Rng;
use rocket::{
    form::{self, Form},
    fs::{relative, FileServer, NamedFile},
    http::Status,
    response::{
        status,
        stream::{Event, EventStream},
        Redirect,
    },
    serde::{json::Json, Deserialize, Serialize},
    tokio::{
        select,
        sync::broadcast::{channel, error::RecvError, Sender},
    },
    Shutdown, State,
};
use rocket_chat::models::*;
use rocket_session_store::{memory::MemoryStore, CookieConfig, Session, SessionStore};
use rsa::{
    pkcs1::EncodeRsaPublicKey,
    pkcs8::{DecodePublicKey, LineEnding},
    Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey,
};
use sha2::{Digest, Sha512};
use std::{error::Error, sync::Mutex, time::Duration};

const PEPPER: &str = "Zk4pGkvF9n5FPXSvrccl0XR33ach0+Vf/rliGZUUc+U=";

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
struct GetRoomsUser {
    #[field(validate = structval(1, 20).or_else(msg!("user must be between 1 and 20 chars")))]
    username: String,
    rsa_key: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct LoginUser {
    #[field(validate = structval(1, 20).or_else(msg!("user must be between 1 and 20 chars")))]
    username: String,
    #[field(validate = len(1..))]
    password: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct SignupUser {
    #[field(validate = structval(1, 20).or_else(msg!("user must be between 1 and 20 chars")))]
    username: String,
    #[field(validate = len(1..))]
    password: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct Message {
    #[field(validate = structval(1, 30).or_else(msg!("room must be between 1 and 30 chars")))]
    room: String,
    #[field(validate = structval(1, 20).or_else(msg!("user must be between 1 and 20 chars")))]
    username: String,
    #[field(validate = len(1..))]
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
struct ChangePassword {
    #[field(validate = structval(1, 20).or_else(msg!("user must be between 1 and 20 chars")))]
    user: String,
    #[field(validate = len(1..))]
    old_password: String,
    #[field(validate = len(1..))]
    new_password: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct Room {
    #[field(validate = structval(1, 30).or_else(msg!("room must be between 1 and 30 chars")))]
    room: String,
    password: Option<String>,
    require_password: bool,
    hidden: bool,
    #[field(validate = structval(1, 20).or_else(msg!("user must be between 1 and 20 chars")))]
    user: String,
    rsa_client_key: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct ToRemoveRoom {
    #[field(validate = structval(1, 30).or_else(msg!("room must be between 1 and 30 chars")))]
    room: String,
    #[field(validate = structval(1, 20).or_else(msg!("user must be between 1 and 20 chars")))]
    user: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct SearchRoom {
    #[field(validate = structval(1, 30).or_else(msg!("room must be between 1 and 30 chars")))]
    room: String,
    require_password: bool,
}

impl SearchRoom {
    fn new(room: String, require_password: bool) -> SearchRoom {
        SearchRoom {
            room: room,
            require_password: require_password,
        }
    }
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct PubRoom {
    room: String,
    key: String,
    messages: Vec<Message>,
}

impl PubRoom {
    fn new(room: String, key: String, messages: Vec<Message>) -> PubRoom {
        PubRoom {
            room: room,
            key: key,
            messages: messages,
        }
    }
}

fn structval<'v>(val: &String, min: usize, max: usize) -> form::Result<'v, ()> {
    let trimmed = val.trim();
    if trimmed.len() < min || trimmed.len() > max {
        Err(rocket::form::Error::validation("invalid string"))?;
    }
    Ok(())
}

#[post("/message", data = "<form>")]
async fn post(
    form: Form<Message>,
    queue: &State<Sender<Message>>,
    session: Session<'_, String>,
) -> Result<(), status::Custom<&'static str>> {
    use diesel::insert_into;

    if let Ok(Some(user)) = session.get().await {
        let connection = &mut rocket_chat::establish_connection();
        let message = form.into_inner();

        if user == message.username.clone() {
            if message.message.clone().trim().len() > 0 {
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
            } else {
                Err(status::Custom(
                    Status::Unauthorized,
                    "no message content found",
                ))
            }
        } else {
            Err(status::Custom(
                Status::Unauthorized,
                "session not maching username",
            ))
        }
    } else {
        Err(status::Custom(Status::Unauthorized, "no valid session"))
    }
}

#[post("/add-room", data = "<form>")]
async fn add_room(
    form: Form<Room>,
    state: &State<AppState>,
    session: Session<'_, String>,
) -> Result<Json<PubRoom>, status::Custom<&'static str>> {
    use rocket_chat::schema::rooms::dsl::*;
    use rocket_chat::schema::rooms_users::dsl::*;
    let connection = &mut rocket_chat::establish_connection();

    if let Ok(Some(username)) = session.get().await {
        let room = form.into_inner();

        if username == room.user.clone() {
            if room.room.clone().trim().len() > 0 {
                if let Ok(roomsdb) = rooms
                    .filter(rocket_chat::schema::rooms::room_name.eq(&room.room))
                    .select(RoomDB::as_select())
                    .load(connection)
                {
                    // Stanza già esistente
                    for r in roomsdb {
                        if (room.require_password.clone()
                            && r.passwd
                                == Some(hash_password(format!(
                                    "{}{}{}",
                                    decrypt_rsa(room.password.clone().unwrap(), state),
                                    r.salt.clone().unwrap(),
                                    PEPPER,
                                ))))
                            || !r.require_password
                        {
                            let result = diesel::insert_into(rooms_users)
                                .values((
                                    rocket_chat::schema::rooms_users::room_name
                                        .eq(room.room.clone()),
                                    rocket_chat::schema::rooms_users::user.eq(&room.user.clone()),
                                ))
                                .execute(connection);
                            if result == Ok(1) {
                                if let Ok(enc) =
                                    encrypt_rsa(r.aes_key.clone(), room.rsa_client_key.clone())
                                {
                                    if let Ok(messages_for_room) = MessageDB::belonging_to(&r)
                                        .select(MessageDB::as_select())
                                        .load(connection)
                                    {
                                        return Ok(Json(PubRoom::new(
                                            room.room,
                                            enc,
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
                                        )));
                                    }
                                } else {
                                    return Err(status::Custom(
                                        Status::InternalServerError,
                                        "RSA error",
                                    ));
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
                    let key = generate_32_byte_random();
                    let sale = generate_32_byte_random();
                    let insert_room = diesel::insert_into(rooms)
                        .values((
                            rocket_chat::schema::rooms::room_name.eq(&room.room),
                            rocket_chat::schema::rooms::passwd.eq(
                                if room.password != Some("null".to_string()) {
                                    Some(hash_password(format!(
                                        "{}{}{}",
                                        decrypt_rsa(room.password.unwrap(), state),
                                        sale,
                                        PEPPER,
                                    )))
                                } else {
                                    None
                                },
                            ),
                            rocket_chat::schema::rooms::require_password.eq(room.require_password),
                            rocket_chat::schema::rooms::hidden_room.eq(room.hidden),
                            rocket_chat::schema::rooms::aes_key.eq(&key),
                            rocket_chat::schema::rooms::salt.eq(sale),
                        ))
                        .execute(connection);
                    let insert_room_user = diesel::insert_into(rooms_users)
                        .values((
                            rocket_chat::schema::rooms_users::room_name.eq(&room.room),
                            rocket_chat::schema::rooms_users::user.eq(&room.user),
                        ))
                        .execute(connection);
                    if insert_room == Ok(1) && insert_room_user == Ok(1) {
                        if let Ok(enc) = encrypt_rsa(key, room.rsa_client_key) {
                            return Ok(Json(PubRoom::new(room.room, enc, Vec::<Message>::new())));
                        } else {
                            return Err(status::Custom(Status::InternalServerError, "RSA error"));
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
            } else {
                Err(status::Custom(
                    Status::Unauthorized,
                    "empty room name not allowed",
                ))
            }
        } else {
            Err(status::Custom(
                Status::Unauthorized,
                "session not matching username",
            ))
        }
    } else {
        Err(status::Custom(Status::Unauthorized, "no valid session"))
    }
}

#[post("/remove-room", data = "<form>")]
async fn remove_room(
    form: Form<ToRemoveRoom>,
    session: Session<'_, String>,
) -> Result<(), status::Custom<&'static str>> {
    let connection = &mut rocket_chat::establish_connection();

    if let Ok(Some(user)) = session.get().await {
        let room = form.clone().room;
        let for_user = form.into_inner().user;
        if user == for_user {
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
        } else {
            Err(status::Custom(
                Status::Unauthorized,
                "session not matching user",
            ))
        }
    } else {
        Err(status::Custom(Status::Unauthorized, "no valid session"))
    }
}

#[get("/get-rooms")]
fn search_rooms() -> Result<Json<Vec<SearchRoom>>, status::Custom<&'static str>> {
    use rocket_chat::schema::rooms::dsl::*;
    let connection = &mut rocket_chat::establish_connection();

    if let Ok(roomsdb) = rooms
        .filter(hidden_room.eq(false))
        .select(RoomDB::as_select())
        .load(connection)
    {
        let pub_rooms = roomsdb
            .iter()
            .map(|room| SearchRoom::new(room.room_name.clone(), room.require_password))
            .collect::<Vec<SearchRoom>>();

        Ok(Json(pub_rooms))
    } else {
        Err(status::Custom(
            Status::InternalServerError,
            "Database error",
        ))
    }
}

#[post("/get-personal-rooms", data = "<form>")]
async fn get_rooms(
    form: Form<GetRoomsUser>,
    session: Session<'_, String>,
) -> Result<Json<Vec<PubRoom>>, status::Custom<&'static str>> {
    let userform = form.into_inner();
    let rsa_key = userform.rsa_key;
    let connection = &mut rocket_chat::establish_connection();
    if let Ok(Some(user)) = session.get().await {
        if user == userform.username.clone() {
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

                Ok(Json(pub_rooms))
            } else {
                Err(status::Custom(
                    Status::InternalServerError,
                    "Database error",
                ))
            }
        } else {
            Err(status::Custom(
                Status::Unauthorized,
                "session not matching user",
            ))
        }
    } else {
        Err(status::Custom(Status::Unauthorized, "no valid session"))
    }
}

#[get("/get-user")]
async fn get_user(session: Session<'_, String>) -> Result<String, Redirect> {
    if let Ok(Some(usr)) = session.get().await {
        Ok(usr)
    } else {
        Err(Redirect::to("/login"))
    }
}

#[post("/login", data = "<form>")]
async fn login(
    form: Form<LoginUser>,
    state: &State<AppState>,
    session: Session<'_, String>,
) -> Result<&'static str, status::Custom<&'static str>> {
    use rocket_chat::schema::users::dsl::*;

    if let Ok(Some(_)) = session.get().await {
        Err(status::Custom(Status::Unauthorized, "Not authorized"))
    } else {
        let userform = form.into_inner();
        let passw = decrypt_rsa(userform.password, state);
        let connection = &mut rocket_chat::establish_connection();

        if let Ok(result) = users
            .limit(1)
            .filter(username.eq(&userform.username))
            .select(UserDB::as_select())
            .load(connection)
        {
            if result.len() > 0
                && hash_password(format!("{}{}{}", passw, result[0].salt, PEPPER))
                    == result[0].passwd
            {
                if let Ok(_) = session.set(userform.username).await {
                    Ok("GRANTED")
                } else {
                    Err(status::Custom(
                        Status::InternalServerError,
                        "Unable to set cookies",
                    ))
                }
            } else {
                Err(status::Custom(Status::Unauthorized, "Not authorized"))
            }
        } else {
            Err(status::Custom(
                Status::InternalServerError,
                "Database error",
            ))
        }
    }
}

#[post("/signup", data = "<form>")]
async fn signup(
    form: Form<SignupUser>,
    state: &State<AppState>,
    session: Session<'_, String>,
) -> Result<&'static str, status::Custom<&'static str>> {
    use rocket_chat::schema::users::dsl::*;

    if let Ok(Some(_)) = session.get().await {
        Err(status::Custom(Status::Unauthorized, "Not authorized"))
    } else {
        let userr = form.into_inner();
        let passw = decrypt_rsa(userr.password, state);
        let sale = generate_32_byte_random();
        let connection = &mut rocket_chat::establish_connection();

        if userr.username.clone().trim().len() > 0 {
            if let Ok(1) = diesel::insert_into(users)
                .values((
                    rocket_chat::schema::users::username.eq(userr.username.clone().trim()),
                    rocket_chat::schema::users::passwd
                        .eq(hash_password(format!("{}{}{}", passw, &sale, PEPPER))),
                    rocket_chat::schema::users::salt.eq(sale),
                ))
                .execute(connection)
            {
                if let Ok(_) = session.set(userr.username.trim().to_string()).await {
                    Ok("GRANTED")
                } else {
                    Err(status::Custom(
                        Status::InternalServerError,
                        "Unable to set cookies",
                    ))
                }
            } else {
                Err(status::Custom(Status::Unauthorized, "Not authorized"))
            }
        } else {
            Err(status::Custom(
                Status::Unauthorized,
                "no username empty allowed",
            ))
        }
    }
}

#[post("/change-pass", data = "<form>")]
async fn change_password(
    form: Form<ChangePassword>,
    state: &State<AppState>,
    session: Session<'_, String>,
) -> Result<&'static str, status::Custom<&'static str>> {
    use rocket_chat::schema::users::dsl::*;
    let change = form.into_inner();
    if let Ok(Some(user)) = session.get().await {
        if user.clone() == change.user {
            let old_password = decrypt_rsa(change.old_password, state);
            let new_password = decrypt_rsa(change.new_password, state);
            let connection = &mut rocket_chat::establish_connection();
            if let Ok(results) = users
                .limit(1)
                .filter(username.eq(&user))
                .select(UserDB::as_select())
                .load(connection)
            {
                if results.len() > 0 {
                    if let Ok(_) = diesel::update(users.filter(username.eq(user).and(passwd.eq(
                        hash_password(format!("{}{}{}", old_password, results[0].salt, PEPPER)),
                    ))))
                    .set(passwd.eq(hash_password(format!(
                        "{}{}{}",
                        new_password, results[0].salt, PEPPER
                    ))))
                    .execute(connection)
                    {
                        return Ok("fatto");
                    } else {
                        return Err(status::Custom(Status::InternalServerError, "db error"));
                    }
                }
            }
            return Err(status::Custom(Status::Unauthorized, "used not there"));
        }
        return Err(status::Custom(Status::Unauthorized, "incongruent data"));
    }
    Err(status::Custom(Status::Unauthorized, "no session found"))
}

#[get("/logout")]
async fn logout(session: Session<'_, String>) -> Redirect {
    if let Ok(_) = session.remove().await {
        Redirect::to(uri!(login_page))
    } else {
        Redirect::to(uri!(chat_page))
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

fn generate_32_byte_random() -> String {
    let mut key = [0u8; 32];
    match rand::thread_rng().try_fill(&mut key) {
        Ok(_) => BASE64_STANDARD.encode(key),
        Err(e) => format!("Errore nel generare la chiave: {}", e),
    }
}

#[get("/login")]
async fn login_page(session: Session<'_, String>) -> Result<Option<NamedFile>, Redirect> {
    if let Ok(Some(_)) = session.get().await {
        Err(Redirect::to(uri!(chat_page)))
    } else {
        Ok(NamedFile::open("pages/login.html").await.ok())
    }
}

#[get("/")]
async fn chat_page(session: Session<'_, String>) -> Result<Option<NamedFile>, Redirect> {
    if let Ok(Some(_)) = session.get().await {
        Ok(NamedFile::open("pages/chat.html").await.ok())
    } else {
        Err(Redirect::to(uri!(login_page)))
    }
}

#[launch]
fn rocket() -> _ {
    let memory_store: MemoryStore<String> = MemoryStore::default();
    let store: SessionStore<String> = SessionStore {
        store: Box::new(memory_store),
        name: "token".into(),
        duration: Duration::from_secs(3600 * 24 * 3),
        cookie: CookieConfig::default(),
    };

    rocket::build()
        .attach(store.fairing())
        .manage(channel::<Message>(1024).0)
        .manage(AppState {
            keys: Mutex::new(Some(generate_key_pair())),
        })
        .mount(
            "/",
            routes![
                login_page,
                chat_page,
                get_user,
                post,
                add_room,
                remove_room,
                search_rooms,
                get_rooms,
                login,
                signup,
                change_password,
                logout,
                get_rsa_pub_key,
                events
            ],
        )
        .mount("/", FileServer::from(relative!("static")))
}
