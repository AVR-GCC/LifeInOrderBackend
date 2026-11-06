use std::collections::HashMap;

// src/crypto.rs
use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, Generate, KeyInit},
};
use diesel::prelude::*;
use crate::{db::models::{
    User, Value,
}, routes::aggregates::get_extended_habits};
use crate::db::schema::values::dsl::{
    date as dv_date, values as values_table, habit_id as dv_habit_id,
};
use crate::{
    db::{
        schema::{
            users::dsl::{
                users as users_table,
            }
        }
    }, utils::{general::{DbValue, bulk_update}, misc_types::Storage}
};

pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, actix_web::Error> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(actix_web::error::ErrorInternalServerError)?;
    let nonce = Nonce::generate();
    let ct = cipher.encrypt(&nonce, plaintext).map_err(actix_web::error::ErrorInternalServerError)?;
    Ok([nonce.as_slice(), &ct].concat())
}

pub fn decrypt(key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>, actix_web::Error> {
    if blob.len() < 12 + 16 {
        return Err(actix_web::error::ErrorInternalServerError("ciphertext too short"));
    }
    let (nonce, ct) = blob.split_at(12);
    let nonce = <&Nonce<_>>::try_from(nonce).map_err(actix_web::error::ErrorInternalServerError)?;
    Aes256Gcm::new_from_slice(key)
        .map_err(actix_web::error::ErrorInternalServerError)?
        .decrypt(nonce, ct)
        .map_err(actix_web::error::ErrorInternalServerError)
}

pub fn new_dek() -> [u8; 32] {
    Key::<Aes256Gcm>::generate().into()
}

pub fn create_user_wrapped_dek(key: &[u8; 32]) -> Result<([u8; 32], Vec<u8>), actix_web::Error> {
    let dek = new_dek();
    let wrapped_dek = encrypt(key, &dek).expect("Failed to encrypt dek");
    Ok((dek, wrapped_dek))
}

pub fn get_user_dek(key: &[u8; 32], wrapped_dek: Vec<u8>) -> Result<Vec<u8>, actix_web::Error> {
    decrypt(key, &wrapped_dek)
}

pub fn create_all_user_wrapped_dek(
    store: &mut Storage,
    bytes: [u8; 32]
) -> Result<HashMap<i32, [u8; 32]>, actix_web::Error> {
    let users = users_table
        .load::<User>(&mut store.db)
        .map_err(|e| {
            println!("Query error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    let mut pairs: Vec<(i32, DbValue)> = vec![];
    let mut user_deks: HashMap<i32, [u8; 32]> = HashMap::new();
    for user in users {
        let (dek, wrapped_dek) = create_user_wrapped_dek(&bytes).expect("Failed to create user wrapped_dek");
        user_deks.insert(user.id, dek);
        pairs.push((user.id, DbValue::Bytes(wrapped_dek)));
    }
    let _ = bulk_update(store, "users", "wrapped_dek", pairs.as_slice());
    Ok(user_deks)
}

pub async fn encrypt_all_users_data(
    store: &mut Storage,
    bytes: [u8; 32]
) -> Result<(), actix_web::Error> {
    let user_deks = create_all_user_wrapped_dek(store, bytes).expect("Failed to init deks");

    for (user_id, dek) in user_deks.iter() {
        let habits = get_extended_habits(&mut store.db, *user_id).await?;
        let habit_ids: Vec<i32> = habits.iter().map(|h| h.habit.id).collect();
        let mut habit_names: Vec<(i32, DbValue)> = vec![];
        let mut option_labels: Vec<(i32, DbValue)> = vec![];
        let mut option_colors: Vec<(i32, DbValue)> = vec![];
        for habit in habits {
            let habit_name = encrypt(&dek, habit.habit.name.as_bytes()).expect("Failed to encrypt habit name");
            habit_names.push((habit.habit.id, DbValue::Bytes(habit_name)));
            for opt in habit.values {
                if let Some(label) = opt.label {
                    let option_label = encrypt(&dek, label.as_bytes()).expect("Failed to encrypt option label");
                    option_labels.push((opt.id, DbValue::Bytes(option_label)));
                }
                if let Some(color) = opt.color {
                    let option_color = encrypt(&dek, color.as_bytes()).expect("Failed to encrypt option color");
                    option_colors.push((opt.id, DbValue::Bytes(option_color)));
                }
            }
        }
        let all_values: Vec<Value> = values_table
            .filter(dv_habit_id.eq_any(&habit_ids))
            .order((dv_date.asc(), dv_habit_id.asc()))
            .load::<Value>(&mut store.db)
            .map_err(|e| {
                println!("Values query error: {:?}", e);
                actix_web::error::ErrorInternalServerError(e)
            })?;
        let mut value_texts: Vec<(i32, DbValue)> = vec![];
        for val in all_values {
            if let Some(text) = val.text {
                let value_text = encrypt(&dek, text.as_bytes()).expect("Failed to encrypt value text");
                value_texts.push((val.id, DbValue::Bytes(value_text)));
            }
        }
        let _ = bulk_update(store, "habits", "name_enc", habit_names.as_slice());
        let _ = bulk_update(store, "options", "label_enc", option_labels.as_slice());
        let _ = bulk_update(store, "options", "color_enc", option_colors.as_slice());
        let _ = bulk_update(store, "values", "text_enc", value_texts.as_slice());
    }

    // let my_vals = values_table
    //     .filter(dv_date.gt(NaiveDate::from_ymd_opt(2026, 9, 1).unwrap()))
    //     .load::<Value>(&mut store.db)
    //     .map_err(|e| {
    //         println!("Query error: {:?}", e);
    //         actix_web::error::ErrorInternalServerError(e)
    //     })?;
    // for my_val in my_vals {
    //     if let Some(txt) = my_val.text {
    //         let encrypted_bytes = encrypt(&bytes, txt.as_bytes()).expect("Failed to encrypt");
    //         let decrypted = decrypt(&bytes, &encrypted_bytes).expect("Failed to dencrypt");
    //         let finaly = String::from_utf8(decrypted).expect("Convert to string failed");
    //     }
    // }
    Ok(())
}
