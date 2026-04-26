use crate::HashMap;
use crate::db::models::{HabitValue, UserHabit};
use core::fmt;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum HabitDayValue {
    Int(i32),
    Text(String),
}

pub type MonthYear = (u32, i32);

#[derive(Clone, Copy)]
pub enum ZoomLevel {
    Day = 1,
    Quarter = 3,
    Half = 6,
    Year = 12,
    TwoYear = 24,
}

impl fmt::Display for ZoomLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ZoomLevel::Day => "day",
            ZoomLevel::Quarter => "quarter",
            ZoomLevel::Half => "half",
            ZoomLevel::Year => "year",
            ZoomLevel::TwoYear => "two_year",
        };
        write!(f, "{s}")
    }
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
    pub values: HashMap<i32, HabitDayValue>,
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
