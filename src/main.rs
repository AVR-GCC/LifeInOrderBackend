mod config;
use crate::config::Config;
use jsonwebtoken::{DecodingKey, EncodingKey};
use actix_files::NamedFile;
use std::path::PathBuf;
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
use crate::routes::users::{login, logout, refresh, signup, verify_token};
use crate::utils::general::get_storage;
use crate::utils::misc_types::{
    AppState, ErrorResponse, RefreshTokenRequest, RouteParams, SocketRequest, SocketResponse, TokenQuery, ValuesOrImage
};

mod db;
mod routes;
mod utils;
use actix_ws::Message;
use futures_util::StreamExt;

async fn ws_handler(
    req: HttpRequest,
    query: web::Query<TokenQuery>,
    body: web::Payload,
    state: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;
    let claims = verify_token(&query.t, &state.decoding_key).expect("Connection refused");
    let user_id = claims.sub;
    println!("user_id: {:?}", user_id);

    actix_web::rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                Message::Text(text) => {
                    let mut store = get_storage(state.clone()).expect("Failed to init storage");
                    let trimmed = text.trim_matches('\0').trim();
                    let req: SocketRequest = serde_json::from_str(trimmed.to_string().as_str())
                        .expect(format!("Malformed socket request: {}", text).as_str());
                    let ret = match req.action {
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

#[post("/signup")]
async fn signup_route(
    state: web::Data<AppState>,
    req_body: web::Json<LoginUser>,
) -> Result<HttpResponse, actix_web::Error> {
    // TODO: check email valid and not taken
    let mut store = get_storage(state.clone()).expect("Failed to init storage");
    let login_user_object = req_body.into_inner();
    let inserted = signup(&mut store, state.encoding_key.clone(), login_user_object).await.expect("Failed to create user");
    Ok(HttpResponse::Ok().json(inserted))
}

#[post("/refresh")]
async fn refresh_route(
    state: web::Data<AppState>,
    req_body: web::Json<RefreshTokenRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let mut store = get_storage(state.clone()).expect("Failed to init storage");
    let refresh_token_req = req_body.into_inner();
    let inserted = refresh(&mut store, state.encoding_key.clone(), refresh_token_req.refresh_token).await?;
    Ok(HttpResponse::Ok().json(inserted))
}

#[post("/logout")]
async fn logout_route(
    state: web::Data<AppState>,
    req_body: web::Json<RefreshTokenRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let store = get_storage(state.clone()).expect("Failed to init storage");
    let logout_res = req_body.into_inner();
    logout(store, logout_res.refresh_token).await?;
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
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Privacy Policy - LifeInOrder</title>
  <meta name="description" content="Privacy Policy for LifeInOrder">
  <style>
    body {
      font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI",
        sans-serif;
      line-height: 1.6;
      max-width: 800px;
      margin: 0 auto;
      padding: 40px 20px;
      color: #222;
      background: #fff;
    }

    h1 {
      line-height: 1.2;
      margin-bottom: 8px;
    }

    h2 {
      margin-top: 32px;
      line-height: 1.3;
    }

    .updated {
      color: #666;
      margin-bottom: 32px;
    }

    a {
      color: inherit;
    }
  </style>
</head>
<body>

  <h1>Privacy Policy for LifeInOrder</h1>

  <p class="updated">
    <strong>Last updated:</strong> September 3, 2026
  </p>

  <p>
    LifeInOrder ("LifeInOrder", "we", "us", or "our") is an application
    designed to help users organize and manage information about their daily
    lives, including habits, journal entries, food logs, and other information
    entered by the user.
  </p>

  <p>
    This Privacy Policy explains what information LifeInOrder collects, how
    we use it, how we protect it, and how you can request deletion of your
    information.
  </p>

  <h2>1. Information We Collect</h2>

  <p>
    When you use LifeInOrder, you may provide information that is stored on
    our servers in order to provide the application's functionality.
  </p>

  <p>This may include:</p>

  <ul>
    <li>
      Information associated with your account, such as your email address
      or other authentication information.
    </li>
    <li>
      Habits and other personal information that you enter into the
      application.
    </li>
    <li>
      Journal entries and notes.
    </li>
    <li>
      Food and nutrition information that you enter.
    </li>
    <li>
      Photos or other files that you choose to store through the application.
    </li>
    <li>
      Other information that you voluntarily enter into LifeInOrder.
    </li>
  </ul>

  <p>
    We collect information that is necessary to provide the functionality of
    the application or that you voluntarily choose to provide.
  </p>

  <h2>2. How We Use Your Information</h2>

  <p>
    We currently use the information you provide solely to operate and
    provide LifeInOrder's functionality to you.
  </p>

  <p>For example, your information may be used to:</p>

  <ul>
    <li>Store and synchronize your LifeInOrder data.</li>
    <li>Display your information to you when you use the application.</li>
    <li>Authenticate your account and provide access to your data.</li>
    <li>Maintain, secure, and troubleshoot the service.</li>
    <li>Respond to requests for support.</li>
  </ul>

  <p>
    We currently do <strong>not</strong> use your personal information or
    user-created content to train artificial intelligence models, serve
    advertising, or sell your information to third parties.
  </p>

  <h2>3. Your Data and Other Users</h2>

  <p>
    Your LifeInOrder data is intended to be accessible only to you through
    your authenticated account.
  </p>

  <p>
    We use account authentication and access controls to help ensure that
    users can access only the data associated with their own accounts.
    You should nevertheless avoid entering information into the application
    that you do not want stored on our servers.
  </p>

  <h2>4. Data Storage and Security</h2>

  <p>
    Your data is transmitted between the application and our servers using
    HTTPS encryption.
  </p>

  <p>
    LifeInOrder stores user data on servers operated by our hosting provider.
    We take reasonable measures to protect stored information against
    unauthorized access, loss, misuse, or disclosure.
  </p>

  <p>
    At present, information stored in our database is not individually
    encrypted at the database-field level. Access to the database and
    application infrastructure is restricted and protected through
    appropriate access controls.
  </p>

  <p>
    No method of transmission or electronic storage can be guaranteed to be
    completely secure.
  </p>

  <h2>5. Sharing of Information</h2>

  <p>
    We do not sell your personal information.
  </p>

  <p>
    We may use third-party infrastructure and service providers to operate
    LifeInOrder, such as hosting providers and authentication providers.
    These providers may process information as necessary to provide their
    services to us.
  </p>

  <p>
    We may also disclose information when required by law, legal process,
    or to protect the rights, safety, or security of LifeInOrder, our users,
    or others.
  </p>

  <h2>6. Data Retention</h2>

  <p>
    We retain your LifeInOrder data while your account remains active and
    as necessary to provide the application's functionality.
  </p>

  <p>
    When you request deletion of your account, we will delete the account
    and associated user data, except where retention is required or permitted
    by law for legitimate purposes such as security, fraud prevention, or
    legal compliance.
  </p>

  <h2>7. Account and Data Deletion</h2>

  <p>
    If you have a LifeInOrder account, you may request deletion of your
    account and associated data.
  </p>

  <p>
    LifeInOrder provides a way to request account deletion both within the
    application and through a web-based mechanism.
  </p>

  <p>
    Once a valid deletion request is processed, we will delete the data
    associated with your account, subject to any limited retention required
    for legitimate legal or security purposes.
  </p>

  <h2>8. Children's Privacy</h2>

  <p>
    LifeInOrder is not specifically directed to children under the age of
    13, and we do not knowingly collect personal information from children
    under 13.
  </p>

  <p>
    If you believe that a child has provided us with personal information,
    please contact us so that we can take appropriate action.
  </p>

  <h2>9. Changes to This Privacy Policy</h2>

  <p>
    We may update this Privacy Policy from time to time as LifeInOrder's
    functionality or data practices change.
  </p>

  <p>
    When we make material changes, we will update the "Last updated" date
    and, where appropriate, provide additional notice within the application.
  </p>

  <h2>10. Contact Us</h2>

  <p>
    If you have questions about this Privacy Policy, your personal
    information, or your account, please contact:
  </p>

  <p>
    <strong>LifeInOrder</strong><br>
    <strong>Email:</strong>
    <a href="mailto:ogoun.d@gmail.com">
      ogoun.d@gmail.com
    </a>
  </p>

</body>
</html>"#;

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))

}
#[get("/logo.png")]
async fn serve_logo() -> Result<NamedFile, actix_web::Error> {
    let path: PathBuf = "./static/logo.png".into();
    Ok(NamedFile::open(path)?)
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

    let app_state = AppState {
        db_pool: pool.clone(),
        redis_client: client,
        encoding_key,
        decoding_key,
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
            .service(refresh_route)
            .service(get_backup_route)
            .service(ping)
            .service(privacy_policy)
            .service(serve_logo)
            .route("/ws", web::get().to(ws_handler))
        //.route("/hey", web::get().to(manual_hello))
    })
    .bind(format!("{}:{}", c.host, c.port))?
    .run()
    .await
}
