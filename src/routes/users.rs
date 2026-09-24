use diesel::prelude::*;
use argon2::{
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
    Argon2
};
use crate::{db::models::{CreateUser, NewUser, User}, routes::aggregates::get_extended_habits, utils::misc_types::ExtendedHabit};
use crate::db::schema::users::dsl::{
    created_at as u_created_at, email as u_email, id as u_id, name as u_name, password_hash as u_password_hash, users,
};
use crate::utils::misc_types::Storage;

fn confirm_password(
    candidate: String,
    hash: String
) -> Result<bool, actix_web::Error> {
    let argon2 = Argon2::default();
    let parsed_hash = PasswordHash::new(&hash).map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(argon2.verify_password(candidate.as_bytes(), &parsed_hash).is_ok())
}

pub async fn login(
    mut store: Storage,
    email: String,
    password: String
) -> Result<Vec<ExtendedHabit>, actix_web::Error> {
    let (id, hash_opt) = users
        .filter(u_email.eq(email))
        .select((
            u_id,
            u_password_hash,
        ))
        .first::<(i32, Option<String>)>(&mut store.db)
        .map_err(|e| {
            println!("Query user error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;
    match hash_opt {
        Some(hash) => {
            let password_correct = confirm_password(password, hash);
            match password_correct {
                Ok(true) => {
                    let habits = get_extended_habits(&mut store.db, id)
                        .await
                        .map_err(actix_web::error::ErrorInternalServerError)?;
                    Ok(habits)
                },
                Ok(false) => {
                    println!("User password does not match");
                    Err(actix_web::error::ErrorUnauthorized("Invalid email of password"))
                },
                Err(e) => Err(actix_web::error::ErrorInternalServerError(e))
            }
        },
        Option::None => {
            println!("User has no password hash");
            Err(actix_web::error::ErrorInternalServerError("User has no password hash"))
        }
    }
}

pub fn create_user(
    mut store: Storage,
    create_user_object: CreateUser,
) -> Result<User, actix_web::Error> {
    println!("Creating user: {:?}", create_user_object);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(create_user_object.password.as_bytes())
        .map_err(actix_web::error::ErrorInternalServerError)?.to_string();
    let new_user = NewUser {
        name: create_user_object.name,
        email: create_user_object.email,
        password_hash: password_hash,
    };

    let inserted = diesel::insert_into(users)
        .values(&new_user)
        .returning((u_id, u_name, u_email, u_password_hash, u_created_at))
        .get_result::<User>(&mut store.db)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    println!("Inserted user: {:?}", inserted);
    Ok(inserted)
}
