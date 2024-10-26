// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CityData {
    pub lat: f64,
    pub lon: f64,
    pub city_name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub enum WeatherCondition {
    #[default]
    Unknown,
    Sunny,
    PartiallyCloudy,
    MostlyCloudy,
    Cloudy,
    SunnyRainy,
    Rainy,
    Stormy,
    Snowy,
    Foggy,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct TemperatureData {
    pub min: f64,
    pub max: f64,
    pub morning: f64,
    pub day: f64,
    pub evening: f64,
    pub night: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct PrecipitationData {
    pub probability: f64,
    pub rain_volume: f64,
    pub snow_volume: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct DayWeatherData {
    pub condition: WeatherCondition,
    pub description: String,

    pub current_temperature: f64,
    pub detailed_temperature: TemperatureData,

    pub precipitation: PrecipitationData,
    pub uv_index: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct ForecastWeatherData {
    pub day_name: String,
    pub weather_data: DayWeatherData,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WeatherData {
    pub current_data: DayWeatherData,
    pub forecast_data: Vec<ForecastWeatherData>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CityWeatherData {
    pub city_data: CityData,
    pub weather_data: WeatherData,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GeoLocationData {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub country: String,
    pub state: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
pub type WeatherControllerPointer = Box<dyn WeatherController + Send>;
#[cfg(target_arch = "wasm32")]
pub type WeatherControllerPointer = Box<dyn WeatherController + Send + 'static>;

pub type WeatherControllerSharedPointer = Arc<Mutex<WeatherControllerPointer>>;

pub trait WeatherController {
    fn load(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    fn save(&self) -> Result<(), Box<dyn std::error::Error>>;

    fn refresh_cities(&mut self) -> Result<Vec<CityWeatherData>, Box<dyn std::error::Error>>;

    fn add_city(
        &mut self,
        city: CityData,
    ) -> Result<Option<CityWeatherData>, Box<dyn std::error::Error>>;

    fn reorder_cities(
        &mut self,
        index: usize,
        new_index: usize,
    ) -> Result<(), Box<dyn std::error::Error>>;

    fn remove_city(&mut self, index: usize) -> Result<(), Box<dyn std::error::Error>>;

    fn search_location(
        &self,
        query: String,
    ) -> Result<Vec<GeoLocationData>, Box<dyn std::error::Error>>;
}
