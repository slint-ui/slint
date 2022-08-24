// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore deque pico

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(any(feature = "pico-st7789", feature = "stm32h735g"), feature(alloc_error_handler))]

extern crate alloc;

#[cfg(feature = "pico-st7789")]
mod pico_st7789;

#[cfg(feature = "pico-st7789")]
pub use pico_st7789::*;

#[cfg(feature = "stm32h735g")]
mod stm32h735g;

#[cfg(feature = "stm32h735g")]
pub use stm32h735g::*;

#[cfg(not(any(feature = "pico-st7789", feature = "stm32h735g")))]
pub use i_slint_core_macros::identity as entry;

#[cfg(not(any(feature = "pico-st7789", feature = "stm32h735g")))]
pub fn init() {}
