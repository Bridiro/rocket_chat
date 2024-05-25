use chrono::NaiveDateTime;

use diesel::prelude::*;

use crate::schema::{admins, email_tokens, messages, rooms, rooms_users, users};

#[derive(Queryable, Selectable, Identifiable, Associations, Debug, PartialEq)]
#[diesel(belongs_to(RoomDB, foreign_key = room_id))]
#[diesel(belongs_to(UserDB, foreign_key = user_id))]
#[diesel(table_name = messages)]
#[diesel(primary_key(message_id))]
pub struct MessageDB {
    pub message_id: i32,
    pub room_id: i32,
    pub user_id: i32,
    pub content: String,
    pub message_time: Option<NaiveDateTime>,
}

#[derive(Queryable, Selectable, Identifiable, Debug, PartialEq)]
#[diesel(table_name = rooms)]
#[diesel(primary_key(id))]
pub struct RoomDB {
    pub id: i32,
    pub room_name: String,
    pub passwd: Option<String>,
    pub require_password: bool,
    pub hidden_room: bool,
    pub aes_key: String,
    pub salt: String,
}

#[derive(Identifiable, Selectable, Queryable, Associations, Debug)]
#[diesel(belongs_to(RoomDB, foreign_key = room_id))]
#[diesel(belongs_to(UserDB, foreign_key = user_id))]
#[diesel(table_name = rooms_users)]
#[diesel(primary_key(room_id, user_id))]
pub struct RoomUserDB {
    pub room_id: i32,
    pub user_id: i32,
}

#[derive(Queryable, Identifiable, Selectable, Debug, PartialEq)]
#[diesel(table_name = users)]
#[diesel(primary_key(id))]
pub struct UserDB {
    pub id: i32,
    pub full_name: String,
    pub surname: String,
    pub email: String,
    pub username: String,
    pub passwd: String,
    pub salt: String,
    pub email_verified: bool,
}

#[derive(Queryable, Selectable, Identifiable, Associations, Debug, PartialEq)]
#[diesel(belongs_to(UserDB, foreign_key = id))]
#[diesel(table_name = admins)]
#[diesel(primary_key(id))]
pub struct AdminDB {
    pub id: i32,
}

#[derive(Queryable, Identifiable, Selectable, Debug, PartialEq)]
#[diesel(belongs_to(UserDB, foreign_key = user_id))]
#[diesel(table_name = email_tokens)]
#[diesel(primary_key(user_id))]
pub struct EmailTokenDB {
    pub user_id: i32,
    pub token: String,
}
