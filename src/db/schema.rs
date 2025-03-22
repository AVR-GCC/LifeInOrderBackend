diesel::table! {
    habit_values (id) {
        id -> Int4,
        habit_id -> Int4,
        color -> Nullable<Varchar>,
        created_at -> Nullable<Timestamp>,
    }
}

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

diesel::joinable!(habit_values -> user_habits (habit_id));
diesel::joinable!(user_days -> users (user_id));
diesel::joinable!(user_habits -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    habit_values,
    user_days,
    user_habits,
    users,
);
