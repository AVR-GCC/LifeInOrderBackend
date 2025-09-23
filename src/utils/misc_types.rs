use crate::db::models::{UserHabit, HabitValue};
use serde::{Serialize, Deserialize};
use crate::HashMap;

#[derive(Serialize, Debug)]
pub struct ExtendedUserHabit {
    pub habit: UserHabit,
    pub values: Vec<HabitValue>,
    pub values_hashmap: HashMap<i32, i32>,
}

#[derive(Serialize, Clone, Debug)]
pub struct DayValuesStruct {
    pub date: String,
    pub values: HashMap<i32, i32>
}

#[derive(Serialize, Clone, Debug)]
pub struct MonthValuesStruct {
    pub date: String,
    pub days: Vec<DayValuesStruct>
}

#[derive(Serialize, Debug)]
pub struct UserListResponse {
    pub dates: Vec<DayValuesStruct>,
    pub habits: Vec<ExtendedUserHabit>,
}

#[derive(Deserialize, Serialize)]
pub struct SequenceUpdateRequest {
    pub ordered_ids: Vec<i32>,
}
