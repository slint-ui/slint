/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!

# SixtyFPS runtime library

**NOTE**: This library is an **internal** crate for the [SixtyFPS project](https://sixtyfps.io).
This crate should **not be used directly** by applications using SixtyFPS.
You should use the `sixtyfps` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.

*/
#![doc(html_logo_url = "https://sixtyfps.io/resources/logo.drawio.svg")]
#![deny(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

/// Unsafe module that is only enabled when the unsafe_single_core feature is on.
/// It re-implements the thread_local macro with statics
#[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
pub(crate) mod unsafe_single_core {
    #![allow(unsafe_code)]
    macro_rules! thread_local_ {
        ($(#[$($meta:tt)*])* $vis:vis static $ident:ident : $ty:ty = $expr:expr) => {
            $(#[$($meta)*])*
            pub(crate) static $ident: crate::unsafe_single_core::FakeThreadStorage<$ty> = {
                fn init() -> $ty { $expr }
                crate::unsafe_single_core::FakeThreadStorage::new(init)
            };
        };
    }
    pub(crate) struct FakeThreadStorage<T, F = fn() -> T>(once_cell::unsync::OnceCell<T>, F);
    impl<T, F> FakeThreadStorage<T, F> {
        pub const fn new(f: F) -> Self {
            Self(once_cell::unsync::OnceCell::new(), f)
        }
    }
    impl<T> FakeThreadStorage<T> {
        pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
            f(self.0.get_or_init(self.1))
        }
        pub fn try_with<R>(&self, f: impl FnOnce(&T) -> R) -> Result<R, ()> {
            Ok(f(self.0.get().ok_or(())?))
        }
    }
    // Safety: the unsafe_single_core feature means we will only be called from a single thread
    unsafe impl<T, F> Send for FakeThreadStorage<T, F> {}
    unsafe impl<T, F> Sync for FakeThreadStorage<T, F> {}

    pub(crate) use thread_local_ as thread_local;

    pub(crate) struct OnceCell<T>(once_cell::unsync::OnceCell<T>);
    impl<T> OnceCell<T> {
        pub const fn new() -> Self {
            Self(once_cell::unsync::OnceCell::new())
        }
        pub fn get(&self) -> Option<&T> {
            self.0.get()
        }
        pub fn get_or_init(&self, f: impl FnOnce() -> T) -> &T {
            self.0.get_or_init(f)
        }
    }

    // Safety: the unsafe_single_core feature means we will only be called from a single thread
    unsafe impl<T> Send for OnceCell<T> {}
    unsafe impl<T> Sync for OnceCell<T> {}
}

pub mod animations;
pub mod backend;
pub mod callbacks;
pub mod component;
pub(crate) mod flickable;
pub mod graphics;
pub mod input;
pub mod item_rendering;
pub mod item_tree;
pub mod items;
pub mod layout;
pub mod model;
pub mod properties;
pub mod sharedvector;
pub mod slice;
pub mod string;
pub mod tests;
pub mod timers;
pub mod window;

#[cfg(feature = "rtti")]
pub mod rtti;

#[doc(inline)]
pub use string::SharedString;

#[doc(inline)]
pub use sharedvector::SharedVector;

#[doc(inline)]
pub use graphics::ImageInner;

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

/// One need to use at least one function in each module in order to get them
/// exported in the final binary.
/// This only use functions from modules which are not otherwise used.
#[doc(hidden)]
#[cold]
#[cfg(not(target_arch = "wasm32"))]
pub fn use_modules() -> usize {
    #[cfg(feature = "ffi")]
    {
        tests::sixtyfps_mock_elapsed_time as usize
            + callbacks::ffi::sixtyfps_callback_init as usize
            + sharedvector::ffi::sixtyfps_shared_vector_empty as usize
            + layout::ffi::sixtyfps_solve_grid_layout as usize
            + item_tree::ffi::sixtyfps_visit_item_tree as usize
            + graphics::ffi::sixtyfps_new_path_elements as usize
            + properties::ffi::sixtyfps_property_init as usize
            + string::ffi::sixtyfps_shared_string_bytes as usize
            + window::ffi::sixtyfps_windowrc_drop as usize
            + component::ffi::sixtyfps_component_init_items as usize
            + timers::ffi::sixtyfps_timer_start as usize
            + graphics::color::ffi::sixtyfps_color_brighter as usize
            + graphics::image::ffi::sixtyfps_image_size as usize
    }
    #[cfg(not(feature = "ffi"))]
    {
        0
    }
}
