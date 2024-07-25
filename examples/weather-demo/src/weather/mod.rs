// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

mod weathercontroller;
mod weatherdisplaycontroller;

mod dummyweathercontroller;

pub use weathercontroller::WeatherControllerPointer;
pub use weathercontroller::WeatherControllerSharedPointer;
pub use weatherdisplaycontroller::WeatherDisplayController;

pub use dummyweathercontroller::DummyWeatherController;

#[cfg(all(not(target_arch = "wasm32"), feature = "open_weather"))]
mod openweathercontroller;

#[cfg(all(not(target_arch = "wasm32"), feature = "open_weather"))]
pub use openweathercontroller::OpenWeatherController;

pub mod utils;
