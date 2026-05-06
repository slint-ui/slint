// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Integrated event loop for Node.js, replacing 16ms setInterval polling.
//!
//! On Unix, async-io watches libuv's backend fd and wakes winit when Node.js
//! has pending work. A spawned local future monitors the fd for readability
//! while winit's pump_events blocks for UI events — whichever fires first
//! returns control to JS so it can run uv_run.
//!
//! On other platforms (Windows, Deno), the JS side falls back to 16ms polling
//! which keeps JS responsive between pumps. Windows can't use the fd watcher
//! because IOCP doesn't expose a pollable fd.

#[cfg(unix)]
mod platform {
    use super::super::ProcessEventsResult;
    use core::ops::ControlFlow;
    use napi::Env;
    use std::cell::{Cell, OnceCell};
    use std::os::fd::BorrowedFd;
    use std::os::raw::c_int;
    use std::time::Duration;

    // libuv function types — exported by the Node.js binary but not in napi-sys,
    // so they're loaded via libloading at runtime.
    type UvBackendFdFn = unsafe extern "C" fn(*mut napi::sys::uv_loop_s) -> c_int;
    type UvBackendTimeoutFn = unsafe extern "C" fn(*mut napi::sys::uv_loop_s) -> c_int;

    /// Cached libuv function pointers and loop handle.
    #[derive(Clone, Copy)]
    struct UvFunctions {
        backend_fd: UvBackendFdFn,
        backend_timeout: UvBackendTimeoutFn,
        uv_loop: *mut napi::sys::uv_loop_s,
    }

    impl UvFunctions {
        fn try_new(env: &Env) -> Option<Self> {
            // Load uv_backend_fd and uv_backend_timeout from the host process
            // (the Node.js binary). These aren't in napi-sys.
            let lib = libloading::os::unix::Library::this();
            // SAFETY: These symbols are exported by the Node.js binary with
            // stable C signatures defined by libuv. The type aliases above
            // match the libuv declarations.
            let backend_fd: UvBackendFdFn =
                *unsafe { lib.get::<UvBackendFdFn>(b"uv_backend_fd").ok()? };
            let backend_timeout: UvBackendTimeoutFn =
                *unsafe { lib.get::<UvBackendTimeoutFn>(b"uv_backend_timeout").ok()? };

            let uv_loop = env.get_uv_event_loop().ok()?;
            if uv_loop.is_null() {
                return None;
            }

            // SAFETY: uv_loop was just validated non-null above.
            let fd = unsafe { backend_fd(uv_loop) };
            if fd < 0 {
                return None;
            }

            Some(Self { backend_fd, backend_timeout, uv_loop })
        }

        fn fd(&self) -> c_int {
            // SAFETY: uv_loop is validated non-null in try_new and owned by
            // Node.js for the process lifetime.
            unsafe { (self.backend_fd)(self.uv_loop) }
        }

        fn timeout_ms(&self) -> c_int {
            // SAFETY: same as fd() above.
            unsafe { (self.backend_timeout)(self.uv_loop) }
        }
    }

    /// Wrapper around libuv's backend fd that doesn't close it on drop.
    struct UvFdWrapper(c_int);

    impl std::os::fd::AsFd for UvFdWrapper {
        fn as_fd(&self) -> BorrowedFd<'_> {
            // SAFETY: The fd is libuv's backend fd (epoll/kqueue), owned by
            // Node.js for the process lifetime. We don't close it on drop.
            unsafe { BorrowedFd::borrow_raw(self.0) }
        }
    }

    thread_local! {
        static CACHED_UV: OnceCell<Option<UvFunctions>> = const { OnceCell::new() };
        static WATCHER_SPAWNED: Cell<bool> = const { Cell::new(false) };
    }

    fn get_uv(env: &Env) -> napi::Result<UvFunctions> {
        CACHED_UV.with(|cell| {
            cell.get_or_init(|| UvFunctions::try_new(env))
                .ok_or_else(|| napi::Error::from_reason("integrated event loop not available"))
        })
    }

    /// Returns true if the integrated event loop is available on this platform/runtime.
    pub(crate) fn has_integrated_event_loop_impl(env: &Env) -> bool {
        get_uv(env).is_ok()
    }

    /// Spawns a persistent future that watches the libuv fd for readability.
    /// Called once; the future loops internally, re-registering with async-io's
    /// reactor after each wake. When the fd becomes readable, the future's waker
    /// fires invoke_from_event_loop, unblocking pump_events.
    fn ensure_watcher_spawned(uv: &UvFunctions) -> napi::Result<()> {
        if WATCHER_SPAWNED.get() {
            return Ok(());
        }
        let async_fd = async_io::Async::new(UvFdWrapper(uv.fd())).map_err(|e| {
            napi::Error::from_reason(format!("failed to create async fd watcher: {e}"))
        })?;
        slint_interpreter::spawn_local(async move {
            loop {
                if async_fd.readable().await.is_err() {
                    break;
                }
            }
        })
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        WATCHER_SPAWNED.set(true);
        Ok(())
    }

    /// Runs one iteration of the integrated event loop.
    ///
    /// Ensures a persistent fd-watcher future is spawned, then blocks in winit's
    /// pump_events. When either a UI event arrives, the libuv fd becomes readable,
    /// or the libuv timeout expires, control returns to JS so Node.js can run uv_run.
    ///
    /// Returns `Exited` when the Slint event loop terminates, `Continue` otherwise.
    pub(crate) fn run_integrated_event_loop_impl(env: &Env) -> napi::Result<ProcessEventsResult> {
        let uv = get_uv(env)?;
        let uv_timeout = uv.timeout_ms();

        if uv_timeout == 0 {
            return pump(Duration::ZERO);
        }

        ensure_watcher_spawned(&uv)?;

        let pump_timeout =
            if uv_timeout < 0 { Duration::MAX } else { Duration::from_millis(uv_timeout as u64) };

        pump(pump_timeout)
    }

    fn pump(timeout: Duration) -> napi::Result<ProcessEventsResult> {
        i_slint_backend_selector::with_platform(|b| {
            b.process_events(timeout, i_slint_core::InternalToken)
        })
        .map_err(|e| napi::Error::from_reason(e.to_string()))
        .map(|result| match result {
            ControlFlow::Break(()) => ProcessEventsResult::Exited,
            ControlFlow::Continue(()) => ProcessEventsResult::Continue,
        })
    }
}

#[cfg(not(unix))]
mod platform {
    use super::super::ProcessEventsResult;
    use napi::Env;

    pub(crate) fn has_integrated_event_loop_impl(_env: &Env) -> bool {
        false
    }

    pub(crate) fn run_integrated_event_loop_impl(_env: &Env) -> napi::Result<ProcessEventsResult> {
        Err(napi::Error::from_reason("integrated event loop not available on this platform"))
    }
}

use napi::Env;

#[napi]
pub fn has_integrated_event_loop(env: Env) -> bool {
    platform::has_integrated_event_loop_impl(&env)
}

#[napi]
pub fn run_integrated_event_loop(env: Env) -> napi::Result<super::ProcessEventsResult> {
    platform::run_integrated_event_loop_impl(&env)
}
