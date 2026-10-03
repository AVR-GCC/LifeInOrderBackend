use chrono::{NaiveDate, Local, Utc, TimeDelta, Days};

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use diesel::prelude::*;
use diesel::dsl::now;
use jsonwebtoken::{Header, encode, decode, EncodingKey, DecodingKey, Validation};
use rand::Rng;
use sha2::{Digest, Sha256};
use crate::{
    db::models::{CreateUser, HabitType, NewHabit, NewRefreshToken, NewUser, NewVOption, NewValue, RefreshToken, User},
    routes::{aggregates::get_extended_habits, habits::create_habit, options::create_option, values::set_value},
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

pub fn verify_token(token: &str, decoding_key: &DecodingKey) -> Result<Claims, jsonwebtoken::errors::Error> {
    let data = decode::<Claims>(
        token,
        decoding_key,
        &Validation::default(),
    )?;
    Ok(data.claims)
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:?}", hasher.finalize())
}

pub async fn logout(
    mut store: Storage,
    refresh_token: String
) -> Result<(), actix_web::Error> {
    let token_hash = hash_token(refresh_token.as_str());
    let _ = diesel::update(refresh_tokens)
        .filter(rt_token_hash.eq(token_hash.clone()))
        .set(rt_revoked_at.eq(now))
        .execute(&mut store.db);
    Ok(())
}

pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub async fn auth_response(
    store: &mut Storage,
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
    store: &mut Storage,
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
    store: &mut Storage,
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

async fn initial_user_values(
    store: &mut Storage,
    user_id: i32
) -> Result<(), actix_web::Error> {
    let general_new_habit = NewHabit {
        user_id,
        name: "General".to_string(),
        weight: 1,
        sequence: 1,
        habit_type: HabitType::Text
    };
    let food_new_habit = NewHabit {
        user_id,
        name: "Food".to_string(),
        weight: 1,
        sequence: 2,
        habit_type: HabitType::Text
    };
    let alcohol_new_habit = NewHabit {
        user_id,
        name: "Alcohol".to_string(),
        weight: 1,
        sequence: 3,
        habit_type: HabitType::Color
    };
    let tabacco_new_habit = NewHabit {
        user_id,
        name: "Tabacco".to_string(),
        weight: 3,
        sequence: 4,
        habit_type: HabitType::Color
    };
    let workout_new_habit = NewHabit {
        user_id,
        name: "Workout".to_string(),
        weight: 2,
        sequence: 5,
        habit_type: HabitType::Color
    };
    let location_new_habit = NewHabit {
        user_id,
        name: "Location".to_string(),
        weight: 2,
        sequence: 6,
        habit_type: HabitType::Color
    };
    let general_habit = create_habit(store, general_new_habit).expect("Failed to create habit");
    let general_new_option = NewVOption {
        habit_id: general_habit.id,
        label: None,
        sequence: 1,
        color: None
    };
    let food_habit = create_habit(store, food_new_habit).expect("Failed to create habit");
    let food_new_option = NewVOption {
        habit_id: food_habit.id,
        label: None,
        sequence: 1,
        color: None
    };
    let alcohol_habit = create_habit(store, alcohol_new_habit).expect("Failed to create habit");
    let alcohol_new_good_option = NewVOption {
        habit_id: alcohol_habit.id,
        label: Some("Dry".to_string()),
        sequence: 1,
        color: Some("#10b981".to_string())
    };
    let alcohol_new_bad_option = NewVOption {
        habit_id: alcohol_habit.id,
        label: Some("Drank".to_string()),
        sequence: 2,
        color: Some("#ef4444".to_string())
    };
    let tabacco_habit = create_habit(store, tabacco_new_habit).expect("Failed to create habit");
    let tabacco_new_good_option = NewVOption {
        habit_id: tabacco_habit.id,
        label: Some("None".to_string()),
        sequence: 1,
        color: Some("#10b981".to_string())
    };
    let tabacco_new_mid_option = NewVOption {
        habit_id: tabacco_habit.id,
        label: Some("Up to 5 cigs".to_string()),
        sequence: 2,
        color: Some("#eeee00".to_string())
    };
    let tabacco_new_mid_bad_option = NewVOption {
        habit_id: tabacco_habit.id,
        label: Some("Made one good choice".to_string()),
        sequence: 3,
        color: Some("#f97316".to_string())
    };
    let tabacco_new_bad_option = NewVOption {
        habit_id: tabacco_habit.id,
        label: Some("Plenty".to_string()),
        sequence: 4,
        color: Some("#ef4444".to_string())
    };
    let workout_habit = create_habit(store, workout_new_habit).expect("Failed to create habit");
    let workout_new_good_option = NewVOption {
        habit_id: workout_habit.id,
        label: Some("Full".to_string()),
        sequence: 1,
        color: Some("#10b981".to_string())
    };
    let workout_new_mid_option = NewVOption {
        habit_id: workout_habit.id,
        label: Some("Broke a sweat".to_string()),
        sequence: 2,
        color: Some("#eeee00".to_string())
    };
    let workout_new_bad_option = NewVOption {
        habit_id: workout_habit.id,
        label: Some("Nothing".to_string()),
        sequence: 3,
        color: Some("#ef4444".to_string())
    };
    let location_habit = create_habit(store, location_new_habit).expect("Failed to create habit");
    let location_new_new_york_option = NewVOption {
        habit_id: location_habit.id,
        label: Some("New York".to_string()),
        sequence: 1,
        color: Some("#0e65e9".to_string())
    };
    let location_new_athens_option = NewVOption {
        habit_id: location_habit.id,
        label: Some("Athens".to_string()),
        sequence: 2,
        color: Some("#08a1f2".to_string())
    };
    let location_new_istanbul_option = NewVOption {
        habit_id: location_habit.id,
        label: Some("Istanbul".to_string()),
        sequence: 3,
        color: Some("#10b981".to_string())
    };
    let location_new_travel_option = NewVOption {
        habit_id: location_habit.id,
        label: Some("Travel".to_string()),
        sequence: 4,
        color: Some("#eeee00".to_string())
    };
    let general_option = create_option(store, general_new_option).expect("Failed to create option");
    let food_option = create_option(store, food_new_option).expect("Failed to create option");

    let alcohol_good_option = create_option(store, alcohol_new_good_option).expect("Failed to create option");
    let alcohol_bad_option = create_option(store, alcohol_new_bad_option).expect("Failed to create option");

    let tabacco_good_option = create_option(store, tabacco_new_good_option).expect("Failed to create option");
    let tabacco_mid_option = create_option(store, tabacco_new_mid_option).expect("Failed to create option");
    let tabacco_mid_bad_option = create_option(store, tabacco_new_mid_bad_option).expect("Failed to create option");
    let tabacco_bad_option = create_option(store, tabacco_new_bad_option).expect("Failed to create option");

    let workout_good_option = create_option(store, workout_new_good_option).expect("Failed to create option");
    let workout_mid_option = create_option(store, workout_new_mid_option).expect("Failed to create option");
    let workout_bad_option = create_option(store, workout_new_bad_option).expect("Failed to create option");

    let location_new_york_option = create_option(store, location_new_new_york_option).expect("Failed to create option");
    let location_istanbul_option = create_option(store, location_new_istanbul_option).expect("Failed to create option");
    let location_athens_option = create_option(store, location_new_athens_option).expect("Failed to create option");
    let location_travel_option = create_option(store, location_new_travel_option).expect("Failed to create option");

    let today: NaiveDate = Local::now().date_naive();

    // Day 1
    let day1_general_new_val = NewValue {
        value_id: general_option.id,
        habit_id: general_habit.id,
        date: today.checked_sub_days(Days::new(1)).expect("Failed to derive date"),
        text: Some("New York. Recovery day. Slept in, unpacked, grocery-shopped and meal-prepped. Started getting back into the normal routine.".to_string()),
        number: None
    };
    let day1_food_new_val = NewValue {
        value_id: food_option.id,
        habit_id: food_habit.id,
        date: today.checked_sub_days(Days::new(1)).expect("Failed to derive date"),
        text: Some("Breakfast: oatmeal, banana and peanut butter. Lunch: turkey sandwich and apple. Dinner: salmon, rice and roasted vegetables. Snack: Greek yogurt and berries.".to_string()),
        number: None
    };
    let day1_alcohol_new_val = NewValue {
        value_id: alcohol_good_option.id,
        habit_id: alcohol_habit.id,
        date: today.checked_sub_days(Days::new(1)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day1_tabacco_new_val = NewValue {
        value_id: tabacco_good_option.id,
        habit_id: tabacco_habit.id,
        date: today.checked_sub_days(Days::new(1)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day1_workout_new_val = NewValue {
        value_id: workout_mid_option.id,
        habit_id: workout_habit.id,
        date: today.checked_sub_days(Days::new(1)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day1_location_new_val = NewValue {
        value_id: location_new_york_option.id,
        habit_id: location_habit.id,
        date: today.checked_sub_days(Days::new(1)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let _ = set_value(store, day1_general_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day1_food_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day1_alcohol_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day1_tabacco_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day1_workout_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day1_location_new_val, user_id).expect("Failed to update value");

    // Day 2
    let day2_general_new_val = NewValue {
        value_id: general_option.id,
        habit_id: general_habit.id,
        date: today.checked_sub_days(Days::new(2)).expect("Failed to derive date"),
        text: Some("Slept late, spent the final day by the coast, drinking and smoking, then flew home overnight.".to_string()),
        number: None
    };
    let day2_food_new_val = NewValue {
        value_id: food_option.id,
        habit_id: food_habit.id,
        date: today.checked_sub_days(Days::new(2)).expect("Failed to derive date"),
        text: Some("Breakfast: eggs, feta, tomatoes and bread. Lunch: pork souvlaki, pita and tzatziki. Airport: turkey sandwich.".to_string()),
        number: None
    };
    let day2_alcohol_new_val = NewValue {
        value_id: alcohol_bad_option.id,
        habit_id: alcohol_habit.id,
        date: today.checked_sub_days(Days::new(2)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day2_tabacco_new_val = NewValue {
        value_id: tabacco_bad_option.id,
        habit_id: tabacco_habit.id,
        date: today.checked_sub_days(Days::new(2)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day2_workout_new_val = NewValue {
        value_id: workout_bad_option.id,
        habit_id: workout_habit.id,
        date: today.checked_sub_days(Days::new(2)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day2_location_new_val = NewValue {
        value_id: location_travel_option.id,
        habit_id: location_habit.id,
        date: today.checked_sub_days(Days::new(2)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let _ = set_value(store, day2_general_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day2_food_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day2_alcohol_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day2_tabacco_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day2_workout_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day2_location_new_val, user_id).expect("Failed to update value");

    // Day 3
    let day3_general_new_val = NewValue {
        value_id: general_option.id,
        habit_id: general_habit.id,
        date: today.checked_sub_days(Days::new(3)).expect("Failed to derive date"),
        text: Some("Athens. Acropolis and central Athens during the day. Long walking day, followed by a relaxed evening near Monastiraki.".to_string()),
        number: None
    };
    let day3_food_new_val = NewValue {
        value_id: food_option.id,
        habit_id: food_habit.id,
        date: today.checked_sub_days(Days::new(3)).expect("Failed to derive date"),
        text: Some("Breakfast: Greek yogurt, honey and fruit. Lunch: Greek salad, feta and bread. Snack: iced coffee and pastry. Dinner: grilled fish, potatoes and vegetables. Dessert: gelato.".to_string()),
        number: None
    };
    let day3_alcohol_new_val = NewValue {
        value_id: alcohol_bad_option.id,
        habit_id: alcohol_habit.id,
        date: today.checked_sub_days(Days::new(3)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day3_tabacco_new_val = NewValue {
        value_id: tabacco_mid_option.id,
        habit_id: tabacco_habit.id,
        date: today.checked_sub_days(Days::new(3)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day3_workout_new_val = NewValue {
        value_id: workout_mid_option.id,
        habit_id: workout_habit.id,
        date: today.checked_sub_days(Days::new(3)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day3_location_new_val = NewValue {
        value_id: location_athens_option.id,
        habit_id: location_habit.id,
        date: today.checked_sub_days(Days::new(3)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let _ = set_value(store, day3_general_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day3_food_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day3_alcohol_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day3_tabacco_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day3_workout_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day3_location_new_val, user_id).expect("Failed to update value");

    // Day 4
    let day4_general_new_val = NewValue {
        value_id: general_option.id,
        habit_id: general_habit.id,
        date: today.checked_sub_days(Days::new(4)).expect("Failed to derive date"),
        text: Some("Slow morning after a late night, then flight to Athens. Walked around Plaka and had dinner with drinks.".to_string()),
        number: None
    };
    let day4_food_new_val = NewValue {
        value_id: food_option.id,
        habit_id: food_habit.id,
        date: today.checked_sub_days(Days::new(4)).expect("Failed to derive date"),
        text: Some("Breakfast: yogurt, honey, walnuts and fruit. Airport: chicken sandwich. Dinner: chicken souvlaki, pita, tzatziki and salad. Dessert: baklava.".to_string()),
        number: None
    };
    let day4_alcohol_new_val = NewValue {
        value_id: alcohol_bad_option.id,
        habit_id: alcohol_habit.id,
        date: today.checked_sub_days(Days::new(4)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day4_tabacco_new_val = NewValue {
        value_id: tabacco_bad_option.id,
        habit_id: tabacco_habit.id,
        date: today.checked_sub_days(Days::new(4)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day4_workout_new_val = NewValue {
        value_id: workout_bad_option.id,
        habit_id: workout_habit.id,
        date: today.checked_sub_days(Days::new(4)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day4_location_new_val = NewValue {
        value_id: location_athens_option.id,
        habit_id: location_habit.id,
        date: today.checked_sub_days(Days::new(4)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let _ = set_value(store, day4_general_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day4_food_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day4_alcohol_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day4_tabacco_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day4_workout_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day4_location_new_val, user_id).expect("Failed to update value");

    // Day 5
    let day5_general_new_val = NewValue {
        value_id: general_option.id,
        habit_id: general_habit.id,
        date: today.checked_sub_days(Days::new(5)).expect("Failed to derive date"),
        text: Some("Full sightseeing day: Sultanahmet, Grand Bazaar and Bosphorus. Very active day followed by a social dinner.".to_string()),
        number: None
    };
    let day5_food_new_val = NewValue {
        value_id: food_option.id,
        habit_id: food_habit.id,
        date: today.checked_sub_days(Days::new(5)).expect("Failed to derive date"),
        text: Some("Breakfast: menemen, bread, olives and tea. Lunch: lamb kebab, bulgur and salad. Snack: Turkish delight. Dinner: grilled sea bass, salad and bread. Dessert: künefe.".to_string()),
        number: None
    };
    let day5_alcohol_new_val = NewValue {
        value_id: alcohol_bad_option.id,
        habit_id: alcohol_habit.id,
        date: today.checked_sub_days(Days::new(5)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day5_tabacco_new_val = NewValue {
        value_id: tabacco_mid_option.id,
        habit_id: tabacco_habit.id,
        date: today.checked_sub_days(Days::new(5)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day5_workout_new_val = NewValue {
        value_id: workout_mid_option.id,
        habit_id: workout_habit.id,
        date: today.checked_sub_days(Days::new(5)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day5_location_new_val = NewValue {
        value_id: location_istanbul_option.id,
        habit_id: location_habit.id,
        date: today.checked_sub_days(Days::new(5)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let _ = set_value(store, day5_general_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day5_food_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day5_alcohol_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day5_tabacco_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day5_workout_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day5_location_new_val, user_id).expect("Failed to update value");

    // Day 6
    let day6_general_new_val = NewValue {
        value_id: general_option.id,
        habit_id: general_habit.id,
        date: today.checked_sub_days(Days::new(6)).expect("Failed to derive date"),
        text: Some("Worked part of the morning, flew to Istanbul, checked into the hotel and had a late dinner.".to_string()),
        number: None
    };
    let day6_food_new_val = NewValue {
        value_id: food_option.id,
        habit_id: food_habit.id,
        date: today.checked_sub_days(Days::new(6)).expect("Failed to derive date"),
        text: Some("Breakfast: oatmeal, banana and coffee. Airport: turkey sandwich and chips. Dinner: chicken döner, rice and salad. Snack: baklava.".to_string()),
        number: None
    };
    let day6_alcohol_new_val = NewValue {
        value_id: alcohol_good_option.id,
        habit_id: alcohol_habit.id,
        date: today.checked_sub_days(Days::new(6)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day6_tabacco_new_val = NewValue {
        value_id: tabacco_mid_bad_option.id,
        habit_id: tabacco_habit.id,
        date: today.checked_sub_days(Days::new(6)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day6_workout_new_val = NewValue {
        value_id: workout_bad_option.id,
        habit_id: workout_habit.id,
        date: today.checked_sub_days(Days::new(6)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day6_location_new_val = NewValue {
        value_id: location_istanbul_option.id,
        habit_id: location_habit.id,
        date: today.checked_sub_days(Days::new(6)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let _ = set_value(store, day6_general_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day6_food_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day6_alcohol_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day6_tabacco_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day6_workout_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day6_location_new_val, user_id).expect("Failed to update value");

    // Day 7
    let day7_general_new_val = NewValue {
        value_id: general_option.id,
        habit_id: general_habit.id,
        date: today.checked_sub_days(Days::new(7)).expect("Failed to derive date"),
        text: Some("Normal workday at home. Cooked dinner and had a quiet evening.".to_string()),
        number: None
    };
    let day7_food_new_val = NewValue {
        value_id: food_option.id,
        habit_id: food_habit.id,
        date: today.checked_sub_days(Days::new(7)).expect("Failed to derive date"),
        text: Some("Breakfast: eggs, toast, coffee. Lunch: turkey sandwich, apple. Dinner: chicken, rice and broccoli. Snack: yogurt.".to_string()),
        number: None
    };
    let day7_alcohol_new_val = NewValue {
        value_id: alcohol_good_option.id,
        habit_id: alcohol_habit.id,
        date: today.checked_sub_days(Days::new(7)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day7_tabacco_new_val = NewValue {
        value_id: tabacco_mid_option.id,
        habit_id: tabacco_habit.id,
        date: today.checked_sub_days(Days::new(7)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day7_workout_new_val = NewValue {
        value_id: workout_good_option.id,
        habit_id: workout_habit.id,
        date: today.checked_sub_days(Days::new(7)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let day7_location_new_val = NewValue {
        value_id: location_new_york_option.id,
        habit_id: location_habit.id,
        date: today.checked_sub_days(Days::new(7)).expect("Failed to derive date"),
        text: None,
        number: None
    };
    let _ = set_value(store, day7_general_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day7_food_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day7_alcohol_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day7_tabacco_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day7_workout_new_val, user_id).expect("Failed to update value");
    let _ = set_value(store, day7_location_new_val, user_id).expect("Failed to update value");
    Ok(())
}

pub async fn signup(
    store: &mut Storage,
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

    initial_user_values(store, inserted.id).await?;

    auth_response(store, encoding_key, inserted.id, Option::None).await
}
