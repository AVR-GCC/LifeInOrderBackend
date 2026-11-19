use core::result::Result;

use diesel::prelude::*;

use crate::db::schema::habits::dsl::{
    habit_type as uh_habit_type, id as uh_id, name_enc as uh_name_enc, sequence as uh_sequence,
    habits as habits_table, weight as uh_weight,
};
use crate::utils::crypto::{Dek, encrypt};
use crate::utils::misc_types::{ClientHabit, ClientNewHabit, Storage};

use crate::db::models::{Habit, NewHabit};

pub fn create_habit(
    store: &mut Storage,
    user_id: i32,
    dek: &Dek,
    client_habit: ClientNewHabit
) -> Result<Habit, actix_web::Error> {
    println!(
        "Creating habit for name: {}, weight: {}, sequence: {}, habit_type: {:?}",
        client_habit.name,
        client_habit.weight,
        client_habit.sequence,
        client_habit.habit_type
    );

    let name_enc = Some(encrypt(dek.as_bytes(), client_habit.name.as_bytes()).expect("Encrypting habit name failed"));

    let new_habit = NewHabit {
        user_id,
        name_enc,
        weight: client_habit.weight,
        sequence: client_habit.sequence,
        habit_type: client_habit.habit_type
    };

    let inserted = diesel::insert_into(habits_table)
        .values(&new_habit)
        .get_result::<Habit>(&mut store.db)
        .map_err(|e| {
            println!("Insert error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    println!("Inserted habit: {:?}", inserted);

    Ok(inserted)
}

pub fn update_habit(
    store: &mut Storage,
    dek: &Dek,
    habit: ClientHabit
) -> Result<ClientHabit, actix_web::Error> {
    println!(
        "Updating user_habit for name: {}, weight: {}, habit_type: {:?}",
        habit.name, habit.weight, habit.habit_type
    );

    let name_enc = Some(encrypt(dek.as_bytes(), habit.name.as_bytes()).expect("Encrypting habit name failed"));

    let inserted = diesel::update(habits_table)
        .filter(uh_id.eq(habit.id))
        .set((
            uh_name_enc.eq(name_enc),
            uh_weight.eq(habit.weight),
            uh_habit_type.eq(habit.habit_type.clone()),
        ))
        .get_result::<Habit>(&mut store.db)
        .map_err(|e| {
            println!("Update error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    println!("Updated user_habit: {:?}", inserted);

    Ok(habit)
}

pub fn delete_habit(
    store: &mut Storage,
    habit_id: i32
) -> Result<usize, actix_web::Error> {
    println!(
        "Deleting user_habit for user_id: {}, id: {}",
        "not yet", habit_id
    );

    let result = diesel::delete(habits_table.filter(uh_id.eq(habit_id)))
        .execute(&mut store.db)
        .map_err(|e| {
            println!("Delete error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    Ok(result)
}

pub async fn reorder_habits(
    store: &mut Storage,
    habit_ids: Vec<i32>,
) -> Result<(), actix_web::Error> {
    let _ = store.db
        .transaction(|db| {
            for (index, habit_id) in habit_ids.iter().enumerate() {
                diesel::update(habits_table.filter(uh_id.eq(habit_id)))
                    .set(uh_sequence.eq(index as i32 + 1))
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
