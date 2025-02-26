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
