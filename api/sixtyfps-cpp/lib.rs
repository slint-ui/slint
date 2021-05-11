/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*! This scrates just expose the function used by the C++ integration */

#[doc(hidden)]
#[cold]
pub fn use_modules() -> usize {
    #[cfg(feature = "sixtyfps-interpreter")]
    sixtyfps_interpreter::use_modules();
    sixtyfps_rendering_backend_default::use_modules();
    sixtyfps_corelib::use_modules()
}

use sixtyfps_rendering_backend_default::backend;

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
    crate::backend()
        .run_event_loop(sixtyfps_corelib::backend::EventLoopQuitBehavior::QuitOnLastWindowClosed);
}

#[no_mangle]
pub unsafe extern "C" fn sixtyfps_quit_event_loop() {
    crate::backend().quit_event_loop();
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

#[cfg(feature = "testing")]
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_testing_init_backend() {
    sixtyfps_rendering_backend_testing::init();
}
