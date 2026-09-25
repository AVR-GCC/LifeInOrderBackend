use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use diesel::prelude::*;
use jsonwebtoken::{Header, encode, EncodingKey};
use rand::Rng;
// use sha2::{Digest, Sha256};
use crate::{
    db::models::{CreateUser, NewUser, User},
    routes::aggregates::get_extended_habits,
    utils::{
        misc_types::{AuthResponse, AuthResponseTokensSection, Claims},
    },
};

use crate::db::schema::users::dsl::{
    created_at as u_created_at, email as u_email, id as u_id, name as u_name,
    password_hash as u_password_hash, users,
};
use crate::utils::misc_types::Storage;

pub fn generate_refresh_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn confirm_password(candidate: String, hash: String) -> Result<bool, actix_web::Error> {
    let argon2 = Argon2::default();
    let parsed_hash = PasswordHash::new(&hash).map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(argon2.verify_password(candidate.as_bytes(), &parsed_hash).is_ok())
}

pub async fn auth_response(
    mut store: Storage,
    encoding_key: EncodingKey,
    sub: i32,
) -> Result<AuthResponse, actix_web::Error> {
    let habits = get_extended_habits(&mut store.db, sub)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let claims = Claims { sub };
    let access_token = encode(&Header::default(), &claims, &encoding_key)
        .expect("Failed to encode access token");
    let token_type = "Bearer".to_string();
    let expires_in = 900;
    let refresh_token = generate_refresh_token();
    let tokens = AuthResponseTokensSection {
        access_token,
        token_type,
        expires_in,
        refresh_token,
    };
    Ok(AuthResponse {
        tokens,
        user: claims,
        habits,
    })
}

pub async fn login(
    mut store: Storage,
    encoding_key: EncodingKey,
    email: String,
    password: String,
) -> Result<AuthResponse, actix_web::Error> {
    let (id, hash_opt) = users
        .filter(u_email.eq(email))
        .select((u_id, u_password_hash))
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
                    auth_response(store, encoding_key, id).await
                }
                Ok(false) => {
                    println!("User password does not match");
                    Err(actix_web::error::ErrorUnauthorized(
                        "Invalid email of password",
                    ))
                }
                Err(e) => Err(actix_web::error::ErrorInternalServerError(e)),
            }
        }
        Option::None => {
            println!("User has no password hash");
            Err(actix_web::error::ErrorInternalServerError(
                "User has no password hash",
            ))
        }
    }
}

pub async fn signup(
    mut store: Storage,
    encoding_key: EncodingKey,
    create_user_object: CreateUser,
) -> Result<AuthResponse, actix_web::Error> {
    println!("Creating user: {:?}", create_user_object);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(create_user_object.password.as_bytes())
        .map_err(actix_web::error::ErrorInternalServerError)?
        .to_string();
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
    auth_response(store, encoding_key, inserted.id).await
}
