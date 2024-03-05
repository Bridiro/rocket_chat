use chrono::NaiveDateTime;

use diesel::prelude::*;

use crate::schema::{messages, rooms, rooms_users, users};

#[derive(Queryable, Selectable, Identifiable, Associations, Debug, PartialEq)]
#[diesel(belongs_to(RoomDB, foreign_key = room_name))]
#[diesel(belongs_to(UserDB, foreign_key = username))]
#[diesel(table_name = messages)]
#[diesel(primary_key(message_id))]
pub struct MessageDB {
    pub message_id: i32,
    pub room_name: String,
    pub username: String,
    pub content: String,
    pub message_time: Option<NaiveDateTime>,
}

#[derive(Queryable, Selectable, Identifiable, Debug, PartialEq)]
#[diesel(table_name = rooms)]
#[diesel(primary_key(room_name))]
pub struct RoomDB {
    pub room_name: String,
    pub passwd: Option<String>,
    pub require_password: bool,
    pub hidden_room: bool,
    pub aes_key: String,
    pub salt: Option<String>,
}

#[derive(Identifiable, Selectable, Queryable, Associations, Debug)]
#[diesel(belongs_to(RoomDB, foreign_key = room_name))]
#[diesel(belongs_to(UserDB, foreign_key = user))]
#[diesel(table_name = rooms_users)]
#[diesel(primary_key(room_name, user))]
pub struct RoomUserDB {
    pub room_name: String,
    pub user: String,
}

#[derive(Queryable, Identifiable, Selectable, Debug, PartialEq)]
#[diesel(table_name = users)]
#[diesel(primary_key(username))]
pub struct UserDB {
    pub username: String,
    pub passwd: String,
    pub salt: String,
}
