// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

#[cfg(target_os = "linux")]
mod fullscreenwindowadapter;

#[cfg(target_os = "linux")]
use std::os::fd::AsFd;

#[cfg(target_os = "linux")]
type DeviceOpener<'a> = dyn Fn(&std::path::Path) -> Result<std::sync::Arc<dyn AsFd>, i_slint_core::platform::PlatformError>
    + 'a;

#[cfg(target_os = "linux")]
mod display {
    pub trait Presenter {
        // Present updated front-buffer to the screen
        fn present(&self) -> Result<(), Box<dyn std::error::Error>>;
    }

    #[cfg(any(feature = "renderer-skia-opengl", feature = "renderer-femtovg"))]
    pub mod egldisplay;
    #[cfg(feature = "renderer-skia-vulkan")]
    pub mod vulkandisplay;
}

#[cfg(target_os = "linux")]
mod renderer {
    #[cfg(any(feature = "renderer-skia-opengl", feature = "renderer-skia-vulkan"))]
    pub mod skia;

    #[cfg(feature = "renderer-femtovg")]
    pub mod femtovg;

    pub fn try_skia_then_femtovg(
        _device_opener: &crate::DeviceOpener,
    ) -> Result<
        Box<dyn crate::fullscreenwindowadapter::Renderer>,
        i_slint_core::platform::PlatformError,
    > {
        #[allow(unused_assignments)]
        let mut result = Err(format!("No renderer configured").into());

        #[cfg(any(feature = "renderer-skia-opengl", feature = "renderer-skia-vulkan"))]
        {
            result = skia::SkiaRendererAdapter::new_try_vulkan_then_opengl(_device_opener);
        }

        #[cfg(feature = "renderer-femtovg")]
        if result.is_err() {
            result = femtovg::FemtoVGRendererAdapter::new(_device_opener);
        }

        result
    }
}

#[cfg(target_os = "linux")]
mod calloop_backend;

#[cfg(target_os = "linux")]
pub use calloop_backend::*;

#[cfg(not(target_os = "linux"))]
mod noop_backend;
#[cfg(not(target_os = "linux"))]
pub use noop_backend::*;

#[doc(hidden)]
pub type NativeWidgets = ();
#[doc(hidden)]
pub type NativeGlobals = ();
#[doc(hidden)]
pub const HAS_NATIVE_STYLE: bool = false;
#[doc(hidden)]
pub mod native_widgets {}
