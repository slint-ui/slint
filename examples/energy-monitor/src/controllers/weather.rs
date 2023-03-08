// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::ui::*;
use chrono::prelude::*;
use core::cmp::Ordering;
use futures::future;
use slint::*;
use std::rc::Rc;
use weer_api::*;

use std::thread;

const WEATHER_API_KEY: &str = "WEATHER_API";
const WEATHER_LAT_KEY: &str = "WEATHER_LAT";
const WEATHER_LONG_KEY: &str = "WEATHER_LONG";
const LAT_BERLIN: f32 = 52.520008;
const LONG_BERLIN: f32 = 13.404954;
const FORECAST_DAYS: i64 = 3;

pub fn setup(window: &MainWindow) -> thread::JoinHandle<()> {
    let window_weak = window.as_weak();

    thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(weather_worker_loop(window_weak))
    })
}

async fn weather_worker_loop(window_weak: Weak<MainWindow>) {
    let api_key = api_key();
    if api_key.is_empty() {
        return;
    }

    // place holders
    window_weak
        .upgrade_in_event_loop(|window| {
            window.global::<WeatherAdapter>().set_week_model(
                VecModel::from_slice(&vec![BarTileModel::default(); FORECAST_DAYS as usize]).into(),
            );
        })
        .unwrap();

    let lat = lat();
    let long = long();

    let now = Local::now();

    let mut forecast_days = vec![];

    let client = Client::new(&api_key, true);
    let mut forecast_list =
        future::join_all((0..FORECAST_DAYS).map(|i| {
            let client = client.clone();
            async move {
                current_forecast(client.clone(), lat, long, now + chrono::Duration::days(i)).await
            }
        }))
        .await;

    for i in 0..forecast_list.len() {
        if let Some((date, forecast)) = forecast_list.remove(0) {
            if i == 1 {
                display_current(
                    window_weak.clone(),
                    forecast.current,
                    SharedString::from(now.format("%e %B %Y").to_string()),
                );
            }

            {
                let forecast = forecast.forecast;
                let mut day = forecast.forecast_day;

                // the api provides only one day in the forecast therefore an iteration is necessary to get all.
                if !day.is_empty() {
                    forecast_days.push((day.remove(0), date.format("%A").to_string()));
                }
            }
        }
    }

    display_forecast(window_weak.clone(), forecast_days);
}

async fn current_forecast(
    client: Client,
    lat: f32,
    long: f32,
    date: DateTime<Local>,
) -> Option<(DateTime<Local>, Forecast)> {
    if let Ok(forecast) = client.forecast().query(Query::Coords(lat, long)).dt(date).call() {
        return Some((date, forecast));
    }

    None
}

fn display_current(window_weak: Weak<MainWindow>, current: Current, current_date: SharedString) {
    window_weak
        .upgrade_in_event_loop(move |window| {
            window
                .global::<WeatherAdapter>()
                .set_current_temperature(SharedString::from(current.temp_c.to_string()));
            window.global::<WeatherAdapter>().set_current_day(current_date);
            window.global::<WeatherAdapter>().set_current_weather_description(SharedString::from(
                current.condition.text.to_string(),
            ));
            window
                .global::<WeatherAdapter>()
                .set_current_temperature_icon(get_icon(&window, &current.condition));
        })
        .unwrap();
}

fn display_forecast(window_weak: Weak<MainWindow>, forecast: Vec<(ForecastDay, String)>) {
    window_weak
        .upgrade_in_event_loop(move |window| {
            let forecast_model = VecModel::default();

            let max_temp = forecast
                .iter()
                .max_by(|lhs, rhs| {
                    if lhs.0.day.temp_c().max() > rhs.0.day.temp_c().max() {
                        Ordering::Greater
                    } else {
                        Ordering::Less
                    }
                })
                .map(|d| d.0.day.temp_c().max())
                .unwrap_or_default();

            let min_temp = forecast
                .iter()
                .min_by(|lhs, rhs| {
                    if lhs.0.day.temp_c().min() > rhs.0.day.temp_c().min() {
                        Ordering::Greater
                    } else {
                        Ordering::Less
                    }
                })
                .map(|d| d.0.day.temp_c().min())
                .unwrap_or_default();

            for (forecast_day, day) in forecast {
                let model = BarTileModel {
                    title: SharedString::from(&day.as_str()[0..3]),
                    max: forecast_day.day.temp_c().max().round() as i32,
                    min: forecast_day.day.temp_c().min().round() as i32,
                    absolute_max: max_temp.round() as i32,
                    absolute_min: min_temp.round() as i32,
                    unit: SharedString::from("°"),
                    icon: get_icon(&window, &forecast_day.day.condition),
                    ..Default::default()
                };

                forecast_model.push(model);
            }

            window.global::<WeatherAdapter>().set_week_model(Rc::new(forecast_model).into());
        })
        .unwrap();
}

fn get_icon(window: &MainWindow, condition: &Condition) -> Image {
    // code mapping can be found at https://www.weatherapi.com/docs/conditions.json
    match condition.code {
        1003 => window.global::<Images>().get_cloudy(),
        1006 => window.global::<Images>().get_cloud(),
        _ => window.global::<Images>().get_sunny(),
    }
}

fn api_key() -> String {
    if let Some(lat) = option_env!("WEATHER_API") {
        return lat.to_string();
    }

    #[cfg(not(feature = "mcu-board-support"))]
    if let Some(lat) = std::env::var_os(WEATHER_API_KEY) {
        if let Some(lat) = lat.to_str() {
            return lat.to_string();
        }
    }

    String::default()
}

fn lat() -> f32 {
    if let Some(lat) = option_env!("WEATHER_LAT") {
        return lat.parse().unwrap_or_default();
    }

    #[cfg(not(feature = "mcu-board-support"))]
    if let Some(lat) = std::env::var_os(WEATHER_LAT_KEY) {
        if let Some(lat) = lat.to_str() {
            return lat.parse().unwrap_or_default();
        }
    }

    LAT_BERLIN
}

fn long() -> f32 {
    if let Some(lat) = option_env!("WEATHER_LONG") {
        return lat.parse().unwrap_or_default();
    }

    #[cfg(not(feature = "mcu-board-support"))]
    if let Some(lat) = std::env::var_os(WEATHER_LONG_KEY) {
        if let Some(lat) = lat.to_str() {
            return lat.parse().unwrap_or_default();
        }
    }

    LONG_BERLIN
}
