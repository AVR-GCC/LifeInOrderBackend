use core::iter::Iterator;

use chrono::Datelike;
use diesel::dsl::now;
use crate::utils::misc_types::Storage;

use diesel::prelude::*;

use crate::db::models::{Value, NewValue};
use crate::db::schema::day_values::dsl::{
    created_at as dv_created_at, date as dv_date, day_values, habit_id as dv_habit_id,
    number as dv_number, text as dv_text, value_id as dv_value_id,
};
use crate::utils::general::get_cache_key;
use crate::utils::misc_types::ZoomLevel;
use redis::Commands;

pub fn set_value(
    mut store: Storage,
    new_value: NewValue,
    user_id: i32,
) -> Result<Value , actix_web::Error> {
    println!(
        "Creating value for value_id: {}, habit_id: {}, date: {}, text: {}, number: {}",
        new_value.value_id,
        new_value.habit_id,
        new_value.date,
        new_value.text.clone().unwrap_or("".to_string()),
        new_value.number.clone().unwrap_or(0)
    );

    let inserted = diesel::insert_into(day_values)
        .values(&new_value.clone())
        .on_conflict((dv_date, dv_habit_id))
        .do_update()
        .set((
            dv_value_id.eq(new_value.value_id),
            dv_text.eq(new_value.text),
            dv_number.eq(new_value.number),
            dv_created_at.eq(now),
        ))
        .get_result::<Value>(&mut store.db)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let year = new_value.date.year();
    let month = new_value.date.month();
    let cache_key = get_cache_key(user_id, year, month, ZoomLevel::Day);
    let _ = store.cache.del::<String, usize>(cache_key);
    let keys: Vec<String> = ZoomLevel::ALL.iter().map(|zoom| { get_cache_key(user_id, year, month, *zoom) }).collect();
    for key in &keys {
        let _ = store.cache.del::<String, usize>(key.to_string());
    }

    Ok(inserted)
}
