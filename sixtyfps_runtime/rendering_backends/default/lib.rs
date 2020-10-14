/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use sixtyfps_corelib::eventloop::ComponentWindow;

cfg_if::cfg_if! {
    if #[cfg(any(target_os="windows", target_os="macos", target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))] {
        use sixtyfps_rendering_backend_qt as default_backend;
        use default_backend::create_gl_window as create_default_window;
    } else {
        use sixtyfps_rendering_backend_gl as default_backend;
        use default_backend::create_gl_window as create_default_window;
    }
}

pub fn create_window() -> ComponentWindow {
    #[cfg(any(
        feature = "sixtyfps-rendering-backend-qt",
        feature = "sixtyfps-rendering-backend-gl"
    ))]
    let backend_config = std::env::var("SIXTYFPS_BACKEND").unwrap_or_default();

    #[cfg(feature = "sixtyfps-rendering-backend-qt")]
    if backend_config == "Qt" {
        return sixtyfps_rendering_backend_qt::create_gl_window();
    }
    #[cfg(feature = "sixtyfps-rendering-backend-gl")]
    if backend_config == "GL" {
        return sixtyfps_rendering_backend_gl::create_gl_window();
    }

    #[cfg(any(
        feature = "sixtyfps-rendering-backend-qt",
        feature = "sixtyfps-rendering-backend-gl"
    ))]
    if !backend_config.is_empty() {
        eprintln!("Could not load rendering backend {}, fallback to default", backend_config)
    }

    create_default_window()
}

pub use default_backend::{native_widgets, NativeWidgets, HAS_NATIVE_STYLE};

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
    use sixtyfps_corelib::eventloop::ffi::ComponentWindowOpaque;
    use sixtyfps_corelib::eventloop::ComponentWindow;

    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_init(out: *mut ComponentWindowOpaque) {
        assert_eq!(
            core::mem::size_of::<ComponentWindow>(),
            core::mem::size_of::<ComponentWindowOpaque>()
        );
        core::ptr::write(out as *mut ComponentWindow, crate::create_window());
    }
}
