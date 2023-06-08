// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

#[cfg(not(target_family = "windows"))]
mod fullscreenwindowadapter;

#[cfg(not(target_family = "windows"))]
mod display {
    pub trait Presenter {
        // Present updated front-buffer to the screen
        fn present(&self) -> Result<(), i_slint_core::platform::PlatformError>;
    }

    #[cfg(any(feature = "renderer-linuxkms-skia-opengl", feature = "renderer-linuxkms-femtovg"))]
    pub mod egldisplay;
    #[cfg(feature = "renderer-linuxkms-skia-vulkan")]
    pub mod vulkandisplay;
}

#[cfg(not(target_family = "windows"))]
mod renderer {
    #[cfg(any(
        feature = "renderer-linuxkms-skia-opengl",
        feature = "renderer-linuxkms-skia-vulkan"
    ))]
    pub mod skia;

    #[cfg(feature = "renderer-linuxkms-femtovg")]
    pub mod femtovg;

    pub fn try_skia_then_femtovg(
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::Renderer>, i_slint_core::platform::PlatformError> {
        #[allow(unused_assignments)]
        let mut result = Err(format!("No renderer configured").into());

        #[cfg(any(
            feature = "renderer-linuxkms-skia-opengl",
            feature = "renderer-linuxkms-skia-vulkan"
        ))]
        {
            result = skia::SkiaRendererAdapter::new_try_vulkan_then_opengl();
        }

        #[cfg(feature = "renderer-linuxkms-femtovg")]
        if result.is_err()
        {
            result = femtovg::FemtoVGRendererAdapter::new();
        }

        result
    }
}

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
