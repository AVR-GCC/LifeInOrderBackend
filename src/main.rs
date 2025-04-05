mod config;
use crate::config::Config;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder, middleware::Logger};
use serde::Serialize;
use chrono::NaiveDate;
use log::debug;
use std::collections::HashMap;

#[macro_use]
extern crate diesel_migrations;
use diesel_migrations::{MigrationHarness, EmbeddedMigrations};
const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel::pg::PgConnection;

use crate::db::schema::user_days::dsl::{user_days, id as ud_id, date as ud_date};
use crate::db::schema::users::dsl::{users, id as u_id, name as u_name, email as u_email, created_at as u_created_at};
use crate::db::schema::user_habits::dsl::{user_habits, id as uh_id, user_id as uh_user_id, habit_type as uh_habit_type, name as uh_name, weight as uh_weight};
use crate::db::schema::habit_values::dsl::{habit_values, id as hv_id, habit_id as hv_habit_id, color as hv_color};
use crate::db::schema::day_values::dsl::{day_values, value_id as dv_value_id, user_day_id as dv_user_day_id};
use crate::db::models::{User, NewUser, UserDay, NewUserDay, UserHabit, NewUserHabit, HabitValue, NewHabitValue, DayValue, NewDayValue, HabitColorDisplay};
mod db;

type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

#[derive(Serialize)]
struct Message {
    content: String,
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello  3  world!")
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

async fn manual_hello() -> impl Responder {
    let response = Message { content: String::from("Hey there biitccchh!") };
    HttpResponse::Ok().json(response)
}

#[post("/users")]
async fn create_user(
    pool: web::Data<DbPool>,
    req_body: web::Json<NewUser>,
) -> Result<HttpResponse, actix_web::Error> {
    let new_user = req_body.into_inner();
    debug!("Creating user: {:?}", new_user);
    let mut conn = pool.get().map_err(|e| {
        debug!("Pool error: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;
    let inserted = diesel::insert_into(users)
        .values(&new_user)
        .returning((u_id, u_name, u_email, u_created_at))
        .get_result::<User>(&mut conn)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    debug!("Inserted user: {:?}", inserted);
    Ok(HttpResponse::Ok().json(inserted))
}

#[post("/user_days")]
async fn create_user_day(
    pool: web::Data<DbPool>,
    req_body: web::Json<NewUserDay>,
) -> Result<HttpResponse, actix_web::Error> {
    let new_user_day = req_body.into_inner();
    debug!("Creating user_day for user_id: {}, date: {:?}", new_user_day.user_id, new_user_day.date);

    let mut conn = pool.get().map_err(actix_web::error::ErrorInternalServerError)?;
    let inserted = diesel::insert_into(user_days)
        .values(&new_user_day)
        .get_result::<UserDay>(&mut conn)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    debug!("Inserted user_day: {:?}", inserted);
    Ok(HttpResponse::Ok().json(inserted))
}

#[post("/user_habits")]
async fn create_user_habit(
    pool: web::Data<DbPool>,
    req_body: web::Json<NewUserHabit>,
) -> Result<HttpResponse, actix_web::Error> {
    let new_user_habit = req_body.into_inner();
    debug!(
        "Creating user_habit for user_id: {}, name: {:?}, weight: {}",
        new_user_habit.user_id, new_user_habit.name, new_user_habit.weight
    );

    let mut conn = pool.get().map_err(|e| {
        debug!("Pool error: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;
    let inserted = diesel::insert_into(user_habits)
        .values(&new_user_habit)
        .get_result::<UserHabit>(&mut conn)
        .map_err(|e| {
            debug!("Insert error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    debug!("Inserted user_habit: {:?}", inserted);
    Ok(HttpResponse::Ok().json(inserted))
}

#[post("/habit_values")]
async fn create_habit_value(
    pool: web::Data<DbPool>,
    req_body: web::Json<NewHabitValue>,
) -> Result<HttpResponse, actix_web::Error> {
    let new_habit_value = req_body.into_inner();
    debug!(
        "Creating user_habit for habit_id: {}, color: {}",
        new_habit_value.habit_id, new_habit_value.color.clone().unwrap_or("".to_string())
    );

    let mut conn = pool.get().map_err(|e| {
        debug!("Pool error: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;
    let inserted = diesel::insert_into(habit_values)
        .values(&new_habit_value)
        .get_result::<HabitValue>(&mut conn)
        .map_err(|e| {
            debug!("Insert error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    debug!("Inserted habit_value: {:?}", inserted);
    Ok(HttpResponse::Ok().json(inserted))
}

#[post("/day_values")]
async fn create_day_value(
    pool: web::Data<DbPool>,
    req_body: web::Json<NewDayValue>,
) -> Result<HttpResponse, actix_web::Error> {
    let new_day_value = req_body.into_inner();
    debug!(
        "Creating day_value for value_id: {}, user_day_id: {}, text: {}, number: {}",
        new_day_value.value_id, new_day_value.user_day_id, new_day_value.text.clone().unwrap_or("".to_string()), new_day_value.number.clone().unwrap_or(0)
    );

    let mut conn = pool.get().map_err(actix_web::error::ErrorInternalServerError)?;
    let inserted = diesel::insert_into(day_values)
        .values(&new_day_value)
        .get_result::<DayValue>(&mut conn)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    debug!("Inserted day_value: {:?}", inserted);
    Ok(HttpResponse::Ok().json(inserted))
}

#[get("/users/{path_user_id}/habit_colors")]
async fn get_habit_colors(
    pool: web::Data<DbPool>,
    path_user_id: web::Path<i32>,
) -> Result<HttpResponse, actix_web::Error> {
    let inner_user_id = path_user_id.into_inner();
    debug!("Fetching habit colors for user_id: {}", inner_user_id);

    let mut conn = pool.get().map_err(|e| {
        debug!("Pool error: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    let habit_data = user_habits
        .inner_join(habit_values.on(hv_habit_id.eq(uh_id)))
        .inner_join(day_values.on(dv_value_id.eq(hv_id)))
        .inner_join(user_days.on(ud_id.eq(dv_user_day_id)))
        .filter(uh_user_id.eq(inner_user_id))
        .filter(uh_habit_type.eq("color"))
        .select(( uh_id, uh_name, uh_weight, hv_color, ud_date ))
        .order(ud_date.asc())
        .load::<(i32, String, i32, Option<String>, NaiveDate)>(&mut conn)
        .map_err(|e| {
            debug!("Query error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    let mut habit_map: HashMap<i32, HabitColorDisplay> = HashMap::new();
    for (data_habit_id, data_habit_name, data_weight, data_color, _date) in habit_data {
        habit_map
            .entry(data_habit_id)
            .or_insert(HabitColorDisplay {
                habit_id: data_habit_id,
                habit_name: data_habit_name,
                weight: data_weight,
                colors: Vec::new(),
            })
            .colors
            .push(data_color);
    }

    let result: Vec<HabitColorDisplay> = habit_map.into_values().collect();
    debug!("Returning {} habits with colors", result.len());
    Ok(HttpResponse::Ok().json(result))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // logger
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // config
    let c = Config::from_env()
        .expect("Server Configuration");

    // db
    let manager = ConnectionManager::<PgConnection>::new(&c.database_url);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool");

    let mut conn = pool.get().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    conn.run_pending_migrations(MIGRATIONS).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // run
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .wrap(Logger::default())
            .service(hello)
            .service(echo)
            .service(create_user)
            .service(create_user_day)
            .service(create_user_habit)
            .service(create_habit_value)
            .service(create_day_value)
            .service(get_habit_colors)
            .route("/hey", web::get().to(manual_hello))
    })
    .bind(format!("{}:{}", c.host, c.port))?
    .run()
    .await
}
