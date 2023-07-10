// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore sharedvector textlayout

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![deny(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

#[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
pub(crate) mod unsafe_single_threaded;
#[cfg(all(not(feature = "std"), not(feature = "unsafe-single-threaded")))]
compile_error!(
    "At least one of the following feature need to be enabled: `std` or `unsafe-single-threaded`"
);

pub mod accessibility;
pub mod animations;
pub mod api;
pub mod callbacks;
pub mod component;
pub mod future;
pub mod graphics;
pub mod input;
pub mod item_focus;
pub mod item_rendering;
pub mod item_tree;
pub mod items;
pub mod layout;
pub mod lengths;
pub mod model;
pub mod platform;
pub mod properties;
pub mod renderer;
pub mod sharedvector;
pub mod slice;
pub mod software_renderer;
pub mod string;
pub mod tests;
pub mod textlayout;
pub mod timers;
pub mod translations;
pub mod window;

#[cfg(feature = "rtti")]
pub mod rtti;

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

use api::PlatformError;
use platform::Platform;

#[cfg(not(slint_int_coord))]
pub type Coord = f32;
#[cfg(slint_int_coord)]
pub type Coord = i32;

/// This type is not exported from the public API crate, so function having this
/// parameter cannot be called from the public API without naming it
pub struct InternalToken;

/// Internal function to access the platform abstraction.
/// The factory function is called if the platform abstraction is not yet
/// initialized, and should be given by the platform_selector
pub fn with_platform<R>(
    factory: impl FnOnce() -> Result<alloc::boxed::Box<dyn Platform + 'static>, PlatformError>,
    f: impl FnOnce(&dyn Platform) -> Result<R, PlatformError>,
) -> Result<R, PlatformError> {
    platform::PLATFORM_INSTANCE.with(|p| match p.get() {
        Some(p) => f(&**p),
        None => {
            platform::set_platform(factory()?).map_err(PlatformError::SetPlatformError)?;
            f(&**p.get().unwrap())
        }
    })
}
