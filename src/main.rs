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

use crate::db::schema::user_days::dsl::*;
use crate::db::models::{UserDay, NewUserDay};
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
    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            .service(hello)
            .service(echo)
            .route("/hey", web::get().to(manual_hello))
    })
    .bind(format!("{}:{}", c.host, c.port))?
    .run()
    .await
}
