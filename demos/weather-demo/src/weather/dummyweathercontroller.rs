// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use chrono::{Duration, Utc};

use crate::weather::utils::*;
use crate::weather::weathercontroller::{
    CityData, CityWeatherData, GeoLocationData, WeatherController,
};

pub struct DummyWeatherController {
    city_weather_data: Vec<CityWeatherData>,
}

impl DummyWeatherController {
    pub fn new() -> Self {
        Self { city_weather_data: vec![] }
    }

    fn generate_dummy_data() -> Vec<CityWeatherData> {
        let json_data = std::include_str!("./dummyweather.json");

        match serde_json::from_str::<Vec<CityWeatherData>>(json_data) {
            Ok(weather_data) => {
                // fix day names
                let mut weather_data = weather_data.clone();
                for city_data in &mut weather_data {
                    let forecast_data = &mut (city_data.weather_data.forecast_data);
                    for (index, data) in forecast_data.iter_mut().enumerate() {
                        if index == 0 {
                            data.day_name = "Today".into();
                        } else {
                            data.day_name =
                                get_day_from_datetime(Utc::now() + Duration::days(index as i64));
                        }
                    }
                }

                return weather_data;
            }
            Err(e) => {
                log::warn!("Cannot read dummy weather data! Error: {e}");
            }
        }

        vec![]
    }
}

impl WeatherController for DummyWeatherController {
    fn load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.city_weather_data = Self::generate_dummy_data();
        Ok(())
    }

    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn refresh_cities(&mut self) -> Result<Vec<CityWeatherData>, Box<dyn std::error::Error>> {
        Ok(self.city_weather_data.clone())
    }

    fn add_city(
        &mut self,
        _city: CityData,
    ) -> Result<Option<CityWeatherData>, Box<dyn std::error::Error>> {
        // not supported for the dummy data
        unimplemented!();
    }

    fn reorder_cities(
        &mut self,
        index: usize,
        new_index: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.city_weather_data.swap(index, new_index);
        Ok(())
    }

    fn remove_city(&mut self, index: usize) -> Result<(), Box<dyn std::error::Error>> {
        self.city_weather_data.remove(index);
        Ok(())
    }

    fn search_location(
        &self,
        _query: String,
    ) -> Result<Vec<GeoLocationData>, Box<dyn std::error::Error>> {
        // not supported for the dummy data
        unimplemented!();
    }
}
