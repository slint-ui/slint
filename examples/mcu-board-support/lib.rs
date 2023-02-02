// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore deque pico

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "pico-st7789")]
mod pico_st7789;
#[cfg(feature = "pico-st7789")]
pub use pico_st7789::*;

#[cfg(feature = "stm32h735g")]
mod stm32h735g;
#[cfg(feature = "stm32h735g")]
pub use stm32h735g::*;

#[cfg(feature = "esp32-s2-kaluga-1")]
mod esp32_s2_kaluga_1;
#[cfg(feature = "esp32-s2-kaluga-1")]
pub use esp32_s2_kaluga_1::*;

#[cfg(feature = "esp32-s3-box")]
mod esp32_s3_box;
#[cfg(feature = "esp32-s3-box")]
pub use esp32_s3_box::*;

#[cfg(not(any(
    feature = "pico-st7789",
    feature = "stm32h735g",
    feature = "esp32-s2-kaluga-1",
    feature = "esp32-s3-box"
)))]
pub use i_slint_core_macros::identity as entry;

#[cfg(not(any(
    feature = "pico-st7789",
    feature = "stm32h735g",
    feature = "esp32-s2-kaluga-1",
    feature = "esp32-s3-box"
)))]
pub fn init() {}
