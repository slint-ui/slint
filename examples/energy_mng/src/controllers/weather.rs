// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::ui::*;
use chrono::prelude::*;
use core::cmp::Ordering;
use slint::*;
use std::rc::Rc;
use weer_api::*;

const WEATHER_API_KEY: &str = "WEATHER_API";
const LAT_BERLIN: f32 = 52.520008;
const LONG_BERLIN: f32 = 13.404954;
const FORECAST_DAYS: i64 = 5;

pub fn setup(window: &MainWindow) {
    if let Some(api_key) = std::env::var_os(WEATHER_API_KEY) {
        if let Some(api_key) = api_key.to_str() {
            let now = Local::now();
            let mut day_counter = 1;
            let mut forecast_days = vec![];

            let client = Client::new(api_key, true);

            loop {
                let forecast_date = now + chrono::Duration::days(day_counter);

                if let Ok(forecast) = client
                    .forecast()
                    .query(Query::Coords(LAT_BERLIN, LONG_BERLIN))
                    .dt(forecast_date)
                    .call()
                {
                    if day_counter == 1 {
                        display_current(
                            &window,
                            &forecast.current,
                            SharedString::from(now.format("%e %B %Y").to_string()),
                        );
                    }

                    {
                        let forecast = forecast.forecast;
                        let mut day = forecast.forecast_day;

                        // the api provides only one day in the forecast therefore an iteration is necessary to get all.
                        if !day.is_empty() {
                            forecast_days
                                .push((day.remove(0), forecast_date.format("%A").to_string()));
                        }
                    }
                }

                day_counter += 1;

                if day_counter > FORECAST_DAYS {
                    break;
                }
            }

            display_forecast(&window, &forecast_days);
        }
    }
}

fn display_current(window: &MainWindow, current: &Current, current_date: SharedString) {
    window
        .global::<WeatherAdapter>()
        .set_current_temperature(SharedString::from(current.temp_c.to_string()));
    window.global::<WeatherAdapter>().set_current_day(current_date);
    window
        .global::<WeatherAdapter>()
        .set_current_weather_description(SharedString::from(current.condition.text.to_string()));
    window
        .global::<WeatherAdapter>()
        .set_current_temperature_icon(get_icon(window, &current.condition));
}

fn display_forecast(window: &MainWindow, forecast: &Vec<(ForecastDay, String)>) {
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
            title: SharedString::from(day),
            max: forecast_day.day.temp_c().max().round() as i32,
            min: forecast_day.day.temp_c().min().round() as i32,
            absolute_max: max_temp.round() as i32,
            absolute_min: min_temp.round() as i32,
            unit: SharedString::from("°"),
            icon: get_icon(window, &forecast_day.day.condition),
            ..Default::default()
        };

        forecast_model.push(model);
    }

    window.global::<WeatherAdapter>().set_week_model(Rc::new(forecast_model).into());
}

fn get_icon(window: &MainWindow, condition: &Condition) -> Image {
    // code mapping can be found at https://www.weatherapi.com/docs/conditions.json
    match condition.code {
        1003 => window.global::<Images>().get_cloudy(),
        1006 => window.global::<Images>().get_cloud(),
        _ => window.global::<Images>().get_sunny(),
    }
}
