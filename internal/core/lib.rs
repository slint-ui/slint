// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore sharedvector swrenderer textlayout

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]
#![deny(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

/// Unsafe module that is only enabled when the unsafe_single_core feature is on.
/// It re-implements the thread_local macro with statics
#[cfg(feature = "unsafe_single_core")]
pub mod unsafe_single_core {
    #![allow(unsafe_code)]
    #[macro_export]
    macro_rules! thread_local_ {
        ($(#[$($meta:tt)*])* $vis:vis static $ident:ident : $ty:ty = $expr:expr) => {
            $(#[$($meta)*])*
            $vis static $ident: crate::unsafe_single_core::FakeThreadStorage<$ty> = {
                fn init() -> $ty { $expr }
                crate::unsafe_single_core::FakeThreadStorage::new(init)
            };
        };
    }
    pub struct FakeThreadStorage<T, F = fn() -> T>(once_cell::unsync::OnceCell<T>, F);
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

    pub use thread_local_ as thread_local;

    pub struct OnceCell<T>(once_cell::unsync::OnceCell<T>);
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

pub mod accessibility;
pub mod animations;
pub mod api;
pub mod backend;
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
pub mod properties;
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

#[cfg(not(slint_int_coord))]
pub type Coord = f32;
#[cfg(slint_int_coord)]
pub type Coord = i32;

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
