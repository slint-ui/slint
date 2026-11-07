// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The entry point on Android, like `slint_main` in the C++ API.
//! `NativeActivity` loads this library and runs `android_main` on a thread of its own,
//! which becomes the Slint thread and calls the C# entry point.

use std::cell::Cell;
use std::ffi::c_void;
use std::sync::OnceLock;

use i_slint_backend_android_activity::android_activity::{MainEvent, PollEvent};
use i_slint_backend_android_activity::{AndroidApp, AndroidPlatform};

/// The activity events that `SlintActivity` passes on, in addition to the saved state.
#[repr(i32)]
enum ActivityEvent {
    Pause = 0,
    Resume = 1,
    LowMemory = 2,
}

/// The C# side of `SlintActivity`. Each callback runs on the Slint thread.
#[repr(C)]
pub struct AndroidCallbacks {
    main: extern "C" fn(),
    event: extern "C" fn(event: i32),
    /// Fills the state with `slint_dotnet_android_state_set`.
    save_state: extern "C" fn(state: *mut c_void),
    restore_state: extern "C" fn(data: *const u8, len: usize),
}

static CALLBACKS: OnceLock<AndroidCallbacks> = OnceLock::new();

/// Sets the C# side. Callable from any thread, before `NativeActivity.onCreate`.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_android_set_callbacks(callbacks: AndroidCallbacks) {
    let _ = CALLBACKS.set(callbacks);
}

/// Sets the state that `save_state` saves.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_android_state_set(
    state: *mut c_void,
    data: *const u8,
    len: usize,
) {
    let state = unsafe { &mut *(state as *mut Vec<u8>) };
    state.clear();
    if len > 0 {
        state.extend_from_slice(unsafe { std::slice::from_raw_parts(data, len) });
    }
}

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    let callbacks = CALLBACKS
        .get()
        .expect("slint_dotnet_android_set_callbacks runs before NativeActivity.onCreate");
    // Android hands back the saved state at every resume, but it only differs from the app's own
    // after the system ended the process, so only the first resume of the process restores it.
    let restored = Cell::new(false);
    let listener = move |event: &PollEvent<'_>| match event {
        PollEvent::Main(MainEvent::SaveState { saver, .. }) => {
            let mut state = Vec::new();
            (callbacks.save_state)(&mut state as *mut Vec<u8> as *mut c_void);
            saver.store(&state);
        }
        PollEvent::Main(MainEvent::Resume { loader, .. }) => {
            if !restored.replace(true)
                && let Some(state) = loader.load()
            {
                (callbacks.restore_state)(state.as_ptr(), state.len());
            }
            (callbacks.event)(ActivityEvent::Resume as i32);
        }
        PollEvent::Main(MainEvent::Pause) => (callbacks.event)(ActivityEvent::Pause as i32),
        PollEvent::Main(MainEvent::LowMemory) => (callbacks.event)(ActivityEvent::LowMemory as i32),
        _ => {}
    };
    i_slint_core::platform::set_platform(Box::new(AndroidPlatform::new_with_event_listener(
        app, listener,
    )))
    .expect("the Android platform is set once");
    crate::check_thread().expect("android_main runs before any other use of Slint");
    (callbacks.main)();
}
