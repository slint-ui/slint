// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

/*!

**NOTE**: This library is an **internal** crate for the [SixtyFPS project](https://sixtyfps.io).
This crate should **not be used directly** by applications using SixtyFPS.
You should use the `sixtyfps` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.


The purpose of this crate is to select the default backend for [SixtyFPS](https://sixtyfps.io)

The backend can either be a runtime or a build time decision.  The runtime decision is decided
by the `SIXTYFPS_BACKEND` environment variable. The built time default depends on the platform.
In order for the crate to be available at runtime, they need to be added as feature

*/
#![doc(html_logo_url = "https://sixtyfps.io/resources/logo.drawio.svg")]
#![cfg_attr(
    not(any(feature = "slint-backend-qt-internal", feature = "slint-backend-gl-internal")),
    no_std
)]

cfg_if::cfg_if! {
    if #[cfg(feature = "slint-backend-qt-internal")] {
        use slint_backend_qt_internal as default_backend;
    } else if #[cfg(feature = "slint-backend-gl-internal")] {
        use slint_backend_gl_internal as default_backend;
    }
}

cfg_if::cfg_if! {
    if #[cfg(any(
            feature = "slint-backend-qt-internal",
            feature = "slint-backend-gl-internal"
        ))] {
        pub fn backend() -> &'static dyn slint_core_internal::backend::Backend {
            slint_core_internal::backend::instance_or_init(|| {
                let backend_config = std::env::var("SIXTYFPS_BACKEND").unwrap_or_default();

                #[cfg(feature = "slint-backend-qt-internal")]
                if backend_config == "Qt" {
                    return Box::new(slint_backend_qt_internal::Backend);
                }
                #[cfg(feature = "slint-backend-gl-internal")]
                if backend_config == "GL" {
                    return Box::new(slint_backend_gl_internal::Backend);
                }

                #[cfg(any(
                    feature = "slint-backend-qt-internal",
                    feature = "slint-backend-gl-internal"
                ))]
                if !backend_config.is_empty() {
                    eprintln!("Could not load rendering backend {}, fallback to default", backend_config)
                }

                #[cfg(feature = "slint-backend-gl-internal")]
                if !default_backend::IS_AVAILABLE {
                    // If Qt is not available always fallback to Gl
                    return Box::new(slint_backend_gl_internal::Backend);
                }

                Box::new(default_backend::Backend)
            })
        }
        pub use default_backend::{
            native_widgets, Backend, NativeGlobals, NativeWidgets, HAS_NATIVE_STYLE,
        };
    } else {
        pub fn backend() -> &'static dyn slint_core_internal::backend::Backend {
            slint_core_internal::backend::instance().expect("no default backend configured, the backend must be initialized manually")
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
    slint_core_internal::use_modules();
    #[cfg(feature = "slint-backend-qt-internal")]
    slint_backend_qt_internal::use_modules();
    #[cfg(feature = "slint-backend-gl-internal")]
    slint_backend_gl_internal::use_modules();
}
