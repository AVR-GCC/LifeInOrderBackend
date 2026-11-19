use core::iter::Iterator;

use crate::utils::crypto::{Dek, encrypt};
use crate::utils::misc_types::{ClientNewValue, ClientValue, Storage};
use chrono::Datelike;
use diesel::dsl::now;

use diesel::prelude::*;

use crate::db::models::{NewValue, Value};
use crate::db::schema::values::dsl::{
    created_at as dv_created_at, date as dv_date, values as values_table, habit_id as dv_habit_id,
    number as dv_number, text_enc as dv_text_enc, value_id as dv_value_id,
};
use crate::utils::general::get_data_cache_key;
use crate::utils::misc_types::ZoomLevel;
use redis::Commands;

pub fn set_value(
    store: &mut Storage,
    user_id: i32,
    dek: &Dek,
    client_value: ClientNewValue,
) -> Result<ClientValue, actix_web::Error> {
    println!(
        "Creating value for value_id: {}, habit_id: {}, date: {}, text_enc: {:?}, number: {}",
        client_value.value_id,
        client_value.habit_id,
        client_value.date,
        client_value.text,
        client_value.number.clone().unwrap_or(0)
    );

    let text_enc = client_value.text.clone().and_then(|text| Some(encrypt(dek.as_bytes(), text.as_bytes()).expect("Encrypting value text failed")));

    let new_value = NewValue {
        value_id: client_value.value_id,
        habit_id: client_value.habit_id,
        date: client_value.date,
        text_enc: text_enc.clone(),
        number: client_value.number
    };

    let inserted = diesel::insert_into(values_table)
        .values(&new_value.clone())
        .on_conflict((dv_date, dv_habit_id))
        .do_update()
        .set((
            dv_value_id.eq(new_value.value_id),
            dv_text_enc.eq(text_enc),
            dv_number.eq(new_value.number),
            dv_created_at.eq(now),
        ))
        .get_result::<Value>(&mut store.db)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let year = new_value.date.year();
    let month = new_value.date.month();
    let cache_key = get_data_cache_key(user_id, year, month, ZoomLevel::Day);
    let _ = store.cache.del::<String, usize>(cache_key);
    let keys: Vec<String> = ZoomLevel::ALL
        .iter()
        .map(|zoom| get_data_cache_key(user_id, year, month, *zoom))
        .collect();
    for key in &keys {
        let _ = store.cache.del::<String, usize>(key.to_string());
    }

    let response = ClientValue {
        id: inserted.id,
        created_at: inserted.created_at,
        value_id: client_value.value_id,
        habit_id: client_value.habit_id,
        date: client_value.date,
        text: client_value.text,
        number: client_value.number,
    };

    Ok(response)
}
