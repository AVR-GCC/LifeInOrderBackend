diesel::table! {
    user_days (id) {
        id -> Int4,
        user_id -> Int4,
        date -> Date,
        created_at -> Timestamp,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        name -> Varchar,
        email -> Varchar,
    }
}

diesel::joinable!(user_days -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    user_days,
    users,
);
