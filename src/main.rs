mod config;
use crate::config::Config;
use crate::routes::aggregates::{get_backup, get_extended_habits, get_list, get_list_socket};
use crate::routes::habits::{create_habit, delete_habit, reorder_habits, update_habit};
use crate::routes::options::{create_option, delete_option, reorder_options, update_option};
use crate::routes::values::set_value;
use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, delete, get, middleware::Logger, post, put, web,
};
use chrono::NaiveDate;
use std::collections::HashMap;
use std::str::FromStr;
use utils::misc_types::SequenceUpdateRequest;

use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};

use crate::db::models::{Habit, NewHabit, NewUser, NewVOption, NewValue, VOption, Value};
use crate::routes::users::create_user;
use crate::utils::general::get_storage;
use crate::utils::misc_types::{
    AppState, MonthValuesStruct, RouteParams, SocketRequest, SocketResponse, UserListResponse, ValuesOrImage, ZoomLevel
};

mod db;
mod routes;
mod utils;
use actix_ws::Message;
use futures_util::StreamExt;

async fn ws_handler(
    req: HttpRequest,
    body: web::Payload,
    state: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;

    actix_web::rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                Message::Text(text) => {
                    println!("text {}", text);
                    let store = get_storage(state.clone()).expect("Failed to init storage");
                    let user_id = 1;
                    let req: SocketRequest = serde_json::from_str(text.to_string().as_str())
                        .expect("Malformed socket request");
                    let ret = match req.action {
                        RouteParams::ListGet(get_list_req) => {
                            let data = get_list_socket(store, user_id, get_list_req.date, get_list_req.zoom, get_list_req.width).await.expect("Failed to get list");
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
                            println!("socket ListGet {}", ans);
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
                            let inserted = create_habit(store, new_habit).expect("Failed to update habit");
                            // let ret = format!("{:?}", inserted);
                            let res = SocketResponse::<Habit> {
                                id: req.id,
                                data: Some(inserted),
                                error: None,
                            };
                            let ans = serde_json::to_string(&res).unwrap();
                            println!("socket HabitPost {}", ans);
                            ans
                        }
                        RouteParams::HabitPut(habit) => {
                            let inserted = update_habit(store, habit).expect("Failed to update habit");
                            // let ret = format!("{:?}", inserted);
                            let res = SocketResponse::<Habit> {
                                id: req.id,
                                data: Some(inserted),
                                error: None,
                            };
                            let ans = serde_json::to_string(&res).unwrap();
                            println!("socket HabitPut {}", ans);
                            ans
                        }
                        RouteParams::HabitsReorder(payload) => {
                            let _result = reorder_habits(store, payload.ordered_ids).await.expect("Failed to reorder habits");
                            // let ret = format!("{:?}", inserted);
                            let res = SocketResponse::<bool> {
                                id: req.id,
                                data: Some(true),
                                error: None,
                            };
                            let ans = serde_json::to_string(&res).unwrap();
                            println!("socket HabitsReorder {}", ans);
                            ans
                        }
                        RouteParams::HabitDelete(habit_id) => {
                            let result = delete_habit(store, habit_id).expect("Failed to delete habit");
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
                            println!("socket HabitDelete {}", ans);
                            ans
                        }
                        RouteParams::OptionPost(new_option) => {
                            let inserted = create_option(store, new_option).expect("Failed to update option");
                            // let ret = format!("{:?}", inserted);
                            let res = SocketResponse::<VOption> {
                                id: req.id,
                                data: Some(inserted),
                                error: None,
                            };
                            let ans = serde_json::to_string(&res).unwrap();
                            println!("socket OptionPost {}", ans);
                            ans
                        }
                        RouteParams::OptionPut(option) => {
                            let inserted = update_option(store, option).expect("Failed to update option");
                            // let ret = format!("{:?}", inserted);
                            let res = SocketResponse::<VOption> {
                                id: req.id,
                                data: Some(inserted),
                                error: None,
                            };
                            let ans = serde_json::to_string(&res).unwrap();
                            println!("socket OptionPut {}", ans);
                            ans
                        }
                        RouteParams::OptionsReorder(payload) => {
                            let _result = reorder_options(store, payload.ordered_ids).await.expect("Failed to reorder options");
                            // let ret = format!("{:?}", inserted);
                            let res = SocketResponse::<bool> {
                                id: req.id,
                                data: Some(true),
                                error: None,
                            };
                            let ans = serde_json::to_string(&res).unwrap();
                            println!("socket OptionsReorder {}", ans);
                            ans
                        }
                        RouteParams::OptionDelete(option_id) => {
                            let result = delete_option(store, option_id).expect("Failed to delete option");
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
                            println!("socket OptionDelete {}", ans);
                            ans
                        }
                        RouteParams::Values(new_value) => {
                            let inserted = set_value(store, new_value, user_id).expect("Failed to update option");
                            // let ans = format!("{:?}", inserted);
                            let res = SocketResponse::<Value> {
                                id: req.id,
                                data: Some(inserted),
                                error: None,
                            };
                            let ans = serde_json::to_string(&res).unwrap();
                            println!("socket Values {}", ans);
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
        return Ok(HttpResponse::NotFound().json("Habit not found"));
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
        return Ok(HttpResponse::NotFound().json("Option not found"));
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
            .service(privacy_policy)
            .route("/ws", web::get().to(ws_handler))
        //.route("/hey", web::get().to(manual_hello))
    })
    .bind(format!("{}:{}", c.host, c.port))?
    .run()
    .await
}
