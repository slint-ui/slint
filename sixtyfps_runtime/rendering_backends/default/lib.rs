/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
The purpose of this crate is to select the default backend for [SixtyFPS](https://sixtyfps.io)

The backend can either be a runtime or a build time decision.  The runtime decision is decided
by the `SIXTYFPS_BACKEND` environment variable. The built time default depends on the platform.
In order for the crate to be available at runtime, they need to be added as feature

*NOTE*: This library is an internal crate for the [SixtyFPS project](https://sixtyfps.io).
This crate should not be used directly by application using SixtyFPS.
You should use the `sixtyfps` crate instead.

*/
#![doc(html_logo_url = "https://sixtyfps.io/resources/logo.drawio.svg")]

cfg_if::cfg_if! {
    if #[cfg(any(target_os="windows", target_os="macos", target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))] {
        use sixtyfps_rendering_backend_qt as default_backend;
    } else {
        use sixtyfps_rendering_backend_gl as default_backend;
    }
}

pub fn backend() -> &'static dyn sixtyfps_corelib::backend::Backend {
    sixtyfps_corelib::backend::instance_or_init(|| {
        #[cfg(any(
            feature = "sixtyfps-rendering-backend-qt",
            feature = "sixtyfps-rendering-backend-gl"
        ))]
        let backend_config = std::env::var("SIXTYFPS_BACKEND").unwrap_or_default();

        #[cfg(feature = "sixtyfps-rendering-backend-qt")]
        if backend_config == "Qt" {
            return Box::new(sixtyfps_rendering_backend_qt::Backend);
        }
        #[cfg(feature = "sixtyfps-rendering-backend-gl")]
        if backend_config == "GL" {
            return Box::new(sixtyfps_rendering_backend_gl::Backend);
        }

        #[cfg(any(
            feature = "sixtyfps-rendering-backend-qt",
            feature = "sixtyfps-rendering-backend-gl"
        ))]
        if !backend_config.is_empty() {
            eprintln!("Could not load rendering backend {}, fallback to default", backend_config)
        }

        #[cfg(feature = "sixtyfps-rendering-backend-gl")]
        if !default_backend::IS_AVAILABLE {
            // If Qt is not available always fallback to Gl
            return Box::new(sixtyfps_rendering_backend_gl::Backend);
        }
        return Box::new(default_backend::Backend);
    })
}

pub use default_backend::{
    native_widgets, Backend, NativeGlobals, NativeWidgets, HAS_NATIVE_STYLE,
};

#[doc(hidden)]
#[cold]
pub fn use_modules() {
    default_backend::use_modules();
    #[cfg(feature = "sixtyfps-rendering-backend-qt")]
    sixtyfps_rendering_backend_qt::use_modules();
    #[cfg(feature = "sixtyfps-rendering-backend-gl")]
    sixtyfps_rendering_backend_gl::use_modules();
}

pub mod ffi {
    use sixtyfps_corelib::window::ffi::ComponentWindowOpaque;
    use sixtyfps_corelib::window::ComponentWindow;

    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_init(out: *mut ComponentWindowOpaque) {
        assert_eq!(
            core::mem::size_of::<ComponentWindow>(),
            core::mem::size_of::<ComponentWindowOpaque>()
        );
        core::ptr::write(out as *mut ComponentWindow, crate::backend().create_window());
    }

    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_run_event_loop() {
        crate::backend().run_event_loop();
    }

    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_register_font_from_path(
        path: &sixtyfps_corelib::SharedString,
        error_str: *mut sixtyfps_corelib::SharedString,
    ) {
        core::ptr::write(
            error_str,
            match crate::backend().register_font_from_path(std::path::Path::new(path.as_str())) {
                Ok(()) => Default::default(),
                Err(err) => err.to_string().into(),
            },
        )
    }
}
