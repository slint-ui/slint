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
use i_slint_core::SlintContext;

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
    Ok(Box::new(i_slint_backend_linuxkms::BackendBuilder::default().build()?))
}

cfg_if::cfg_if! {
    if #[cfg(target_os = "android")] {
        const DEFAULT_BACKEND_NAME: &str = "";
    } else if #[cfg(all(feature = "i-slint-backend-qt", not(no_qt)))] {
        use i_slint_backend_qt as default_backend;
        const DEFAULT_BACKEND_NAME: &str = "qt";
    } else if #[cfg(feature = "i-slint-backend-winit")] {
        use i_slint_backend_winit as default_backend;
        const DEFAULT_BACKEND_NAME: &str = "winit";
    } else if #[cfg(all(feature = "i-slint-backend-linuxkms", target_os = "linux"))] {
        use i_slint_backend_linuxkms as default_backend;
        const DEFAULT_BACKEND_NAME: &str = "linuxkms";
    } else {
        const DEFAULT_BACKEND_NAME: &str = "";
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
                            format!("Error from {backend_name} backend: {err}").into()
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
            let (event_loop, _renderer) = parse_backend_env_var(backend_config.as_str());

            match event_loop {
                #[cfg(all(feature = "i-slint-backend-qt", not(no_qt)))]
                "qt" => return Ok(Box::new(i_slint_backend_qt::Backend::new())),
                #[cfg(feature = "i-slint-backend-winit")]
                "winit" => return i_slint_backend_winit::Backend::new_with_renderer_by_name((!_renderer.is_empty()).then_some(_renderer)).map(|b| Box::new(b) as Box<dyn Platform + 'static>),
                #[cfg(all(feature = "i-slint-backend-linuxkms", target_os = "linux"))]
                "linuxkms" => {
                    let mut builder = i_slint_backend_linuxkms::BackendBuilder::default();
                    if !_renderer.is_empty() {
                        builder = builder.with_renderer_name(_renderer.into());
                    }
                    return builder.build().map(|b| Box::new(b) as Box<dyn Platform + 'static>)
                },
                _ => {},
            }

            if !backend_config.is_empty() {
                eprintln!("Could not load rendering backend {backend_config}, fallback to default")
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

pub fn parse_backend_env_var(backend_config: &str) -> (&str, &str) {
    backend_config.split_once('-').unwrap_or(match backend_config {
        "qt" => ("qt", ""),
        "gl" | "winit" => ("winit", ""),
        "femtovg" => ("winit", "femtovg"),
        "skia" => ("winit", "skia"),
        "sw" | "software" => ("winit", "software"),
        "linuxkms" => ("linuxkms", ""),
        x => (x, ""),
    })
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

pub mod api;
