// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

mod skiawindowadapter;

#[cfg(not(target_family = "windows"))]
mod calloop_backend;
#[cfg(not(target_family = "windows"))]
pub use calloop_backend::*;

#[cfg(target_family = "windows")]
mod noop_backend;
#[cfg(target_family = "windows")]
pub use noop_backend::*;

#[doc(hidden)]
pub type NativeWidgets = ();
#[doc(hidden)]
pub type NativeGlobals = ();
#[doc(hidden)]
pub const HAS_NATIVE_STYLE: bool = false;
#[doc(hidden)]
pub mod native_widgets {}
