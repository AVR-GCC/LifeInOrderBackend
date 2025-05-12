use crate::db::models::{UserHabit, HabitValue};
use serde::{Serialize, Deserialize};
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

#[derive(Deserialize, Serialize)]
pub struct SequenceUpdateRequest {
    pub ordered_ids: Vec<i32>,
}
