// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

/*! This crate just expose the function used by the C++ integration */

use core::ffi::c_void;
use i_slint_core::window::{ffi::WindowAdapterRcOpaque, WindowAdapter};
use std::rc::Rc;

#[cfg(feature = "experimental")]
pub mod platform;

/// One need to make sure something from the crate is exported,
/// otherwise its symbols are not going to be in the final binary
#[cfg(feature = "slint-interpreter")]
pub use slint_interpreter;

#[no_mangle]
pub unsafe extern "C" fn slint_windowrc_init(out: *mut WindowAdapterRcOpaque) {
    assert_eq!(
        core::mem::size_of::<Rc<dyn WindowAdapter>>(),
        core::mem::size_of::<WindowAdapterRcOpaque>()
    );
    let win = i_slint_backend_selector::with_platform(|b| b.create_window_adapter()).unwrap();
    core::ptr::write(out as *mut Rc<dyn WindowAdapter>, win);
}

#[no_mangle]
pub unsafe extern "C" fn slint_ensure_backend() {
    i_slint_backend_selector::with_platform(|_b| {
        // Nothing to do, just make sure a backend was created
        Ok(())
    })
    .unwrap()
}

#[no_mangle]
pub unsafe extern "C" fn slint_run_event_loop() {
    i_slint_backend_selector::with_platform(|b| b.run_event_loop()).unwrap();
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
    })
    .unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn slint_quit_event_loop() {
    i_slint_core::api::quit_event_loop().unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn slint_register_font_from_path(
    win: *const WindowAdapterRcOpaque,
    path: &i_slint_core::SharedString,
    error_str: *mut i_slint_core::SharedString,
) {
    let window_adapter = &*(win as *const Rc<dyn WindowAdapter>);
    core::ptr::write(
        error_str,
        match window_adapter.renderer().register_font_from_path(std::path::Path::new(path.as_str()))
        {
            Ok(()) => Default::default(),
            Err(err) => err.to_string().into(),
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn slint_register_font_from_data(
    win: *const WindowAdapterRcOpaque,
    data: i_slint_core::slice::Slice<'static, u8>,
    error_str: *mut i_slint_core::SharedString,
) {
    let window_adapter = &*(win as *const Rc<dyn WindowAdapter>);
    core::ptr::write(
        error_str,
        match window_adapter.renderer().register_font_from_memory(data.as_slice()) {
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
