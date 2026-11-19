use crate::HashMap;
use crate::db::models::{HabitType, NewUser};
use crate::utils::socket_registry::SocketRegistry;
use chrono::{NaiveDate, NaiveDateTime};
use core::fmt;
use diesel::prelude::*;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use jsonwebtoken::{DecodingKey, EncodingKey};
use redis::{FromRedisValue, ParsingError, RedisWrite, ToRedisArgs, ToSingleRedisArg, Value};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum HabitDayValue {
    Int(i32),
    Text(String),
}

pub type MonthYear = (u32, i32);

#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
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
            ZoomLevel::Day => "Day",
            ZoomLevel::Quarter => "Quarter",
            ZoomLevel::Half => "Half",
            ZoomLevel::Year => "Year",
            ZoomLevel::TwoYear => "TwoYear",
        };
        write!(f, "{s}")
    }
}

impl FromStr for ZoomLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Day" => Ok(ZoomLevel::Day),
            "Quarter" => Ok(ZoomLevel::Quarter),
            "Half" => Ok(ZoomLevel::Half),
            "Year" => Ok(ZoomLevel::Year),
            "TwoYear" => Ok(ZoomLevel::TwoYear),
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
        ZoomLevel::TwoYear,
    ];
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ValuesDataEntry(pub (i32, HabitType, NaiveDate, i32, Option<Vec<u8>>));
pub type ValuesDataEntryDec = (i32, HabitType, NaiveDate, i32, Option<String>);

impl ToRedisArgs for ValuesDataEntry {
    fn write_redis_args<W>(&self, out: &mut W) where W: ?Sized + RedisWrite {
        let (a, b, c, d, e) = &self.0;
        a.write_redis_args(out);
        b.write_redis_args(out);
        c.to_string().write_redis_args(out);
        d.write_redis_args(out);
        e.write_redis_args(out);
    }
}

impl FromRedisValue for ValuesDataEntry {
    fn from_redis_value(v: Value) -> Result<Self, ParsingError> {
        // If you fetch them as an array/multi-bulk response from Redis:
        let values: Vec<Value> = FromRedisValue::from_redis_value(v.clone())?;
        if values.len() < 5 {
            return Err(ParsingError::from("Malformed RedisValuesEntry in cache - bad tuple size"));
        }
        let a = i32::from_redis_value(values[0].clone())?;
        let b = HabitType::from_redis_value(values[1].clone())?;

        let date_str: String = FromRedisValue::from_redis_value(values[2].clone())?;
        let c = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .map_err(|_| ParsingError::from("Malformed RedisValuesEntry in cache - invalid date"))?;

        let d = i32::from_redis_value(values[3].clone())?;
        let e = Option::<Vec<u8>>::from_redis_value(values[4].clone())?;

        Ok(ValuesDataEntry((a, b, c, d, e)))
    }
}

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
    pub data: Vec<ValuesDataEntry>,
}

#[derive(Serialize, Debug)]
pub struct ExtendedHabit {
    pub habit: ClientHabit,
    pub values: Vec<ClientOption>,
    pub values_hashmap: HashMap<i32, i32>,
}

#[derive(Serialize, Clone, Debug)]
pub struct DayValuesStruct {
    pub date: String,
    pub values: HashMap<i32, HabitDayValue>,
}

#[derive(Serialize, Clone, Debug)]
pub struct PeriodImageStruct {
    pub range: DateRange,
    pub image: String,
    pub zoom: ZoomLevel,
}

#[derive(Serialize, Clone, Debug)]
pub struct MonthValuesStruct {
    pub range: DateRange,
    pub days: Vec<DayValuesStruct>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct UserListRequest {
    pub width: i32,
    pub date: NaiveDate,
    pub zoom: ZoomLevel,
}

pub enum ValuesOrImage {
    Values(MonthValuesStruct),
    Image(PeriodImageStruct),
}

#[derive(Serialize, Debug)]
pub struct UserListResponse {
    pub dates: Vec<DayValuesStruct>,
    pub habits: Vec<ExtendedHabit>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SequenceUpdateRequest {
    pub ordered_ids: Vec<i32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Clone)]
pub struct AppState {
    pub db_pool: Pool<ConnectionManager<PgConnection>>,
    pub redis_client: redis::Client,
    pub encoding_key: EncodingKey,
    pub decoding_key: DecodingKey,
    pub postmark_api_key: String,
    pub socket_registry: SocketRegistry,
    pub data_encrypt_key: [u8; 32],
}

pub struct Storage {
    pub db: PooledConnection<ConnectionManager<PgConnection>>,
    pub cache: redis::Connection,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PasswordChangeRequest {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenQuery {
    pub t: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "route", content = "params")]
pub enum RouteParams {
    #[serde(rename = "refresh-post")]
    RefreshPost(TokenQuery),
    #[serde(rename = "users-put")]
    UsersPut(PasswordChangeRequest),
    #[serde(rename = "values-post")]
    Values(ClientNewValue),
    #[serde(rename = "options-post")]
    OptionPost(ClientNewOption),
    #[serde(rename = "options-put")]
    OptionPut(ClientOption),
    #[serde(rename = "options-reorder-post")]
    OptionsReorder(SequenceUpdateRequest),
    #[serde(rename = "options-delete")]
    OptionDelete(i32),
    #[serde(rename = "habits-post")]
    HabitPost(ClientNewHabit),
    #[serde(rename = "habits-put")]
    HabitPut(ClientHabit),
    #[serde(rename = "habits-reorder-post")]
    HabitsReorder(SequenceUpdateRequest),
    #[serde(rename = "habits-delete")]
    HabitDelete(i32),
    #[serde(rename = "list-get")]
    ListGet(UserListRequest),
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

#[derive(Serialize)]
pub struct ErrorResponse {
    pub message: String,
}

#[derive(Serialize)]
pub struct AuthResponseTokensSection {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u32,
    pub refresh_token: String,
}

#[derive(Queryable, Deserialize, Serialize, Debug)]
pub struct FrontendUser {
    pub id: i32,
    pub email: String,
    pub created_at: NaiveDateTime,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Claims {
    pub sub: String,
    pub user: FrontendUser,
    pub exp: usize,
    pub iss: String,
    pub family_id: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub tokens: AuthResponseTokensSection,
    pub claims: Claims,
    pub habits: Vec<ExtendedHabit>,
}

#[derive(Deserialize, Serialize)]
pub struct UserOTP {
    pub otp: u32,
    pub user: NewUser,
}

#[derive(Deserialize, Serialize)]
pub struct EmailOTP {
    pub otp: u32,
    pub email: String,
}

#[derive(Deserialize, Serialize)]
pub struct UserIdOTP {
    pub otp: u32,
    pub user_id: i32,
}

impl ToRedisArgs for UserIdOTP {
    fn write_redis_args<W: ?Sized + RedisWrite>(&self, out: &mut W) {
        let str = serde_json::to_string(self)
            .expect("Convert to redis value error - malformed UserIdOTP");
        str.write_redis_args(out);
    }
}

impl FromRedisValue for UserIdOTP {
    fn from_redis_value(v: Value) -> Result<Self, ParsingError> {
        let str = String::from_redis_value(v)?;
        let res: UserIdOTP = serde_json::from_str(&str)
            .map_err(|_| ParsingError::from("Malformed UserIdOTP in cache"))?;
        Ok(res)
    }
}

impl ToRedisArgs for UserOTP {
    fn write_redis_args<W: ?Sized + RedisWrite>(&self, out: &mut W) {
        let str =
            serde_json::to_string(self).expect("Convert to redis value error - malformed UserOTP");
        str.write_redis_args(out);
    }
}

impl FromRedisValue for UserOTP {
    fn from_redis_value(v: Value) -> Result<Self, ParsingError> {
        let str = String::from_redis_value(v)?;
        let res: UserOTP = serde_json::from_str(&str)
            .map_err(|_| ParsingError::from("Malformed UserOTP in cache"))?;
        Ok(res)
    }
}

impl ToSingleRedisArg for UserOTP {}
impl ToSingleRedisArg for UserIdOTP {}


#[derive(Deserialize, Serialize, Debug)]
pub struct ClientNewHabit {
    pub name: String,
    pub weight: i32,
    pub sequence: i32,
    pub habit_type: HabitType,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ClientNewOption {
    pub label: Option<String>,
    pub sequence: i32,
    pub habit_id: i32,
    pub color: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ClientNewValue {
    pub value_id: i32,
    pub habit_id: i32,
    pub date: NaiveDate,
    pub text: Option<String>,
    pub number: Option<i32>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ClientHabit {
    pub id: i32,
    pub name: String,
    pub weight: i32,
    pub sequence: i32,
    pub habit_type: HabitType,
    pub created_at: NaiveDateTime,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ClientOption {
    pub id: i32,
    pub label: Option<String>,
    pub sequence: i32,
    pub habit_id: i32,
    pub color: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ClientValue {
    pub id: i32,
    pub value_id: i32,
    pub habit_id: i32,
    pub date: NaiveDate,
    pub text: Option<String>,
    pub number: Option<i32>,
    pub created_at: NaiveDateTime,
}
