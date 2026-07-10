// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell:ignore dummyweathercontroller weathercontroller weatherdisplaycontroller openmeteocontroller
mod weathercontroller;
mod weatherdisplaycontroller;

mod dummyweathercontroller;

pub use weathercontroller::WeatherControllerPointer;
pub use weathercontroller::WeatherControllerSharedPointer;
pub use weatherdisplaycontroller::WeatherDisplayController;

pub use dummyweathercontroller::DummyWeatherController;

#[cfg(all(not(target_arch = "wasm32"), feature = "open_meteo"))]
mod openmeteocontroller;

#[cfg(all(not(target_arch = "wasm32"), feature = "open_meteo"))]
pub use openmeteocontroller::OpenMeteoController;

pub mod utils;
