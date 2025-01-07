// Copyright © 2025 David Haig
// SPDX-License-Identifier: MIT

#![cfg_attr(feature = "mcu", no_std)]

extern crate alloc;

pub mod controller;
pub mod slint_backend;

#[cfg(feature = "mcu")]
pub mod mcu;

#[cfg(feature = "mcu")]
pub use defmt::{debug, error, info, trace, warn};

#[cfg(feature = "simulator")]
pub mod simulator;

#[cfg(feature = "simulator")]
pub use log::{debug, error, info, trace, warn};
