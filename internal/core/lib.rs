// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore sharedvector textlayout

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![deny(unsafe_code)]
#![no_std]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

#[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
pub(crate) mod unsafe_single_threaded;
#[cfg(all(not(feature = "std"), not(feature = "unsafe-single-threaded")))]
compile_error!(
    "At least one of the following feature need to be enabled: `std` or `unsafe-single-threaded`"
);
#[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
use crate::unsafe_single_threaded::thread_local;
#[cfg(feature = "std")]
use std::thread_local;

pub mod accessibility;
pub mod animations;
pub mod api;
pub mod callbacks;
pub mod component_factory;
pub mod context;
pub mod date_time;
pub mod future;
pub mod graphics;
pub mod input;
pub mod item_focus;
pub mod item_rendering;
pub mod item_tree;
pub mod items;
pub mod layout;
pub mod lengths;
pub mod menus;
pub mod model;
pub mod platform;
pub mod properties;
pub mod renderer;
#[cfg(feature = "rtti")]
pub mod rtti;
pub mod sharedvector;
pub mod slice;
#[cfg(feature = "software-renderer")]
pub mod software_renderer;
pub mod string;
pub mod tests;
pub mod textlayout;
pub mod timers;
pub mod translations;
pub mod window;

#[doc(inline)]
pub use string::SharedString;

#[doc(inline)]
pub use sharedvector::SharedVector;

#[doc(inline)]
pub use graphics::{ImageInner, StaticTextures};

#[doc(inline)]
pub use properties::Property;

#[doc(inline)]
pub use callbacks::Callback;

#[doc(inline)]
pub use graphics::Color;

#[doc(inline)]
pub use graphics::Brush;

#[doc(inline)]
pub use graphics::RgbaColor;

#[cfg(feature = "std")]
#[doc(inline)]
pub use graphics::PathData;

#[doc(inline)]
pub use graphics::BorderRadius;

pub use context::{with_global_context, SlintContext};

#[cfg(not(slint_int_coord))]
pub type Coord = f32;
#[cfg(slint_int_coord)]
pub type Coord = i32;

/// This type is not exported from the public API crate, so function having this
/// parameter cannot be called from the public API without naming it
pub struct InternalToken;
