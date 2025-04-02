use std::io::Write;
use diesel::prelude::*;
use diesel::backend::Backend;
use diesel::deserialize::{self, FromSql};
use diesel::serialize::{self, ToSql, Output};
use diesel::pg::Pg;
use diesel::sql_types::Text;
use serde::{Serialize, Deserialize};
use chrono::{NaiveDate, NaiveDateTime};

#[derive(Queryable, Serialize, Debug)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub created_at: NaiveDateTime
}

#[derive(Insertable, Deserialize, Debug)]
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

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = crate::db::schema::user_days)]
pub struct NewUserDay {
    pub user_id: i32,
    pub date: NaiveDate,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
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
            "color" => Ok(HabitType::Color),
            "text" => Ok(HabitType::Text),
            "number" => Ok(HabitType::Number),
            _ => Err(format!("Unknown habit type: {}", s).into()),
        }
    }
}

// Convert from Rust HabitType to DB VARCHAR
impl ToSql<Text, Pg> for HabitType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            HabitType::Color => "color",
            HabitType::Text => "text",
            HabitType::Number => "number",
        };
        out.write_all(value.as_bytes())?;
        Ok(serialize::IsNull::No)
    }
}

#[derive(Queryable, Serialize, Debug)]
pub struct UserHabit {
    pub id: i32,
    pub user_id: i32,
    pub name: String,
    pub weight: i32,
    #[diesel(sql_type = Text)]
    pub habit_type: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize)]
#[diesel(table_name = crate::db::schema::user_habits)]
pub struct NewUserHabit {
    pub user_id: i32,
    pub name: String,
    pub weight: i32,
    pub habit_type: String,
}

#[derive(Queryable, Serialize, Debug)]
pub struct HabitValue {
    pub id: i32,
    pub habit_id: i32,
    pub color: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = crate::db::schema::habit_values)]
pub struct NewHabitValue {
    pub habit_id: i32,
    pub color: Option<String>,
}

#[derive(Queryable, Serialize, Debug)]
pub struct DayValue {
    pub id: i32,
    pub value_id: i32,
    pub user_day_id: i32,
    pub text: String,
    pub number: i32,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = crate::db::schema::day_values)]
pub struct NewDayValue {
    pub value_id: i32,
    pub user_day_id: i32,
    pub text: String,
    pub number: i32,
}
