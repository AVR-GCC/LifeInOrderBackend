use image::{ImageBuffer, Rgb};
use chrono::Duration;
use std::collections::HashMap;
use chrono::NaiveDate;
use crate::utils::misc_types::UserListResponse;

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
                    Rgb([200, 200, 200]) // Default gray
                }
            }
            _ => Rgb([128, 128, 128]), // Light gray for no data
        }
    }

    // Create a map of dates for quick lookup
    let mut date_values: HashMap<String, &HashMap<i32, i32>> = HashMap::new();
    for day_values_item in &data.dates {
        date_values.insert(day_values_item.date.clone(), &day_values_item.values);
    }

    // Find date range
    let dates: Vec<NaiveDate> = data.dates
        .iter()
        .filter_map(|d| d.date.parse().ok())
        .collect();
    
    if dates.is_empty() {
        return Err("No valid dates found".into());
    }

    let min_date = *dates.iter().min().unwrap();
    let max_date = *dates.iter().max().unwrap();
    
    // Generate all dates in range
    let mut all_dates = Vec::new();
    let mut current_date = min_date;
    while current_date <= max_date {
        all_dates.push(current_date);
        current_date = current_date + Duration::days(1);
    }

    let image_height = (all_dates.len() as i32) * row_height;
    let mut img = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(total_width as u32, image_height as u32);

    // Calculate total weight for proportional width calculation
    let total_weight: i32 = data.habits.iter().map(|h| h.habit.weight).sum();
    
    if total_weight == 0 {
        return Err("Total habit weight is zero".into());
    }

    // For each date row
    for (row_idx, date) in all_dates.iter().enumerate() {
        let date_str = date.to_string();
        let day_values_for_date = date_values.get(&date_str);

        let mut x_offset = 0;

        // For each habit (sorted by sequence)
        for habit in &data.habits {
            let habit_width = (total_width * habit.habit.weight) / total_weight;

            // Get the value for this habit on this date
            let value_id = day_values_for_date.and_then(|values| values.get(&habit.habit.id));

            // Find the corresponding habit value and its color
            let color = if let Some(&val_id) = value_id {
                habit.values
                    .iter()
                    .find(|v| v.id == val_id)
                    .map(|v| &v.color)
                    .unwrap_or(&None)
            } else {
                &None
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
                        for chunk in buffer[row_offset..row_offset + pixels_to_fill].chunks_exact_mut(3) {
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
    encoder.encode(&img, img.width(), img.height(), image::ColorType::Rgb8.into())?;
    
    Ok(webp_data)
}
