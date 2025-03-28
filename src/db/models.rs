use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use chrono::{NaiveDate, NaiveDateTime};

#[derive(Queryable, Serialize)]
pub struct User {
    pub id: i32,
    pub name: String,
}

#[derive(Insertable, Deserialize)]
#[diesel(table_name = crate::db::schema::users)]
pub struct NewUser {
    pub name: String,
    pub email: String
}

#[derive(Debug, Queryable, Serialize)]
pub struct UserDay {
    pub id: i32,
    pub user_id: i32,
    pub date: NaiveDate,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize)]
#[diesel(table_name = crate::db::schema::user_days)]
pub struct NewUserDay {
    pub user_id: i32,
    pub date: NaiveDate,
}

#[derive(Queryable, Serialize)]
pub struct UserHabit {
    pub id: i32,
    pub user_id: i32,
    pub name: String,
    pub weight: i32,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize)]
#[diesel(table_name = crate::db::schema::user_habits)]
pub struct NewUserHabit {
    pub user_id: i32,
    pub name: String,
    pub weight: i32,
}

#[derive(Queryable, Serialize)]
pub struct HabitValue {
    pub id: i32,
    pub habit_id: i32,
    pub color: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize)]
#[diesel(table_name = crate::db::schema::habit_values)]
pub struct NewHabitValue {
    pub habit_id: i32,
    pub color: String,
}

#[derive(Queryable, Serialize)]
pub struct DayValue {
    pub id: i32,
    pub value_id: i32,
    pub user_day_id: i32,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize)]
#[diesel(table_name = crate::db::schema::day_values)]
pub struct NewDayValue {
    pub value_id: i32,
    pub user_day_id: i32,
}
