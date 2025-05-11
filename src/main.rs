mod config;
use crate::config::Config;
use actix_web::{get, post, delete, web, App, HttpResponse, HttpServer, middleware::Logger};
use diesel::dsl::now;
use chrono::NaiveDate;
use chrono::NaiveDateTime;
use log::debug;
use utils::misc_types::SequenceUpdateRequest;
use std::collections::HashMap;

#[macro_use]
extern crate diesel_migrations;
use diesel_migrations::{MigrationHarness, EmbeddedMigrations};
const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel::pg::PgConnection;

use crate::db::schema::users::dsl::{users, id as u_id, name as u_name, email as u_email, created_at as u_created_at};
use crate::db::schema::user_habits::dsl::{user_habits, id as uh_id, user_id as uh_user_id, habit_type as uh_habit_type, name as uh_name, weight as uh_weight, sequence as uh_sequence, created_at as uh_created_at};
use crate::db::schema::habit_values::dsl::{habit_values, id as hv_id, label as hv_label, sequence as hv_sequence, habit_id as hv_habit_id, color as hv_color, created_at as hv_created_at};
use crate::db::schema::day_values::dsl::{day_values, value_id as dv_value_id, date as dv_date, habit_id as dv_habit_id, text as dv_text, number as dv_number, created_at as dv_created_at};
use crate::db::models::{User, NewUser, UserHabit, NewUserHabit, HabitValue, NewHabitValue, DayValue, NewDayValue};
use crate::utils::misc_types::{UserListResponse, ExtendedUserHabit, DayValuesStruct};
mod db;
mod utils;

type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

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

#[delete("/user_habits/{id}")]
async fn delete_user_habit(
    pool: web::Data<DbPool>,
    path_user_habit_id: web::Path<i32>,
) -> Result<HttpResponse, actix_web::Error> {
    let inner_user_habit_id = path_user_habit_id.into_inner();
    println!(
        "Deleting user_habit for user_id: {}, id: {}",
        "not yet", inner_user_habit_id
    );

    let mut conn = pool.get().map_err(|e| {
        debug!("Pool error: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;
    let result = diesel::delete(user_habits.filter(uh_id.eq(inner_user_habit_id)))
        .execute(&mut conn);
    match result {
        Ok(0) => Ok(HttpResponse::NotFound().json("User not found")),
        Ok(_) => Ok(HttpResponse::Ok().json("User deleted")),
        Err(err) => {
            eprintln!("Error deleting user: {:?}", err);
            Ok(HttpResponse::InternalServerError().json("Internal error"))
        }
    }
}

#[post("/user_habits/reorder")]
async fn reorder_user_habits(
    pool: web::Data<DbPool>,
    req: web::Json<SequenceUpdateRequest>,
) -> Result<HttpResponse, actix_web::Error> {
   let user_habit_ids = req.into_inner().ordered_user_habit_ids.clone();

   let result: Result<_, actix_web::Error> = Ok(web::block(move || {
       let mut connection = pool.get().map_err(|e| {
           debug!("Pool error: {:?}", e);
           actix_web::error::ErrorInternalServerError(e)
       }).expect("Connection to db failed");

       let _ = connection.transaction(|conn| {
           for (index, user_habit_id) in user_habit_ids.iter().enumerate() {
               diesel::update(user_habits.filter(uh_id.eq(user_habit_id)))
                   .set(uh_sequence.eq(index as i32))
                   .execute(conn)?;
               }
           diesel::result::QueryResult::Ok(())
       }).map_err(|e| {
           debug!("Pool error: {:?}", e);
           actix_web::error::ErrorInternalServerError(e)
       });
    })
    .await);

    match result {
        Ok(_) => Ok(HttpResponse::Ok().json("Sequence updated")),
        Err(e) => {
            eprintln!("Error updating sequence: {:?}", e);
            Ok(HttpResponse::InternalServerError().json("Failed to update sequence"))
        }
    }
}

#[post("/user_habits")]
async fn create_user_habit(
    pool: web::Data<DbPool>,
    req_body: web::Json<NewUserHabit>,
) -> Result<HttpResponse, actix_web::Error> {
    let new_user_habit = req_body.into_inner();
    debug!(
        "Creating user_habit for user_id: {}, name: {:?}, weight: {}, sequence: {}",
        new_user_habit.user_id, new_user_habit.name, new_user_habit.weight, new_user_habit.sequence
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
    println!(
        "Creating day_value for value_id: {}, habit_id: {}, date: {}, text: {}, number: {}",
        new_day_value.value_id, new_day_value.habit_id, new_day_value.date, new_day_value.text.clone().unwrap_or("".to_string()), new_day_value.number.clone().unwrap_or(0)
    );
    let mut conn = pool.get().map_err(actix_web::error::ErrorInternalServerError)?;
    let inserted = diesel::insert_into(day_values)
        .values(&new_day_value.clone())
        .on_conflict((dv_date, dv_habit_id))
        .do_update()
        .set((
            dv_value_id.eq(new_day_value.value_id),
            dv_text.eq(new_day_value.text),
            dv_number.eq(new_day_value.number),
            dv_created_at.eq(now)
        ))
        .get_result::<DayValue>(&mut conn)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(inserted))
}

#[get("/users/{path_user_id}/list")]
async fn get_habit_values(
    pool: web::Data<DbPool>,
    path_user_id: web::Path<i32>,
) -> Result<HttpResponse, actix_web::Error> {
    let inner_user_id = path_user_id.into_inner();
    debug!("Fetching user list for user_id: {}", inner_user_id);

    let mut conn = pool.get().map_err(|e| {
        debug!("Pool error: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    // Fetch data
    let habit_data = user_habits
        .inner_join(habit_values.on(hv_habit_id.eq(uh_id)))
        .inner_join(day_values.on(dv_value_id.eq(hv_id)))
        .filter(uh_user_id.eq(inner_user_id))
        .select((
            uh_id,
            dv_date,
            dv_value_id,
        ))
        .order(dv_date.asc())
        .load::<(i32, NaiveDate, i32)>(&mut conn)
        .map_err(|e| {
            debug!("Query error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    let habit_value = user_habits
        .inner_join(habit_values.on(hv_habit_id.eq(uh_id)))
        .filter(uh_user_id.eq(inner_user_id))
        .select((
            uh_id, uh_name, uh_weight, uh_sequence, uh_habit_type, uh_user_id, uh_created_at,
            hv_id, hv_label, hv_sequence, hv_color, hv_created_at
        ))
        .load::<(
            i32, String, i32, i32, String, i32, NaiveDateTime,
            i32, Option<String>, i32, Option<String>, NaiveDateTime
        )>(&mut conn)
        .map_err(|e| {
            debug!("Query error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;
    // Build response
    let mut dates_map: HashMap<String, HashMap<i32, i32>> = HashMap::new();
    let mut habits_map: HashMap<i32, ExtendedUserHabit> = HashMap::new();

    for (habit_id, date, day_value_id) in habit_data {
        // Dates: date -> habit_id -> day_value_id
        let date_str = date.to_string();
        dates_map
            .entry(date_str)
            .or_insert_with(HashMap::new)
            .insert(habit_id, day_value_id);
    }

    let mut dates: Vec<DayValuesStruct> = dates_map.into_iter().map(|(key, values)| {
        let date = key.to_string();
        DayValuesStruct { date, values }
    }).collect();

    dates.sort_by(|a, b| a.date.cmp(&b.date));

    for (
        habit_id, habit_name, habit_weight, habit_sequence, habit_type, habit_user_id, habit_created_at,
        value_id, value_label, value_sequence, value_color, value_created_at
    ) in habit_value {
        // Habits: habit_id -> details with values
        let habit_entry = habits_map.entry(habit_id).or_insert(ExtendedUserHabit {
            habit: UserHabit {
                id: habit_id,
                name: habit_name,
                weight: habit_weight,
                sequence: habit_sequence,
                habit_type,
                user_id: habit_user_id,
                created_at: habit_created_at
            },
            values: Vec::new(),
            values_hashmap: HashMap::new(),
        });
        habit_entry.values.push(HabitValue {
            id: value_id,
            label: value_label,
            sequence: value_sequence,
            habit_id,
            color: value_color,
            created_at: value_created_at,
        });
    }

    let mut habits: Vec<ExtendedUserHabit> = habits_map.into_iter().map(|(_, mut habit)| {
        habit.values.sort_by(|a, b| a.sequence.cmp(&b.sequence));
        for (index, value) in habit.values.iter().enumerate() {
            habit.values_hashmap.insert(value.id, index.try_into().unwrap());
        };
        habit
    }).collect();

    habits.sort_by(|a, b| a.habit.sequence.cmp(&b.habit.sequence));

    let response = UserListResponse { dates, habits };
    debug!("Returning user list with {} dates, {} habits", response.dates.len(), response.habits.len());
    Ok(HttpResponse::Ok().json(response))
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
            .service(create_user)
            .service(create_user_habit)
            .service(create_habit_value)
            .service(create_day_value)
            .service(get_habit_values)
            .service(delete_user_habit)
            .service(reorder_user_habits)
            //.route("/hey", web::get().to(manual_hello))
    })
    .bind(format!("{}:{}", c.host, c.port))?
    .run()
    .await
}
