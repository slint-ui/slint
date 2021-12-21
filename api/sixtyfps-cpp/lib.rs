// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

/*! This crate just expose the function used by the C++ integration */

use core::ffi::c_void;
use sixtyfps_corelib::window::ffi::WindowRcOpaque;
use sixtyfps_corelib::window::WindowRc;
use sixtyfps_rendering_backend_default::backend;

#[doc(hidden)]
#[cold]
pub fn use_modules() -> usize {
    #[cfg(feature = "sixtyfps-interpreter")]
    sixtyfps_interpreter::use_modules();
    sixtyfps_rendering_backend_default::use_modules();
    sixtyfps_corelib::use_modules()
}

#[no_mangle]
pub unsafe extern "C" fn sixtyfps_windowrc_init(out: *mut WindowRcOpaque) {
    assert_eq!(core::mem::size_of::<WindowRc>(), core::mem::size_of::<WindowRcOpaque>());
    core::ptr::write(out as *mut WindowRc, crate::backend().create_window());
}

#[no_mangle]
pub unsafe extern "C" fn sixtyfps_run_event_loop() {
    crate::backend()
        .run_event_loop(sixtyfps_corelib::backend::EventLoopQuitBehavior::QuitOnLastWindowClosed);
}

/// Will execute the given functor in the main thread
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_post_event(
    event: extern "C" fn(user_data: *mut c_void),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
) {
    struct UserData {
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    }
    impl Drop for UserData {
        fn drop(&mut self) {
            if let Some(x) = self.drop_user_data {
                x(self.user_data)
            }
        }
    }
    unsafe impl Send for UserData {}
    let ud = UserData { user_data, drop_user_data };

    crate::backend().post_event(Box::new(move || {
        let ud = &ud;
        event(ud.user_data);
    }));
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

#[no_mangle]
pub unsafe extern "C" fn sixtyfps_register_font_from_data(
    data: sixtyfps_corelib::slice::Slice<'static, u8>,
    error_str: *mut sixtyfps_corelib::SharedString,
) {
    core::ptr::write(
        error_str,
        match crate::backend().register_font_from_memory(data.as_slice()) {
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
