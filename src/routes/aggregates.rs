use actix_web::HttpResponse;
use chrono::{Datelike, NaiveDate, NaiveDateTime, Utc};
use std::collections::HashMap;
use base64::{engine::general_purpose::STANDARD, Engine as _};

use redis::Commands;

use diesel::pg::PgConnection;
use diesel::prelude::*;

use crate::db::models::{
    Value, HabitType, User,
};
use crate::db::schema::values::dsl::{
    date as dv_date, values as values_table, habit_id as dv_habit_id,
};
use crate::db::schema::options::dsl::{
    color_enc as hv_color_enc, created_at as hv_created_at, habit_id as hv_habit_id, options as options_table,
    id as hv_id, label_enc as hv_label_enc, sequence as hv_sequence,
};
use crate::db::schema::habits::dsl::{
    created_at as uh_created_at, habit_type as uh_habit_type, id as uh_id, name_enc as uh_name_enc,
    // name_enc as uh_name_enc,
    sequence as uh_sequence, habits as habits_table, user_id as uh_user_id, weight as uh_weight,
};
use crate::db::schema::users::dsl::{
    created_at as u_created_at, email as u_email, id as u_id, password_hash as u_password_hash, users, wrapped_dek as u_wrapped_dek
};
use crate::utils::crypto::Dek;
use crate::utils::general::{
    create_period_image, decrypt_opt, decrypt_str, get_data_cache_key, get_month_user_values_list, get_next_date, get_user_values_dates_map
};
use crate::utils::misc_types::{ClientHabit, ClientOption, DateRange, ExtendedHabit, PeriodImageStruct, Storage, UserListResponse, ValuesOrImage, ZoomLevel};

pub async fn get_extended_habits(
    db: &mut PgConnection,
    user_id: i32,
    dek: &Dek
) -> Result<Vec<ExtendedHabit>, actix_web::Error> {
    let habit_value = habits_table
        .inner_join(options_table.on(hv_habit_id.eq(uh_id)))
        .filter(uh_user_id.eq(user_id))
        .select((
            uh_id,
            uh_name_enc,
            uh_weight,
            uh_sequence,
            uh_habit_type,
            uh_created_at,
            hv_id,
            hv_label_enc,
            hv_sequence,
            hv_color_enc,
            hv_created_at,
        ))
        .load::<(
            i32,
            Option<Vec<u8>>,
            i32,
            i32,
            HabitType,
            NaiveDateTime,
            i32,
            Option<Vec<u8>>,
            i32,
            Option<Vec<u8>>,
            NaiveDateTime,
        )>(db)
        .map_err(|e| {
            println!("Query error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    let mut habits_map: HashMap<i32, ExtendedHabit> = HashMap::new();

    for (
        habit_id,
        habit_name_enc,
        habit_weight,
        habit_sequence,
        habit_type,
        habit_created_at,
        option_id,
        option_label_enc,
        option_sequence,
        option_color_enc,
        option_created_at,
    ) in habit_value
    {
        // Habits: habit_id -> details with values

        let habit_entry = habits_map.entry(habit_id).or_insert(ExtendedHabit {
            habit: ClientHabit {
                id: habit_id,
                name: decrypt_str(habit_name_enc, dek),
                weight: habit_weight,
                sequence: habit_sequence,
                habit_type,
                created_at: habit_created_at,
            },
            values: Vec::new(),
            values_hashmap: HashMap::new(),
        });
        let option_label = decrypt_opt(option_label_enc, dek);
        let option_color = decrypt_opt(option_color_enc, dek);
        habit_entry.values.push(ClientOption {
            id: option_id,
            label: option_label,
            sequence: option_sequence,
            habit_id,
            color: option_color,
            created_at: option_created_at,
        });
    }

    let mut habits: Vec<ExtendedHabit> = habits_map
        .into_iter()
        .map(|(_, mut habit)| {
            habit.values.sort_by(|a, b| a.sequence.cmp(&b.sequence));
            for (index, value) in habit.values.iter().enumerate() {
                habit.values_hashmap.insert(value.id, index.try_into().unwrap());
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
    dek: &Dek,
    date: NaiveDate,
    zoom: ZoomLevel,
    width: i32,
) -> Result<ValuesOrImage, actix_web::Error> {
    let year = date.year();
    let month = date.month();
    let start_date = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let (to_month, to_year) = get_next_date((month, year), zoom);
    let end_date = NaiveDate::from_ymd_opt(to_year, to_month, 1).unwrap();
    let end_month = end_date.month();
    let end_year = end_date.year();
    let start = format!("{}-{:02}-01", year, month);
    let end = format!(
        "{}-{:02}-01",
        end_year,
        end_month,
    );
    let range = DateRange { start, end };

    if matches!(zoom, ZoomLevel::Day) {
        let dates_map = get_user_values_dates_map(
            &mut store.cache,
            &mut store.db,
            user_id,
            dek,
            start_date,
            end_date,
        )
        .await?;

        let month_values = get_month_user_values_list(month, year, user_id, &dates_map);
        Ok(ValuesOrImage::Values(month_values))
    } else {
        let key = get_data_cache_key(user_id, year, month, zoom);
        let value_opt: Option<String> = store.cache.get(&key).unwrap();
        if let Some(cache_value) = value_opt {
            let period_image_struct = PeriodImageStruct { range, image: cache_value, zoom };
            Ok(ValuesOrImage::Image(period_image_struct))
        } else {
            let dates_map = get_user_values_dates_map(
                &mut store.cache,
                &mut store.db,
                user_id,
                dek,
                start_date,
                end_date,
            )
            .await?;
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
            let habits = get_extended_habits(&mut store.db, user_id, dek)
                .await
                .map_err(actix_web::error::ErrorInternalServerError)?;

            let habits = habits
                .into_iter()
                .filter(|habit| habit.habit.habit_type == HabitType::Color)
                .collect();
            let response = UserListResponse { dates, habits };

            match create_period_image(response, width, row_height) {
                Ok(webp_data) => {
                    let mut base64_image = String::with_capacity("data:image/webp;base64,".len() + (webp_data.len() + 2) / 3 * 4);
                    base64_image.push_str("data:image/webp;base64,");
                    STANDARD.encode_string(&webp_data, &mut base64_image);
                    let _: () = store.cache
                        .set(key, &base64_image)
                        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
                    let period_image_struct = PeriodImageStruct { range, image: base64_image, zoom };
                    Ok(ValuesOrImage::Image(period_image_struct))
                },
                Err(e) => {
                    println!("Error generating visualization: {:?}", e);
                    Err(actix_web::error::ErrorInternalServerError(e))
                }
            }
        }
    }
}

pub async fn get_backup(
    mut store: Storage,
    dek: &Dek,
    user_id: i32,
) -> Result<HttpResponse, actix_web::Error> {
    println!("Creating backup for user_id: {}", user_id);
    // Fetch user info
    let user = users
        .filter(u_id.eq(user_id))
        .select((u_id, u_email, u_created_at, u_password_hash, u_wrapped_dek))
        .first::<User>(&mut store.db)
        .map_err(|e| {
            println!("User query error: {:?}", e);
            actix_web::error::ErrorNotFound("User not found")
        })?;

    // Fetch all habits with their values
    let habits = get_extended_habits(&mut store.db, user_id, dek).await?;

    // Collect all habit ids
    let habit_ids: Vec<i32> = habits.iter().map(|h| h.habit.id).collect();

    // Fetch all values for all of the user's habits
    let all_day_values: Vec<Value> = values_table
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
        "values": all_day_values.iter().map(|dv| {
            serde_json::json!({
                "id": dv.id,
                "habit_id": dv.habit_id,
                "value_id": dv.value_id,
                "date": dv.date.format("%Y-%m-%d").to_string(),
                "text": decrypt_str(dv.text_enc.clone(), dek),
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
