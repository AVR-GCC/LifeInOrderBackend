use crate::db::models::{UserHabit, HabitValue};
use serde::Serialize;
use crate::HashMap;

#[derive(Serialize)]
pub struct ExtendedUserHabit {
    pub habit: UserHabit,
    pub values: Vec<HabitValue>,
    pub values_hashmap: HashMap<i32, i32>,
}

#[derive(Serialize)]
pub struct DayValuesStruct {
    pub date: String,
    pub values: HashMap<i32, i32>
}

#[derive(Serialize)]
pub struct UserListResponse {
    pub dates: Vec<DayValuesStruct>,
    pub habits: Vec<ExtendedUserHabit>,
}
