// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore deque pico

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![no_std]

extern crate alloc;

#[cfg(feature = "pico-st7789")]
#[path = "pico_st7789/pico_st7789.rs"]
mod pico_st7789;
#[cfg(feature = "pico-st7789")]
pub use pico_st7789::*;

#[cfg(feature = "pico2-st7789")]
#[path = "pico2_st7789/pico2_st7789.rs"]
mod pico2_st7789;
#[cfg(feature = "pico2-st7789")]
pub use pico2_st7789::*;

#[cfg(feature = "stm32h735g")]
#[path = "stm32h735g/stm32h735g.rs"]
mod stm32h735g;
#[cfg(feature = "stm32h735g")]
pub use stm32h735g::*;

#[cfg(feature = "stm32u5g9j-dk2")]
#[path = "stm32u5g9j_dk2/stm32u5g9j_dk2.rs"]
mod stm32u5g9j_dk2;
#[cfg(feature = "stm32u5g9j-dk2")]
pub use stm32u5g9j_dk2::*;

#[cfg(feature = "esp32-s3-box-3")]
#[path = "esp32_s3_box_3/esp32_s3_box_3.rs"]
mod esp32_s3_box_3;
#[cfg(feature = "esp32-s3-box-3")]
pub use esp32_s3_box_3::*;
#[cfg(feature = "esp32-s3-box-3")]
pub use esp_hal::main as entry;

#[cfg(not(any(
    feature = "pico-st7789",
    feature = "pico2-st7789",
    feature = "stm32h735g",
    feature = "stm32u5g9j-dk2",
    feature = "esp32-s3-box-3"
)))]
pub use i_slint_core_macros::identity as entry;

#[cfg(not(any(
    feature = "pico-st7789",
    feature = "pico2-st7789",
    feature = "stm32h735g",
    feature = "stm32u5g9j-dk2",
    feature = "esp32-s3-box-3"
)))]
pub fn init() {}

#[cfg(feature = "stm32u5g9j-dk2")]
mod embassy;

pub mod prelude {
    #[cfg(feature = "esp32-s3-box-3")]
    pub use esp_hal;
}
