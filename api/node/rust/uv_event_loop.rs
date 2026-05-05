// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Integrated event loop for Node.js, replacing 16ms setInterval polling.
//!
//! On Unix, a background thread watches libuv's backend fd via poll() and wakes
//! winit when Node.js has pending work. Both threads sleep on kernel primitives
//! when idle — zero CPU waste, near-instant response to UI and I/O events.
//!
//! On other platforms (Windows, Deno), the JS side falls back to 16ms polling
//! which keeps JS responsive between pumps. Windows can't use the fd watcher
//! because IOCP doesn't expose a pollable fd.

#[cfg(unix)]
mod platform {
    use super::super::ProcessEventsResult;
    use napi::Env;
    use std::os::fd::{AsFd, BorrowedFd};
    use std::os::raw::c_int;
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::Duration;

    // libuv function types — exported by the Node.js binary but not in napi-sys,
    // so they're loaded via libloading at runtime.
    type UvBackendFdFn = unsafe extern "C" fn(*mut napi::sys::uv_loop_s) -> c_int;
    type UvBackendTimeoutFn = unsafe extern "C" fn(*mut napi::sys::uv_loop_s) -> c_int;

    /// Cached libuv function pointers and loop handle.
    struct UvFunctions {
        backend_fd: UvBackendFdFn,
        backend_timeout: UvBackendTimeoutFn,
        uv_loop: *mut napi::sys::uv_loop_s,
    }

    // SAFETY: The uv_loop pointer belongs to Node.js and stays valid for the
    // process lifetime. uv_backend_fd() and uv_backend_timeout() read fields set
    // once at loop init and never mutated — this relies on libuv implementation
    // behavior, not an API guarantee, but all major versions work this way.
    unsafe impl Send for UvFunctions {}
    unsafe impl Sync for UvFunctions {}

    impl UvFunctions {
        fn try_new(env: &Env) -> Option<Self> {
            // Load uv_backend_fd and uv_backend_timeout from the host process
            // (the Node.js binary). These aren't in napi-sys.
            let lib = libloading::os::unix::Library::this();
            let backend_fd: UvBackendFdFn =
                *unsafe { lib.get::<UvBackendFdFn>(b"uv_backend_fd").ok()? };
            let backend_timeout: UvBackendTimeoutFn =
                *unsafe { lib.get::<UvBackendTimeoutFn>(b"uv_backend_timeout").ok()? };

            let uv_loop = env.get_uv_event_loop().ok()?;
            if uv_loop.is_null() {
                return None;
            }

            // Check that the backend fd is usable (returns -1 on Windows/IOCP)
            let fd = unsafe { backend_fd(uv_loop) };
            if fd < 0 {
                return None;
            }

            Some(Self { backend_fd, backend_timeout, uv_loop })
        }

        fn fd(&self) -> c_int {
            unsafe { (self.backend_fd)(self.uv_loop) }
        }

        fn timeout_ms(&self) -> c_int {
            unsafe { (self.backend_timeout)(self.uv_loop) }
        }
    }

    /// Persistent watcher thread state, shared between the main thread and
    /// the watcher. The thread is spawned once and reused across iterations.
    struct WatcherState {
        inner: Mutex<WatcherInner>,
        condvar: Condvar,
    }

    struct WatcherInner {
        watching: bool,
        shutdown: bool,
        /// Poll timeout: None means block indefinitely (no libuv timers pending).
        timeout: Option<rustix::event::Timespec>,
    }

    /// Persistent watcher that owns the thread and the cancel pipe.
    /// Drop order: the explicit Drop impl signals shutdown + writes to the
    /// cancel pipe, so by the time Rust drops the remaining fields the thread
    /// has already exited or will exit on its next condvar wake. The thread
    /// owns its own dup'd cancel_r, so closing ours here is safe.
    struct Watcher {
        state: Arc<WatcherState>,
        cancel_w: std::os::fd::OwnedFd,
        cancel_r: std::os::fd::OwnedFd,
        _thread: std::thread::JoinHandle<()>,
    }

    impl Watcher {
        fn new(uv_fd: c_int) -> napi::Result<Self> {
            let (cancel_r, cancel_w) =
                rustix::pipe::pipe_with(rustix::pipe::PipeFlags::NONBLOCK)
                    .map_err(|e| napi::Error::from_reason(format!("failed to create pipe: {e}")))?;

            // Dup the read end so the thread owns its own fd — no raw fd sharing.
            let thread_cancel_r = cancel_r
                .try_clone()
                .map_err(|e| napi::Error::from_reason(format!("failed to dup cancel fd: {e}")))?;

            let state = Arc::new(WatcherState {
                inner: Mutex::new(WatcherInner { watching: false, shutdown: false, timeout: None }),
                condvar: Condvar::new(),
            });

            let thread_state = state.clone();
            let thread = std::thread::Builder::new()
                .name("slint-uv-watcher".into())
                .spawn(move || watcher_thread(thread_state, uv_fd, thread_cancel_r))
                .map_err(|e| napi::Error::from_reason(format!("failed to spawn watcher: {e}")))?;

            Ok(Self { state, cancel_w, cancel_r, _thread: thread })
        }

        /// Tells the watcher thread to start polling, then returns.
        /// Safe to call only when the thread is idle (after cancel() or initial creation).
        fn start(&self, timeout: Option<Duration>) {
            // Drain leftover cancel bytes. Safe: the thread is idle on the condvar.
            let mut buf = [0u8; 16];
            while rustix::io::read(self.cancel_r.as_fd(), &mut buf).unwrap_or(0) > 0 {}

            let mut inner = self.state.inner.lock().unwrap();
            inner.watching = true;
            inner.timeout = timeout.map(|d| rustix::event::Timespec {
                tv_sec: d.as_secs() as _,
                tv_nsec: d.subsec_nanos() as _,
            });
            self.state.condvar.notify_one();
        }

        /// Cancels the current poll and waits for the thread to go idle.
        fn cancel(&self) {
            let _ = rustix::io::write(&self.cancel_w, b"x");
            let mut inner = self.state.inner.lock().unwrap();
            while inner.watching {
                inner = self.state.condvar.wait(inner).unwrap();
            }
        }
    }

    impl Drop for Watcher {
        fn drop(&mut self) {
            {
                let mut inner = self.state.inner.lock().unwrap();
                inner.shutdown = true;
                self.state.condvar.notify_one();
            }
            let _ = rustix::io::write(&self.cancel_w, b"x");
        }
    }

    fn watcher_thread(state: Arc<WatcherState>, uv_fd: c_int, cancel_r: std::os::fd::OwnedFd) {
        // SAFETY: uv_fd is owned by libuv and stays valid for the process lifetime.
        let uv_borrowed = unsafe { BorrowedFd::borrow_raw(uv_fd) };

        loop {
            // Wait until the main thread tells us to start watching
            let timeout = {
                let mut guard = state.inner.lock().unwrap();
                loop {
                    if guard.shutdown {
                        return;
                    }
                    if guard.watching {
                        break;
                    }
                    guard = state.condvar.wait(guard).unwrap();
                }
                guard.timeout
            };

            let mut pfds = [
                rustix::event::PollFd::new(&uv_borrowed, rustix::event::PollFlags::IN),
                rustix::event::PollFd::new(&cancel_r, rustix::event::PollFlags::IN),
            ];
            let _ = rustix::event::poll(&mut pfds, timeout.as_ref());

            // Wake winit only if the cancel pipe didn't fire
            if !pfds[1].revents().contains(rustix::event::PollFlags::IN) {
                let _ = i_slint_core::api::invoke_from_event_loop(|| {});
            }

            // Signal the main thread that we're idle
            let mut guard = state.inner.lock().unwrap();
            guard.watching = false;
            state.condvar.notify_one();
        }
    }

    thread_local! {
        static CACHED_UV: std::cell::RefCell<Option<UvFunctions>> = const { std::cell::RefCell::new(None) };
        static CACHED_WATCHER: std::cell::RefCell<Option<Watcher>> = const { std::cell::RefCell::new(None) };
    }

    fn with_uv(
        env: &Env,
        f: impl FnOnce(&UvFunctions) -> napi::Result<ProcessEventsResult>,
    ) -> napi::Result<ProcessEventsResult> {
        CACHED_UV.with(|cell| {
            let mut cached = cell.borrow_mut();
            if cached.is_none() {
                *cached = UvFunctions::try_new(env);
            }
            match cached.as_ref() {
                Some(uv) => f(uv),
                None => Err(napi::Error::from_reason("integrated event loop not available")),
            }
        })
    }

    fn with_watcher(
        uv_fd: c_int,
        f: impl FnOnce(&Watcher) -> napi::Result<ProcessEventsResult>,
    ) -> napi::Result<ProcessEventsResult> {
        CACHED_WATCHER.with(|cell| {
            let mut cached = cell.borrow_mut();
            if cached.is_none() {
                *cached = Some(Watcher::new(uv_fd)?);
            }
            f(cached.as_ref().unwrap())
        })
    }

    /// Returns true if the integrated event loop is available on this platform/runtime.
    pub(crate) fn has_integrated_event_loop_impl(env: &Env) -> bool {
        CACHED_UV.with(|cell| {
            let mut cached = cell.borrow_mut();
            if cached.is_none() {
                *cached = UvFunctions::try_new(env);
            }
            cached.is_some()
        })
    }

    /// Runs one iteration of the integrated event loop.
    ///
    /// Blocks in winit's pump_events until a UI event arrives,
    /// the libuv backend timeout expires,
    /// or the background watcher detects I/O on the libuv fd.
    /// Always returns to JS afterwards so Node.js can run uv_run
    /// (fire timers, complete I/O, execute callbacks).
    ///
    /// Returns `Exited` when the Slint event loop terminates, `Continue` otherwise.
    pub(crate) fn run_integrated_event_loop_impl(env: &Env) -> napi::Result<ProcessEventsResult> {
        with_uv(env, run_integrated_event_loop_inner)
    }

    fn run_integrated_event_loop_inner(uv: &UvFunctions) -> napi::Result<ProcessEventsResult> {
        let uv_timeout = uv.timeout_ms();

        if uv_timeout == 0 {
            return pump_once(Duration::ZERO);
        }

        // When uv_backend_timeout returns -1 (no timers), block indefinitely:
        // the fd watcher wakes winit on libuv I/O, winit handles UI events.
        let (poll_timeout, pump_timeout) = if uv_timeout < 0 {
            (None, Duration::MAX)
        } else {
            let d = Duration::from_millis(uv_timeout as u64);
            (Some(d), d)
        };

        with_watcher(uv.fd(), |watcher| {
            watcher.start(poll_timeout);

            let pump_result = i_slint_backend_selector::with_platform(|b| {
                b.process_events(pump_timeout, i_slint_core::InternalToken)
            })
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;

            watcher.cancel();

            match pump_result {
                core::ops::ControlFlow::Break(()) => Ok(ProcessEventsResult::Exited),
                core::ops::ControlFlow::Continue(()) => Ok(ProcessEventsResult::Continue),
            }
        })
    }

    /// Non-blocking pump: process all pending winit events and return.
    fn pump_once(timeout: Duration) -> napi::Result<ProcessEventsResult> {
        i_slint_backend_selector::with_platform(|b| {
            b.process_events(timeout, i_slint_core::InternalToken)
        })
        .map_err(|e| napi::Error::from_reason(e.to_string()))
        .map(|result| match result {
            core::ops::ControlFlow::Break(()) => ProcessEventsResult::Exited,
            core::ops::ControlFlow::Continue(()) => ProcessEventsResult::Continue,
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
