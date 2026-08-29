mod config;
use crate::config::Config;
use crate::routes::aggregates::{get_backup, get_extended_habits, get_list};
use crate::routes::habits::{create_habit, delete_habit, reorder_habits, update_habit};
use crate::routes::options::{create_option, delete_option, reorder_options, update_option};
use crate::routes::values::set_value;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, delete, get, middleware::Logger, post, put, web};
use chrono::{NaiveDate};
use std::collections::HashMap;
use std::str::FromStr;
use utils::misc_types::SequenceUpdateRequest;

use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};

use crate::db::models::{
    Habit, NewHabit, NewUser, NewVOption, NewValue, VOption, Value
};
use crate::utils::general::{get_storage};
use crate::utils::misc_types::{AppState, RouteParams, SocketRequest, SocketResponse, UserListResponse, ZoomLevel};
use crate::routes::users::create_user;

mod db;
mod utils;
mod routes;
use actix_ws::Message;
use futures_util::StreamExt;

async fn ws_handler(req: HttpRequest, body: web::Payload, state: web::Data<AppState>) -> Result<HttpResponse, actix_web::Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;

    actix_web::rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                Message::Text(text) => {
                    println!("text {}", text);
                    let store = get_storage(state.clone()).expect("Failed to init storage");
                    let user_id = 1;
                    let req: SocketRequest = serde_json::from_str(text.to_string().as_str()).expect("Malformed socket request");
                    match req.action {
                        RouteParams::Values(new_value) => {
                            let inserted = set_value(store, new_value, user_id).expect("Failed to update option");
                            // let ret = format!("{:?}", inserted);
                            let res = SocketResponse::<Value> {
                                id: req.id,
                                data: Some(inserted),
                                error: None
                            };
                            let ret = serde_json::to_string(&res).unwrap();
                            println!("ret {}", ret);
                            if session.text(ret).await.is_err() {
                                break; // client disconnected
                            }
                        }
                    }
                    if let Ok(num) = text.trim().parse::<i64>() {
                        let incremented = num + 1;
                        println!("Backend received {num}, sending {incremented}");

                        if session.text(incremented.to_string()).await.is_err() {
                            break; // client disconnected
                        }
                    }
                }
                Message::Close(reason) => {
                    let _ = session.close(reason).await;
                    break;
                }
                _ => {}
            }
        }
    });

    Ok(response)
}

#[post("/users")]
async fn create_user_route(
    state: web::Data<AppState>,
    req_body: web::Json<NewUser>,
) -> Result<HttpResponse, actix_web::Error> {
    let store = get_storage(state).expect("Failed to init storage");
    let new_user = req_body.into_inner();
    let inserted = create_user(store, new_user).expect("Failed to create user");
    Ok(HttpResponse::Ok().json(inserted))
}

#[post("/habits")]
async fn create_habit_route(
    state: web::Data<AppState>,
    req_body: web::Json<NewHabit>,
) -> Result<HttpResponse, actix_web::Error> {
    let store = get_storage(state).expect("Failed to init storage");
    let new_habit = req_body.into_inner();
    let inserted = create_habit(store, new_habit).expect("Failed to create habit");
    Ok(HttpResponse::Ok().json(inserted))
}

#[put("/habits")]
async fn update_habit_route(
    state: web::Data<AppState>,
    req_body: web::Json<Habit>,
) -> Result<HttpResponse, actix_web::Error> {
    let store = get_storage(state).expect("Failed to init storage");
    let new_habit = req_body.into_inner();
    let inserted = update_habit(store, new_habit).expect("Failed to update habit");
    Ok(HttpResponse::Ok().json(inserted))
}

#[delete("/habits/{id}")]
async fn delete_habit_route(
    state: web::Data<AppState>,
    path_habit_id: web::Path<i32>,
) -> Result<HttpResponse, actix_web::Error> {
    let store = get_storage(state).expect("Failed to init storage");
    let habit_id = path_habit_id.into_inner();
    let result = delete_habit(store, habit_id).expect("Failed to delete habit");
    if result == 0 {
        return Ok(HttpResponse::NotFound().json("Habit not found"))
    }
    Ok(HttpResponse::Ok().json("Habit deleted"))
}

#[post("/habits/reorder")]
async fn reorder_habits_route(
    state: web::Data<AppState>,
    req: web::Json<SequenceUpdateRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let store = get_storage(state).expect("Failed to init storage");
    let habit_ids = req.into_inner().ordered_ids.clone();
    let _result = reorder_habits(store, habit_ids).await.expect("Failed to reorder habits");
    Ok(HttpResponse::Ok().json("Sequence updated"))
}

#[post("/options")]
async fn create_option_route(
    state: web::Data<AppState>,
    req_body: web::Json<NewVOption>,
) -> Result<HttpResponse, actix_web::Error> {
    let store = get_storage(state).expect("Failed to init storage");
    let new_option = req_body.into_inner();
    let inserted = create_option(store, new_option).expect("Failed to create option");
    Ok(HttpResponse::Ok().json(inserted))
}

#[put("/options")]
async fn update_option_route(
    state: web::Data<AppState>,
    req_body: web::Json<VOption>,
) -> Result<HttpResponse, actix_web::Error> {
    let store = get_storage(state).expect("Failed to init storage");
    let option = req_body.into_inner();
    let inserted = update_option(store, option).expect("Failed to update option");
    Ok(HttpResponse::Ok().json(inserted))
}

#[delete("/options/{id}")]
async fn delete_option_route(
    state: web::Data<AppState>,
    path_option_id: web::Path<i32>,
) -> Result<HttpResponse, actix_web::Error> {
    let store = get_storage(state).expect("Failed to init storage");
    let option_id = path_option_id.into_inner();
    let result = delete_option(store, option_id).expect("Failed to delete option");
    if result == 0 {
        return Ok(HttpResponse::NotFound().json("Option not found"))
    }
    Ok(HttpResponse::Ok().json("Option deleted"))
}

#[post("/options/reorder")]
async fn reorder_options_route(
    state: web::Data<AppState>,
    req: web::Json<SequenceUpdateRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let option_ids = req.into_inner().ordered_ids.clone();
    let store = get_storage(state).expect("Failed to init storage");
    let _result = reorder_options(store, option_ids).await.expect("Failed to reorder options");
    Ok(HttpResponse::Ok().json("Sequence updated"))
}

#[post("/values")]
async fn set_value_route(
    state: web::Data<AppState>,
    req_body: web::Json<NewValue>,
) -> Result<HttpResponse, actix_web::Error> {
    let store = get_storage(state).expect("Failed to init storage");
    let user_id = 1;
    let new_value = req_body.into_inner();
    let inserted = set_value(store, new_value, user_id).expect("Failed to update option");
    Ok(HttpResponse::Ok().json(inserted))
}

#[get("/users/{path_user_id}/config")]
async fn get_config_route(
    state: web::Data<AppState>,
    path_user_id: web::Path<i32>,
) -> Result<HttpResponse, actix_web::Error> {
    let inner_user_id = path_user_id.into_inner();
    let mut store = get_storage(state).expect("Failed to init storage");
    let config = get_extended_habits(&mut store.db, inner_user_id).await?;
    Ok(HttpResponse::Ok().json(config))
}

#[get("/users/{path_user_id}/list")]
async fn get_list_route(
    state: web::Data<AppState>,
    path_user_id: web::Path<i32>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = path_user_id.into_inner();
    let store = get_storage(state).expect("Failed to init storage");


    if let (Some(date), Some(zoom), Some(count)) =
        (query.get("date"), query.get("zoom"), query.get("count"))
    {
        let date = NaiveDate::from_str(date).unwrap();
        let count: u32 = u32::from_str(count).unwrap();
        let zoom: ZoomLevel = zoom.parse().unwrap();
        let width: i32 = query
            .get("width")
            .and_then(|w| w.parse().ok())
            .unwrap_or(1080);
        get_list(store, user_id, date, count, zoom, width).await
    } else {
        Ok(HttpResponse::Ok().json(UserListResponse {
            dates: Vec::new(),
            habits: Vec::new(),
        }))
    }
}

#[get("/users/{path_user_id}/backup")]
async fn get_backup_route(
    state: web::Data<AppState>,
    path_user_id: web::Path<i32>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = path_user_id.into_inner();
    let store = get_storage(state).expect("Failed to init storage");
    get_backup(store, user_id).await
}

#[get("/")]
async fn ping() -> Result<HttpResponse, actix_web::Error> {
    Ok(HttpResponse::Ok().json("Get your life in order!"))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // crypto
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // logger
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // config
    let c = Config::from_env().expect("Server Configuration");

    // db
    let manager = ConnectionManager::<PgConnection>::new(&c.database_url);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool");

    let mut db = pool
        .get()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    db.run_pending_migrations(MIGRATIONS)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // cache
    let client = redis::Client::open(c.cache_url).expect("Failed to open cache client");

    let app_state = AppState {
        db_pool: pool.clone(),
        redis_client: client,
    };

    // run
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(Logger::default())
            .service(create_user_route)
            .service(create_habit_route)
            .service(update_habit_route)
            .service(delete_habit_route)
            .service(reorder_habits_route)
            .service(create_option_route)
            .service(update_option_route)
            .service(delete_option_route)
            .service(reorder_options_route)
            .service(set_value_route)
            .service(get_list_route)
            .service(get_config_route)
            .service(get_backup_route)
            .service(ping)
            .route("/ws", web::get().to(ws_handler))
        //.route("/hey", web::get().to(manual_hello))
    })
    .bind(format!("{}:{}", c.host, c.port))?
    .run()
    .await
}
