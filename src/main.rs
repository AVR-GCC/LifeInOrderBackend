mod config;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{Duration, Instant};
use crate::config::Config;
use crate::utils::socket_registry::SocketRegistry;
use crate::utils::users::generate_token;
use jsonwebtoken::{DecodingKey, EncodingKey};
use crate::routes::aggregates::{get_backup, get_list};
use crate::routes::habits::{create_habit, delete_habit, reorder_habits, update_habit};
use crate::routes::options::{create_option, delete_option, reorder_options, update_option};
use crate::routes::values::set_value;
use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, get, middleware::Logger, post, web,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use std::collections::HashMap;

use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};

use crate::db::models::{Habit, LoginUser, NewHabit, VOption, Value};
use crate::routes::users::{change_password, confirm_email, login, logout, refresh, signup};
use crate::utils::{
    users::verify_token,
    general::get_storage,
    misc_types::{
        AppState, EmailOTP, ErrorResponse, RefreshTokenRequest, RouteParams, SocketRequest,
        SocketResponse, TokenQuery, ValuesOrImage,
    }
};

mod db;
mod routes;
mod utils;
use actix_ws::{Message, CloseCode, CloseReason};
use futures_util::StreamExt;
use tokio_util::sync::CancellationToken;

const LEAD: u64 = 60;

fn deadline(exp_size: usize, at_expiry: bool) -> Instant {
    let exp: u64 = exp_size.try_into().expect("Error converting expiration to u64");
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let target = if at_expiry { exp } else { exp.saturating_sub(LEAD) };
    Instant::now() + Duration::from_secs(target.saturating_sub(now))
}

fn reason(code: u16, description: &str) -> CloseReason {
    CloseReason {
        code: CloseCode::Other(code),
        description: Some(description.into()),
    }
}

async fn ws_handler(
    req: HttpRequest,
    query: web::Query<TokenQuery>,
    body: web::Payload,
    state: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;
    let claims = match verify_token(&query.t, &state.decoding_key) {
        Ok(c) => c,
        Err(e) => {
            println!("WS auth failed: {:?}", e);
            let close_object = Some(CloseReason {
                code: CloseCode::Other(4001),
                description: Some("token expired or invalid".into()),
            });
            let _ = session.close(close_object).await;
            return Ok(response);
        }
    };
    let user_id = i32::from_str_radix(claims.sub.as_str(), 10).expect("Malformed user_id");
    println!("authenticated: {}", user_id);
    if session.text("connected").await.is_err() {
        return Ok(response);
    }
    let cancel = CancellationToken::new();
    let conn_id = generate_token();
    let socket_registry = state.socket_registry.clone();
    socket_registry.add(claims.family_id.clone(), user_id, conn_id.clone(), cancel.clone());
    let mut exp = claims.exp;
    let mut nudged = false;
    let mut timer = Box::pin(tokio::time::sleep_until(deadline(exp, false)));

    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    // println!("cancelled {}", user_id);
                    let _ = session.close(Some(reason(4000, "signed out"))).await;
                    break;
                }
                _ = &mut timer => {
                    if !nudged {
                        // println!("nudging... {}", user_id);
                        nudged = true;
                        if session.text(r#"{"data":"refresh_required"}"#).await.is_err() { break; }
                        timer.as_mut().reset(deadline(exp, true));
                    } else {
                        // println!("token expired... {}", user_id);
                        let _ = session.close(Some(reason(4002, "token expired"))).await;
                        break;
                    }
                }
                msg = msg_stream.next() => match msg {
                    Some(Ok(Message::Text(text))) => {
                        let mut store = get_storage(state.clone()).expect("Failed to init storage");
                        let trimmed = text.trim_matches('\0').trim();
                        let req: SocketRequest = serde_json::from_str(trimmed.to_string().as_str())
                            .expect(format!("Malformed socket request: {}", text).as_str());
                        let ret = match req.action {
                            RouteParams::RefreshPost(rpr) => {
                                    let claims_res = verify_token(&rpr.t, &state.decoding_key);
                                    // println!("refresh posted {}", user_id);
                                    if let Ok(claims) = claims_res {
                                        exp = claims.exp;
                                        nudged = false;
                                        timer.as_mut().reset(deadline(exp, false));
                                        // println!("refresh approved {}", user_id);
                                    }
                                    r#"{"data":"token_processed"}"#.to_string()
                            }
                            RouteParams::UsersPut(passwords) => {
                                // println!("claims.family_id.clone(): {:?}", claims.family_id.clone());
                                let change_password_res = change_password(state.clone(), user_id, passwords.old_password, passwords.new_password, claims.family_id.clone()).await;
                                let res = if let Err(e) = change_password_res {
                                    SocketResponse::<bool> {
                                        id: req.id,
                                        data: None,
                                        error: Some(format!("{:?}", e))
                                    }
                                } else {
                                    SocketResponse::<bool> {
                                        id: req.id,
                                        data: Some(true),
                                        error: None,
                                    }
                                };
                                let ans = serde_json::to_string(&res).unwrap();
                                println!("socket UsersPut {}", ans);
                                ans
                            }
                            RouteParams::ListGet(get_list_req) => {
                                let data = get_list(store, user_id, get_list_req.date, get_list_req.zoom, get_list_req.width).await.expect("Failed to get list");
                                let str_data = match data {
                                    ValuesOrImage::Values(values_list) => {
                                        serde_json::to_string(&values_list).unwrap()
                                    }
                                    ValuesOrImage::Image(image_data) => {
                                        serde_json::to_string(&image_data).unwrap()
                                    }
                                };
                                let res = SocketResponse::<String> {
                                    id: req.id,
                                    data: Some(str_data),
                                    error: None,
                                };
                                let ans = serde_json::to_string(&res).unwrap();
                                // println!("socket ListGet {}", ans);
                                ans
                            }
                            RouteParams::HabitPost(new_habit_req) => {
                                let new_habit = NewHabit {
                                    user_id,
                                    name: new_habit_req.name,
                                    weight: new_habit_req.weight,
                                    sequence: new_habit_req.sequence,
                                    habit_type: new_habit_req.habit_type,
                                };
                                let inserted = create_habit(&mut store, new_habit).expect("Failed to update habit");
                                // let ret = format!("{:?}", inserted);
                                let res = SocketResponse::<Habit> {
                                    id: req.id,
                                    data: Some(inserted),
                                    error: None,
                                };
                                let ans = serde_json::to_string(&res).unwrap();
                                println!("HabitPost {}", ans);
                                ans
                            }
                            RouteParams::HabitPut(habit) => {
                                let inserted = update_habit(&mut store, habit).expect("Failed to update habit");
                                // let ret = format!("{:?}", inserted);
                                let res = SocketResponse::<Habit> {
                                    id: req.id,
                                    data: Some(inserted),
                                    error: None,
                                };
                                let ans = serde_json::to_string(&res).unwrap();
                                // println!("HabitPut {}", ans);
                                ans
                            }
                            RouteParams::HabitsReorder(payload) => {
                                let _result = reorder_habits(&mut store, payload.ordered_ids).await.expect("Failed to reorder habits");
                                // let ret = format!("{:?}", inserted);
                                let res = SocketResponse::<bool> {
                                    id: req.id,
                                    data: Some(true),
                                    error: None,
                                };
                                let ans = serde_json::to_string(&res).unwrap();
                                // println!("HabitsReorder {}", ans);
                                ans
                            }
                            RouteParams::HabitDelete(habit_id) => {
                                let result = delete_habit(&mut store, habit_id).expect("Failed to delete habit");
                                // let ret = format!("{:?}", inserted);
                                let res = SocketResponse::<bool> {
                                    id: req.id,
                                    data: if result == 0 { None } else { Some(true) },
                                    error: if result == 0 {
                                        Some("Delete failed".to_string())
                                    } else {
                                        None
                                    },
                                };
                                let ans = serde_json::to_string(&res).unwrap();
                                // println!("HabitDelete {}", ans);
                                ans
                            }
                            RouteParams::OptionPost(new_option) => {
                                let inserted = create_option(&mut store, new_option).expect("Failed to update option");
                                // let ret = format!("{:?}", inserted);
                                let res = SocketResponse::<VOption> {
                                    id: req.id,
                                    data: Some(inserted),
                                    error: None,
                                };
                                let ans = serde_json::to_string(&res).unwrap();
                                // println!("OptionPost {}", ans);
                                ans
                            }
                            RouteParams::OptionPut(option) => {
                                let inserted = update_option(&mut store, option).expect("Failed to update option");
                                // let ret = format!("{:?}", inserted);
                                let res = SocketResponse::<VOption> {
                                    id: req.id,
                                    data: Some(inserted),
                                    error: None,
                                };
                                let ans = serde_json::to_string(&res).unwrap();
                                // println!("OptionPut {}", ans);
                                ans
                            }
                            RouteParams::OptionsReorder(payload) => {
                                let _result = reorder_options(&mut store, payload.ordered_ids).await.expect("Failed to reorder options");
                                // let ret = format!("{:?}", inserted);
                                let res = SocketResponse::<bool> {
                                    id: req.id,
                                    data: Some(true),
                                    error: None,
                                };
                                let ans = serde_json::to_string(&res).unwrap();
                                // println!("OptionsReorder {}", ans);
                                ans
                            }
                            RouteParams::OptionDelete(option_id) => {
                                let result = delete_option(&mut store, option_id).expect("Failed to delete option");
                                // let ret = format!("{:?}", inserted);
                                let res = SocketResponse::<bool> {
                                    id: req.id,
                                    data: if result == 0 { None } else { Some(true) },
                                    error: if result == 0 {
                                        Some("Delete failed".to_string())
                                    } else {
                                        None
                                    },
                                };
                                let ans = serde_json::to_string(&res).unwrap();
                                // println!("OptionDelete {}", ans);
                                ans
                            }
                            RouteParams::Values(new_value) => {
                                let inserted = set_value(&mut store, new_value, user_id).expect("Failed to update option");
                                // let ans = format!("{:?}", inserted);
                                let res = SocketResponse::<Value> {
                                    id: req.id,
                                    data: Some(inserted),
                                    error: None,
                                };
                                let ans = serde_json::to_string(&res).unwrap();
                                // println!("Values {}", ans);
                                ans
                            }
                        };
                        if session.text(ret).await.is_err() {
                            break; // client disconnected
                        }
                    }
                    Some(Ok(Message::Close(reason))) => {
                        println!("Close {:?}", reason);
                        let _ = session.close(reason).await;
                        break;
                    }
                    _ => {}
                }
            }
        }
        socket_registry.remove(claims.family_id, conn_id);
        // println!("socket closed {}", user_id);
    });

    Ok(response)
}

#[post("/login")]
async fn login_route(
    state: web::Data<AppState>,
    req_body: web::Json<LoginUser>,
) -> Result<HttpResponse, actix_web::Error> {
    let mut store = get_storage(state.clone()).expect("Failed to init storage");
    let login_user = req_body.into_inner();
    let auth_res_opt = login(
        &mut store,
        state.encoding_key.clone(),
        login_user.email,
        login_user.password
    ).await;
    match auth_res_opt {
        Ok(auth_res) => Ok(HttpResponse::Ok().json(auth_res)),
        Err(_) => Ok(HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Wrong email or password".to_string(),
        }))
    }
}

#[post("/confirm_email")]
async fn confirm_email_route(
    state: web::Data<AppState>,
    req_body: web::Json<EmailOTP>,
) -> Result<HttpResponse, actix_web::Error> {
    let mut store = get_storage(state.clone()).expect("Failed to init storage");
    let email_otp = req_body.into_inner();
    let inserted_opt = confirm_email(&mut store, state.encoding_key.clone(), email_otp).await;
    match inserted_opt {
        Ok(inserted) => Ok(HttpResponse::Ok().json(inserted)),
        Err(_) => Ok(HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Invalid email confirmation".to_string(),
        }))
    }
}

#[post("/signup")]
async fn signup_route(
    state: web::Data<AppState>,
    req_body: web::Json<LoginUser>,
) -> Result<HttpResponse, actix_web::Error> {
    let mut store = get_storage(state.clone()).expect("Failed to init storage");
    let login_user_object = req_body.into_inner();
    let res = signup(&mut store, state.postmark_api_key.clone(), login_user_object).await;
    match res {
        Ok(_) => Ok(HttpResponse::Ok().finish()),
        Err(_) => Ok(HttpResponse::InternalServerError().json(ErrorResponse {
            message: "Signup failed".to_string(),
        }))
    }
}

#[post("/refresh")]
async fn refresh_route(
    state: web::Data<AppState>,
    req_body: web::Json<RefreshTokenRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let refresh_token_req = req_body.into_inner();
    let inserted_opt = refresh(state, refresh_token_req.refresh_token).await;
    match inserted_opt {
        Ok(inserted) => Ok(HttpResponse::Ok().json(inserted)),
        Err(_) => Ok(HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Authentication failed".to_string(),
        }))
    }
}

#[post("/logout")]
async fn logout_route(
    state: web::Data<AppState>,
    req_body: web::Json<RefreshTokenRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let logout_res = req_body.into_inner();
    if let Err(_) = logout(state, logout_res.refresh_token).await {
        return Ok(HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Logout failed".to_string(),
        }));
    }
    Ok(HttpResponse::NoContent().finish())
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

#[get("/privacy")]
async fn privacy_policy() -> Result<HttpResponse, actix_web::Error> {
    let html = include_str!("../static/privacy.html");

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

const EMAIL_LOGO: &[u8] = include_bytes!("../static/logo.png");
#[get("/static/logo.png")]
async fn serve_embedded_logo() -> Result<HttpResponse, actix_web::Error> {
    Ok(HttpResponse::Ok()
        .content_type("image/png")
        .body(EMAIL_LOGO))
}

const BIMI_LOGO: &[u8] = include_bytes!("../static/logo-bimi.svg");
#[get("/.well-known/bimi/logo.svg")]
async fn bimi_logo() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("image/svg+xml")
        .body(BIMI_LOGO)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // crypto
    println!("Starting main - installing rustls crypto provider");
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    println!("Installed rustls crypto provider - initializing env logger");
    // logger
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    println!("Initialized env logger - Getting server config");

    // config
    let c = Config::from_env().expect("Server Configuration");
    println!("Got server config - Connecting to pg db");

    // db
    let manager = ConnectionManager::<PgConnection>::new(&c.database_url);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool");

    println!("Connected to pg db - Connecting to pool");
    let mut db = pool
        .get()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    println!("Connected to pool - Running migrations");
    db.run_pending_migrations(MIGRATIONS)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    println!("Migrations run - Connecting to redis");
    // cache
    let client = redis::Client::open(c.cache_url).expect("Failed to open cache client");
    println!("Connected to redis - Aquiring JWT keys");

    // JWT keys
    let secret_bytes = BASE64.decode(&c.jwt_secret).unwrap();
    let encoding_key = EncodingKey::from_secret(&secret_bytes);
    let decoding_key = DecodingKey::from_secret(&secret_bytes);
    println!("JWT keys aquired - Creating AppState");

    // Socket registry
    let socket_registry = SocketRegistry::default();

    let app_state = AppState {
        db_pool: pool.clone(),
        redis_client: client,
        encoding_key,
        decoding_key,
        postmark_api_key: c.postmark_api_key,
        socket_registry,
    };

    println!("AppState created - Running server");
    // run
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(Logger::default())
            .service(logout_route)
            .service(signup_route)
            .service(login_route)
            .service(confirm_email_route)
            .service(refresh_route)
            .service(get_backup_route)
            .service(ping)
            .service(privacy_policy)
            .service(serve_embedded_logo)
            .service(bimi_logo)
            .route("/ws", web::get().to(ws_handler))
        //.route("/hey", web::get().to(manual_hello))
    })
    .bind(format!("{}:{}", c.host, c.port))?
    .run()
    .await
}
