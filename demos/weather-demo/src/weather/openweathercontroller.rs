// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![cfg(all(not(target_arch = "wasm32"), feature = "open_weather"))]

use chrono::DateTime;
use openweather_sdk::responses::{GeocodingResponse, OneCallResponse};
use openweather_sdk::{Language, OpenWeather, Units};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io;
use std::io::{BufReader, BufWriter, Write};
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;
use std::vec;
use tokio::sync::Mutex;

use crate::weather::utils::*;
use crate::weather::weathercontroller::{
    CityData, CityWeatherData, DayWeatherData, ForecastWeatherData, GeoLocationData,
    PrecipitationData, TemperatureData, WeatherCondition, WeatherController, WeatherData,
};

#[cfg(target_os = "android")]
use std::env;

const CITIES_STORED_FILE_NAME: &str = "cities_data.json";
const ORGANIZATION_QUALIFIER: &str = "dev"; // have to match android app name in cargo.toml
const ORGANIZATION_NAME: &str = "slint.examples"; // have to match android app name in cargo.toml
const APPLICATION_NAME: &str = "weatherdemo"; // have to match app android name in cargo.toml

fn project_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    {
        match env::var("ANDROID_DATA") {
            Ok(data_root) => {
                if data_root.is_empty() {
                    return None;
                } else {
                    let project_name = format!(
                        "{}.{}.{}",
                        ORGANIZATION_QUALIFIER, ORGANIZATION_NAME, APPLICATION_NAME
                    );
                    return Some(PathBuf::from(format!(
                        "{}/data/{}/files",
                        data_root, project_name
                    )));
                }
            }
            Err(_e) => {
                log::warn!("Cannot read ANDROID_DATA, persistence not available.");
                return None;
            }
        }
    }

    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    {
        if let Some(project_dir) = directories::ProjectDirs::from(
            ORGANIZATION_QUALIFIER,
            ORGANIZATION_NAME,
            APPLICATION_NAME,
        ) {
            return Some(project_dir.data_dir().to_path_buf());
        };

        None
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WeatherClient {
    pub city_data: CityData,
    pub weather_data: Option<OneCallResponse>,
}

pub struct OpenWeatherController {
    tokio_runtime: tokio::runtime::Runtime,
    weather_api: OpenWeather,
    city_clients: Arc<Mutex<Vec<WeatherClient>>>,
    storage_path: Option<PathBuf>,
}

impl OpenWeatherController {
    pub fn new(api_key: String) -> Self {
        let mut weather_api = OpenWeather::new(api_key, Units::Metric, Language::English);
        weather_api.one_call.fields.minutely = false;
        weather_api.one_call.fields.hourly = false;
        weather_api.one_call.fields.alerts = false;

        let storage_path;
        if let Some(project_dir) = project_data_dir() {
            storage_path = Some(project_dir.as_path().join(CITIES_STORED_FILE_NAME));
        } else {
            storage_path = None;
            log::error!("Failed to initialize project dir. Persistent data will not be loaded");
        }

        Self {
            tokio_runtime: tokio::runtime::Runtime::new().unwrap(),
            weather_api,
            city_clients: Arc::new(Mutex::new(vec![])),
            storage_path,
        }
    }

    fn weather_condition_from_icon_icon_type(icon_type: &str) -> WeatherCondition {
        match icon_type {
            "01d" | "01n" => WeatherCondition::Sunny,
            "02d" | "02n" => WeatherCondition::PartiallyCloudy,
            "03d" | "03n" => WeatherCondition::MostlyCloudy,
            "04d" | "04n" => WeatherCondition::Cloudy,
            "10d" | "10n" => WeatherCondition::SunnyRainy,
            "09d" | "09n" => WeatherCondition::Rainy,
            "11d" | "11n" => WeatherCondition::Stormy,
            "13d" | "13n" => WeatherCondition::Snowy,
            "50d" | "50n" => WeatherCondition::Foggy,
            _ => WeatherCondition::Unknown,
        }
    }

    fn current_day_weather_data_from_response(
        weather_response: &Option<OneCallResponse>,
    ) -> DayWeatherData {
        if let Some(weather_data) = weather_response {
            if let Some(current) = &weather_data.current {
                let weather_details = &current.weather[0];
                let today_weather_info =
                    weather_data.daily.as_ref().and_then(|daily| daily.first());

                let detailed_temp = match today_weather_info {
                    Some(info) => {
                        let temp = info.temp;
                        TemperatureData {
                            min: temp.min,
                            max: temp.max,

                            morning: temp.morn,
                            day: temp.day,
                            evening: temp.eve,
                            night: temp.night,
                        }
                    }
                    None => TemperatureData {
                        min: current.temp,
                        max: current.temp,

                        morning: current.temp,
                        day: current.temp,
                        evening: current.temp,
                        night: current.temp,
                    },
                };

                return DayWeatherData {
                    description: weather_details.description.clone(),
                    condition: Self::weather_condition_from_icon_icon_type(&weather_details.icon),
                    current_temperature: current.temp,
                    detailed_temperature: detailed_temp,
                    precipitation: PrecipitationData::default(),
                    uv_index: 0.0,
                };
            }
        }

        DayWeatherData::default()
    }

    fn forecast_day_weather_data_from_response(
        weather_response: &Option<OneCallResponse>,
    ) -> Vec<ForecastWeatherData> {
        let mut forecast_weather_info: Vec<ForecastWeatherData> = vec![];

        if let Some(weather_data) = weather_response {
            if let Some(daily_weather_data) = &weather_data.daily {
                for day_weather_data in daily_weather_data.iter() {
                    if let Some(datetime) = DateTime::from_timestamp(day_weather_data.datetime, 0) {
                        let weather_details = &day_weather_data.weather[0];

                        let detailed_temperature = TemperatureData {
                            min: day_weather_data.temp.min,
                            max: day_weather_data.temp.max,

                            morning: day_weather_data.temp.morn,
                            day: day_weather_data.temp.day,
                            evening: day_weather_data.temp.eve,
                            night: day_weather_data.temp.night,
                        };

                        let precipitation: PrecipitationData = PrecipitationData {
                            probability: day_weather_data.pop,
                            rain_volume: day_weather_data.rain.unwrap_or(0 as f64),
                            snow_volume: day_weather_data.snow.unwrap_or(0 as f64),
                        };

                        let day_weather_info = DayWeatherData {
                            description: weather_details.description.clone(),
                            condition: Self::weather_condition_from_icon_icon_type(
                                &weather_details.icon,
                            ),
                            current_temperature: day_weather_data.temp.day,
                            detailed_temperature,
                            precipitation,
                            uv_index: day_weather_data.uvi,
                        };

                        // TODO: localization
                        forecast_weather_info.push(ForecastWeatherData {
                            day_name: get_day_from_datetime(datetime),
                            weather_data: day_weather_info,
                        });
                    }
                }
            }
        }

        forecast_weather_info
    }

    fn city_weather_data_from_client(city_client: &WeatherClient) -> CityWeatherData {
        let current_data = Self::current_day_weather_data_from_response(&city_client.weather_data);
        let forecast_data =
            Self::forecast_day_weather_data_from_response(&city_client.weather_data);

        CityWeatherData {
            city_data: city_client.city_data.clone(),
            weather_data: WeatherData { current_data, forecast_data },
        }
    }

    fn geo_location_data_from_response(response: &GeocodingResponse) -> GeoLocationData {
        GeoLocationData {
            name: response.name.clone(),
            state: response.state.clone(),
            country: response.country.clone(),
            lat: response.lat,
            lon: response.lon,
        }
    }
}

impl WeatherController for OpenWeatherController {
    fn load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(storage_path) = &self.storage_path {
            log::debug!("Loading data from: {:?}", storage_path.to_str());

            let file = File::open(storage_path.as_path())?;
            let reader = BufReader::new(file);

            let city_clients_data: Vec<WeatherClient> = serde_json::from_reader(reader)?;
            log::debug!("Successfully loaded {} cities", city_clients_data.len());

            let city_clients = self.city_clients.clone();
            self.tokio_runtime.block_on(async move {
                let mut city_clients = city_clients.lock().await;
                *city_clients = city_clients_data;
                Ok(())
            })
        } else {
            Err(Box::new(io::Error::new(io::ErrorKind::NotFound, "Storage path not initialized")))
        }
    }

    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(storage_path) = &self.storage_path {
            log::debug!("Saving data to: {:?}", storage_path.display());

            // Ensure the parent directories exist
            if let Some(parent_dir) = storage_path.parent() {
                std::fs::create_dir_all(parent_dir)?;
            }

            let file = File::create(storage_path)?;
            let mut writer = BufWriter::new(file);
            let city_clients = self.city_clients.clone();

            self.tokio_runtime.block_on(async move {
                let city_clients = city_clients.lock().await;
                serde_json::to_writer(&mut writer, city_clients.deref())?;
                writer.flush()?;

                Ok(())
            })
        } else {
            Err(Box::new(io::Error::new(io::ErrorKind::NotFound, "Storage path not initialized")))
        }
    }

    fn refresh_cities(&mut self) -> Result<Vec<CityWeatherData>, Box<dyn std::error::Error>> {
        log::debug!("Refreshing all the clients!");

        let city_clients_clone = self.city_clients.clone();
        let weather_api = self.weather_api.clone();

        self.tokio_runtime.block_on(async move {
            let mut city_clients = city_clients_clone.lock().await;

            let mut errors = vec![];
            for client in city_clients.iter_mut() {
                // TODO: Spawn all tasks at once and join them later.
                if let Err(e) = client.refresh_weather(&weather_api).await {
                    errors.push(e);
                }
            }
            log::debug!("Refreshing weather finished!");

            if !errors.is_empty() && errors.len() == city_clients.len() {
                return Err(errors.pop().unwrap());
            }
            Ok(city_clients.iter().map(Self::city_weather_data_from_client).collect())
        })
    }

    fn add_city(
        &mut self,
        city: CityData,
    ) -> Result<Option<CityWeatherData>, Box<dyn std::error::Error>> {
        log::debug!("Adding new city: {city:?}");
        let city_clients_clone = self.city_clients.clone();
        let weather_api = self.weather_api.clone();

        self.tokio_runtime.block_on(async move {
            let mut city_clients = city_clients_clone.lock().await;
            match city_clients.iter().position(|client| client.city_data == city) {
                Some(_) => {
                    log::info!("City already present in list!");
                    Ok(None)
                }
                None => {
                    // Add to list and refresh
                    let mut client = WeatherClient::new(city.lat, city.lon, &city.city_name);
                    client.refresh_weather(&weather_api).await?;
                    let city_weather_data = Self::city_weather_data_from_client(&client);
                    city_clients.push(client);
                    Ok(Some(city_weather_data))
                }
            }
        })
    }

    fn reorder_cities(
        &mut self,
        index: usize,
        new_index: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let city_clients = self.city_clients.clone();
        self.tokio_runtime.block_on(async move {
            let mut city_clients = city_clients.lock().await;
            city_clients.swap(index, new_index);
        });
        Ok(())
    }

    fn remove_city(&mut self, index: usize) -> Result<(), Box<dyn std::error::Error>> {
        let city_clients = self.city_clients.clone();
        self.tokio_runtime.block_on(async move {
            let mut city_clients = city_clients.lock().await;
            city_clients.remove(index);
        });
        Ok(())
    }

    fn search_location(
        &self,
        query: String,
    ) -> Result<Vec<GeoLocationData>, Box<dyn std::error::Error>> {
        log::debug!("Searching for: {query}");
        let weather_api = self.weather_api.clone();

        if query.is_empty() {
            return Ok(vec![]);
        }

        self.tokio_runtime.block_on(async move {
            let response_data = weather_api.geocoding.get_geocoding(&query, None, None, 0).await?;

            log::debug!("Search result: {response_data:?}");

            let mut unique_response_data: Vec<GeocodingResponse> = Vec::new();
            for element in response_data {
                if !unique_response_data.iter().any(|existing_element| {
                    if existing_element.name == element.name
                        && existing_element.country == element.country
                        && existing_element.state == element.state
                    {
                        return true;
                    }
                    false
                }) {
                    unique_response_data.push(element);
                }
            }

            Ok(unique_response_data.iter().map(Self::geo_location_data_from_response).collect())
        })
    }
}

impl WeatherClient {
    pub fn new(lat: f64, lon: f64, cname: &str) -> Self {
        Self { city_data: CityData { lat, lon, city_name: cname.to_string() }, weather_data: None }
    }

    pub async fn refresh_weather(
        &mut self,
        weather_api: &OpenWeather,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let res = weather_api.one_call.call(self.city_data.lat, self.city_data.lon).await;
        log::debug!("Weather response: {res:?}");

        match res {
            Ok(response_data) => {
                self.weather_data = Some(response_data);
                log::debug!("Response received at: {:?}", chrono::offset::Local::now().timestamp());

                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}
