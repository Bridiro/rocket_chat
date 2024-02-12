// @generated automatically by Diesel CLI.

diesel::table! {
    messages (message_id) {
        message_id -> Integer,
        #[max_length = 30]
        room_name -> Varchar,
        #[max_length = 20]
        username -> Varchar,
        content -> Text,
        message_time -> Nullable<Datetime>,
    }
}

diesel::table! {
    rooms (room_name) {
        #[max_length = 30]
        room_name -> Varchar,
        passwd -> Nullable<Text>,
        require_password -> Bool,
        hidden_room -> Bool,
        aes_key -> Text,
    }
}

diesel::table! {
    rooms_users (room_name, user) {
        #[max_length = 30]
        room_name -> Varchar,
        #[max_length = 20]
        user -> Varchar,
    }
}

diesel::table! {
    users (username) {
        #[max_length = 20]
        username -> Varchar,
        passwd -> Text,
    }
}

diesel::joinable!(messages -> rooms (room_name));
diesel::joinable!(messages -> users (username));
diesel::joinable!(rooms_users -> rooms (room_name));
diesel::joinable!(rooms_users -> users (user));

diesel::allow_tables_to_appear_in_same_query!(
    messages,
    rooms,
    rooms_users,
    users,
);
