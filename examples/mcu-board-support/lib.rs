// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore deque pico

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![no_std]

extern crate alloc;

#[cfg(feature = "pico-st7789")]
mod pico_st7789;
#[cfg(feature = "pico-st7789")]
pub use pico_st7789::*;

#[cfg(feature = "pico2-st7789")]
mod pico2_st7789;
#[cfg(feature = "pico2-st7789")]
pub use pico2_st7789::*;

#[cfg(feature = "stm32h735g")]
mod stm32h735g;
#[cfg(feature = "stm32h735g")]
pub use stm32h735g::*;

#[cfg(feature = "stm32u5g9j-dk2")]
mod stm32u5g9j_dk2;
#[cfg(feature = "stm32u5g9j-dk2")]
pub use stm32u5g9j_dk2::*;

// #[cfg(feature = "esp32-s3-box")]
// mod esp32_s3_box;
// #[cfg(feature = "esp32-s3-box")]
// pub use esp32_s3_box::*;

#[cfg(not(any(
    feature = "pico-st7789",
    feature = "pico2-st7789",
    feature = "stm32h735g",
    feature = "stm32u5g9j-dk2",
//    feature = "esp32-s3-box"
)))]
pub use i_slint_core_macros::identity as entry;

#[cfg(not(any(
    feature = "pico-st7789",
    feature = "pico2-st7789",
    feature = "stm32h735g",
    feature = "stm32u5g9j-dk2",
//    feature = "esp32-s3-box"
)))]
pub fn init() {}

#[cfg(feature = "stm32u5g9j-dk2")]
mod embassy;
