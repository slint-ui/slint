// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore sharedvector swrenderer textlayout

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]
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
pub mod string;
#[cfg(feature = "swrenderer")]
pub mod swrenderer;
pub mod tests;
pub mod timers;
pub mod window;

#[cfg(feature = "rtti")]
pub mod rtti;

#[cfg(feature = "text_layout")]
pub mod textlayout;

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
use platform::Platform;

#[cfg(not(slint_int_coord))]
pub type Coord = f32;
#[cfg(slint_int_coord)]
pub type Coord = i32;

/// Internal function to access the platform abstraction.
/// The factory function is called if the platform abstraction is not yet
/// initialized, and should be given by the platform_selector
pub fn with_platform(
    factory: impl FnOnce() -> alloc::boxed::Box<dyn Platform + 'static>,
    f: impl FnOnce(&dyn Platform) -> R,
) -> R {
    platform::PLATFORM_INSTANCE.with(|p| match p.get() {
        Some(p) => f(&**p),
        None => {
            platform::set_platform(factory())
                .expect("platform already initialized in another thread");
            f(&**p.get().unwrap())
        }
    })
}

/// One need to use at least one function in each module in order to get them
/// exported in the final binary.
/// This only use functions from modules which are not otherwise used.
#[doc(hidden)]
#[cold]
#[cfg(not(target_arch = "wasm32"))]
pub fn use_modules() -> usize {
    #[cfg(feature = "ffi")]
    {
        tests::slint_mock_elapsed_time as usize
            + callbacks::ffi::slint_callback_init as usize
            + sharedvector::ffi::slint_shared_vector_empty as usize
            + layout::ffi::slint_solve_grid_layout as usize
            + item_tree::ffi::slint_visit_item_tree as usize
            + graphics::ffi::slint_new_path_elements as usize
            + properties::ffi::slint_property_init as usize
            + string::ffi::slint_shared_string_bytes as usize
            + window::ffi::slint_windowrc_drop as usize
            + component::ffi::slint_register_component as usize
            + timers::ffi::slint_timer_start as usize
            + graphics::color::ffi::slint_color_brighter as usize
            + graphics::image::ffi::slint_image_size as usize
    }
    #[cfg(not(feature = "ffi"))]
    {
        0
    }
}
