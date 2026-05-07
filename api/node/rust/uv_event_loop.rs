// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Integrated event loop for Node.js, replacing 16ms setInterval polling.
//!
//! On Unix, a persistent future watches libuv's backend fd (epoll/kqueue)
//! for readability via `async-io`.  The wake chain:
//!
//! 1. `Platform::process_events(timeout)` blocks waiting for events.
//! 2. async-io's reactor thread polls the libuv fd.
//! 3. When readable, the reactor wakes the future's `Waker`.
//! 4. `Waker::wake` calls `invoke_from_event_loop`,
//!    posting an event to the platform backend.
//! 5. `run_integrated_event_loop` sees the flag and breaks out of its loop.
//! 6. Control returns to JS; Node's `uv_run` picks up the pending I/O.
//! 7. JS re-enters via `setTimeout(pump, 0)`.
//!
//! The loop in step 5 is key: `process_events` returns on any platform event
//! (mouse moves, repaints, etc.), but we only return to JS when the libuv fd
//! signals readiness or the uv timeout elapses.
//!
//! On Windows/Deno the JS side falls back to 16ms polling.

#[cfg(unix)]
mod platform {
    use super::super::ProcessEventsResult;
    use napi::Env;
    use std::cell::{Cell, OnceCell};
    use std::os::fd::BorrowedFd;
    use std::os::raw::c_int;
    use std::rc::Rc;
    use std::time::Duration;

    type UvBackendFdFn = unsafe extern "C" fn(*mut napi::sys::uv_loop_s) -> c_int;
    type UvBackendTimeoutFn = unsafe extern "C" fn(*mut napi::sys::uv_loop_s) -> c_int;

    #[derive(Clone, Copy)]
    struct UvFunctions {
        backend_fd: UvBackendFdFn,
        backend_timeout: UvBackendTimeoutFn,
        uv_loop: *mut napi::sys::uv_loop_s,
    }

    impl UvFunctions {
        /// Looks up libuv symbols at runtime so this addon also works in
        /// non-libuv runtimes (e.g. Deno), where these symbols are absent.
        fn try_new(env: &Env) -> Option<Self> {
            let lib = libloading::os::unix::Library::this();
            // SAFETY: These symbols are exported by the Node.js binary with
            // stable C signatures matching the type aliases above.
            let backend_fd: UvBackendFdFn =
                *unsafe { lib.get::<UvBackendFdFn>(b"uv_backend_fd").ok()? };
            let backend_timeout: UvBackendTimeoutFn =
                *unsafe { lib.get::<UvBackendTimeoutFn>(b"uv_backend_timeout").ok()? };

            let uv_loop = env.get_uv_event_loop().ok()?;
            if uv_loop.is_null() {
                return None;
            }

            // SAFETY: uv_loop validated non-null above.
            let fd = unsafe { backend_fd(uv_loop) };
            if fd < 0 {
                return None;
            }

            Some(Self { backend_fd, backend_timeout, uv_loop })
        }

        fn fd(&self) -> c_int {
            // SAFETY: uv_loop validated in try_new, owned by Node.js for the process lifetime.
            unsafe { (self.backend_fd)(self.uv_loop) }
        }

        fn timeout_ms(&self) -> c_int {
            // SAFETY: same as fd().
            unsafe { (self.backend_timeout)(self.uv_loop) }
        }
    }

    /// Wrapper that borrows libuv's backend fd without closing it on drop.
    struct UvFdWrapper(c_int);

    impl std::os::fd::AsFd for UvFdWrapper {
        fn as_fd(&self) -> BorrowedFd<'_> {
            // SAFETY: libuv owns this fd for the process lifetime.
            unsafe { BorrowedFd::borrow_raw(self.0) }
        }
    }

    thread_local! {
        static CACHED_UV: OnceCell<Option<UvFunctions>> = const { OnceCell::new() };
        static WATCHER_FLAG: OnceCell<Rc<Cell<bool>>> = const { OnceCell::new() };
    }

    fn get_uv(env: &Env) -> napi::Result<UvFunctions> {
        CACHED_UV.with(|cell| {
            cell.get_or_init(|| UvFunctions::try_new(env))
                .ok_or_else(|| napi::Error::from_reason("integrated event loop not available"))
        })
    }

    pub(crate) fn has_integrated_event_loop_impl(env: &Env) -> bool {
        get_uv(env).is_ok()
    }

    /// Spawns a persistent future watching the libuv fd and returns
    /// the flag it sets when the fd becomes readable.
    fn ensure_watcher_spawned(uv: &UvFunctions) -> napi::Result<Rc<Cell<bool>>> {
        WATCHER_FLAG.with(|cell| {
            if let Some(flag) = cell.get() {
                return Ok(flag.clone());
            }

            // new_nonblocking: the fd is a kqueue/epoll handle, not a real I/O
            // fd — setting non-blocking mode via ioctl fails on macOS kqueue.
            let async_fd = async_io::Async::new_nonblocking(UvFdWrapper(uv.fd())).map_err(|e| {
                napi::Error::from_reason(format!("failed to create async fd watcher: {e}"))
            })?;

            let flag = Rc::new(Cell::new(false));
            let flag_for_future = flag.clone();
            slint_interpreter::spawn_local(async move {
                loop {
                    // readable() completing means FutureRunner::wake called
                    // invoke_from_event_loop, which made process_events return.
                    if async_fd.readable().await.is_err() {
                        break;
                    }
                    flag_for_future.set(true);
                }
            })
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;

            cell.set(flag.clone()).ok();
            Ok(flag)
        })
    }

    /// Keeps processing platform events until the libuv fd becomes readable
    /// (JS has pending work) or the uv timeout elapses (JS timer is due).
    /// This avoids returning to JS unnecessarily on every mouse move or
    /// window event that only concerns the platform backend.
    pub(crate) fn run_integrated_event_loop_impl(env: &Env) -> napi::Result<ProcessEventsResult> {
        let uv = get_uv(env)?;
        let fd_ready = ensure_watcher_spawned(&uv)?;

        loop {
            let uv_timeout = uv.timeout_ms();
            let timeout = if uv_timeout < 0 {
                Duration::MAX
            } else {
                Duration::from_millis(uv_timeout as u64)
            };

            if let ProcessEventsResult::Exited = crate::process_events_with_timeout(timeout)? {
                return Ok(ProcessEventsResult::Exited);
            }

            if uv_timeout == 0 || fd_ready.replace(false) {
                return Ok(ProcessEventsResult::Continue);
            }
        }
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
