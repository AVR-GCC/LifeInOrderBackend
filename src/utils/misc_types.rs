use crate::HashMap;
use crate::db::models::{HabitValue, UserHabit};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub enum ZoomLevel {
    Day,
    Quarter,
    Half,
    Year,
    TwoYear,
}

impl FromStr for ZoomLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "day" => Ok(ZoomLevel::Day),
            "quarter" => Ok(ZoomLevel::Quarter),
            "half" => Ok(ZoomLevel::Half),
            "year" => Ok(ZoomLevel::Year),
            "two_year" => Ok(ZoomLevel::TwoYear),
            _ => Err(format!("{s} is not a valid zoom value")),
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct DateRange {
    pub start: String,
    pub end: String,
}

#[derive(Serialize, Debug)]
pub struct ExtendedUserHabit {
    pub habit: UserHabit,
    pub values: Vec<HabitValue>,
    pub values_hashmap: HashMap<i32, i32>,
}

#[derive(Serialize, Clone, Debug)]
pub struct DayValuesStruct {
    pub date: String,
    pub values: HashMap<i32, i32>,
}

#[derive(Serialize, Clone, Debug)]
pub struct MonthValuesStruct {
    pub range: DateRange,
    pub days: Vec<DayValuesStruct>,
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
