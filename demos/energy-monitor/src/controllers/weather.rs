// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::ui::*;
use chrono::prelude::*;
use slint::{Image, SharedString, VecModel, Weak};

use std::thread;

const WEATHER_LAT_KEY: &str = "WEATHER_LAT";
const WEATHER_LONG_KEY: &str = "WEATHER_LONG";
const LAT_BERLIN: f32 = 52.520008;
const LONG_BERLIN: f32 = 13.404954;
const FORECAST_DAYS: usize = 3;

pub fn setup(window: &MainWindow) -> thread::JoinHandle<()> {
    let window_weak = window.as_weak();

    thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(weather_worker_loop(window_weak))
    })
}

// --- Open-Meteo API response types ---

#[derive(serde::Deserialize)]
struct ForecastResponse {
    current: CurrentResponse,
    daily: DailyResponse,
}

#[derive(serde::Deserialize)]
struct CurrentResponse {
    temperature_2m: f64,
    weather_code: i32,
}

#[derive(serde::Deserialize)]
struct DailyResponse {
    time: Vec<String>,
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
    weather_code: Vec<i32>,
}

fn wmo_description(code: i32) -> &'static str {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51..=55 => "Drizzle",
        56 | 57 => "Freezing drizzle",
        61 => "Slight rain",
        63 => "Moderate rain",
        65 => "Heavy rain",
        66 | 67 => "Freezing rain",
        71..=75 => "Snowfall",
        77 => "Snow grains",
        80..=82 => "Rain showers",
        85 | 86 => "Snow showers",
        95..=99 => "Thunderstorm",
        _ => "Unknown",
    }
}

async fn weather_worker_loop(window_weak: Weak<MainWindow>) {
    let lat = lat();
    let long = long();

    let url = format!(
        "https://api.open-meteo.com/v1/forecast\
         ?latitude={lat}&longitude={long}\
         &current=temperature_2m,weather_code\
         &daily=temperature_2m_max,temperature_2m_min,weather_code\
         &timezone=auto&forecast_days={FORECAST_DAYS}"
    );

    let resp = match reqwest::get(&url).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to fetch weather: {e}");
            return;
        }
    };

    let data: ForecastResponse = match resp.json().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to parse weather response: {e}");
            return;
        }
    };

    let now = Local::now();
    let current_date = SharedString::from(now.format("%e %B %Y").to_string());

    // Display current weather
    {
        let current_temp =
            SharedString::from(format!("{}°", data.current.temperature_2m.round() as i32));
        let description = SharedString::from(wmo_description(data.current.weather_code));
        let weather_code = data.current.weather_code;

        window_weak
            .upgrade_in_event_loop(move |window| {
                window.global::<WeatherAdapter>().set_current_temperature(current_temp);
                window.global::<WeatherAdapter>().set_current_day(current_date);
                window.global::<WeatherAdapter>().set_current_weather_description(description);
                window
                    .global::<WeatherAdapter>()
                    .set_current_temperature_icon(get_icon(&window, weather_code));
            })
            .unwrap();
    }

    // Display forecast
    let daily = &data.daily;
    if daily.time.is_empty() {
        return;
    }

    let max_temp = daily.temperature_2m_max.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_temp = daily.temperature_2m_min.iter().cloned().fold(f64::INFINITY, f64::min);

    let mut forecast_items: Vec<(i32, i32, i32, String)> = Vec::new();
    for (i, date_str) in daily.time.iter().enumerate() {
        let day_name = if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            date.format("%A").to_string()
        } else {
            date_str.clone()
        };
        forecast_items.push((
            daily.temperature_2m_max[i].round() as i32,
            daily.temperature_2m_min[i].round() as i32,
            daily.weather_code[i],
            day_name,
        ));
    }

    let absolute_max = max_temp.round() as i32;
    let absolute_min = min_temp.round() as i32;

    window_weak
        .upgrade_in_event_loop(move |window| {
            let forecast_model = VecModel::default();

            for (max, min, weather_code, day) in &forecast_items {
                let model = BarTileModel {
                    title: SharedString::from(&day.as_str()[0..3]),
                    max: *max,
                    min: *min,
                    absolute_max,
                    absolute_min,
                    unit: SharedString::from("°"),
                    icon: get_icon(&window, *weather_code),
                };
                forecast_model.push(model);
            }

            window
                .global::<WeatherAdapter>()
                .set_week_model(std::rc::Rc::new(forecast_model).into());
        })
        .unwrap();
}

fn get_icon(window: &MainWindow, wmo_code: i32) -> Image {
    match wmo_code {
        2 => window.global::<Images>().get_cloudy(),
        3..=99 => window.global::<Images>().get_cloud(),
        _ => window.global::<Images>().get_sunny(),
    }
}

fn lat() -> f32 {
    if let Some(lat) = option_env!("WEATHER_LAT") {
        return lat.parse().unwrap_or_default();
    }

    #[cfg(not(feature = "mcu-board-support"))]
    if let Some(lat) = std::env::var_os(WEATHER_LAT_KEY)
        && let Some(lat) = lat.to_str()
    {
        return lat.parse().unwrap_or_default();
    }

    LAT_BERLIN
}

fn long() -> f32 {
    if let Some(lat) = option_env!("WEATHER_LONG") {
        return lat.parse().unwrap_or_default();
    }

    #[cfg(not(feature = "mcu-board-support"))]
    if let Some(lat) = std::env::var_os(WEATHER_LONG_KEY)
        && let Some(lat) = lat.to_str()
    {
        return lat.parse().unwrap_or_default();
    }

    LONG_BERLIN
}
