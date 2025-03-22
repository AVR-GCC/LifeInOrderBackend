diesel::table! {
    user_days (id) {
        id -> Int4,
        user_id -> Int4,
        date -> Date,
        created_at -> Timestamp,
    }
}

diesel::table! {
    user_habits (id) {
        id -> Int4,
        user_id -> Int4,
        name -> Varchar,
        weight -> Int4,
        #[sql_name = "type"]
        type_ -> Varchar,
        created_at -> Nullable<Timestamp>,
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
diesel::joinable!(user_habits -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    user_days,
    user_habits,
    users,
);
