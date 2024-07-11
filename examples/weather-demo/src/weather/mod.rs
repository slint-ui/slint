// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

mod weathercontroller;
mod weatherdisplaycontroller;

mod dummyweathercontroller;

pub use weathercontroller::WeatherControllerPointer;
pub use weathercontroller::WeatherControllerSharedPointer;
pub use weatherdisplaycontroller::WeatherDisplayController;

pub use dummyweathercontroller::DummyWeatherController;

#[cfg(not(target_arch = "wasm32"))]
mod openweathercontroller;

#[cfg(not(target_arch = "wasm32"))]
pub use openweathercontroller::OpenWeatherController;

pub mod utils;
