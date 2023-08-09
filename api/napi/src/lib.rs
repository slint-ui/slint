// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

mod interpreter;
pub use interpreter::*;

mod types;
pub use types::*;

use napi::{bindgen_prelude::*, Env};

#[macro_use]
extern crate napi_derive;

#[napi]
pub fn mock_elapsed_time(ms: f64) {
    i_slint_core::tests::slint_mock_elapsed_time(ms as _);
}

#[cfg(target_os = "macos")]
extern "C" {
    fn uv_backend_fd(l: *const napi::sys::uv_loop_s) -> std::ffi::c_int;
    fn uv_backend_timeout(l: *const napi::sys::uv_loop_s) -> std::ffi::c_int;
    fn uv_run(l: *const napi::sys::uv_loop_s, mode: std::ffi::c_int) -> std::ffi::c_int;
}

#[cfg(target_os = "macos")]
struct PolledUVLoop {
    loop_ptr: std::sync::atomic::AtomicUsize,
    poll_fd: std::os::fd::RawFd,
    poll_timeout: std::ffi::c_int,
}

#[cfg(target_os = "macos")]
impl PolledUVLoop {
    unsafe fn new(uv_loop: *const napi::sys::uv_loop_s) -> Self {
        let poll_fd = uv_backend_fd(uv_loop);
        let poll_timeout = uv_backend_timeout(uv_loop);
        Self { loop_ptr: std::sync::atomic::AtomicUsize::new(uv_loop as _), poll_fd, poll_timeout }
    }

    unsafe fn update(&mut self) {
        let uv_loop: *const napi::sys::uv_loop_s =
            self.loop_ptr.load(std::sync::atomic::Ordering::Relaxed) as _;
        self.poll_fd = unsafe { uv_backend_fd(uv_loop) };
        self.poll_timeout = unsafe { uv_backend_timeout(uv_loop) };
    }

    unsafe fn run_once(&mut self) {
        let uv_loop: *const napi::sys::uv_loop_s =
            self.loop_ptr.load(std::sync::atomic::Ordering::Relaxed) as _;
        let mut r = 1;
        while r != 0 {
            r = uv_run(uv_loop, /*no wait */ 2);
            //eprintln!("uv_run() returned {r}");
            break;
        }
    }

    unsafe fn poll(&mut self) {
        let pollfd = nix::poll::PollFd::new(self.poll_fd, nix::poll::PollFlags::POLLIN);
        //eprintln!("polling with timeout {}", self.poll_timeout);
        let ready_fds = nix::poll::poll(&mut [pollfd], self.poll_timeout).unwrap();
        //eprintln!("returned from poll {ready_fds}");
        // Handle EINTR
        //assert_eq!(ready_fds, 1);
    }
}

#[cfg(target_os = "macos")]
fn start_event_loop_watcher_thread(env: Env) -> napi::Result<napi::JsUndefined> {
    use napi::sys::uv_loop_s;

    let polled_loop = unsafe {
        PolledUVLoop::new(
            env.get_uv_event_loop().map_err(|e| napi::Error::from_reason(e.to_string()))?,
        )
    };
    let mut polled_loop = std::sync::Arc::new(std::sync::Mutex::new(polled_loop));

    eprintln!("starting thread");

    std::thread::Builder::new()
        .name("slint libuv event poll thread".into())
        .spawn(move || {
            let wait_for_uv_run = std::sync::Arc::new(std::sync::Condvar::new());

            loop {
                let mut locked_loop = polled_loop.lock().unwrap();
                unsafe {
                    locked_loop.poll();
                }

                let loop_clone = polled_loop.clone();
                let wait_for_uv_run_clone = wait_for_uv_run.clone();

                slint_interpreter::invoke_from_event_loop(move || {
                    eprintln!("processing libuv events in slint thread");
                    let mut locked_loop = loop_clone.lock().unwrap();
                    unsafe {
                        locked_loop.run_once();
                        locked_loop.update();
                    }
                    eprintln!("finished processing libuv events");

                    wait_for_uv_run_clone.notify_one();
                });

                wait_for_uv_run.wait(locked_loop);
            }
        })
        .unwrap();

    env.get_undefined()
}

#[napi]
pub fn run_event_loop(env: Env) -> napi::Result<napi::JsUndefined> {
    #[cfg(target_os = "macos")]
    start_event_loop_watcher_thread(env)?;

    slint_interpreter::run_event_loop()
        .map_err(|e| napi::Error::from_reason(e.to_string()))
        .and_then(|_| env.get_undefined())
}

#[napi]
pub fn run_event_loop_with_callback(
    env: Env,
    callback: napi::JsFunction,
) -> napi::Result<napi::JsUndefined> {
    // Cannot use invoke_from_event_loop before that.
    slint::private_unstable_api::ensure_backend().unwrap();

    let callback_ref =
        crate::interpreter::component_instance::RefCountedReference::new(&env, callback)?;
    let callback_ref = send_wrapper::SendWrapper::new(callback_ref);

    slint_interpreter::invoke_from_event_loop(move || {
        let callback: napi::JsFunction = callback_ref.take().get().unwrap();
        callback.call_without_args(None).ok();
    })
    .unwrap();

    run_event_loop(env)
}

#[napi]
pub fn quit_event_loop() -> napi::Result<()> {
    slint_interpreter::quit_event_loop().map_err(|e| napi::Error::from_reason(e.to_string()))
}
