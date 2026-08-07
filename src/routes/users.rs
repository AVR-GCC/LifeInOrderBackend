use diesel::prelude::*;

use crate::db::models::{NewUser, User};
use crate::db::schema::users::dsl::{
    created_at as u_created_at, email as u_email, id as u_id, name as u_name, users,
};
use crate::utils::misc_types::Storage;

pub fn create_user(
    mut store: Storage,
    new_user: NewUser,
) -> Result<User, actix_web::Error> {
    println!("Creating user: {:?}", new_user);
    let inserted = diesel::insert_into(users)
        .values(&new_user)
        .returning((u_id, u_name, u_email, u_created_at))
        .get_result::<User>(&mut store.db)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    println!("Inserted user: {:?}", inserted);
    Ok(inserted)
}
