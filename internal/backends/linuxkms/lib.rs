// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

#[cfg(target_os = "linux")]
mod fullscreenwindowadapter;

#[cfg(target_os = "linux")]
use std::os::fd::OwnedFd;

#[cfg(target_os = "linux")]
type DeviceOpener<'a> = dyn Fn(&std::path::Path) -> Result<std::rc::Rc<OwnedFd>, i_slint_core::platform::PlatformError>
    + 'a;

#[cfg(all(target_os = "linux", feature = "drm"))]
mod drmoutput;

#[cfg(target_os = "linux")]
mod display;

#[cfg(target_os = "linux")]
mod renderer {
    use i_slint_core::platform::PlatformError;

    use crate::fullscreenwindowadapter::FullscreenRenderer;

    #[cfg(any(feature = "renderer-skia-opengl", feature = "renderer-skia-vulkan"))]
    pub mod skia;

    #[cfg(feature = "renderer-femtovg")]
    pub mod femtovg;

    #[cfg(feature = "renderer-software")]
    pub mod sw;

    pub fn try_skia_then_femtovg_then_software(
        _device_opener: &crate::DeviceOpener,
    ) -> Result<Box<dyn FullscreenRenderer>, PlatformError> {
        #[allow(unused)]
        type FactoryFn =
            fn(&crate::DeviceOpener) -> Result<Box<(dyn FullscreenRenderer)>, PlatformError>;

        let renderers = [
            #[cfg(any(feature = "renderer-skia-opengl", feature = "renderer-skia-vulkan"))]
            (
                "Skia",
                skia::SkiaRendererAdapter::new_try_vulkan_then_opengl_then_software as FactoryFn,
            ),
            #[cfg(feature = "renderer-femtovg")]
            ("FemtoVG", femtovg::FemtoVGRendererAdapter::new as FactoryFn),
            #[cfg(feature = "renderer-software")]
            ("Software", sw::SoftwareRendererAdapter::new as FactoryFn),
            ("", |_| Err(PlatformError::NoPlatform)),
        ];

        let mut renderer_errors: Vec<String> = Vec::new();
        for (name, factory) in renderers {
            match factory(_device_opener) {
                Ok(renderer) => return Ok(renderer),
                Err(err) => {
                    renderer_errors.push(if !name.is_empty() {
                        format!("Error from {} renderer: {}", name, err).into()
                    } else {
                        "No renderers configured.".into()
                    });
                }
            }
        }

        Err(PlatformError::Other(format!(
            "Could not initialize any renderer for LinuxKMS backend.\n{}",
            renderer_errors.join("\n")
        )))
    }
}

#[cfg(target_os = "linux")]
mod calloop_backend;

#[cfg(target_os = "linux")]
use calloop_backend::*;

#[cfg(not(target_os = "linux"))]
mod noop_backend;
use i_slint_core::api::PlatformError;
#[cfg(not(target_os = "linux"))]
use noop_backend::*;

#[derive(Default)]
pub struct BackendBuilder {
    pub(crate) renderer_name: Option<String>,
    #[cfg(target_os = "linux")]
    pub(crate) input_event_hook: Option<Box<dyn Fn(&input::Event) -> bool>>,
}

impl BackendBuilder {
    pub fn with_renderer_name(mut self, name: String) -> Self {
        self.renderer_name = Some(name);
        self
    }

    #[cfg(target_os = "linux")]
    pub fn with_input_event_hook(mut self, event_hook: Box<dyn Fn(&input::Event) -> bool>) -> Self {
        self.input_event_hook = Some(event_hook);
        self
    }

    pub fn build(self) -> Result<Backend, PlatformError> {
        Backend::build(self)
    }
}

#[doc(hidden)]
pub type NativeWidgets = ();
#[doc(hidden)]
pub type NativeGlobals = ();
#[doc(hidden)]
pub const HAS_NATIVE_STYLE: bool = false;
#[doc(hidden)]
pub mod native_widgets {}
