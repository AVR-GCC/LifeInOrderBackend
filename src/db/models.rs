use chrono::{NaiveDate, NaiveDateTime};
use diesel::backend::Backend;
use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Text;
use serde::{Deserialize, Serialize};
use std::io::Write;

// #[derive(Serialize)]
// pub struct DayColor {
//     pub color: Option<String>,
//     pub date: NaiveDate,
// }

#[derive(Queryable, Serialize, Debug)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = crate::db::schema::users)]
pub struct NewUser {
    pub name: String,
    pub email: String,
}

#[derive(diesel_derive_enum::DbEnum, Debug, PartialEq, Deserialize, Serialize)]
#[db_enum(existing_type_path = "crate::db::schema::sql_types::HabitType")]
pub enum HabitType {
    Color,
    Text,
    Number,
}

// Convert from DB VARCHAR (habit_type column) to Rust HabitType
impl FromSql<Text, Pg> for HabitType {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let s = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        match s.as_str() {
            "Color" => Ok(HabitType::Color),
            "Text" => Ok(HabitType::Text),
            "Number" => Ok(HabitType::Number),
            _ => Err(format!("Unknown habit type: {}", s).into()),
        }
    }
}

// Convert from Rust HabitType to DB VARCHAR
impl ToSql<Text, Pg> for HabitType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            HabitType::Color => "Color",
            HabitType::Text => "Text",
            HabitType::Number => "Number",
        };
        out.write_all(value.as_bytes())?;
        Ok(serialize::IsNull::No)
    }
}

impl std::fmt::Display for HabitType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            HabitType::Color => "Color",
            HabitType::Text => "Text",
            HabitType::Number => "Number",
        };
        write!(f, "{}", s)
    }
}

#[derive(Queryable, Deserialize, Serialize, Debug)]
pub struct Habit {
    pub id: i32,
    pub user_id: i32,
    pub name: String,
    pub weight: i32,
    pub sequence: i32,
    pub habit_type: HabitType,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = crate::db::schema::user_habits)]
pub struct NewHabit {
    pub user_id: i32,
    pub name: String,
    pub weight: i32,
    pub sequence: i32,
    pub habit_type: HabitType,
}

#[derive(Queryable, Deserialize, Serialize, Debug)]
pub struct VOption {
    pub id: i32,
    pub label: Option<String>,
    pub sequence: i32,
    pub habit_id: i32,
    pub color: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = crate::db::schema::habit_values)]
pub struct NewVOption {
    pub habit_id: i32,
    pub label: Option<String>,
    pub sequence: i32,
    pub color: Option<String>,
}

#[derive(Queryable, Serialize, Debug)]
pub struct Value {
    pub id: i32,
    pub value_id: i32,
    pub habit_id: i32,
    pub date: NaiveDate,
    pub text: Option<String>,
    pub number: Option<i32>,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::db::schema::day_values)]
pub struct NewValue {
    pub value_id: i32,
    pub habit_id: i32,
    pub date: NaiveDate,
    pub text: Option<String>,
    pub number: Option<i32>,
}
