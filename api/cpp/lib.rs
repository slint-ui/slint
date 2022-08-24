// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*! This crate just expose the function used by the C++ integration */

use core::ffi::c_void;
use i_slint_core::window::{ffi::PlatformWindowRcOpaque, PlatformWindow};
use std::rc::Rc;

#[doc(hidden)]
#[cold]
pub fn use_modules() -> usize {
    #[cfg(feature = "slint-interpreter")]
    slint_interpreter::use_modules();
    i_slint_backend_selector::use_modules();
    i_slint_core::use_modules()
}

#[no_mangle]
pub unsafe extern "C" fn slint_windowrc_init(out: *mut PlatformWindowRcOpaque) {
    assert_eq!(
        core::mem::size_of::<Rc<dyn PlatformWindow>>(),
        core::mem::size_of::<PlatformWindowRcOpaque>()
    );
    let win = i_slint_backend_selector::with_platform_abstraction(|b| b.create_window());
    core::ptr::write(out as *mut Rc<dyn PlatformWindow>, win);
}

#[no_mangle]
pub unsafe extern "C" fn slint_run_event_loop() {
    i_slint_backend_selector::with_platform_abstraction(|b| {
        b.run_event_loop(i_slint_core::platform::EventLoopQuitBehavior::QuitOnLastWindowClosed)
    });
}

/// Will execute the given functor in the main thread
#[no_mangle]
pub unsafe extern "C" fn slint_post_event(
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

    i_slint_core::api::invoke_from_event_loop(move || {
        let ud = &ud;
        event(ud.user_data);
    });
}

#[no_mangle]
pub unsafe extern "C" fn slint_quit_event_loop() {
    i_slint_core::api::quit_event_loop()
}

#[no_mangle]
pub unsafe extern "C" fn slint_register_font_from_path(
    win: *const PlatformWindowRcOpaque,
    path: &i_slint_core::SharedString,
    error_str: *mut i_slint_core::SharedString,
) {
    let platform_window = &*(win as *const Rc<dyn PlatformWindow>);
    core::ptr::write(
        error_str,
        match platform_window
            .renderer()
            .register_font_from_path(std::path::Path::new(path.as_str()))
        {
            Ok(()) => Default::default(),
            Err(err) => err.to_string().into(),
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn slint_register_font_from_data(
    win: *const PlatformWindowRcOpaque,
    data: i_slint_core::slice::Slice<'static, u8>,
    error_str: *mut i_slint_core::SharedString,
) {
    let platform_window = &*(win as *const Rc<dyn PlatformWindow>);
    core::ptr::write(
        error_str,
        match platform_window.renderer().register_font_from_memory(data.as_slice()) {
            Ok(()) => Default::default(),
            Err(err) => err.to_string().into(),
        },
    )
}

#[cfg(feature = "testing")]
#[no_mangle]
pub unsafe extern "C" fn slint_testing_init_backend() {
    i_slint_backend_testing::init();
}
