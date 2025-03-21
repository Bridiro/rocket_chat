#[macro_use]
extern crate rocket;

use base64::prelude::*;
use diesel::prelude::*;
use rand::Rng;
use rocket::serde::json::serde_json;
use rocket::tokio;
use rocket::tokio::sync::mpsc::{self, unbounded_channel, UnboundedReceiver, UnboundedSender};
use rocket::tokio::sync::RwLock;
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
use rocket_session_store::{memory::MemoryStore, Session, SessionStore};
use rsa::{
    pkcs1::EncodeRsaPublicKey,
    pkcs8::{DecodePublicKey, LineEnding},
    Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey,
};
use sha2::{Digest, Sha512};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::{error::Error, path::PathBuf, sync::Mutex, time::Duration};
use ws::Message;
use rocket::http::private::cookie::CookieBuilder;

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
struct GetPersonalChats {
    user_id: i32,
    rsa_key: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct WhoAmI {
    id: i32,
    admin: i32,
    username: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct LoginUser {
    #[field(validate = structval(1, 20).or_else(msg!("user must be between 1 and 20 chars")))]
    username: String,
    #[field(validate = len(8..))]
    password: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct SignupUser {
    #[field(validate = structval(1, 100).or_else(msg!("user must be between 1 and 100 chars")))]
    full_name: String,
    #[field(validate = structval(1, 100).or_else(msg!("user must be between 1 and 100 chars")))]
    surname: String,
    #[field(validate = structval(1, 100).or_else(msg!("user must be between 1 and 100 chars")))]
    email: String,
    #[field(validate = structval(1, 20).or_else(msg!("user must be between 1 and 20 chars")))]
    username: String,
    #[field(validate = len(8..))]
    password: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct UserId {
    id: i32,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct GroupMessage {
    room_id: i32,
    user_id: i32,
    user_name: String,
    #[field(validate = len(1..))]
    message: String,
}

impl GroupMessage {
    fn new(room_id: i32, user_id: i32, user_name: String, message: String) -> GroupMessage {
        GroupMessage {
            room_id: room_id,
            user_id: user_id,
            user_name: user_name,
            message: message,
        }
    }
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct ChangePassword {
    user_id: i32,
    #[field(validate = len(8..))]
    old_password: String,
    #[field(validate = len(8..))]
    new_password: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct Room {
    room_id: i32,
    room_name: String,
    password: Option<String>,
    require_password: bool,
    hidden: bool,
    user_id: i32,
    rsa_client_key: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct AddRoom {
    room_name: String,
    password: Option<String>,
    require_password: bool,
    hidden: bool,
    user_id: i32,
    rsa_client_key: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct ToRemoveRoom {
    room_id: i32,
    user_id: i32,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct SearchRoom {
    room_id: i32,
    #[field(validate = structval(1, 30).or_else(msg!("room must be between 1 and 30 chars")))]
    room_name: String,
    require_password: bool,
}

impl SearchRoom {
    fn new(room_id: i32, room_name: String, require_password: bool) -> SearchRoom {
        SearchRoom {
            room_id: room_id,
            room_name: room_name,
            require_password: require_password,
        }
    }
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct PubRoom {
    room_id: i32,
    room_name: String,
    key: String,
    messages: Vec<GroupMessage>,
}

impl PubRoom {
    fn new(room_id: i32, room_name: String, key: String, messages: Vec<GroupMessage>) -> PubRoom {
        PubRoom {
            room_id: room_id,
            room_name: room_name,
            key: key,
            messages: messages,
        }
    }
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct Direct {
    user_id: i32,
    user_name: String,
    key: String,
    messages: Vec<DirectMessage>,
}

impl Direct {
    fn new(user_id: i32, user_name: String, key: String, messages: Vec<DirectMessage>) -> Direct {
        Direct {
            user_id: user_id,
            user_name: user_name,
            key: key,
            messages: messages,
        }
    }
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct DirectMessage {
    user_id: i32,
    message: String,
}

impl DirectMessage {
    fn new(user_id: i32, message: String) -> DirectMessage {
        DirectMessage {
            user_id: user_id,
            message: message,
        }
    }
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct AddDirect {
    user_id: i32,
    username: String,
    rsa_key: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct DirectToAdd {
    id: i32,
    key: String,
}

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
struct DeleteDirect {
    user_id: i32,
    recipient_id: i32,
}

fn structval<'v>(val: &String, min: usize, max: usize) -> form::Result<'v, ()> {
    let trimmed = val.trim();
    if trimmed.len() < min || trimmed.len() > max {
        Err(rocket::form::Error::validation("invalid string"))?;
    }
    Ok(())
}

type Users = Arc<RwLock<HashMap<i32, mpsc::UnboundedSender<Message>>>>;
type Groups = Arc<RwLock<HashMap<i32, HashSet<i32>>>>;

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
enum ChatMessage {
    Direct {
        sender: i32,
        recipient: i32,
        content: String,
    },
    Group {
        sender_id: i32,
        sender_name: String,
        group_id: i32,
        content: String,
    },
}

#[get("/messages/<user_id>")]
async fn messages<'r>(
    user_id: i32,
    ws: ws::WebSocket,
    users: &'r State<Users>,
    groups: &'r State<Arc<RwLock<Groups>>>,
    session: Session<'_, (i32, i32, String)>,
) -> Result<ws::Channel<'r>, status::Custom<&'static str>> {
    use rocket::futures::{SinkExt, StreamExt};

    if let Ok(Some(user)) = session.get().await {
        if user.0 == user_id {
            let (tx, mut rx): (UnboundedSender<Message>, UnboundedReceiver<Message>) =
                unbounded_channel();
            users.write().await.insert(user_id, tx);
            {
                let group_write_lock = groups.write().await;
                let mut groups = group_write_lock.write().await;

                if !users.read().await.contains_key(&user_id) {
                    return Err(status::Custom(
                        Status::InternalServerError,
                        "error inserting message",
                    ));
                }

                if let Ok(groups_for_user) = rocket_chat::schema::rooms_users::table
                    .filter(rocket_chat::schema::rooms_users::user_id.eq(user_id))
                    .select(RoomUserDB::as_select())
                    .load(&mut rocket_chat::establish_connection())
                {
                    for group in groups_for_user {
                        if let Some(group_members) = groups.get_mut(&group.room_id) {
                            group_members.insert(user_id);
                        } else {
                            let mut new_group = HashSet::new();
                            new_group.insert(user_id);
                            groups.insert(group.room_id, new_group);
                        }
                    }
                }
            }

            Ok(ws.channel(move |mut stream| {
                Box::pin(async move {
                    loop {
                        tokio::select! {
                            Some(Ok(msg)) = stream.next() => {
                                if let Message::Text(text) = msg {
                                    if let Ok(chat_message) = serde_json::from_str::<ChatMessage>(&text) {
                                        match chat_message {
                                            ChatMessage::Direct { recipient, .. } => {
                                                let users = users.read().await;
                                                if let Some(sender) = users.get(&recipient) {
                                                    if let Err(err) = sender.send(Message::text(text.clone())) {
                                                        eprintln!("Failed to send message to recipient: {:?}", err);
                                                    }
                                                }
                                            },
                                            ChatMessage::Group { group_id, .. } => {
                                                let groups_lock = groups.read().await;
                                                let groups = groups_lock.read().await;
                                                if let Some(members) = groups.get(&group_id) {
                                                    let users = users.read().await;
                                                    for member in members {
                                                        if member != &user_id {
                                                            if let Some(sender) = users.get(member) {
                                                                if let Err(err) = sender.send(Message::text(text.clone())) {
                                                                    eprintln!("Failed to send message to group member: {:?}", err);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            },
                                        }
                                        save_msg_db(chat_message);
                                    } else {
                                        eprintln!("Failed to deserialize incoming message: {:?}", text);
                                    }
                                }
                            },
                            Some(msg) = rx.recv() => {
                                if stream.send(msg).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }

                    {
                        users.write().await.remove(&user_id);
                        let group_write_lock = groups.write().await;
                        let mut groups = group_write_lock.write().await;
                        if let Ok(groups_for_user) = rocket_chat::schema::rooms_users::table
                            .filter(rocket_chat::schema::rooms_users::user_id.eq(&user_id))
                            .select(RoomUserDB::as_select())
                            .load(&mut rocket_chat::establish_connection())
                        {
                            for group in groups_for_user {
                                if let Some(group_members) = groups.get_mut(&group.room_id) {
                                    group_members.remove(&user_id);
                                }
                            }
                        }
                    }

                    Ok(())
                })
            }))
        } else {
            return Err(status::Custom(Status::Unauthorized, "no valid session"));
        }
    } else {
        return Err(status::Custom(Status::Unauthorized, "no valid session"));
    }
}

#[post("/message", data = "<form>")]
async fn post(
    form: Form<GroupMessage>,
    queue: &State<Sender<GroupMessage>>,
    session: Session<'_, (i32, i32, String)>,
) -> Result<(), status::Custom<&'static str>> {
    use diesel::insert_into;

    if let Ok(Some(user)) = session.get().await {
        let connection = &mut rocket_chat::establish_connection();
        let message = form.into_inner();

        if user.1 == 1 {
            if let Ok(_) = insert_into(rocket_chat::schema::messages::dsl::messages)
                .values((
                    rocket_chat::schema::messages::room_id.eq(&message.room_id),
                    rocket_chat::schema::messages::user_id.eq(&message.user_id),
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
                "session not maching username",
            ))
        }
    } else {
        Err(status::Custom(Status::Unauthorized, "no valid session"))
    }
}

#[post("/add-room", data = "<form>")]
async fn add_room(
    form: Form<AddRoom>,
    state: &State<AppState>,
    session: Session<'_, (i32, i32, String)>,
    groups: &State<Arc<RwLock<Groups>>>,
) -> Result<Json<PubRoom>, status::Custom<&'static str>> {
    use rocket_chat::schema::rooms::dsl::*;
    use rocket_chat::schema::rooms_users::dsl::*;
    let connection = &mut rocket_chat::establish_connection();

    if let Ok(Some(user)) = session.get().await {
        let room = form.into_inner();

        if user.0 == room.user_id {
            if let Ok(roomsdb) = rooms
                .filter(rocket_chat::schema::rooms::room_name.eq(&room.room_name))
                .select(RoomDB::as_select())
                .load::<RoomDB>(connection)
            {
                // Stanza già esistente
                for r in roomsdb {
                    if (room.require_password.clone()
                        && r.passwd
                            == Some(hash_password(format!(
                                "{}{}{}",
                                decrypt_rsa(room.password.clone().unwrap(), state),
                                r.salt.clone(),
                                PEPPER,
                            ))))
                        || !r.require_password
                    {
                        let result = diesel::insert_into(rooms_users)
                            .values((
                                rocket_chat::schema::rooms_users::room_id.eq(r.id),
                                rocket_chat::schema::rooms_users::user_id.eq(&room.user_id),
                            ))
                            .execute(connection);
                        if result == Ok(1) {
                            let group_write_lock = groups.write().await;
                            let mut groups = group_write_lock.write().await;
                            if let Some(group_members) = groups.get_mut(&r.id) {
                                group_members.insert(room.user_id);
                            } else {
                                let mut new_group = HashSet::new();
                                new_group.insert(room.user_id);
                                groups.insert(r.id, new_group);
                            }
                            if let Ok(enc) =
                                encrypt_rsa(r.aes_key.clone(), room.rsa_client_key.clone())
                            {
                                if let Ok(messages_with_user) = rocket_chat::schema::messages::table
                                    .filter(rocket_chat::schema::messages::room_id.eq(r.id))
                                    .inner_join(rocket_chat::schema::users::table)
                                    .select((MessageDB::as_select(), UserDB::as_select()))
                                    .load::<(MessageDB, UserDB)>(connection)
                                {
                                    return Ok(Json(PubRoom::new(
                                        r.id,
                                        room.room_name,
                                        enc,
                                        messages_with_user
                                            .iter()
                                            .map(|(m, u)| {
                                                GroupMessage::new(
                                                    m.room_id,
                                                    m.user_id,
                                                    u.username.clone(),
                                                    m.content.clone(),
                                                )
                                            })
                                            .collect::<Vec<GroupMessage>>(),
                                    )));
                                } else {
                                    return Err(status::Custom(
                                        Status::InternalServerError,
                                        "Database error",
                                    ));
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
                        rocket_chat::schema::rooms::room_name.eq(&room.room_name),
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
                let inserted_id = rocket_chat::schema::rooms::table
                    .filter(rocket_chat::schema::rooms::room_name.eq(&room.room_name))
                    .select(rocket_chat::schema::rooms::id)
                    .first::<i32>(connection);
                let insert_room_user = diesel::insert_into(rooms_users)
                    .values((
                        rocket_chat::schema::rooms_users::room_id.eq(inserted_id.as_ref().unwrap()),
                        rocket_chat::schema::rooms_users::user_id.eq(&room.user_id),
                    ))
                    .execute(connection);
                if insert_room == Ok(1) && insert_room_user == Ok(1) {
                    let group_write_lock = groups.write().await;
                    let mut groups = group_write_lock.write().await;
                    let mut new_group = HashSet::new();
                    new_group.insert(room.user_id);
                    groups.insert(*inserted_id.as_ref().unwrap(), new_group);
                    if let Ok(enc) = encrypt_rsa(key, room.rsa_client_key) {
                        return Ok(Json(PubRoom::new(
                            inserted_id.unwrap(),
                            room.room_name,
                            enc,
                            Vec::<GroupMessage>::new(),
                        )));
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
    session: Session<'_, (i32, i32, String)>,
    groups: &State<Arc<RwLock<Groups>>>,
) -> Result<(), status::Custom<&'static str>> {
    use rocket_chat::schema::rooms_users::dsl::*;

    let connection = &mut rocket_chat::establish_connection();

    if let Ok(Some(user)) = session.get().await {
        let room = form.room_id;
        let for_user = form.user_id;
        if user.0 == for_user {
            if let Ok(_) = diesel::delete(
                rocket_chat::schema::rooms_users::table
                    .filter(room_id.eq(room).and(user_id.eq(for_user))),
            )
            .execute(connection)
            {
                let group_write_lock = groups.write().await;
                let mut groups = group_write_lock.write().await;
                if let Some(group_members) = groups.get_mut(&room) {
                    group_members.remove(&for_user);
                }
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
            .map(|room| SearchRoom::new(room.id, room.room_name.clone(), room.require_password))
            .collect::<Vec<SearchRoom>>();

        Ok(Json(pub_rooms))
    } else {
        Err(status::Custom(
            Status::InternalServerError,
            "Database error",
        ))
    }
}

#[post("/add-direct", data = "<form>")]
async fn add_direct(
    form: Form<AddDirect>,
    session: Session<'_, (i32, i32, String)>,
) -> Result<Json<DirectToAdd>, status::Custom<&'static str>> {
    use rocket_chat::schema::directs::dsl::*;
    let connection = &mut rocket_chat::establish_connection();

    if let Ok(Some(user)) = session.get().await {
        let userform = form.into_inner();
        if user.0 == userform.user_id {
            if let Ok(recipient) = rocket_chat::schema::users::table
                .filter(rocket_chat::schema::users::username.eq(userform.username))
                .select(UserDB::as_select())
                .limit(1)
                .load::<UserDB>(connection)
            {
                let key = generate_32_byte_random();
                if let Ok(_) = diesel::insert_into(directs)
                    .values((
                        user1_id.eq(userform.user_id),
                        user2_id.eq(recipient[0].id),
                        aes_key.eq(&key),
                    ))
                    .execute(connection)
                {
                    Ok(Json(DirectToAdd {
                        id: recipient[0].id,
                        key: encrypt_rsa(key, userform.rsa_key).unwrap(),
                    }))
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
                "session not matching user",
            ))
        }
    } else {
        Err(status::Custom(Status::Unauthorized, "no valid session"))
    }
}

#[post("/delete-direct", data = "<form>")]
async fn delete_direct(
    form: Form<DeleteDirect>,
    session: Session<'_, (i32, i32, String)>,
) -> Result<(), status::Custom<&'static str>> {
    use rocket_chat::schema::directs::dsl::*;
    let connection = &mut rocket_chat::establish_connection();

    if let Ok(Some(user)) = session.get().await {
        let userform = form.into_inner();
        if user.0 == userform.user_id {
            if let Ok(_) = diesel::delete(
                directs.filter(
                    user1_id
                        .eq(userform.user_id)
                        .and(user2_id.eq(userform.recipient_id))
                        .or(user1_id
                            .eq(userform.recipient_id)
                            .and(user2_id.eq(userform.user_id))),
                ),
            )
            .execute(connection)
            {
                Ok(())
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

#[post("/get-directs", data = "<form>")]
async fn get_directs(
    form: Form<GetPersonalChats>,
    session: Session<'_, (i32, i32, String)>,
) -> Result<Json<Vec<Direct>>, status::Custom<&'static str>> {
    let userform = form.into_inner();
    let rsa_key = userform.rsa_key;
    let connection = &mut rocket_chat::establish_connection();
    if let Ok(Some(user)) = session.get().await {
        if user.0 == userform.user_id {
            if let Ok(directs) = rocket_chat::schema::directs::table
                .filter(
                    rocket_chat::schema::directs::user1_id
                        .eq(userform.user_id)
                        .or(rocket_chat::schema::directs::user2_id.eq(userform.user_id)),
                )
                .select(DirectDB::as_select())
                .load(connection)
            {
                if let Ok(recipient) = rocket_chat::schema::users::table
                    .filter(rocket_chat::schema::users::id.eq(
                        if directs[0].user1_id == userform.user_id {
                            directs[0].user2_id
                        } else {
                            directs[0].user1_id
                        },
                    ))
                    .select(UserDB::as_select())
                    .limit(1)
                    .load::<UserDB>(connection)
                {
                    let mut pub_directs: Vec<Direct> = Vec::new();
                    for direct in directs {
                        if let Ok(messages) = DirectMessageDB::belonging_to(&direct)
                            .select(DirectMessageDB::as_select())
                            .load(connection)
                        {
                            pub_directs.push(Direct::new(
                                recipient[0].id,
                                recipient[0].username.clone(),
                                encrypt_rsa(direct.aes_key, rsa_key.clone()).unwrap(),
                                messages
                                    .iter()
                                    .map(|m| DirectMessage::new(m.sender_id, m.message.clone()))
                                    .collect::<Vec<DirectMessage>>(),
                            ));
                        } else {
                            return Err(status::Custom(
                                Status::InternalServerError,
                                "Database error",
                            ));
                        }
                    }

                    Ok(Json(pub_directs))
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
                "session not matching user",
            ))
        }
    } else {
        Err(status::Custom(Status::Unauthorized, "no valid session"))
    }
}

#[post("/get-personal-rooms", data = "<form>")]
async fn get_rooms(
    form: Form<GetPersonalChats>,
    session: Session<'_, (i32, i32, String)>,
) -> Result<Json<Vec<PubRoom>>, status::Custom<&'static str>> {
    let userform = form.into_inner();
    let rsa_key = userform.rsa_key;
    let connection = &mut rocket_chat::establish_connection();
    if let Ok(Some(user)) = session.get().await {
        if user.0 == userform.user_id {
            if let Ok(room_with_roomuser) = rocket_chat::schema::rooms::table
                .inner_join(rocket_chat::schema::rooms_users::table)
                .filter(rocket_chat::schema::rooms_users::user_id.eq(userform.user_id))
                .select((RoomDB::as_select(), RoomUserDB::as_select()))
                .load::<(RoomDB, RoomUserDB)>(connection)
            {
                let mut pub_rooms: Vec<PubRoom> = Vec::new();
                for (room, _room_user) in room_with_roomuser {
                    if let Ok(messages_with_user) = rocket_chat::schema::messages::table
                        .filter(rocket_chat::schema::messages::room_id.eq(room.id))
                        .inner_join(rocket_chat::schema::users::table)
                        .select((MessageDB::as_select(), UserDB::as_select()))
                        .load::<(MessageDB, UserDB)>(connection)
                    {
                        pub_rooms.push(PubRoom::new(
                            room.id,
                            room.room_name,
                            encrypt_rsa(room.aes_key, rsa_key.clone()).unwrap(),
                            messages_with_user
                                .iter()
                                .map(|(m, u)| {
                                    GroupMessage::new(
                                        m.room_id,
                                        m.user_id,
                                        u.username.clone(),
                                        m.content.clone(),
                                    )
                                })
                                .collect::<Vec<GroupMessage>>(),
                        ));
                    } else {
                        return Err(status::Custom(
                            Status::InternalServerError,
                            "Database error",
                        ));
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

#[get("/whoami")]
async fn whoami(session: Session<'_, (i32, i32, String)>) -> Result<Json<WhoAmI>, Redirect> {
    if let Ok(Some(usr)) = session.get().await {
        Ok(Json(WhoAmI {
            id: usr.0,
            admin: usr.1,
            username: usr.2,
        }))
    } else {
        Err(Redirect::to("/login"))
    }
}

#[post("/login", data = "<form>")]
async fn login(
    form: Form<LoginUser>,
    state: &State<AppState>,
    session: Session<'_, (i32, i32, String)>,
) -> Result<Json<UserId>, status::Custom<&'static str>> {
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
                if result[0].email_verified {
                    if let Ok(admin) = AdminDB::belonging_to(&result[0])
                        .select(AdminDB::as_select())
                        .load(connection)
                    {
                        if let Ok(_) = session
                            .set((
                                result[0].id,
                                if admin.len() > 0 { 1 } else { 0 },
                                userform.username,
                            ))
                            .await
                        {
                            Ok(Json(UserId { id: result[0].id }))
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
                    Err(status::Custom(Status::Unauthorized, "Email not verified"))
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
    session: Session<'_, (i32, i32, String)>,
) -> Result<Json<UserId>, status::Custom<&'static str>> {
    use rocket_chat::schema::email_tokens::dsl::*;
    use rocket_chat::schema::users::dsl::*;

    if let Ok(Some(_)) = session.get().await {
        Err(status::Custom(Status::Unauthorized, "Not authorized"))
    } else {
        let userr = form.into_inner();
        let full_namee = userr.full_name.clone();
        let surnamee = userr.surname.clone();
        let usernamee = userr.username.clone();
        let emaill = userr.email.clone();
        let passw = decrypt_rsa(userr.password, state);
        let sale = generate_32_byte_random();
        let connection = &mut rocket_chat::establish_connection();

        if let Ok(1) = diesel::insert_into(users)
            .values((
                full_name.eq(full_namee.trim()),
                surname.eq(surnamee.trim()),
                username.eq(usernamee.trim()),
                email.eq(emaill.trim()),
                passwd.eq(hash_password(format!(
                    "{}{}{}",
                    passw.trim(),
                    &sale,
                    PEPPER
                ))),
                salt.eq(sale),
            ))
            .execute(connection)
        {
            if let Ok(result) = users
                .limit(1)
                .filter(username.eq(&usernamee))
                .select(UserDB::as_select())
                .load(connection)
            {
                if result.len() > 0 {
                    let random_token = generate_32_byte_random();
                    if let Ok(1) = diesel::insert_into(email_tokens)
                        .values((user_id.eq(result[0].id), token.eq(&random_token)))
                        .execute(connection)
                    {
                        while let Err(_) = rocket_chat::send_email(
                            &emaill,
                            "Email verification",
                            &("http://localhost:8000/verify-email/".to_owned() + &random_token),
                        )
                        .await
                        .await
                        {}
                    }
                    Ok(Json(UserId { id: result[0].id }))
                } else {
                    Err(status::Custom(Status::Unauthorized, "Not authorized"))
                }
            } else {
                Err(status::Custom(
                    Status::InternalServerError,
                    "Database error",
                ))
            }
        } else {
            Err(status::Custom(Status::Unauthorized, "Not authorized"))
        }
    }
}

#[get("/verify-email/<emailtoken..>")]
fn confirm_email(emailtoken: PathBuf) -> Redirect {
    use rocket_chat::schema::email_tokens::dsl::*;
    use rocket_chat::schema::users::dsl::*;

    let connection = &mut rocket_chat::establish_connection();

    let tokenstring = emailtoken.into_os_string().into_string().unwrap();

    if let Ok(result) = email_tokens
        .limit(1)
        .filter(token.eq(&tokenstring))
        .select(user_id)
        .load::<i32>(connection)
    {
        if result.len() > 0 {
            if let Ok(_) = diesel::update(users.filter(id.eq(result[0])))
                .set(email_verified.eq(true))
                .execute(connection)
            {
                if let Ok(_) =
                    diesel::delete(email_tokens.filter(token.eq(tokenstring))).execute(connection)
                {
                    return Redirect::to(uri!(login_page));
                }
                Redirect::to(uri!(login_page))
            } else {
                Redirect::to(uri!(login_page))
            }
        } else {
            Redirect::to(uri!(login_page))
        }
    } else {
        Redirect::to(uri!(login_page))
    }
}

#[post("/change-pass", data = "<form>")]
async fn change_password(
    form: Form<ChangePassword>,
    state: &State<AppState>,
    session: Session<'_, (i32, i32, String)>,
) -> Result<&'static str, status::Custom<&'static str>> {
    use rocket_chat::schema::users::dsl::*;
    let change = form.into_inner();
    if let Ok(Some(user)) = session.get().await {
        if user.0 == change.user_id {
            let old_password = decrypt_rsa(change.old_password, state);
            let new_password = decrypt_rsa(change.new_password, state);
            let connection = &mut rocket_chat::establish_connection();
            if let Ok(results) = users
                .limit(1)
                .filter(id.eq(change.user_id))
                .select(UserDB::as_select())
                .load(connection)
            {
                if results.len() > 0 {
                    if let Ok(_) =
                        diesel::update(users.filter(id.eq(change.user_id).and(passwd.eq(
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
async fn logout(session: Session<'_, (i32, i32, String)>) -> Redirect {
    if let Ok(_) = session.remove().await {
        Redirect::to(uri!(login_page))
    } else {
        Redirect::to(uri!(chat_page))
    }
}

#[get("/events")]
async fn events(queue: &State<Sender<GroupMessage>>, mut end: Shutdown) -> EventStream![] {
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

fn save_msg_db(msg: ChatMessage) {
    let connection = &mut rocket_chat::establish_connection();

    match msg {
        ChatMessage::Direct {
            sender,
            recipient,
            content,
        } => {
            if let Ok(direct) = rocket_chat::schema::directs::table
                .filter(
                    rocket_chat::schema::directs::user1_id
                        .eq(sender)
                        .or(rocket_chat::schema::directs::user2_id.eq(sender)),
                )
                .filter(
                    rocket_chat::schema::directs::user1_id
                        .eq(recipient)
                        .or(rocket_chat::schema::directs::user2_id.eq(recipient)),
                )
                .select(DirectDB::as_select())
                .load::<DirectDB>(connection)
            {
                if let Ok(_) =
                    diesel::insert_into(rocket_chat::schema::direct_messages::dsl::direct_messages)
                        .values((
                            rocket_chat::schema::direct_messages::chat_id.eq(direct[0].id),
                            rocket_chat::schema::direct_messages::sender_id.eq(sender),
                            rocket_chat::schema::direct_messages::message.eq(content),
                        ))
                        .execute(connection)
                {
                    return;
                }
            }
        }
        ChatMessage::Group {
            sender_id,
            group_id,
            content,
            ..
        } => {
            if let Ok(_) = diesel::insert_into(rocket_chat::schema::messages::dsl::messages)
                .values((
                    rocket_chat::schema::messages::room_id.eq(group_id),
                    rocket_chat::schema::messages::user_id.eq(sender_id),
                    rocket_chat::schema::messages::content.eq(content),
                ))
                .execute(connection)
            {
                return;
            }
        }
    }
}

#[get("/login")]
async fn login_page(
    session: Session<'_, (i32, i32, String)>,
) -> Result<Option<NamedFile>, Redirect> {
    if let Ok(Some(_)) = session.get().await {
        Err(Redirect::to(uri!(chat_page)))
    } else {
        Ok(NamedFile::open("pages/login.html").await.ok())
    }
}

#[get("/signup")]
async fn signup_page(
    session: Session<'_, (i32, i32, String)>,
) -> Result<Option<NamedFile>, Redirect> {
    if let Ok(Some(_)) = session.get().await {
        Err(Redirect::to(uri!(chat_page)))
    } else {
        Ok(NamedFile::open("pages/signup.html").await.ok())
    }
}

#[get("/")]
async fn chat_page(
    session: Session<'_, (i32, i32, String)>,
) -> Result<Option<NamedFile>, Redirect> {
    if let Ok(Some(_)) = session.get().await {
        Ok(NamedFile::open("pages/chat.html").await.ok())
    } else {
        Err(Redirect::to(uri!(login_page)))
    }
}

#[launch]
fn rocket() -> _ {
    use rocket_chat::schema::rooms::dsl::*;
    let connection = &mut rocket_chat::establish_connection();

    if let Ok(roomsdb) = rooms
        .filter(room_name.eq("lobby"))
        .select(RoomDB::as_select())
        .load(connection)
    {
        if roomsdb.len() == 0 {
            let key = generate_32_byte_random();
            let sale = generate_32_byte_random();
            let _ = diesel::insert_into(rooms)
                .values((
                    room_name.eq("lobby"),
                    require_password.eq(false),
                    hidden_room.eq(false),
                    aes_key.eq(key),
                    salt.eq(sale),
                ))
                .execute(connection);
        }
    }

    let memory_store: MemoryStore<(i32, i32, String)> = MemoryStore::default();
    let store: SessionStore<(i32, i32, String)> = SessionStore {
        store: Box::new(memory_store),
        name: "token".into(),
        duration: Duration::from_secs(3600 * 24 * 3),
        cookie_builder: CookieBuilder::new("", "").path("/"),
    };

    let users: Users = Arc::new(RwLock::new(HashMap::new()));
    let groups: Groups = Arc::new(RwLock::new(HashMap::new()));

    rocket::build()
        .attach(store.fairing())
        .manage(channel::<GroupMessage>(1024).0)
        .manage(AppState {
            keys: Mutex::new(Some(generate_key_pair())),
        })
        .manage(users)
        .manage(Arc::new(RwLock::new(groups)))
        .mount(
            "/",
            routes![
                messages,
                login_page,
                signup_page,
                chat_page,
                whoami,
                post,
                add_room,
                remove_room,
                search_rooms,
                add_direct,
                delete_direct,
                get_directs,
                get_rooms,
                login,
                signup,
                confirm_email,
                change_password,
                logout,
                get_rsa_pub_key,
                events
            ],
        )
        .mount("/", FileServer::from(relative!("static")))
}
