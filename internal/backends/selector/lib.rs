// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![cfg_attr(
    not(any(
        feature = "i-slint-backend-qt",
        feature = "i-slint-backend-winit",
        feature = "i-slint-backend-linuxkms"
    )),
    no_std
)]
#![allow(unused)]

extern crate alloc;

use alloc::boxed::Box;
use i_slint_core::platform::Platform;
use i_slint_core::platform::PlatformError;
use i_slint_core::OpenGLAPI;
use i_slint_core::SlintContext;
use i_slint_core::SlintRenderer;

#[cfg(all(feature = "i-slint-backend-qt", not(no_qt), not(target_os = "android")))]
fn create_qt_backend() -> Result<Box<dyn Platform + 'static>, PlatformError> {
    Ok(Box::new(default_backend::Backend::new()))
}

#[cfg(all(feature = "i-slint-backend-winit", not(target_os = "android")))]
fn create_winit_backend() -> Result<Box<dyn Platform + 'static>, PlatformError> {
    Ok(Box::new(i_slint_backend_winit::Backend::new()?))
}

#[cfg(all(feature = "i-slint-backend-linuxkms", target_os = "linux"))]
fn create_linuxkms_backend() -> Result<Box<dyn Platform + 'static>, PlatformError> {
    Ok(Box::new(i_slint_backend_linuxkms::Backend::new()?))
}

cfg_if::cfg_if! {
    if #[cfg(target_os = "android")] {
    } else if #[cfg(all(feature = "i-slint-backend-qt", not(no_qt)))] {
        use i_slint_backend_qt as default_backend;
    } else if #[cfg(feature = "i-slint-backend-winit")] {
        use i_slint_backend_winit as default_backend;
    } else if #[cfg(all(feature = "i-slint-backend-linuxkms", target_os = "linux"))] {
        use i_slint_backend_linuxkms as default_backend;
    } else {

    }
}

pub struct PlatformBuilder {
    opengl_api: Option<OpenGLAPI>,
    renderer: Option<SlintRenderer>,
}

impl PlatformBuilder {
    /// Creates a new PlatformBuilder for configuring aspects of the Platform.
    pub fn new() -> PlatformBuilder {
        PlatformBuilder { opengl_api: None, renderer: None }
    }

    /// Configures this builder to use the specified OpenGL API when building the platform later.
    pub fn with_opengl_api(mut self, opengl_api: OpenGLAPI) -> Self {
        self.opengl_api = Some(opengl_api);
        self
    }

    /// Configures this builder to use the specified renderer when building the platform later.
    pub fn with_renderer(mut self, renderer: SlintRenderer) -> Self {
        self.renderer = Some(renderer);
        self
    }

    /// Builds the platform with the parameters configured previously. Set the resulting platform
    /// with `slint::platform::set_platform()`:
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use i_slint_core::OpenGLAPI;
    /// use i_slint_core::platform;
    /// use i_slint_backend_selector::PlatformBuilder;
    ///
    /// let platform = PlatformBuilder::new()
    ///     .with_opengl_api(OpenGLAPI::GL(None))
    ///     .build()
    ///     .unwrap();
    /// platform::set_platform(platform).unwrap();
    /// ```
    pub fn build(self) -> Result<Box<dyn Platform + 'static>, PlatformError> {
        let builder = i_slint_backend_winit::Backend::builder().with_allow_fallback(false);

        let builder = match self.opengl_api {
            Some(api) => builder.with_opengl_api(api),
            None => builder,
        };

        let builder = match self.renderer {
            Some(SlintRenderer::Femtovg) => builder.with_renderer_name("femtovg"),
            Some(SlintRenderer::Skia) => builder.with_renderer_name("skia"),
            Some(SlintRenderer::Software) => builder.with_renderer_name("software"),
            None => builder,
        };

        Ok(Box::new(builder.build()?))
    }
}

cfg_if::cfg_if! {
    if #[cfg(all(not(target_os = "android"), any(
            all(feature = "i-slint-backend-qt", not(no_qt)),
            feature = "i-slint-backend-winit",
            all(feature = "i-slint-backend-linuxkms", target_os = "linux")
        )))] {
        fn create_default_backend() -> Result<Box<dyn Platform + 'static>, PlatformError> {
            use alloc::borrow::Cow;

            let backends = [
                #[cfg(all(feature = "i-slint-backend-qt", not(no_qt)))]
                ("Qt", create_qt_backend as fn() -> Result<Box<(dyn Platform + 'static)>, PlatformError>),
                #[cfg(feature = "i-slint-backend-winit")]
                ("Winit", create_winit_backend as fn() -> Result<Box<(dyn Platform + 'static)>, PlatformError>),
                #[cfg(all(feature = "i-slint-backend-linuxkms", target_os = "linux"))]
                ("LinuxKMS", create_linuxkms_backend as fn() -> Result<Box<(dyn Platform + 'static)>, PlatformError>),
                ("", || Err(PlatformError::NoPlatform)),
            ];

            let mut backend_errors: Vec<Cow<str>> = Vec::new();

            for (backend_name, backend_factory) in backends {
                match backend_factory() {
                    Ok(platform) => return Ok(platform),
                    Err(err) => {
                        backend_errors.push(if !backend_name.is_empty() {
                            format!("Error from {} backend: {}", backend_name, err).into()
                        } else {
                            "No backends configured.".into()
                        });
                    },
                }
            }

            Err(PlatformError::Other(format!("Could not initialize backend.\n{}", backend_errors.join("\n"))))
        }

        pub fn create_backend() -> Result<Box<dyn Platform + 'static>, PlatformError>  {

            let backend_config = std::env::var("SLINT_BACKEND").unwrap_or_default();

            let backend_config = backend_config.to_lowercase();
            let (event_loop, _renderer) = backend_config.split_once('-').unwrap_or(match backend_config.as_str() {
                "qt" => ("qt", ""),
                "gl" | "winit" => ("winit", ""),
                "femtovg" => ("winit", "femtovg"),
                "skia" => ("winit", "skia"),
                "sw" | "software" => ("winit", "software"),
                "linuxkms" => ("linuxkms", ""),
                x => (x, ""),
            });

            match event_loop {
                #[cfg(all(feature = "i-slint-backend-qt", not(no_qt)))]
                "qt" => return Ok(Box::new(i_slint_backend_qt::Backend::new())),
                #[cfg(feature = "i-slint-backend-winit")]
                "winit" => return i_slint_backend_winit::Backend::new_with_renderer_by_name((!_renderer.is_empty()).then_some(_renderer)).map(|b| Box::new(b) as Box<dyn Platform + 'static>),
                #[cfg(all(feature = "i-slint-backend-linuxkms", target_os = "linux"))]
                "linuxkms" => return i_slint_backend_linuxkms::Backend::new_with_renderer_by_name((!_renderer.is_empty()).then(|| _renderer)).map(|b| Box::new(b) as Box<dyn Platform + 'static>),
                _ => {},
            }

            if !backend_config.is_empty() {
                eprintln!("Could not load rendering backend {}, fallback to default", backend_config)
            }
            create_default_backend()
        }
        pub use default_backend::{
            native_widgets, Backend, NativeGlobals, NativeWidgets, HAS_NATIVE_STYLE,
        };
    } else {
        pub fn create_backend() -> Result<Box<dyn Platform + 'static>, PlatformError> {
            Err(PlatformError::NoPlatform)
        }
        pub mod native_widgets {}
        pub type NativeWidgets = ();
        pub type NativeGlobals = ();
        pub const HAS_NATIVE_STYLE: bool = false;
    }
}

/// Run the callback with the platform abstraction.
/// Create the backend if it does not exist yet
pub fn with_platform<R>(
    f: impl FnOnce(&dyn Platform) -> Result<R, PlatformError>,
) -> Result<R, PlatformError> {
    with_global_context(|ctx| f(ctx.platform()))?
}

/// Run the callback with the [`SlintContext`].
/// Create the backend if it does not exist yet
pub fn with_global_context<R>(f: impl FnOnce(&SlintContext) -> R) -> Result<R, PlatformError> {
    let mut platform_created = false;
    let result = i_slint_core::with_global_context(
        || {
            let backend = create_backend();
            platform_created = backend.is_ok();
            backend
        },
        f,
    );

    #[cfg(feature = "system-testing")]
    if result.is_ok() && platform_created {
        i_slint_backend_testing::systest::init();
    }

    result
}
