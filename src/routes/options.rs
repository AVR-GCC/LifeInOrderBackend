use core::result::Result;

use diesel::prelude::*;

use crate::db::models::{VOption, NewVOption};
use crate::db::schema::habit_values::dsl::{color as hv_color, habit_values, id as hv_id, label as hv_label, sequence as hv_sequence};
use crate::utils::misc_types::Storage;

use actix_web::web;

pub fn create_option(
    mut store: Storage,
    new_option: NewVOption
) -> Result<VOption, actix_web::Error> {
    println!(
        "Creating user_habit for habit_id: {}, color: {}",
        new_option.habit_id,
        new_option.color.clone().unwrap_or("".to_string())
    );

    let inserted = diesel::insert_into(habit_values)
        .values(&new_option)
        .get_result::<VOption>(&mut store.db)
        .map_err(|e| {
            println!("Insert error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    println!("Inserted option: {:?}", inserted);

    Ok(inserted)
}

pub fn update_option(
    mut store: Storage,
    option: VOption
) -> Result<VOption, actix_web::Error> {
    println!(
        "Updating option for habit_id: {}, color: {}",
        option.habit_id,
        option.color.clone().unwrap_or("".to_string())
    );

    let inserted = diesel::update(habit_values)
        .filter(hv_id.eq(option.id))
        .set((
            hv_label.eq(option.label),
            hv_color.eq(option.color),
        ))
        .get_result::<VOption>(&mut store.db)
        .map_err(|e| {
            println!("Update error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    println!("Updated option: {:?}", inserted);
    Ok(inserted)
}

pub fn delete_option(
    mut store: Storage,
    option_id: i32
) -> Result<usize, actix_web::Error> {
    println!(
        "Deleting option for user_id: {}, id: {}",
        "not yet", option_id
    );

    let result =
        diesel::delete(habit_values.filter(hv_id.eq(option_id)))
        .execute(&mut store.db)
        .map_err(|e| {
            println!("Delete error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    Ok(result)
}

pub async fn reorder_options(
    mut store: Storage,
    option_ids: Vec<i32>
) -> Result<(), actix_web::Error> {
    let _result: Result<_, actix_web::Error> = Ok(web::block(move || {

        let _ = store.db
            .transaction(|db| {
                for (index, option_id) in option_ids.iter().enumerate() {
                    diesel::update(habit_values.filter(hv_id.eq(option_id)))
                        .set(hv_sequence.eq(index as i32 + 1))
                        .execute(db)?;
                }
                diesel::result::QueryResult::Ok(())
            })
            .map_err(|e| {
                println!("Pool error: {:?}", e);
                actix_web::error::ErrorInternalServerError(e)
            });
    })
        .await);
    Ok(())
}
