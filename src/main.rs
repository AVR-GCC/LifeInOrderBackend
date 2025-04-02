mod config;
use crate::config::Config;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder, middleware::Logger};
use serde::Serialize;
use log::debug;

#[macro_use]
extern crate diesel_migrations;
use diesel_migrations::{MigrationHarness, EmbeddedMigrations};
const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel::pg::PgConnection;

use crate::db::schema::{user_days::dsl::*, users::dsl::*, user_habits::dsl::*, habit_values::dsl::*};
use crate::db::models::{User, NewUser, UserDay, NewUserDay, UserHabit, NewUserHabit, HabitValue, NewHabitValue};
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
        .returning((
                crate::db::schema::users::dsl::id,
                crate::db::schema::users::dsl::name,
                email,
                crate::db::schema::users::dsl::created_at
        ))
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
            .route("/hey", web::get().to(manual_hello))
    })
    .bind(format!("{}:{}", c.host, c.port))?
    .run()
    .await
}
