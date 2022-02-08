// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]
#![cfg_attr(not(any(feature = "i-slint-backend-qt", feature = "i-slint-backend-gl")), no_std)]

cfg_if::cfg_if! {
    if #[cfg(feature = "i-slint-backend-qt")] {
        use i_slint_backend_qt as default_backend;
    } else if #[cfg(feature = "i-slint-backend-gl")] {
        use i_slint_backend_gl as default_backend;
    }
}

cfg_if::cfg_if! {
    if #[cfg(any(
            feature = "i-slint-backend-qt",
            feature = "i-slint-backend-gl"
        ))] {
        pub fn backend() -> &'static dyn i_slint_core::backend::Backend {
            i_slint_core::backend::instance_or_init(|| {
                let backend_config = std::env::var("SLINT_BACKEND").or_else(|_| {
                    let legacy_fallback = std::env::var("SIXTYFPS_BACKEND");
                    if legacy_fallback.is_ok() {
                        eprintln!("Using `SIXTYFPS_BACKEND` environment variable for dynamic backend selection. This is deprecated, use `SLINT_BACKEND` instead.")
                    }
                    legacy_fallback
                }).unwrap_or_default();

                #[cfg(feature = "i-slint-backend-qt")]
                if backend_config == "Qt" {
                    return Box::new(i_slint_backend_qt::Backend);
                }
                #[cfg(feature = "i-slint-backend-gl")]
                if backend_config == "GL" {
                    return Box::new(i_slint_backend_gl::Backend);
                }

                #[cfg(any(
                    feature = "i-slint-backend-qt",
                    feature = "i-slint-backend-gl"
                ))]
                if !backend_config.is_empty() {
                    eprintln!("Could not load rendering backend {}, fallback to default", backend_config)
                }

                #[cfg(feature = "i-slint-backend-gl")]
                if !default_backend::IS_AVAILABLE {
                    // If Qt is not available always fallback to Gl
                    return Box::new(i_slint_backend_gl::Backend);
                }

                Box::new(default_backend::Backend)
            })
        }
        pub use default_backend::{
            native_widgets, Backend, NativeGlobals, NativeWidgets, HAS_NATIVE_STYLE,
        };
    } else {
        pub fn backend() -> &'static dyn i_slint_core::backend::Backend {
            i_slint_core::backend::instance().expect("no default backend configured, the backend must be initialized manually")
        }

        pub type NativeWidgets = ();
        pub type NativeGlobals = ();
        pub mod native_widgets {}
        pub const HAS_NATIVE_STYLE: bool = false;
    }
}

#[doc(hidden)]
#[cold]
#[cfg(not(target_arch = "wasm32"))]
pub fn use_modules() {
    i_slint_core::use_modules();
    #[cfg(feature = "i-slint-backend-qt")]
    i_slint_backend_qt::use_modules();
    #[cfg(feature = "i-slint-backend-gl")]
    i_slint_backend_gl::use_modules();
}
