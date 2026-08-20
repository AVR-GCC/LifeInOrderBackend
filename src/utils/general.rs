use crate::db::models::HabitType;
use crate::db::schema::day_values::dsl::{
    date as dv_date, day_values, text as dv_text, value_id as dv_value_id,
};
use crate::db::schema::habit_values::dsl::{habit_id as hv_habit_id, habit_values, id as hv_id};
use crate::db::schema::user_habits::dsl::{
    habit_type as uh_habit_type, id as uh_id, user_habits, user_id as uh_user_id,
};
use crate::utils::misc_types::{
    AppState, DateRange, DateValuesMap, DayValuesStruct, GetCacheValuesAndMissingRangesResult, HabitDayValue, MonthValuesStruct, MonthYear, NaiveDateRange, Storage, UserListResponse, ValuesDataEntry, ZoomLevel
};
use chrono::{Datelike, Duration, Months, NaiveDate};
use diesel::ExpressionMethods;
use diesel::JoinOnDsl;
use diesel::QueryDsl;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use image::{ImageBuffer, Rgb};
use redis::Commands;
use std::collections::HashMap;
use actix_web::web;

// use tokio::time::sleep;

//delay_and_return(5).await.unwrap();
// async fn delay_and_return(sec: u64) -> Result<(), ()> {
//    sleep(tokio::time::Duration::from_secs(sec)).await;
//    Ok(())
// }

pub fn get_next_date((month, year): MonthYear, zoom: ZoomLevel) -> MonthYear {
    let min_date = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let max_date = min_date
        .checked_add_months(Months::new(zoom as u32))
        .unwrap();
    (max_date.month(), max_date.year())
}

pub fn get_month_user_values_list(
    month: u32,
    year: i32,
    _user_id: i32,
    dates_map: &DateValuesMap,
) -> MonthValuesStruct {
    let min_date = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let (max_month, max_year) = get_next_date((month, year), ZoomLevel::Day);
    let max_date = NaiveDate::from_ymd_opt(max_year, max_month, 1).unwrap();
    let days = fill_dates_list(Some(min_date), Some(max_date), dates_map);
    let start = format!("{}-{:02}-01", year, month);
    let end = format!(
        "{}-{:02}-{:02}",
        max_date.year(),
        max_date.month(),
        max_date.day()
    );
    let range = DateRange { start, end };
    MonthValuesStruct { days, range }
}

pub fn fill_dates_list(
    from_date: Option<NaiveDate>,
    to_date: Option<NaiveDate>,
    dates_map: &DateValuesMap,
) -> Vec<DayValuesStruct> {
    let min_date = from_date.unwrap_or(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap());
    let max_date = to_date.unwrap_or(NaiveDate::from_ymd_opt(2030, 12, 31).unwrap());
    let mut dates: Vec<DayValuesStruct> = Vec::new();
    let mut current_date = min_date;
    while current_date < max_date {
        let date_str = current_date.to_string();
        let values = dates_map.get(&date_str).unwrap_or(&HashMap::new()).clone();
        dates.push(DayValuesStruct {
            date: date_str,
            values,
        });
        current_date = current_date + Duration::days(1);
    }
    dates
}

pub fn get_cache_key(user_id: i32, year: i32, month: u32, zoom: ZoomLevel) -> String {
    let (cache_year, cache_month) = match zoom {
        ZoomLevel::Day => (year, month),
        ZoomLevel::Quarter => {
            let cm = match month {
                m if m < 4 => 1,
                m if m < 7 => 4,
                m if m < 10 => 7,
                _ => 10
            };
            (year, cm)
        },
        ZoomLevel::Half => {
            let cm = if month < 7 { 1 } else { 7 };
            (year, cm)
        },
        ZoomLevel::Year => (year, 1),
        ZoomLevel::TwoYear => {
            let cy = if year % 2 == 0 { year } else { year - 1 };
            (cy, 1)
        }
    };
    format!("{user_id}-{zoom}-{cache_year}-{cache_month}")
}

pub async fn get_day_level_cache_data(
    cache: &mut redis::Connection,
    user_id: i32,
    year: i32,
    month: u32,
) -> Option<DateValuesMap> {
    let key = get_cache_key(user_id, year, month, ZoomLevel::Day);
    let value: Option<String> = cache.get(key).unwrap();
    value.and_then(|v| serde_json::from_str(&v).ok())
}

pub async fn get_cache_values_and_missing_ranges(
    cache: &mut redis::Connection,
    user_id: i32,
    from_date: NaiveDate,
    to_date: NaiveDate,
) -> Result<GetCacheValuesAndMissingRangesResult, actix_web::Error> {
    let mut data: DateValuesMap = HashMap::new();
    let mut ranges: Vec<NaiveDateRange> = Vec::new();
    let to_month = to_date.month();
    let to_year = to_date.year();
    let mut current_month = from_date.month();
    let mut current_year = from_date.year();
    let mut range_start: Option<NaiveDate> =
        NaiveDate::from_ymd_opt(current_year, current_month, 1);
    while current_month < to_month || current_year < to_year {
        let value_opt: Option<DateValuesMap> =
            get_day_level_cache_data(cache, user_id, current_year, current_month).await;
        if let Some(cache_value) = value_opt {
            data.extend(cache_value);
            if let Some(start) = range_start {
                let month = start.month();
                let year = start.year();
                if current_year != year || current_month != month {
                    let end = NaiveDate::from_ymd_opt(current_year, current_month, 1).unwrap();
                    let range = NaiveDateRange { start, end };
                    ranges.push(range);
                }
                range_start = None;
            }
        } else {
            if matches!(range_start, None) {
                range_start = NaiveDate::from_ymd_opt(current_year, current_month, 1);
            }
        }
        (current_month, current_year) =
            get_next_date((current_month, current_year), ZoomLevel::Day);
    }
    if let Some(start) = range_start {
        let end = NaiveDate::from_ymd_opt(current_year, current_month, 1).unwrap();
        let range = NaiveDateRange { start, end };
        ranges.push(range);
    }
    let result = GetCacheValuesAndMissingRangesResult { data, ranges };
    Ok(result)
}

pub async fn get_user_values_data(
    conn: &mut PgConnection,
    user_id: i32,
    from_date: NaiveDate,
    to_date: NaiveDate,
) -> Result<Vec<ValuesDataEntry>, actix_web::Error> {
    let value_data: Vec<ValuesDataEntry> = user_habits
        .inner_join(habit_values.on(hv_habit_id.eq(uh_id)))
        .inner_join(day_values.on(dv_value_id.eq(hv_id)))
        .filter(dv_date.ge(from_date))
        .filter(dv_date.lt(to_date))
        .filter(uh_user_id.eq(user_id))
        .select((uh_id, uh_habit_type, dv_date, dv_value_id, dv_text))
        .order(dv_date.asc())
        .load::<ValuesDataEntry>(conn)
        .map_err(|e| {
            println!("Query error: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;
    Ok(value_data)
}

pub fn values_data_into_map(
    user_id: i32,
    value_data: Vec<ValuesDataEntry>,
) -> HashMap<String, DateValuesMap> {
    let mut dates_maps_map: HashMap<String, DateValuesMap> = HashMap::new();

    for (habit_id, habit_type, date, value_id, text) in value_data {
        // Dates: date -> habit_id -> HabitDayValue
        let month = date.month();
        let year = date.year();
        let date_str = date.to_string();
        let cache_key = get_cache_key(user_id, year, month, ZoomLevel::Day);
        let value = if habit_type == HabitType::Text {
            HabitDayValue::Text(text.unwrap_or_default())
        } else {
            HabitDayValue::Int(value_id)
        };
        dates_maps_map
            .entry(cache_key)
            .or_insert_with(&HashMap::new)
            .entry(date_str)
            .or_insert_with(HashMap::new)
            .insert(habit_id, value);
    }

    dates_maps_map
}

pub async fn get_user_values_dates_map(
    cache: &mut redis::Connection,
    conn: &mut PgConnection,
    user_id: i32,
    from_date_opt: Option<NaiveDate>,
    to_date_opt: Option<NaiveDate>,
) -> Result<DateValuesMap, actix_web::Error> {
    let from_date = from_date_opt.unwrap_or(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap());
    let to_date = to_date_opt.unwrap_or(NaiveDate::from_ymd_opt(2030, 1, 1).unwrap());
    let res = get_cache_values_and_missing_ranges(cache, user_id, from_date, to_date).await?;
    // dbg!(&res.ranges);
    let mut dates_map: DateValuesMap = res.data;
    for range in &res.ranges {
        let value_data: Vec<ValuesDataEntry> =
            get_user_values_data(conn, user_id, range.start, range.end).await?;
        // Build response
        let dates_map_map: HashMap<String, DateValuesMap> =
            values_data_into_map(user_id, value_data);
        for (key, map) in dates_map_map {
            let json = serde_json::to_string(&map)
                .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
            let _: () = cache
                .set(key, json)
                .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
            dates_map.extend(map);
        }
    }

    Ok(dates_map)
}

pub fn create_period_image(
    data: UserListResponse,
    total_width: i32,
    row_height: i32,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Parse hex color to RGB
    fn parse_color(color_str: &Option<String>) -> Rgb<u8> {
        match color_str {
            Some(hex) if hex.len() >= 6 => {
                let hex = hex.trim_start_matches('#');
                if hex.len() >= 6 {
                    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(200);
                    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(200);
                    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(200);
                    Rgb([r, g, b])
                } else {
                    Rgb([200, 200, 200]) // Light gray for no data
                }
            }
            _ => Rgb([85, 85, 85]), // Default gray
        }
    }

    // Find date range
    let dates: Vec<NaiveDate> = data
        .dates
        .iter()
        .filter_map(|d| d.date.parse().ok())
        .collect();

    if dates.is_empty() {
        return Err("No valid dates found".into());
    }

    let image_height = (data.dates.len() as i32) * row_height;
    let mut img = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(total_width as u32, image_height as u32);

    // Calculate total weight for proportional width calculation
    let total_weight: i32 = data.habits.iter().map(|h| h.habit.weight).sum();

    if total_weight == 0 {
        return Ok(Vec::new());
    }

    // For each date row
    for (row_idx, date_values) in data.dates.iter().enumerate() {
        let mut x_offset = 0;
        let mut remainder: f64 = 0.0;

        // For each habit (sorted by sequence)
        for habit in &data.habits {
            let exact_width =
                (total_width as f64 * habit.habit.weight as f64) / total_weight as f64;
            let mut habit_width = exact_width as i32;
            remainder += exact_width - habit_width as f64;
            if remainder >= 1.0 {
                habit_width += 1;
                remainder -= 1.0;
            }

            // Get the value for this habit on this date
            let day_value = date_values.values.get(&habit.habit.id);

            // Find the corresponding habit value and its color
            let color = match day_value {
                Some(HabitDayValue::Int(val_id)) => habit
                    .values
                    .iter()
                    .find(|v| v.id == *val_id)
                    .map(|v| &v.color)
                    .unwrap_or(&None),
                Some(HabitDayValue::Text(_)) => &None,
                None => &None,
            };

            let rgb_color = parse_color(color);

            // Fill the rectangle for this habit efficiently using direct buffer manipulation
            let y_start = row_idx as i32 * row_height;
            let y_end = ((row_idx as i32 + 1) * row_height).min(image_height);
            let x_start = x_offset;
            let x_end = (x_offset + habit_width).min(total_width);

            if x_start < x_end && y_start < y_end {
                let buffer = img.as_mut();
                let width = total_width as usize;

                // Fill each row of the rectangle
                for y in y_start..y_end {
                    let row_offset = (y as usize * width + x_start as usize) * 3;
                    let pixels_to_fill = (x_end - x_start) as usize * 3;

                    if row_offset + pixels_to_fill <= buffer.len() {
                        // Fill the entire row segment at once using chunks
                        for chunk in
                            buffer[row_offset..row_offset + pixels_to_fill].chunks_exact_mut(3)
                        {
                            chunk[0] = rgb_color.0[0]; // R
                            chunk[1] = rgb_color.0[1]; // G
                            chunk[2] = rgb_color.0[2]; // B
                        }
                    }
                }
            }
            x_offset += habit_width;
        }
    }

    // Encode as WebP
    let mut webp_data = Vec::new();
    let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut webp_data);
    encoder.encode(
        &img,
        img.width(),
        img.height(),
        image::ColorType::Rgb8.into(),
    )?;

    Ok(webp_data)
}

pub fn get_storage(state: web::Data<AppState>) -> Result<Storage, actix_web::Error> {
    let db = state.db_pool.get().map_err(|e| {
        println!("Pool error: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;
    let cache = state
        .redis_client
        .get_connection()
        .expect("Failed to get cache connection");
    Ok(Storage { db, cache })
}
