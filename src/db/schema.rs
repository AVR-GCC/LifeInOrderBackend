// @generated automatically by Diesel CLI.

diesel::table! {
    day_values (id) {
        id -> Int4,
        value_id -> Int4,
        user_day_id -> Int4,
        text -> Nullable<Varchar>,
        number -> Nullable<Int4>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    habit_values (id) {
        id -> Int4,
        label -> Nullable<Varchar>,
        sequence -> Int4,
        habit_id -> Int4,
        color -> Nullable<Varchar>,
        created_at -> Timestamp,
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
        sequence -> Int4,
        habit_type -> Varchar,
        created_at -> Timestamp,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        name -> Varchar,
        email -> Varchar,
        created_at -> Timestamp,
    }
}

diesel::joinable!(day_values -> habit_values (value_id));
diesel::joinable!(day_values -> user_days (user_day_id));
diesel::joinable!(habit_values -> user_habits (habit_id));
diesel::joinable!(user_days -> users (user_id));
diesel::joinable!(user_habits -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    day_values,
    habit_values,
    user_days,
    user_habits,
    users,
);
