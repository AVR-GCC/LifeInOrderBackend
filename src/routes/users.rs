use chrono::Utc;
use actix_web::web;

use crate::db::schema::users::dsl::{
    created_at as u_created_at, email as u_email, id as u_id, password_hash as u_password_hash,
    users,
    wrapped_dek as u_wrapped_dek
};
use crate::utils::crypto::{create_user_wrapped_dek, get_user_dek, unwrap_dek};
use crate::utils::{
    users::{
        EmailConfirmationTemplate, auth_response, confirm_password, get_email_confirmation_cache_key, get_otp_login_cache_key, hash_token, initial_user_values
    },
    general::get_storage,
    misc_types::{
        AppState, AuthResponse, EmailOTP, UserOTP
    }
};
use crate::db::models::{LoginUser, NewUser, RefreshToken, User};
use argon2::{
    Argon2,
    password_hash::PasswordHasher,
};
use diesel::prelude::*;
use diesel::dsl::now;
use jsonwebtoken::EncodingKey;
use postmark::{
    Query,
    api::{Body, email::SendEmailRequest},
    reqwest::PostmarkClient,
};
use rand::RngExt;

use crate::db::schema::refresh_tokens::dsl::{
    created_at as rt_created_at, expires_at as rt_expires_at, family_id as rt_family_id,
    id as rt_id, refresh_tokens, revoked_at as rt_revoked_at, token_hash as rt_token_hash,
    user_id as rt_user_id,
};
use crate::utils::misc_types::{Storage, UserIdOTP};
use redis::{Commands, RedisResult};
use askama::Template;

pub async fn change_password(
    state: web::Data<AppState>,
    user_id: i32,
    old_password: String,
    new_password: String,
    family_id: String,
) -> Result<(), actix_web::Error> {
    let mut store = get_storage(state.clone()).expect("Failed to init storage");
    let hash_opt = users
        .filter(u_id.eq(user_id))
        .select(u_password_hash)
        .first::<Option<String>>(&mut store.db)
        .map_err(|e| {
            println!("Query user error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;
    let approved = match hash_opt {
        Some(hash) => {
            let password_correct = confirm_password(old_password, hash);
            match password_correct {
                Ok(x) => x,
                Err(_) => false
            }
        }
        Option::None => true
    };
    if !approved {
        return Err(actix_web::error::ErrorUnauthorized("Wrong existing password"));
    }
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(new_password.as_bytes())
        .map_err(actix_web::error::ErrorInternalServerError)?
        .to_string();
    let _ = diesel::update(users)
        .filter(u_id.eq(user_id))
        .set(u_password_hash.eq(password_hash))
        .execute(&mut store.db);
    let _ = diesel::update(refresh_tokens)
        .filter(rt_user_id.eq(user_id).and(rt_revoked_at.is_null()).and(rt_family_id.ne(family_id.clone())))
        .set(rt_revoked_at.eq(now))
        .execute(&mut store.db);
    state.socket_registry.close_user(user_id, family_id);
    Ok(())
}

pub async fn logout(
    state: web::Data<AppState>,
    refresh_token: String
) -> Result<(), actix_web::Error> {
    let mut store = get_storage(state.clone()).expect("Failed to init storage");
    let token_hash = hash_token(refresh_token.as_str());
    let rt_opt = diesel::update(refresh_tokens)
        .filter(rt_token_hash.eq(token_hash.clone()))
        .set(rt_revoked_at.eq(now))
        .get_result::<RefreshToken>(&mut store.db);
    if let Ok(rt) = rt_opt {
        state.socket_registry.close(rt.family_id);
    }
    Ok(())
}

pub async fn refresh(
    state: web::Data<AppState>,
    refresh_token: String,
) -> Result<AuthResponse, actix_web::Error> {
    let mut store = get_storage(state.clone()).expect("Failed to init storage");
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
            rt_created_at,
        ))
        .first::<RefreshToken>(&mut store.db)
        .map_err(|e| {
            println!("Query refresh token error: {:?}", e);
            actix_web::error::ErrorUnauthorized(e)
        })?;
    if stored.revoked_at.is_some() {
        let rt_opt = diesel::update(refresh_tokens)
            .filter(rt_token_hash.eq(token_hash.clone()))
            .set(rt_revoked_at.eq(now))
            .get_result::<RefreshToken>(&mut store.db);
        if let Ok(rt) = rt_opt {
            state.socket_registry.close(rt.family_id);
        }
        return Err(actix_web::error::ErrorUnauthorized("Token reuse detected"));
    }
    if stored.expires_at < Utc::now() {
        return Err(actix_web::error::ErrorUnauthorized("Token expired"));
    }
    let _ = diesel::update(refresh_tokens)
        .filter(rt_token_hash.eq(token_hash))
        .set(rt_revoked_at.eq(now))
        .execute(&mut store.db);

    let dek = get_user_dek(&mut store, stored.user_id, &state.data_encrypt_key, Option::None)
        .map_err(|e| {
            println!("Error getting user dek: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;
    auth_response(&mut store, state.encoding_key.clone(), stored.user_id, &dek, Some(stored.family_id)).await
}

pub async fn login(
    store: &mut Storage,
    encoding_key: EncodingKey,
    data_encrypt_key: &[u8; 32],
    email: String,
    password: String,
) -> Result<AuthResponse, actix_web::Error> {
    let (id, hash_opt, wrapped_dek) = users
        .filter(u_email.eq(email))
        .select((u_id, u_password_hash, u_wrapped_dek))
        .first::<(i32, Option<String>, Option<Vec<u8>>)>(&mut store.db)
        .map_err(|e| {
            println!("Query user error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;
    match hash_opt {
        Some(hash) => {
            let password_correct = confirm_password(password, hash);
            match password_correct {
                Ok(true) => {
                    let dek = get_user_dek(store, id, data_encrypt_key, wrapped_dek)
                        .map_err(|e| {
                            println!("Error getting user dek: {:?}", e);
                            actix_web::error::ErrorInternalServerError(e)
                        })?;
                    auth_response(store, encoding_key, id, &dek, Option::None).await
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
    store: &mut Storage,
    postmark_api_key: String,
    login_user_object: LoginUser,
) -> Result<(), actix_web::Error> {
    let otp = rand::rng().random_range(100_000..1_000_000);
    let client = PostmarkClient::builder().server_token(postmark_api_key).build();

    let id_res = users
        .filter(u_email.eq(&login_user_object.email))
        .select(u_id)
        .first::<i32>(&mut store.db);
    let template = match id_res {
        Ok(user_id) => {
            // Email already exists send a login otp
            let key = get_otp_login_cache_key(login_user_object.email.clone());
            let user_id_otp = UserIdOTP {
                user_id,
                otp
            };
            let _: () = store.cache.set_ex(key, user_id_otp, 600)
                .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
            EmailConfirmationTemplate { otp: &otp.to_string(), email_exists: true }
        },
        Err(diesel::result::Error::NotFound) => {
            let key = get_email_confirmation_cache_key(login_user_object.email.clone());
            let argon2 = Argon2::default();
            let password_hash = argon2
                .hash_password(login_user_object.password.as_bytes())
                .map_err(actix_web::error::ErrorInternalServerError)?
                .to_string();
            let new_user = NewUser {
                email: login_user_object.email.clone(),
                password_hash,
            };
            let user_otp = UserOTP {
                otp,
                user: new_user,
            };
            let _: () = store.cache.set_ex(key, user_otp, 600)
                .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

            EmailConfirmationTemplate { otp: &otp.to_string(), email_exists: false }
        },
        Err(_) => {
            println!("Error querying existing email");
            return Err(actix_web::error::ErrorInternalServerError("Error querying existing email"));
        }
    };
    let html_body = template.render().map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    let req = SendEmailRequest::builder()
        .from("auth@life-in-order.com")
        .to(login_user_object.email)
        .subject(format!("{otp} is your verification code"))
        .body(Body::html(html_body))
        .message_stream("outbound")
        .build();
    let _response = req.execute(&client).await.map_err(|e| {
        println!("e: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;
    Ok(())
}

pub async fn confirm_email(
    store: &mut Storage,
    encoding_key: EncodingKey,
    data_encrypt_key: &[u8; 32],
    email_otp: EmailOTP,
) -> Result<AuthResponse, actix_web::Error> {
    let new_user_key = get_email_confirmation_cache_key(email_otp.email.clone());
    let user_id_otp_key = get_otp_login_cache_key(email_otp.email.clone());
    let new_user_otp_opt: RedisResult<UserOTP> = store.cache.get(&new_user_key);
    let user_id_otp_opt: RedisResult<UserIdOTP> = store.cache.get(&user_id_otp_key);
    let (user_id, wrapped_dek) = match (new_user_otp_opt, user_id_otp_opt) {
        (Ok(new_user_otp), Err(_)) => {
            if email_otp.otp != new_user_otp.otp {
                println!("New user bad otp");
                return Err(actix_web::error::ErrorUnauthorized(
                    "Invalid email confirmation",
                ));
            }
            println!("Creating user: {:?}", new_user_otp.user);
            let inserted = diesel::insert_into(users)
                .values(&new_user_otp.user)
                .returning((u_id, u_email, u_created_at, u_password_hash, u_wrapped_dek))
                .get_result::<User>(&mut store.db)
                .map_err(actix_web::error::ErrorInternalServerError)?;

            let (dek, wrapped_dek) = create_user_wrapped_dek(data_encrypt_key)
                .map_err(actix_web::error::ErrorInternalServerError)?;

            println!("Inserted user: {:?}", inserted);
            initial_user_values(store, inserted.id, &dek).await?;
            let _ = store.cache.del::<String, usize>(new_user_key);

            let _ = diesel::update(users)
                .filter(u_id.eq(inserted.id))
                .set(u_wrapped_dek.eq(wrapped_dek.clone()))
                .execute(&mut store.db)
                .map_err(actix_web::error::ErrorInternalServerError)?;
            (inserted.id, wrapped_dek)
        },
        (Err(_), Ok(user_id_otp)) => {
            if user_id_otp.otp != email_otp.otp {
                println!("Existing user bad otp");
                return Err(actix_web::error::ErrorUnauthorized(
                    "Invalid email confirmation",
                ));
            }
            println!("Logged in user: {}", user_id_otp.user_id);
            let _ = store.cache.del::<String, usize>(user_id_otp_key);

            let wrapped_dek = users
                .filter(u_id.eq(user_id_otp.user_id))
                .select(u_wrapped_dek)
                .first::<Option<Vec<u8>>>(&mut store.db)
                .map_err(|e| {
                    println!("Query error: {:?}", e);
                    actix_web::error::ErrorInternalServerError(e)
                })?
                .unwrap();

            (user_id_otp.user_id, wrapped_dek)
        },
        (_, __) => {
            println!("Email otp not found in cache");
            return Err(actix_web::error::ErrorUnauthorized(
                "Invalid email confirmation",
            ));
        }
    };

    let dek = unwrap_dek(&data_encrypt_key, &wrapped_dek).expect("Failed to get user dek");

    auth_response(store, encoding_key, user_id, &dek, Option::None).await
}
