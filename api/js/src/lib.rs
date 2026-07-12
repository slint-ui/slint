// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Slint runtime library for JavaScript: the Node.js binding (napi) on native
//! targets, the browser binding (wasm-bindgen) on wasm32.

#[cfg(not(target_arch = "wasm32"))]
#[macro_use]
extern crate napi_derive;

#[cfg(not(target_arch = "wasm32"))]
mod node;
#[cfg(not(target_arch = "wasm32"))]
pub use node::*;

#[cfg(target_arch = "wasm32")]
mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::*;
