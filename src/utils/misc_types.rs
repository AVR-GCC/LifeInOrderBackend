use crate::HashMap;
use crate::db::models::{Habit, HabitType, NewValue, VOption};
use chrono::NaiveDate;
use core::fmt;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use redis::{FromRedisValue, ParsingError, RedisWrite, ToRedisArgs, Value};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum HabitDayValue {
    Int(i32),
    Text(String),
}

impl ToRedisArgs for HabitDayValue {
    fn write_redis_args<W: ?Sized + RedisWrite>(&self, out: &mut W) {
        let str = match self {
            HabitDayValue::Int(n) => format!("i:{n}"),
            HabitDayValue::Text(s) => format!("t:{s}"),
        };
        str.write_redis_args(out);
    }
}

impl FromRedisValue for HabitDayValue {
    fn from_redis_value(v: Value) -> Result<Self, ParsingError> {
        let str = String::from_redis_value(v)?;

        if let Some(rest) = str.strip_prefix("i:") {
            let n = rest
                .parse::<i32>()
                .map_err(|_| ParsingError::from("Invalid int payload"))?;
            Ok(HabitDayValue::Int(n))
        } else if let Some(rest) = str.strip_prefix("t:") {
            Ok(HabitDayValue::Text(rest.to_string()))
        } else {
            Err(ParsingError::from("Missing type prefix 'i:' or 't:'"))
        }
    }
}

pub type MonthYear = (u32, i32);

#[derive(Clone, Copy, Debug)]
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

impl ZoomLevel {
    pub const ALL: [ZoomLevel; 5] = [
        ZoomLevel::Day,
        ZoomLevel::Quarter,
        ZoomLevel::Half,
        ZoomLevel::Year,
        ZoomLevel::TwoYear
    ];
}

pub type ValuesDataEntry = (i32, HabitType, NaiveDate, i32, Option<String>);

pub type DateValuesMap = HashMap<String, HashMap<i32, HabitDayValue>>;

#[derive(Serialize, Clone, Debug)]
pub struct DateRange {
    pub start: String,
    pub end: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaiveDateRange {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GetCacheValuesAndMissingRangesResult {
    pub ranges: Vec<NaiveDateRange>,
    pub data: DateValuesMap,
}

#[derive(Serialize, Debug)]
pub struct ExtendedHabit {
    pub habit: Habit,
    pub values: Vec<VOption>,
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
    pub habits: Vec<ExtendedHabit>,
}

#[derive(Deserialize, Serialize)]
pub struct SequenceUpdateRequest {
    pub ordered_ids: Vec<i32>,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub db_pool: Pool<ConnectionManager<PgConnection>>,
    pub redis_client: redis::Client,
}

pub struct Storage {
    pub db: PooledConnection<ConnectionManager<PgConnection>>,
    pub cache: redis::Connection,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "route", content = "params")]
pub enum RouteParams {
    #[serde(rename = "values")]
    Values(NewValue),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SocketRequest {
    pub id: String,
    #[serde(flatten)]
    pub action: RouteParams,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SocketResponse<T> {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

// impl<T> SocketResponse<T> {
//     /// Helper to construct a successful response
//     pub fn success(id: impl Into<String>, data: T) -> Self {
//         Self {
//             id: id.into(),
//             error: None,
//             data: Some(data),
//         }
//     }
//
//     /// Helper to construct an error response
//     pub fn error(id: impl Into<String>, error: impl Into<String>) -> Self {
//         Self {
//             id: id.into(),
//             error: Some(error.into()),
//             data: None,
//         }
//     }
// }
