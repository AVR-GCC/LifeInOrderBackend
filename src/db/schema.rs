// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "habit_type"))]
    pub struct HabitType;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::HabitType;

    habits (id) {
        id -> Int4,
        user_id -> Int4,
        name -> Varchar,
        weight -> Int4,
        sequence -> Int4,
        habit_type -> HabitType,
        created_at -> Timestamp,
    }
}

diesel::table! {
    options (id) {
        id -> Int4,
        label -> Nullable<Varchar>,
        sequence -> Int4,
        habit_id -> Int4,
        color -> Nullable<Varchar>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    refresh_tokens (id) {
        id -> Int4,
        user_id -> Int4,
        token_hash -> Text,
        family_id -> Text,
        expires_at -> Timestamptz,
        revoked_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        name -> Varchar,
        email -> Varchar,
        created_at -> Timestamp,
        #[max_length = 255]
        password_hash -> Nullable<Varchar>,
    }
}

diesel::table! {
    values (id) {
        id -> Int4,
        value_id -> Int4,
        habit_id -> Int4,
        date -> Date,
        text -> Nullable<Varchar>,
        number -> Nullable<Int4>,
        created_at -> Timestamp,
    }
}

diesel::joinable!(habits -> users (user_id));
diesel::joinable!(options -> habits (habit_id));
diesel::joinable!(refresh_tokens -> users (user_id));
diesel::joinable!(values -> habits (habit_id));
diesel::joinable!(values -> options (value_id));

diesel::allow_tables_to_appear_in_same_query!(habits, options, refresh_tokens, users, values,);
