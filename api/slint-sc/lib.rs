// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Slint SC: a safety-critical subset of Slint.
//!
//! This crate is a stand-alone, `no_std`, `no_alloc`, dependency-free runtime
//! meant to be paired with the Slint compiler in `--slint-sc` mode.  It
//! intentionally implements only a tiny subset of Slint (no event loop,
//! no models, no text, no callbacks, no animations) and only three visual
//! elements (`Window`, `Rectangle`, `Image`) with their geometry properties.
//!
//! The user controls the event loop and calls the generated component's
//! `render` method once per frame with a buffer that implements
//! [`api::TargetPixelBuffer`].
//!
//! This is a prototype whose purpose is to make the documentation in
//! `docs/safety/` correspond to something real.  The API and internals may
//! change in incompatible ways.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod api;
pub mod private_unstable_api;

pub use api::*;

/// Includes the generated file produced by `slint-build` with the
/// safety-critical generator enabled, mirroring `slint::include_modules!()`.
#[macro_export]
macro_rules! include_modules {
    () => {
        include!(env!("SLINT_INCLUDE_GENERATED"));
    };
}
