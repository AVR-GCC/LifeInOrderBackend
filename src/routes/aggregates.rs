use actix_web::{HttpResponse};
use chrono::{Datelike, NaiveDate, NaiveDateTime, Utc};
use std::collections::HashMap;

use diesel::pg::PgConnection;
use diesel::prelude::*;

use crate::db::models::{
    Value, Habit, HabitType, User, VOption
};
use crate::db::schema::day_values::dsl::{
    date as dv_date, day_values, habit_id as dv_habit_id,
};
use crate::db::schema::habit_values::dsl::{
    color as hv_color, created_at as hv_created_at, habit_id as hv_habit_id, habit_values,
    id as hv_id, label as hv_label, sequence as hv_sequence,
};
use crate::db::schema::user_habits::dsl::{
    created_at as uh_created_at, habit_type as uh_habit_type, id as uh_id, name as uh_name,
    sequence as uh_sequence, user_habits, user_id as uh_user_id, weight as uh_weight,
};
use crate::db::schema::users::dsl::{
    created_at as u_created_at, email as u_email, id as u_id, name as u_name, users,
};
use crate::utils::general::{
    create_period_image, get_month_user_values_list, get_next_date, get_user_values_dates_map
};
use crate::utils::misc_types::{ExtendedHabit, Storage, UserListResponse, ZoomLevel};

pub async fn get_extended_habits(
    db: &mut PgConnection,
    user_id: i32,
) -> Result<Vec<ExtendedHabit>, actix_web::Error> {
    let habit_value = user_habits
        .inner_join(habit_values.on(hv_habit_id.eq(uh_id)))
        .filter(uh_user_id.eq(user_id))
        .select((
            uh_id,
            uh_name,
            uh_weight,
            uh_sequence,
            uh_habit_type,
            uh_user_id,
            uh_created_at,
            hv_id,
            hv_label,
            hv_sequence,
            hv_color,
            hv_created_at,
        ))
        .load::<(
            i32,
            String,
            i32,
            i32,
            HabitType,
            i32,
            NaiveDateTime,
            i32,
            Option<String>,
            i32,
            Option<String>,
            NaiveDateTime,
        )>(db)
        .map_err(|e| {
            println!("Query error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    let mut habits_map: HashMap<i32, ExtendedHabit> = HashMap::new();

    for (
        habit_id,
        habit_name,
        habit_weight,
        habit_sequence,
        habit_type,
        habit_user_id,
        habit_created_at,
        value_id,
        value_label,
        value_sequence,
        value_color,
        value_created_at,
    ) in habit_value
    {
        // Habits: habit_id -> details with values
        let habit_entry = habits_map.entry(habit_id).or_insert(ExtendedHabit {
            habit: Habit {
                id: habit_id,
                name: habit_name,
                weight: habit_weight,
                sequence: habit_sequence,
                habit_type,
                user_id: habit_user_id,
                created_at: habit_created_at,
            },
            values: Vec::new(),
            values_hashmap: HashMap::new(),
        });
        habit_entry.values.push(VOption {
            id: value_id,
            label: value_label,
            sequence: value_sequence,
            habit_id,
            color: value_color,
            created_at: value_created_at,
        });
    }

    let mut habits: Vec<ExtendedHabit> = habits_map
        .into_iter()
        .map(|(_, mut habit)| {
            habit.values.sort_by(|a, b| a.sequence.cmp(&b.sequence));
            for (index, value) in habit.values.iter().enumerate() {
                habit
                    .values_hashmap
                    .insert(value.id, index.try_into().unwrap());
            }
            habit
        })
        .collect();

    habits.sort_by(|a, b| a.habit.sequence.cmp(&b.habit.sequence));

    Ok(habits)
}

pub async fn get_list(
    mut store: Storage,
    user_id: i32,
    date: NaiveDate,
    count: u32,
    zoom: ZoomLevel,
    width: i32,
) -> Result<HttpResponse, actix_web::Error> {
    let year = date.year();
    let month = date.month();
    let start_date = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let (mut to_month, mut to_year) = (month, year);
    for _ in 0..count {
        (to_month, to_year) = get_next_date((to_month, to_year), zoom);
    }
    let end_date = NaiveDate::from_ymd_opt(to_year, to_month, 1).unwrap();

    // dbg!(date);
    // dbg!(start_date);
    // dbg!(end_date);
    let dates_map = get_user_values_dates_map(
        &mut store.cache,
        &mut store.db,
        user_id,
        Some(start_date),
        Some(end_date),
    )
    .await?;

    let habits = get_extended_habits(&mut store.db, user_id)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    if matches!(zoom, ZoomLevel::Day) {
        let mut dates = Vec::new();
        let (mut cur_month, mut cur_year) = (month, year);
        for _ in 0..count {
            let month_values =
            get_month_user_values_list(cur_month, cur_year, user_id, &dates_map);
            dates.push(month_values);
            (cur_month, cur_year) = get_next_date((cur_month, cur_year), zoom);
        }
        Ok(HttpResponse::Ok().json(dates))
    } else {
        let row_height = match zoom {
            ZoomLevel::Quarter => 8,
            ZoomLevel::Half => 4,
            ZoomLevel::Year => 2,
            ZoomLevel::TwoYear => 1,
            _ => 1,
        };
        let mut dates = Vec::new();
        let mut current_month = start_date.month();
        let mut current_year = start_date.year();
        let end_month = end_date.month();
        let end_year = end_date.year();

        while current_month != end_month || current_year != end_year {
            let mut month_values = get_month_user_values_list(
                current_month,
                current_year,
                user_id,
                &dates_map,
            );
            dates.append(&mut month_values.days);
            if current_month == 12 {
                current_month = 1;
                current_year += 1;
            } else {
                current_month += 1;
            }
        }
        let habits = habits
            .into_iter()
            .filter(|habit| habit.habit.habit_type == HabitType::Color)
            .collect();
        let response = UserListResponse { dates, habits };
        match create_period_image(response, width, row_height) {
            Ok(webp_data) => Ok(HttpResponse::Ok()
                .content_type("image/webp")
                .body(webp_data)),
            Err(e) => {
                println!("Error generating visualization: {:?}", e);
                Err(actix_web::error::ErrorInternalServerError(e))
            }
        }
    }
}

pub async fn get_backup(
    mut store: Storage,
    user_id: i32,
) -> Result<HttpResponse, actix_web::Error> {
    println!("Creating backup for user_id: {}", user_id);
    // Fetch user info
    let user = users
        .filter(u_id.eq(user_id))
        .select((u_id, u_name, u_email, u_created_at))
        .first::<User>(&mut store.db)
        .map_err(|e| {
            println!("User query error: {:?}", e);
            actix_web::error::ErrorNotFound("User not found")
        })?;

    // Fetch all habits with their values
    let habits = get_extended_habits(&mut store.db, user_id).await?;

    // Collect all habit ids
    let habit_ids: Vec<i32> = habits.iter().map(|h| h.habit.id).collect();

    // Fetch all day_values for all of the user's habits
    let all_day_values: Vec<Value> = day_values
        .filter(dv_habit_id.eq_any(&habit_ids))
        .order((dv_date.asc(), dv_habit_id.asc()))
        .load::<Value>(&mut store.db)
        .map_err(|e| {
            println!("Day values query error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    // Build the backup JSON
    let backup = serde_json::json!({
        "backup_date": Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "user": {
            "id": user.id,
            "name": user.name,
            "email": user.email,
            "created_at": user.created_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
        },
        "habits": habits.iter().map(|h| {
            serde_json::json!({
                "id": h.habit.id,
                "name": h.habit.name,
                "weight": h.habit.weight,
                "sequence": h.habit.sequence,
                "habit_type": h.habit.habit_type,
                "created_at": h.habit.created_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
                "values": h.values.iter().map(|v| {
                    serde_json::json!({
                        "id": v.id,
                        "label": v.label,
                        "sequence": v.sequence,
                        "color": v.color,
                        "created_at": v.created_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "day_values": all_day_values.iter().map(|dv| {
            serde_json::json!({
                "id": dv.id,
                "habit_id": dv.habit_id,
                "value_id": dv.value_id,
                "date": dv.date.format("%Y-%m-%d").to_string(),
                "text": dv.text,
                "number": dv.number,
                "created_at": dv.created_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
            })
        }).collect::<Vec<_>>(),
    });

    let body = serde_json::to_string_pretty(&backup)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let filename = format!(
        "life_in_order_backup_user_{}_{}.json",
        user_id,
        Utc::now().format("%Y%m%d_%H%M%S")
    );

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", filename),
        ))
        .body(body))
}
