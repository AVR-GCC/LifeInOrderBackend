use core::result::Result;

use diesel::prelude::*;

use crate::db::schema::user_habits::dsl::{habit_type as uh_habit_type, id as uh_id, name as uh_name, sequence as uh_sequence, user_habits, weight as uh_weight};
use crate::utils::misc_types::{Storage};

use crate::db::models::{Habit, NewHabit};
use actix_web::web;

pub fn create_habit(
    mut store: Storage,
    new_habit: NewHabit
) -> Result<Habit, actix_web::Error> {
    println!(
        "Creating habit for user_id: {}, name: {:?}, weight: {}, sequence: {}, habit_type: {:?}",
        new_habit.user_id, new_habit.name, new_habit.weight, new_habit.sequence, new_habit.habit_type
    );

    let inserted = diesel::insert_into(user_habits)
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
    mut store: Storage,
    habit: Habit
) -> Result<Habit, actix_web::Error> {
    println!(
        "Updating user_habit for name: {}, weight: {}, habit_type: {:?}",
        habit.name, habit.weight, habit.habit_type
    );

    let inserted = diesel::update(user_habits)
        .filter(uh_id.eq(habit.id))
        .set((
            uh_name.eq(habit.name),
            uh_weight.eq(habit.weight),
            uh_habit_type.eq(habit.habit_type),
        ))
        .get_result::<Habit>(&mut store.db)
        .map_err(|e| {
            println!("Update error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    println!("Updated user_habit: {:?}", inserted);

    Ok(inserted)
}

pub fn delete_habit(
    mut store: Storage,
    habit_id: i32
) -> Result<usize, actix_web::Error> {
    println!(
        "Deleting user_habit for user_id: {}, id: {}",
        "not yet", habit_id
    );

    let result =
        diesel::delete(user_habits.filter(uh_id.eq(habit_id)))
        .execute(&mut store.db)
        .map_err(|e| {
            println!("Delete error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    Ok(result)
}

pub async fn reorder_habits(
    mut store: Storage,
    habit_ids: Vec<i32>
) -> Result<(), actix_web::Error> {
    let _result: Result<_, actix_web::Error> = Ok(web::block(move || {

        let _ = store.db
            .transaction(|db| {
                for (index, habit_id) in habit_ids.iter().enumerate() {
                    diesel::update(user_habits.filter(uh_id.eq(habit_id)))
                        .set(uh_sequence.eq(index as i32 + 1))
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
