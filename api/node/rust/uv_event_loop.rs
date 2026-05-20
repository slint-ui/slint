// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore epoll libuv libuv's nonblocking unref
//! Integrated event loop for Node.js.
//!
//! Replaces the 16ms `setInterval` polling with a `uv_prepare_t`
//! callback that pumps Slint events on every libuv iteration.
//!
//! Prepare callbacks run after the timer phase but before I/O poll,
//! so `uv_backend_timeout()` returns an accurate next-timer deadline.
//!
//! On Windows/Deno the JS side falls back to 16ms polling.

/// Safe wrappers around the libuv C API.
/// Symbols are resolved at runtime so the addon also loads in
/// non-libuv runtimes (e.g. Deno).
#[cfg(unix)]
mod uv {
    use napi::Env;
    use std::os::fd::BorrowedFd;
    use std::os::raw::c_int;

    type UvHandleSizeFn = unsafe extern "C" fn(c_int) -> usize;
    type UvBackendFdFn = unsafe extern "C" fn(*mut napi::sys::uv_loop_s) -> c_int;
    type UvBackendTimeoutFn = unsafe extern "C" fn(*mut napi::sys::uv_loop_s) -> c_int;
    type UvPrepareInitFn = unsafe extern "C" fn(*mut napi::sys::uv_loop_s, *mut u8) -> c_int;
    type UvPrepareStartFn = unsafe extern "C" fn(*mut u8, unsafe extern "C" fn(*mut u8)) -> c_int;
    type UvPrepareStopFn = unsafe extern "C" fn(*mut u8) -> c_int;
    type UvAsyncInitFn = unsafe extern "C" fn(
        *mut napi::sys::uv_loop_s,
        *mut u8,
        unsafe extern "C" fn(*mut u8),
    ) -> c_int;
    type UvAsyncSendFn = unsafe extern "C" fn(*mut u8) -> c_int;
    type UvUnrefFn = unsafe extern "C" fn(*mut u8);
    type UvCloseFn = unsafe extern "C" fn(*mut u8, Option<unsafe extern "C" fn(*mut u8)>);
    type UvUpdateTimeFn = unsafe extern "C" fn(*mut napi::sys::uv_loop_s);

    /// Resolved libuv function pointers and loop handle.
    /// Valid for the process lifetime (Node.js owns the loop).
    #[derive(Clone, Copy)]
    pub(super) struct Functions {
        backend_fd: UvBackendFdFn,
        backend_timeout: UvBackendTimeoutFn,
        prepare_init: UvPrepareInitFn,
        prepare_start: UvPrepareStartFn,
        prepare_stop: UvPrepareStopFn,
        async_init: UvAsyncInitFn,
        async_send: UvAsyncSendFn,
        uv_unref: UvUnrefFn,
        uv_close: UvCloseFn,
        update_time: UvUpdateTimeFn,
        uv_loop: *mut napi::sys::uv_loop_s,
        prepare_layout: std::alloc::Layout,
        async_layout: std::alloc::Layout,
    }

    impl Functions {
        /// Resolve libuv symbols from the host process.
        /// Returns `None` if any symbol is missing or the fd is invalid.
        pub(super) fn try_new(env: &Env) -> Option<Self> {
            // SAFETY: loading from the current process is always valid.
            let lib = libloading::os::unix::Library::this();
            // SAFETY: stable C signatures exported by the Node.js binary.
            let handle_size = *unsafe { lib.get::<UvHandleSizeFn>(b"uv_handle_size").ok()? };
            let backend_fd = *unsafe { lib.get::<UvBackendFdFn>(b"uv_backend_fd").ok()? };
            let backend_timeout =
                *unsafe { lib.get::<UvBackendTimeoutFn>(b"uv_backend_timeout").ok()? };
            let prepare_init = *unsafe { lib.get::<UvPrepareInitFn>(b"uv_prepare_init").ok()? };
            let prepare_start = *unsafe { lib.get::<UvPrepareStartFn>(b"uv_prepare_start").ok()? };
            let prepare_stop = *unsafe { lib.get::<UvPrepareStopFn>(b"uv_prepare_stop").ok()? };
            let async_init = *unsafe { lib.get::<UvAsyncInitFn>(b"uv_async_init").ok()? };
            let async_send = *unsafe { lib.get::<UvAsyncSendFn>(b"uv_async_send").ok()? };
            let uv_unref = *unsafe { lib.get::<UvUnrefFn>(b"uv_unref").ok()? };
            let uv_close = *unsafe { lib.get::<UvCloseFn>(b"uv_close").ok()? };
            let update_time = *unsafe { lib.get::<UvUpdateTimeFn>(b"uv_update_time").ok()? };

            let uv_loop = env.get_uv_event_loop().ok()?;
            if uv_loop.is_null() {
                return None;
            }

            // SAFETY: uv_loop is non-null.
            let fd = unsafe { backend_fd(uv_loop) };
            if fd < 0 {
                return None;
            }

            /// `UV_PREPARE` value from the `uv_handle_type` enum.
            const UV_PREPARE: c_int = 9;
            /// `UV_ASYNC` value from the `uv_handle_type` enum.
            const UV_ASYNC: c_int = 1;
            let prepare_size = unsafe { handle_size(UV_PREPARE) };
            let prepare_layout = std::alloc::Layout::from_size_align(prepare_size, 8).ok()?;
            let async_size = unsafe { handle_size(UV_ASYNC) };
            let async_layout = std::alloc::Layout::from_size_align(async_size, 8).ok()?;

            Some(Self {
                backend_fd,
                backend_timeout,
                prepare_init,
                prepare_start,
                prepare_stop,
                async_init,
                async_send,
                uv_unref,
                uv_close,
                update_time,
                uv_loop,
                prepare_layout,
                async_layout,
            })
        }

        /// Backend fd (epoll/kqueue) for the libuv event loop.
        pub(super) fn backend_fd(&self) -> c_int {
            // SAFETY: uv_loop is valid for the process lifetime.
            unsafe { (self.backend_fd)(self.uv_loop) }
        }

        /// Milliseconds until the next libuv timer, or -1 if none.
        pub(super) fn backend_timeout_ms(&self) -> c_int {
            // SAFETY: same as backend_fd.
            unsafe { (self.backend_timeout)(self.uv_loop) }
        }

        /// Refresh libuv's cached clock so `backend_timeout_ms` returns
        /// an up-to-date value after blocking in `process_events`.
        pub(super) fn update_time(&self) {
            unsafe { (self.update_time)(self.uv_loop) }
        }
    }

    /// Heap-allocated `uv_prepare_t` handle.
    ///
    /// Heap-allocated (not embedded in a struct) because `uv_close`
    /// is async — libuv accesses the handle after close returns.
    /// The close callback deallocates the buffer.
    pub(super) struct PrepareHandle {
        ptr: *mut u8,
        fns: Functions,
    }

    impl PrepareHandle {
        /// Allocate and initialize a prepare handle on the loop.
        pub(super) fn new(fns: Functions) -> napi::Result<Self> {
            let layout = fns.prepare_layout;
            let ptr = unsafe { std::alloc::alloc(layout) };
            assert!(!ptr.is_null(), "failed to allocate uv_prepare_t");

            let rc = unsafe { (fns.prepare_init)(fns.uv_loop, ptr) };
            if rc != 0 {
                unsafe { std::alloc::dealloc(ptr, layout) };
                return Err(napi::Error::from_reason("uv_prepare_init failed"));
            }

            Ok(Self { ptr, fns })
        }

        /// Start the prepare handle with the given callback.
        /// The callback is stored in the handle's `data` field and
        /// invoked from an internal `extern "C"` trampoline.
        pub(super) fn start(&mut self, cb: fn()) -> napi::Result<()> {
            // SAFETY: libuv handles have `void* data` at offset 0.
            unsafe { *(self.ptr as *mut usize) = cb as usize };

            unsafe extern "C" fn trampoline(handle: *mut u8) {
                // SAFETY: `data` was set to a fn() pointer in start().
                let cb: fn() = unsafe { std::mem::transmute(*(handle as *const usize)) };
                cb();
            }

            let rc = unsafe { (self.fns.prepare_start)(self.ptr, trampoline) };
            if rc != 0 {
                return Err(napi::Error::from_reason("uv_prepare_start failed"));
            }
            Ok(())
        }

        /// Milliseconds until the next libuv timer, or -1 if none.
        pub(super) fn backend_timeout_ms(&self) -> std::os::raw::c_int {
            self.fns.backend_timeout_ms()
        }

        /// Refresh libuv's cached clock.
        pub(super) fn update_time(&self) {
            self.fns.update_time()
        }

        /// Stop the prepare handle.
        pub(super) fn stop(&self) {
            unsafe { (self.fns.prepare_stop)(self.ptr) };
        }
    }

    /// Close a libuv handle and schedule deallocation.
    ///
    /// Must go through `uv_close` — libuv may still reference the
    /// handle after the stop call returns.  The layout size is stashed
    /// in the handle's `data` field so `close_cb` can recover it.
    ///
    /// # Safety
    /// `ptr` must be a valid, initialized libuv handle allocated with
    /// the given `layout`, and `uv_close` must be the matching fn ptr.
    unsafe fn uv_close_and_dealloc(ptr: *mut u8, layout: std::alloc::Layout, uv_close: UvCloseFn) {
        unsafe extern "C" fn close_cb(handle: *mut u8) {
            let size = unsafe { *(handle as *const usize) };
            let layout = unsafe { std::alloc::Layout::from_size_align_unchecked(size, 8) };
            unsafe { std::alloc::dealloc(handle, layout) };
        }
        unsafe {
            *(ptr as *mut usize) = layout.size();
            uv_close(ptr, Some(close_cb));
        }
    }

    impl Drop for PrepareHandle {
        fn drop(&mut self) {
            if !self.ptr.is_null() {
                unsafe {
                    uv_close_and_dealloc(self.ptr, self.fns.prepare_layout, self.fns.uv_close)
                };
            }
        }
    }

    /// Heap-allocated `uv_async_t` handle.
    ///
    /// Wraps a libuv async handle that invokes a C callback when
    /// `send()` is called.  The handle is unref'd so it doesn't
    /// keep the Node.js process alive on its own.
    pub(super) struct AsyncHandle {
        ptr: *mut u8,
        fns: Functions,
    }

    impl AsyncHandle {
        pub(super) fn new(fns: Functions, cb: unsafe extern "C" fn(*mut u8)) -> napi::Result<Self> {
            let layout = fns.async_layout;
            let ptr = unsafe { std::alloc::alloc(layout) };
            assert!(!ptr.is_null(), "failed to allocate uv_async_t");

            let rc = unsafe { (fns.async_init)(fns.uv_loop, ptr, cb) };
            if rc != 0 {
                unsafe { std::alloc::dealloc(ptr, layout) };
                return Err(napi::Error::from_reason("uv_async_init failed"));
            }

            // Don't let this handle keep Node.js alive.
            unsafe { (fns.uv_unref)(ptr) };

            Ok(Self { ptr, fns })
        }

        /// Signal the async handle, waking libuv's event loop.
        pub(super) fn send(&self) {
            unsafe { (self.fns.async_send)(self.ptr) };
        }
    }

    impl Drop for AsyncHandle {
        fn drop(&mut self) {
            if !self.ptr.is_null() {
                unsafe { uv_close_and_dealloc(self.ptr, self.fns.async_layout, self.fns.uv_close) };
            }
        }
    }

    /// Borrows libuv's backend fd without closing it on drop.
    pub(super) struct FdWrapper(pub(super) c_int);

    impl std::os::fd::AsFd for FdWrapper {
        fn as_fd(&self) -> BorrowedFd<'_> {
            // SAFETY: libuv owns this fd for the process lifetime.
            unsafe { BorrowedFd::borrow_raw(self.0) }
        }
    }
}

#[cfg(unix)]
mod platform {
    use super::super::ProcessEventsResult;
    use super::uv;
    use napi::Env;
    use napi::bindgen_prelude::*;
    use std::cell::{Cell, OnceCell, RefCell};
    use std::rc::Rc;
    use std::time::Duration;

    struct PrepareState {
        fd_ready: Rc<Cell<bool>>,
        prepare_handle: uv::PrepareHandle,
        async_handle: uv::AsyncHandle,
        env: Env,
        on_exit: FunctionRef<crate::DynArgs, Unknown<'static>>,
    }

    struct ThreadState {
        uv: OnceCell<Option<uv::Functions>>,
        watcher_flag: OnceCell<Rc<Cell<bool>>>,
        prepare: RefCell<Option<Box<PrepareState>>>,
        quit_requested: Cell<bool>,
    }

    thread_local! {
        static TLS: ThreadState = const {
            ThreadState {
                uv: OnceCell::new(),
                watcher_flag: OnceCell::new(),
                prepare: RefCell::new(None),
                quit_requested: Cell::new(false),
            }
        };
    }

    fn get_uv(env: &Env) -> napi::Result<uv::Functions> {
        TLS.with(|tls| {
            tls.uv
                .get_or_init(|| uv::Functions::try_new(env))
                .ok_or_else(|| napi::Error::from_reason("integrated event loop isn't available"))
        })
    }

    pub(crate) fn has_integrated_event_loop_impl(env: &Env) -> bool {
        get_uv(env).is_ok()
    }

    /// Request the integrated event loop to exit.
    pub(crate) fn request_quit() {
        TLS.with(|tls| tls.quit_requested.set(true));
    }

    /// Spawn a future that watches the libuv fd for readability and
    /// sets the returned flag when I/O arrives.
    fn ensure_watcher_spawned(uv: &uv::Functions) -> napi::Result<Rc<Cell<bool>>> {
        TLS.with(|tls| {
            if let Some(flag) = tls.watcher_flag.get() {
                return Ok(flag.clone());
            }

            // new_nonblocking: ioctl to set non-blocking fails on macOS kqueue fds.
            let async_fd = async_io::Async::new_nonblocking(uv::FdWrapper(uv.backend_fd()))
                .map_err(|e| {
                    napi::Error::from_reason(format!("failed to create async fd watcher: {e}"))
                })?;

            let flag = Rc::new(Cell::new(false));
            let flag_for_future = flag.clone();
            slint_interpreter::spawn_local(async move {
                loop {
                    if async_fd.readable().await.is_err() {
                        break;
                    }
                    flag_for_future.set(true);
                }
            })
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;

            tls.watcher_flag.set(flag.clone()).ok();
            Ok(flag)
        })
    }

    /// Call the JS on_exit callback.
    /// Uses `run_in_scope` because the prepare callback has no HandleScope.
    /// Silently skipped if V8 is already torn down.
    fn call_on_exit(state: &PrepareState) {
        let _ = state.env.run_in_scope(|| {
            let f = state.on_exit.borrow_back(&state.env)?;
            f.call(crate::DynArgs(vec![]))
        });
    }

    /// Libuv prepare callback — runs after timers, before I/O poll.
    /// Wrapped in `run_in_scope` to provide V8 with a HandleScope,
    /// since Slint event processing may invoke JS callbacks.
    fn prepare_cb() {
        TLS.with(|tls| {
            let borrow = tls.prepare.borrow();
            let Some(state) = borrow.as_deref() else { return };
            let env = state.env;
            let result = env
                .run_in_scope(|| Ok(process_slint_events(state)))
                .unwrap_or(ProcessEventsResult::Exited);
            drop(borrow);
            if matches!(result, ProcessEventsResult::Exited) {
                if let Some(state) = tls.prepare.borrow_mut().take() {
                    cleanup_on_exit(state);
                }
            }
        });
    }

    /// Process Slint events, blocking up to `uv_backend_timeout()` ms.
    fn process_slint_events(state: &PrepareState) -> ProcessEventsResult {
        loop {
            let uv_timeout = state.prepare_handle.backend_timeout_ms();
            let timeout =
                if uv_timeout < 0 { None } else { Some(Duration::from_millis(uv_timeout as u64)) };

            match crate::process_events_with_timeout(timeout) {
                Ok(ProcessEventsResult::Exited) | Err(_) => return ProcessEventsResult::Exited,
                Ok(ProcessEventsResult::Continue) => {}
            }

            if TLS.with(|tls| tls.quit_requested.replace(false)) {
                return ProcessEventsResult::Exited;
            }

            state.prepare_handle.update_time();
            if state.fd_ready.replace(false) || uv_timeout == 0 {
                // Wake libuv so it doesn't sleep in its I/O poll and
                // the prepare callback fires again on the next iteration.
                state.async_handle.send();
                return ProcessEventsResult::Continue;
            }
        }
    }

    /// Stop the prepare handle and notify JS.
    /// The handle is deallocated via `uv_close` when `state` drops.
    fn cleanup_on_exit(state: Box<PrepareState>) {
        state.prepare_handle.stop();
        call_on_exit(&state);
    }

    /// Register a `uv_prepare_t` that pumps Slint events on every
    /// libuv iteration.
    /// Returns immediately; calls `on_exit` when the loop terminates.
    pub(crate) fn start_integrated_event_loop_impl(
        env: &Env,
        on_exit: crate::DynFunction<'_>,
    ) -> napi::Result<()> {
        let uv = get_uv(env)?;

        // Check that the backend supports process_events
        // (the testing backend doesn't).
        crate::process_events_with_timeout(Some(Duration::ZERO))?;

        let fd_ready = ensure_watcher_spawned(&uv)?;
        let on_exit = on_exit.create_ref()?;
        let mut prepare_handle = uv::PrepareHandle::new(uv)?;
        prepare_handle.start(prepare_cb)?;
        unsafe extern "C" fn noop_cb(_handle: *mut u8) {}
        let async_handle = uv::AsyncHandle::new(uv, noop_cb)?;

        let state =
            Box::new(PrepareState { fd_ready, prepare_handle, async_handle, env: *env, on_exit });

        // Ref'd handle keeps Node.js alive until on_exit fires.
        // Clear stale quit request from a previous run.
        TLS.with(|tls| {
            tls.quit_requested.set(false);
            *tls.prepare.borrow_mut() = Some(state);
        });

        Ok(())
    }
}

#[cfg(not(unix))]
mod platform {
    use napi::Env;

    pub(crate) fn has_integrated_event_loop_impl(_env: &Env) -> bool {
        false
    }

    pub(crate) fn start_integrated_event_loop_impl(
        _env: &Env,
        _on_exit: crate::DynFunction<'_>,
    ) -> napi::Result<()> {
        Err(napi::Error::from_reason("integrated event loop isn't available on this platform"))
    }

    pub(crate) fn request_quit() {}
}

use napi::Env;

pub(crate) use platform::request_quit;

#[napi]
pub fn has_integrated_event_loop(env: Env) -> bool {
    platform::has_integrated_event_loop_impl(&env)
}

#[napi]
pub fn start_integrated_event_loop(env: &Env, on_exit: crate::DynFunction<'_>) -> napi::Result<()> {
    platform::start_integrated_event_loop_impl(env, on_exit)
}
