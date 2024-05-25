// @generated automatically by Diesel CLI.

diesel::table! {
    admins (id) {
        id -> Integer,
    }
}

diesel::table! {
    email_tokens (user_id) {
        user_id -> Integer,
        token -> Text,
    }
}

diesel::table! {
    messages (message_id) {
        message_id -> Integer,
        room_id -> Integer,
        user_id -> Integer,
        content -> Text,
        message_time -> Nullable<Datetime>,
    }
}

diesel::table! {
    rooms (id) {
        id -> Integer,
        #[max_length = 30]
        room_name -> Varchar,
        passwd -> Nullable<Text>,
        require_password -> Bool,
        hidden_room -> Bool,
        aes_key -> Text,
        salt -> Text,
    }
}

diesel::table! {
    rooms_users (room_id, user_id) {
        room_id -> Integer,
        user_id -> Integer,
    }
}

diesel::table! {
    users (id) {
        id -> Integer,
        #[max_length = 100]
        full_name -> Varchar,
        #[max_length = 100]
        surname -> Varchar,
        #[max_length = 100]
        email -> Varchar,
        #[max_length = 20]
        username -> Varchar,
        passwd -> Text,
        salt -> Text,
        email_verified -> Bool,
    }
}

diesel::joinable!(admins -> users (id));
diesel::joinable!(email_tokens -> users (user_id));
diesel::joinable!(messages -> rooms (room_id));
diesel::joinable!(messages -> users (user_id));
diesel::joinable!(rooms_users -> rooms (room_id));
diesel::joinable!(rooms_users -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    admins,
    email_tokens,
    messages,
    rooms,
    rooms_users,
    users,
);
