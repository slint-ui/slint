// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};
use std::rc::Rc;

use crate::ui;
use ui::{
    AppWindow, BusyLayerController, CityWeather, CityWeatherInfo, GeoLocation, GeoLocationEntry,
    IconType, TemperatureInfo, WeatherForecastInfo, WeatherInfo,
};

use crate::weather::weathercontroller::{
    CityData, CityWeatherData, DayWeatherData, ForecastWeatherData, GeoLocationData,
    WeatherCondition, WeatherControllerSharedPointer,
};

#[cfg(not(target_arch = "wasm32"))]
use async_std::task::spawn as spawn_task;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local as spawn_task;

pub struct WeatherDisplayController {
    data_controller: WeatherControllerSharedPointer,
}

fn forecast_graph_command(
    model: ModelRc<WeatherForecastInfo>,
    days_count: i32,
    width: f32,
    height: f32,
) -> SharedString {
    if days_count == 0 || width == 0.0 || height == 0.0 {
        return SharedString::new();
    }

    let temperatures: Vec<f32> = model
        .clone()
        .iter()
        .take(days_count as usize)
        .map(|info| info.weather_info.detailed_temp.day)
        .collect();

    const MIN_MAX_MARGIN: f32 = 5.0;
    let min_temperature = match temperatures.iter().min_by(|a, b| a.total_cmp(b)) {
        Some(min) => min - MIN_MAX_MARGIN,
        None => 0.0,
    };
    let max_temperature = match temperatures.iter().max_by(|a, b| a.total_cmp(b)) {
        Some(max) => max + MIN_MAX_MARGIN,
        None => 50.0,
    };

    let max_temperature_value = max_temperature - min_temperature;
    let temperature_ratio = height / max_temperature_value;

    let day_width = width / days_count as f32;
    let max_day_shift = days_count as f32 * day_width;

    let border_command =
        format!(
        "M 0 0 M {max_width} 0 M {max_width} {max_temperature_value} M 0 {max_temperature_value} ",
        max_width=max_day_shift, max_temperature_value=max_temperature_value * temperature_ratio);

    let mut command = border_command;

    let day_shift = |index: f32| -> f32 { index * day_width + 0.5 * day_width };
    let day_temperature =
        |temperature: f32| -> f32 { (max_temperature - temperature) * temperature_ratio };

    for (index, &temperature) in temperatures.iter().enumerate() {
        if index == 0 {
            command += format!(
                "M {x} {y} ",
                x = day_shift(index as f32),
                y = day_temperature(temperature)
            )
            .as_str();
        }

        if let Some(next_temperature) = temperatures.get(index + 1) {
            let next_temperature = *next_temperature;

            let day1 = day_shift(index as f32);
            let day2 = day_shift(index as f32 + 1.0);
            let temp1 = day_temperature(temperature);
            let temp2 = day_temperature(next_temperature);

            let day_mid = (day1 + day2) / 2.0;
            let temp_mid = (temp1 + temp2) / 2.0;

            let cp_day1 = (day_mid + day1) / 2.0;
            let cp_day2 = (day_mid + day2) / 2.0;

            // Q {x1} {y1} {cx1} {cy1} Q {x2} {y2} {cx2} {cy2}
            command += format!(
                "Q {cp_day1} {temp1} {day_mid} {temp_mid} Q {cp_day2} {temp2} {day2} {temp2} "
            )
            .as_str();
        }
    }

    SharedString::from(command)
}

impl WeatherDisplayController {
    pub fn new(data_controller: &WeatherControllerSharedPointer) -> Self {
        Self { data_controller: data_controller.clone() }
    }

    pub fn initialize_ui(&self, window: &AppWindow, support_add_city: bool) {
        let city_weather = window.global::<CityWeather>();
        let geo_location = window.global::<GeoLocation>();

        // initialized models
        city_weather
            .set_city_weather(ModelRc::from(Rc::new(VecModel::<CityWeatherInfo>::from(vec![]))));
        geo_location
            .set_result_list(ModelRc::from(Rc::new(VecModel::<GeoLocationEntry>::from(vec![]))));

        // initialize state
        city_weather.set_can_add_city(support_add_city);

        // handle callbacks
        city_weather.on_get_forecast_graph_command(forecast_graph_command);

        city_weather.on_refresh_all({
            let window_weak = window.as_weak();
            let data_controller = self.data_controller.clone();

            move || Self::refresh_cities(&window_weak, &data_controller)
        });

        city_weather.on_reorder({
            let window_weak = window.as_weak();
            let data_controller = self.data_controller.clone();

            move |index, new_index| {
                if let Err(e) =
                    Self::reorder_cities(&window_weak, &data_controller, index, new_index)
                {
                    log::warn!("Failed to reorder city from {} to {}: {}", index, new_index, e);
                }
            }
        });

        city_weather.on_delete({
            let window_weak = window.as_weak();
            let data_controller = self.data_controller.clone();

            move |index| {
                if let Err(e) = Self::remove_city(&window_weak, &data_controller, index) {
                    log::warn!("Failed to remove city from {}: {}", index, e);
                }
            }
        });

        geo_location.on_search_location({
            let window_weak = window.as_weak();
            let data_controller = self.data_controller.clone();

            move |location| Self::search_location(&window_weak, &data_controller, location)
        });

        geo_location.on_add_location({
            let window_weak = window.as_weak();
            let data_controller = self.data_controller.clone();

            move |location| {
                Self::add_city(&window_weak, &data_controller, location);
            }
        });
    }

    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn refresh(&self, window: &AppWindow) {
        Self::set_busy(window);

        let window_weak = window.as_weak();
        Self::refresh_cities(&window_weak, &self.data_controller);
    }

    pub fn load(&self, window: &AppWindow) {
        Self::set_busy(window);

        let window_weak = window.as_weak();
        let data_controller = self.data_controller.clone();

        spawn_task(async move {
            let city_data_res = async {
                let mut data_controller = data_controller.lock().unwrap();
                data_controller.load()?;
                data_controller.refresh_cities()
            }
            .await;

            let city_data = match city_data_res {
                Ok(city_data) => Some(city_data),
                Err(e) => {
                    log::warn!("Failed to load cities: {}.", e);
                    None
                }
            };

            Self::check_update_error(window_weak.upgrade_in_event_loop(move |window| {
                if let Some(city_data) = city_data {
                    WeatherDisplayController::update_displayed_cities(&window, city_data);
                }
                Self::unset_busy(&window);
            }));
        });
    }

    fn refresh_cities(
        window_weak: &Weak<AppWindow>,
        data_controller: &WeatherControllerSharedPointer,
    ) {
        let window_weak = window_weak.clone();
        let data_controller = data_controller.clone();

        spawn_task(async move {
            let city_data_res = async { data_controller.lock().unwrap().refresh_cities() }.await;

            let city_data = match city_data_res {
                Ok(city_data) => Some(city_data),
                Err(e) => {
                    log::warn!("Failed to update cities: {}.", e);
                    None
                }
            };

            Self::check_update_error(window_weak.upgrade_in_event_loop(move |window| {
                if let Some(city_data) = city_data {
                    WeatherDisplayController::update_displayed_cities(&window, city_data);
                }
                Self::unset_busy(&window);
            }));
        });
    }

    fn add_city(
        window_weak: &Weak<AppWindow>,
        data_controller: &WeatherControllerSharedPointer,
        location: GeoLocationEntry,
    ) {
        let city = CityData {
            lat: location.lat as f64,
            lon: location.lon as f64,
            city_name: String::from(&location.name),
        };
        let city_data_res = data_controller.lock().unwrap().add_city(city);

        // update ui
        let window = window_weak.upgrade().unwrap();
        match city_data_res {
            Ok(city_data) => {
                if let Some(city_data) = city_data {
                    let city_weather = window.global::<CityWeather>();
                    let city_weather_list = city_weather.get_city_weather();

                    let city_weather = Self::city_weather_info_from_data(&city_data);
                    city_weather_list
                        .as_any()
                        .downcast_ref::<slint::VecModel<CityWeatherInfo>>()
                        .unwrap()
                        .push(city_weather);
                }
            }
            Err(e) => {
                log::warn!("Failed to add city: {}.", e);
            }
        }

        Self::unset_busy(&window);
    }

    fn reorder_cities(
        window_weak: &Weak<AppWindow>,
        data_controller: &WeatherControllerSharedPointer,
        index: i32,
        new_index: i32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pos: usize = index.try_into()?;
        let new_pos: usize = new_index.try_into()?;

        data_controller.lock().unwrap().reorder_cities(pos, new_pos)?;

        // update ui
        let window = window_weak.upgrade().unwrap();
        let city_weather = window.global::<CityWeather>();
        let city_weather_list = city_weather.get_city_weather();

        let pos_data = city_weather_list.row_data(pos).ok_or(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Index out of bounds",
        )))?;
        let new_pos_data = city_weather_list.row_data(new_pos).ok_or(Box::new(
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Index out of bounds"),
        ))?;

        city_weather_list.set_row_data(pos, new_pos_data);
        city_weather_list.set_row_data(new_pos, pos_data);
        Ok(())
    }

    fn remove_city(
        window_weak: &Weak<AppWindow>,
        data_controller: &WeatherControllerSharedPointer,
        index: i32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pos: usize = index.try_into()?;

        data_controller.lock().unwrap().remove_city(pos)?;

        // update ui
        let window = window_weak.upgrade().unwrap();
        let city_weather = window.global::<CityWeather>();
        let city_weather_list = city_weather.get_city_weather();

        let model = city_weather_list
            .as_any()
            .downcast_ref::<slint::VecModel<CityWeatherInfo>>()
            .expect("CityWeatherInfo model is not provided!");

        model.remove(pos);
        Ok(())
    }

    fn search_location(
        window_weak: &Weak<AppWindow>,
        data_controller: &WeatherControllerSharedPointer,
        query: slint::SharedString,
    ) {
        let window_weak = window_weak.clone();
        let data_controller = data_controller.clone();
        let query = query.to_string();

        spawn_task(async move {
            let locations_res =
                async { data_controller.lock().unwrap().search_location(query) }.await;

            let locations = match locations_res {
                Ok(locations) => Some(locations),
                Err(e) => {
                    log::warn!("Failed to search for location: {}.", e);
                    None
                }
            };

            Self::check_update_error(window_weak.upgrade_in_event_loop(move |window| {
                if let Some(locations) = locations {
                    WeatherDisplayController::update_location_search_results(&window, locations);
                }
            }));
        });
    }

    fn update_displayed_cities(window: &AppWindow, data: Vec<CityWeatherData>) {
        let display_vector: Vec<CityWeatherInfo> =
            data.iter().map(Self::city_weather_info_from_data).collect();

        let city_weather = window.global::<CityWeather>().get_city_weather();
        let model = city_weather
            .as_any()
            .downcast_ref::<VecModel<CityWeatherInfo>>()
            .expect("City weather model not set.");

        model.set_vec(display_vector);
    }

    fn update_location_search_results(window: &AppWindow, result: Vec<GeoLocationData>) {
        let display_vector: Vec<GeoLocationEntry> =
            result.iter().map(Self::geo_location_entry_from_data).collect();

        let geo_location = window.global::<GeoLocation>().get_result_list();
        let model = geo_location
            .as_any()
            .downcast_ref::<VecModel<GeoLocationEntry>>()
            .expect("Geo location entry model not set.");

        model.set_vec(display_vector);
    }

    fn set_busy(window: &AppWindow) {
        window.global::<BusyLayerController>().invoke_set_busy();
    }

    fn unset_busy(window: &AppWindow) {
        window.global::<BusyLayerController>().invoke_unset_busy();
    }

    fn check_update_error<E: std::fmt::Display>(result: Result<(), E>) {
        if let Err(e) = result {
            log::error!("Error while updating UI: {}", e);
        }
    }

    fn icon_type_from_condition(condition: &WeatherCondition) -> IconType {
        match condition {
            WeatherCondition::Sunny => IconType::Sunny,
            WeatherCondition::PartiallyCloudy => IconType::PartiallyCloudy,
            WeatherCondition::MostlyCloudy => IconType::MostlyCloudy,
            WeatherCondition::Cloudy => IconType::Cloudy,
            WeatherCondition::SunnyRainy => IconType::SunnyRainy,
            WeatherCondition::Rainy => IconType::Rainy,
            WeatherCondition::Stormy => IconType::Stormy,
            WeatherCondition::Snowy => IconType::Snowy,
            WeatherCondition::Foggy => IconType::Foggy,
            _ => IconType::Unknown,
        }
    }

    fn weather_info_from_data(data: &DayWeatherData) -> WeatherInfo {
        WeatherInfo {
            description: SharedString::from(&data.description),
            icon_type: Self::icon_type_from_condition(&data.condition),
            current_temp: data.current_temperature as f32,
            detailed_temp: TemperatureInfo {
                min: data.detailed_temperature.min as f32,
                max: data.detailed_temperature.max as f32,

                morning: data.detailed_temperature.morning as f32,
                day: data.detailed_temperature.day as f32,
                evening: data.detailed_temperature.evening as f32,
                night: data.detailed_temperature.night as f32,
            },
            uv: data.uv_index as i32,
            precipitation_prob: data.precipitation.probability as f32,
            rain: data.precipitation.rain_volume as f32,
            snow: data.precipitation.snow_volume as f32,
        }
    }

    fn forecast_weather_info_from_data(data: &[ForecastWeatherData]) -> Vec<WeatherForecastInfo> {
        data.iter()
            .map(|forecast_data| WeatherForecastInfo {
                day_name: SharedString::from(&forecast_data.day_name),
                weather_info: Self::weather_info_from_data(&forecast_data.weather_data),
            })
            .collect()
    }

    fn city_weather_info_from_data(data: &CityWeatherData) -> CityWeatherInfo {
        let current_weather_info = Self::weather_info_from_data(&data.weather_data.current_data);
        let forecast_weather_info =
            Self::forecast_weather_info_from_data(&data.weather_data.forecast_data);

        CityWeatherInfo {
            city_name: SharedString::from(&data.city_data.city_name),
            current_weather: current_weather_info,
            forecast_weather: Rc::new(slint::VecModel::from(forecast_weather_info)).into(),
        }
    }

    fn geo_location_entry_from_data(data: &GeoLocationData) -> GeoLocationEntry {
        GeoLocationEntry {
            name: SharedString::from(&data.name),
            state: SharedString::from(data.state.as_deref().unwrap_or_default()),
            country: SharedString::from(&data.country),
            lat: data.lat as f32,
            lon: data.lon as f32,
        }
    }
}
