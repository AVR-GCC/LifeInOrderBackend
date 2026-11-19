use core::result::Result;

use diesel::prelude::*;

use crate::db::models::{NewVOption, VOption};
use crate::db::schema::options::dsl::{
    color_enc as hv_color_enc, options as options_table, id as hv_id, label_enc as hv_label_enc, sequence as hv_sequence,
};
use crate::utils::crypto::{Dek, encrypt};
use crate::utils::misc_types::{ClientNewOption, ClientOption, Storage};

pub fn create_option(
    store: &mut Storage,
    dek: &Dek,
    client_option: ClientNewOption,
) -> Result<ClientOption, actix_web::Error> {
    println!(
        "Creating user_habit for habit_id: {}, color: {:?}",
        client_option.habit_id,
        client_option.color
    );

    let label_enc = client_option.label.clone().and_then(|label| Some(encrypt(dek.as_bytes(), label.as_bytes()).expect("Encrypting option label failed")));
    let color_enc = client_option.color.clone().and_then(|color| Some(encrypt(dek.as_bytes(), color.as_bytes()).expect("Encrypting option color failed")));

    let new_option = NewVOption {
        habit_id: client_option.habit_id,
        label_enc,
        sequence: client_option.sequence,
        color_enc
    };

    let inserted = diesel::insert_into(options_table)
        .values(&new_option)
        .get_result::<VOption>(&mut store.db)
        .map_err(|e| {
            println!("Insert error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    println!("Inserted option: {:?}", inserted);

    let created_option = ClientOption {
        id: inserted.id,
        habit_id: client_option.habit_id,
        label: client_option.label,
        sequence: client_option.sequence,
        color: client_option.color,
        created_at: inserted.created_at
    };

    Ok(created_option)
}

pub fn update_option(
    store: &mut Storage,
    dek: &Dek,
    option_update: ClientOption
) -> Result<ClientOption, actix_web::Error> {
    println!(
        "Updating option_update for habit_id: {}, color: {:?}",
        option_update.habit_id,
        option_update.color
    );

    let label_enc = option_update.label.clone().and_then(|label| Some(encrypt(dek.as_bytes(), label.as_bytes()).expect("Encrypting option label failed")));
    let color_enc = option_update.color.clone().and_then(|color| Some(encrypt(dek.as_bytes(), color.as_bytes()).expect("Encrypting option color failed")));

    let inserted = diesel::update(options_table)
        .filter(hv_id.eq(option_update.id))
        .set((
            hv_label_enc.eq(label_enc),
            hv_color_enc.eq(color_enc),
        ))
        .get_result::<VOption>(&mut store.db)
        .map_err(|e| {
            println!("Update error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    println!("Updated option: {:?}", inserted);
    Ok(option_update)
}

pub fn delete_option(
    store: &mut Storage,
    option_id: i32
) -> Result<usize, actix_web::Error> {
    println!(
        "Deleting option for user_id: {}, id: {}",
        "not yet", option_id
    );

    let result = diesel::delete(options_table.filter(hv_id.eq(option_id)))
        .execute(&mut store.db)
        .map_err(|e| {
            println!("Delete error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    Ok(result)
}

pub async fn reorder_options(
    store: &mut Storage,
    option_ids: Vec<i32>,
) -> Result<(), actix_web::Error> {
    let _ = store.db
        .transaction(|db| {
            for (index, option_id) in option_ids.iter().enumerate() {
                diesel::update(options_table.filter(hv_id.eq(option_id)))
                    .set(hv_sequence.eq(index as i32 + 1))
                    .execute(db)?;
            }
            diesel::result::QueryResult::Ok(())
        })
        .map_err(|e| {
            println!("Pool error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        });
    Ok(())
}
