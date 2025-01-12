// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::sync::{Arc, Mutex};

use crate::ui::*;

use crate::weather;
use weather::DummyWeatherController;
use weather::{WeatherControllerPointer, WeatherControllerSharedPointer, WeatherDisplayController};

#[cfg(all(not(target_arch = "wasm32"), feature = "open_weather"))]
use weather::OpenWeatherController;

pub struct AppHandler {
    weather_controller: WeatherControllerSharedPointer,
    weather_display_controller: WeatherDisplayController,
    window: Option<AppWindow>,
    support_add_city: bool,
}

impl AppHandler {
    pub fn new() -> Self {
        #[cfg_attr(any(target_arch = "wasm32", not(feature = "open_weather")), allow(unused_mut))]
        let mut support_add_city = false;

        #[cfg_attr(any(target_arch = "wasm32", not(feature = "open_weather")), allow(unused_mut))]
        let mut data_controller_opt: Option<WeatherControllerPointer> = None;

        #[cfg(all(not(target_arch = "wasm32"), feature = "open_weather"))]
        {
            if let Some(api_key) = std::option_env!("OPEN_WEATHER_API_KEY") {
                data_controller_opt = Some(Box::new(OpenWeatherController::new(api_key.into())));
                support_add_city = true;
            }
        }

        let data_controller = match data_controller_opt {
            Some(data_controller_some) => data_controller_some,
            None => {
                log::info!("Weather API key not provided. Using dummy data.");
                Box::new(DummyWeatherController::new())
            }
        };
        let data_controller: WeatherControllerSharedPointer = Arc::new(Mutex::new(data_controller));

        Self {
            weather_controller: data_controller.clone(),
            weather_display_controller: WeatherDisplayController::new(&data_controller),
            window: None,
            support_add_city,
        }
    }

    pub fn save(&self) {
        log::debug!("Saving state");
        if let Err(e) = self.weather_controller.lock().unwrap().save() {
            log::warn!("Error while saving state: {}", e)
        }
    }

    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn reload(&self) {
        log::debug!("Reloading state");
        if let Some(window) = &self.window {
            self.weather_display_controller.refresh(window); // load new weather data
        } else {
            log::warn!("Cannot reload state, window not available.");
        }
    }

    pub fn initialize_ui(&mut self) {
        let window = AppWindow::new().expect("Cannot create main window!");
        self.weather_display_controller.initialize_ui(&window, self.support_add_city);

        self.window = Some(window);
    }

    pub fn run(&self) -> Result<(), slint::PlatformError> {
        let window = self.window.as_ref().expect("Cannot access main window!");
        self.weather_display_controller.load(window);
        window.run()
    }
}
