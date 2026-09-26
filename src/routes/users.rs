use chrono::{Utc, TimeDelta};

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use diesel::prelude::*;
use diesel::dsl::now;
use jsonwebtoken::{Header, encode, EncodingKey};
use rand::Rng;
use sha2::{Digest, Sha256};
use crate::{
    db::models::{CreateUser, NewRefreshToken, NewUser, RefreshToken, User},
    routes::aggregates::get_extended_habits,
    utils::misc_types::{AuthResponse, AuthResponseTokensSection, Claims},
};
use crate::db::schema::users::dsl::{
    created_at as u_created_at, email as u_email, id as u_id, name as u_name,
    password_hash as u_password_hash, users,
};

use crate::db::schema::refresh_tokens::dsl::{
    refresh_tokens, id as rt_id, user_id as rt_user_id, token_hash as rt_token_hash, family_id as rt_family_id,
    expires_at as rt_expires_at, revoked_at as rt_revoked_at, created_at as rt_created_at
};
use crate::utils::misc_types::Storage;

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:?}", hasher.finalize())
}

pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub async fn auth_response(
    mut store: Storage,
    encoding_key: EncodingKey,
    sub: i32,
    family_id: Option<String>
) -> Result<AuthResponse, actix_web::Error> {
    // claims
    let exp = (Utc::now() + TimeDelta::minutes(10)).timestamp() as usize;
    let iss = "lifeinorder".to_string();
    let claims = Claims { sub, exp, iss };

    // tokens
    let access_token = encode(&Header::default(), &claims, &encoding_key)
        .expect("Failed to encode access token");
    let token_type = "Bearer".to_string();
    let expires_in = 900;
    let refresh_token = generate_token();
    let token_hash = hash_token(&refresh_token);
    let expires_at = Utc::now() + TimeDelta::days(30);
    let tokens = AuthResponseTokensSection {
        access_token,
        token_type,
        expires_in,
        refresh_token,
    };

    // save refresh token
    let new_refresh_token = NewRefreshToken {
        user_id: sub,
        token_hash,
        family_id: family_id.unwrap_or(generate_token()),
        expires_at,
        revoked_at: Option::None
    };
    diesel::insert_into(refresh_tokens)
        .values(&new_refresh_token)
        .execute(&mut store.db)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // data
    let habits = get_extended_habits(&mut store.db, sub)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(AuthResponse {
        tokens,
        user: claims,
        habits,
    })
}

pub async fn refresh(
    mut store: Storage,
    encoding_key: EncodingKey,
    refresh_token: String
) -> Result<AuthResponse, actix_web::Error> {
    let token_hash = hash_token(refresh_token.as_str());
    let stored = refresh_tokens
        .filter(rt_token_hash.eq(token_hash.clone()))
        .select((
            rt_id,
            rt_user_id,
            rt_token_hash,
            rt_family_id,
            rt_expires_at,
            rt_revoked_at,
            rt_created_at
        ))
        .first::<RefreshToken>(&mut store.db)
        .map_err(|e| {
            println!("Query refresh token error: {:?}", e);
            actix_web::error::ErrorUnauthorized(e)
        })?;
    if matches!(stored.revoked_at, Some(_)) {
        let _ = diesel::update(refresh_tokens)
            .filter(rt_family_id.eq(stored.family_id))
            .set(rt_revoked_at.eq(now))
            .execute(&mut store.db);
        return Err(actix_web::error::ErrorUnauthorized("Token reuse detected"));
    }
    if stored.expires_at < Utc::now() {
        return Err(actix_web::error::ErrorUnauthorized("Token expired"));
    }
    let _ = diesel::update(refresh_tokens)
        .filter(rt_token_hash.eq(token_hash))
        .set(rt_revoked_at.eq(now))
        .execute(&mut store.db);
    auth_response(store, encoding_key, stored.user_id, Some(stored.family_id)).await
}

fn confirm_password(candidate: String, hash: String) -> Result<bool, actix_web::Error> {
    let argon2 = Argon2::default();
    let parsed_hash = PasswordHash::new(&hash).map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(argon2.verify_password(candidate.as_bytes(), &parsed_hash).is_ok())
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
                    auth_response(store, encoding_key, id, Option::None).await
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
    auth_response(store, encoding_key, inserted.id, Option::None).await
}
