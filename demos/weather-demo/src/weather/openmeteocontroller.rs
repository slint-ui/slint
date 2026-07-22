// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell:ignore dummyweather

#![cfg(all(not(target_arch = "wasm32"), feature = "open_meteo"))]

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;
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

// --- Open-Meteo API response types ---

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ForecastResponse {
    current: CurrentResponse,
    daily: DailyResponse,
    hourly: HourlyResponse,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct CurrentResponse {
    temperature_2m: f64,
    weather_code: i32,
    uv_index: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct DailyResponse {
    time: Vec<String>,
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
    weather_code: Vec<i32>,
    precipitation_probability_max: Vec<f64>,
    rain_sum: Vec<f64>,
    snowfall_sum: Vec<f64>,
    uv_index_max: Vec<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct HourlyResponse {
    time: Vec<String>,
    temperature_2m: Vec<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct GeocodingSearchResponse {
    results: Option<Vec<GeocodingResult>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct GeocodingResult {
    name: String,
    latitude: f64,
    longitude: f64,
    country: Option<String>,
    admin1: Option<String>,
}

// --- Persistent city data ---

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct WeatherClient {
    city_data: CityData,
    weather_data: Option<ForecastResponse>,
}

pub struct OpenMeteoController {
    tokio_runtime: tokio::runtime::Runtime,
    http_client: reqwest::Client,
    city_clients: Arc<Mutex<Vec<WeatherClient>>>,
    storage_path: Option<PathBuf>,
}

fn weather_condition_from_wmo(code: i32) -> WeatherCondition {
    match code {
        0 | 1 => WeatherCondition::Sunny,
        2 => WeatherCondition::PartiallyCloudy,
        3 => WeatherCondition::Cloudy,
        45 | 48 => WeatherCondition::Foggy,
        51..=55 => WeatherCondition::SunnyRainy,
        56..=67 => WeatherCondition::Rainy,
        71..=77 => WeatherCondition::Snowy,
        80..=82 => WeatherCondition::Rainy,
        85 | 86 => WeatherCondition::Snowy,
        95..=99 => WeatherCondition::Stormy,
        _ => WeatherCondition::Unknown,
    }
}

fn description_from_wmo(code: i32) -> &'static str {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 => "Fog",
        48 => "Depositing rime fog",
        51 => "Light drizzle",
        53 => "Moderate drizzle",
        55 => "Dense drizzle",
        56 => "Light freezing drizzle",
        57 => "Dense freezing drizzle",
        61 => "Slight rain",
        63 => "Moderate rain",
        65 => "Heavy rain",
        66 => "Light freezing rain",
        67 => "Heavy freezing rain",
        71 => "Slight snowfall",
        73 => "Moderate snowfall",
        75 => "Heavy snowfall",
        77 => "Snow grains",
        80 => "Slight rain showers",
        81 => "Moderate rain showers",
        82 => "Violent rain showers",
        85 => "Slight snow showers",
        86 => "Heavy snow showers",
        95 => "Thunderstorm",
        96 => "Thunderstorm with slight hail",
        99 => "Thunderstorm with heavy hail",
        _ => "Unknown",
    }
}

fn hourly_temp(hourly: &[f64], day_index: usize, hour: usize, fallback: f64) -> f64 {
    let i = day_index * 24 + hour;
    if i < hourly.len() { hourly[i] } else { fallback }
}

impl OpenMeteoController {
    pub fn new() -> Self {
        let storage_path;
        if let Some(project_dir) = project_data_dir() {
            storage_path = Some(project_dir.as_path().join(CITIES_STORED_FILE_NAME));
        } else {
            storage_path = None;
            log::error!("Failed to initialize project dir. Persistent data will not be loaded");
        }

        Self {
            tokio_runtime: tokio::runtime::Runtime::new().unwrap(),
            http_client: reqwest::Client::new(),
            city_clients: Arc::new(Mutex::new(Vec::new())),
            storage_path,
        }
    }

    fn current_day_weather_data_from_response(
        response: &Option<ForecastResponse>,
    ) -> DayWeatherData {
        if let Some(data) = response {
            let current = &data.current;
            let daily = &data.daily;
            let hourly = &data.hourly.temperature_2m;
            let fallback = current.temperature_2m;

            let detailed_temp = if !daily.time.is_empty() {
                TemperatureData {
                    min: daily.temperature_2m_min[0],
                    max: daily.temperature_2m_max[0],
                    morning: hourly_temp(hourly, 0, 6, fallback),
                    day: hourly_temp(hourly, 0, 12, fallback),
                    evening: hourly_temp(hourly, 0, 18, fallback),
                    night: hourly_temp(hourly, 0, 0, fallback),
                }
            } else {
                TemperatureData {
                    min: fallback,
                    max: fallback,
                    morning: fallback,
                    day: fallback,
                    evening: fallback,
                    night: fallback,
                }
            };

            return DayWeatherData {
                description: description_from_wmo(current.weather_code).to_string(),
                condition: weather_condition_from_wmo(current.weather_code),
                current_temperature: fallback,
                detailed_temperature: detailed_temp,
                precipitation: PrecipitationData::default(),
                uv_index: current.uv_index,
            };
        }

        DayWeatherData::default()
    }

    fn forecast_day_weather_data_from_response(
        response: &Option<ForecastResponse>,
    ) -> Vec<ForecastWeatherData> {
        let mut forecast_weather_info: Vec<ForecastWeatherData> = Vec::new();

        if let Some(data) = response {
            let daily = &data.daily;
            let hourly = &data.hourly.temperature_2m;

            for (i, date_str) in daily.time.iter().enumerate() {
                let day_name = if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                    let datetime: DateTime<Utc> = date.and_hms_opt(0, 0, 0).unwrap().and_utc();
                    get_day_from_datetime(datetime)
                } else {
                    date_str.clone()
                };

                let fallback = hourly_temp(hourly, i, 12, 0.0);

                let detailed_temperature = TemperatureData {
                    min: daily.temperature_2m_min[i],
                    max: daily.temperature_2m_max[i],
                    morning: hourly_temp(hourly, i, 6, fallback),
                    day: fallback,
                    evening: hourly_temp(hourly, i, 18, fallback),
                    night: hourly_temp(hourly, i, 0, fallback),
                };

                let precipitation = PrecipitationData {
                    probability: daily.precipitation_probability_max[i] / 100.0,
                    rain_volume: daily.rain_sum[i],
                    snow_volume: daily.snowfall_sum[i],
                };

                let weather_code = daily.weather_code[i];
                let day_weather_info = DayWeatherData {
                    description: description_from_wmo(weather_code).to_string(),
                    condition: weather_condition_from_wmo(weather_code),
                    current_temperature: fallback,
                    detailed_temperature,
                    precipitation,
                    uv_index: daily.uv_index_max[i],
                };

                forecast_weather_info
                    .push(ForecastWeatherData { day_name, weather_data: day_weather_info });
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

    fn default_cities() -> Vec<WeatherClient> {
        let json_data = std::include_str!("./dummyweather.json");
        match serde_json::from_str::<Vec<CityWeatherData>>(json_data) {
            Ok(data) => data
                .into_iter()
                .map(|c| {
                    WeatherClient::new(c.city_data.lat, c.city_data.lon, &c.city_data.city_name)
                })
                .collect(),
            Err(e) => {
                log::warn!("Cannot parse default city list: {e}");
                Vec::new()
            }
        }
    }
}

impl WeatherController for OpenMeteoController {
    fn load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let city_clients_data = if let Some(storage_path) = &self.storage_path
            && storage_path.exists()
        {
            log::debug!("Loading data from: {:?}", storage_path.to_str());
            let file = File::open(storage_path.as_path())?;
            let reader = BufReader::new(file);
            serde_json::from_reader(reader)?
        } else {
            log::info!("No saved data found, seeding from default city list.");
            Self::default_cities()
        };

        log::debug!("Loaded {} cities", city_clients_data.len());

        let city_clients = self.city_clients.clone();
        self.tokio_runtime.block_on(async move {
            let mut city_clients = city_clients.lock().await;
            *city_clients = city_clients_data;
            Ok(())
        })
    }

    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(storage_path) = &self.storage_path {
            log::debug!("Saving data to: {:?}", storage_path.display());

            if let Some(parent_dir) = storage_path.parent() {
                std::fs::create_dir_all(parent_dir)?;
            }

            let file = File::create(storage_path)?;
            let mut writer = BufWriter::new(file);
            let city_clients = self.city_clients.clone();

            self.tokio_runtime.block_on(async move {
                let city_clients = city_clients.lock().await;
                serde_json::to_writer(&mut writer, &*city_clients)?;
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
        let http_client = self.http_client.clone();

        self.tokio_runtime.block_on(async move {
            let mut city_clients = city_clients_clone.lock().await;

            // Fetch all cities concurrently.
            let fetches: Vec<_> = city_clients
                .iter()
                .map(|client| WeatherClient::fetch_weather(&http_client, &client.city_data))
                .collect();
            let results = futures::future::join_all(fetches).await;

            let mut error_count = 0;
            let mut last_error: Option<reqwest::Error> = None;
            for (client, result) in city_clients.iter_mut().zip(results) {
                match result {
                    Ok(data) => client.weather_data = Some(data),
                    Err(e) => {
                        error_count += 1;
                        last_error = Some(e);
                    }
                }
            }
            log::debug!("Refreshing weather finished!");

            if error_count > 0 && error_count == city_clients.len() {
                return Err(last_error.unwrap().into());
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
        let http_client = self.http_client.clone();

        self.tokio_runtime.block_on(async move {
            let mut city_clients = city_clients_clone.lock().await;
            match city_clients.iter().position(|client| client.city_data == city) {
                Some(_) => {
                    log::info!("City already present in list!");
                    Ok(None)
                }
                None => {
                    let mut client = WeatherClient::new(city.lat, city.lon, &city.city_name);
                    client.refresh_weather(&http_client).await?;
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

        if query.is_empty() {
            return Ok(Vec::new());
        }

        let http_client = self.http_client.clone();

        self.tokio_runtime.block_on(async move {
            let url = format!(
                "https://geocoding-api.open-meteo.com/v1/search?name={}&count=5&language=en",
                urlencoding::encode(&query)
            );

            let response: GeocodingSearchResponse =
                http_client.get(&url).send().await?.json().await?;

            log::debug!("Search result: {response:?}");

            let results = response.results.unwrap_or_default();
            Ok(results
                .into_iter()
                .map(|loc| GeoLocationData {
                    name: loc.name,
                    state: loc.admin1,
                    country: loc.country.unwrap_or_default(),
                    lat: loc.latitude,
                    lon: loc.longitude,
                })
                .collect())
        })
    }
}

impl WeatherClient {
    pub fn new(lat: f64, lon: f64, cname: &str) -> Self {
        Self { city_data: CityData { lat, lon, city_name: cname.to_string() }, weather_data: None }
    }

    async fn fetch_weather(
        http_client: &reqwest::Client,
        city: &CityData,
    ) -> Result<ForecastResponse, reqwest::Error> {
        let url = format!(
            "https://api.open-meteo.com/v1/forecast\
             ?latitude={}&longitude={}\
             &current=temperature_2m,weather_code,uv_index\
             &daily=temperature_2m_max,temperature_2m_min,weather_code,\
             precipitation_probability_max,rain_sum,snowfall_sum,uv_index_max\
             &hourly=temperature_2m\
             &timezone=auto&forecast_days=8",
            city.lat, city.lon
        );

        let response_data: ForecastResponse = http_client.get(&url).send().await?.json().await?;
        log::debug!("Response received at: {:?}", chrono::offset::Local::now().timestamp());
        Ok(response_data)
    }

    async fn refresh_weather(
        &mut self,
        http_client: &reqwest::Client,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let data = Self::fetch_weather(http_client, &self.city_data).await?;
        self.weather_data = Some(data);
        Ok(())
    }
}
