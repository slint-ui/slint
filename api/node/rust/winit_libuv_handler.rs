// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! winit-driven libuv integration for the `node-slint` runner.
//!
//! Used on Windows where `uv_backend_fd()` returns -1 and the
//! `uv_prepare`-based path in [`crate::uv_event_loop`] can't be set up.
//! Winit owns the event loop and libuv is ticked once per iteration from
//! [`CustomApplicationHandler::about_to_wait`].
//!
//! On Linux and macOS the `uv_prepare` path is preferred — it leaves the
//! integration backend-agnostic.

use i_slint_backend_winit::winit::event_loop::ActiveEventLoop;
use i_slint_backend_winit::{CustomApplicationHandler, EventResult};

/// `UV_RUN_NOWAIT`: poll for I/O once but don't block if there are no pending callbacks.
const UV_RUN_NOWAIT: std::ffi::c_int = 2;

type UvRunFn =
    unsafe extern "C" fn(loop_: *mut std::ffi::c_void, mode: std::ffi::c_int) -> std::ffi::c_int;
type UvBackendTimeoutFn =
    unsafe extern "C" fn(loop_: *mut std::ffi::c_void) -> std::ffi::c_int;

struct UvFunctions {
    uv_run: UvRunFn,
    uv_backend_timeout: UvBackendTimeoutFn,
}

impl UvFunctions {
    fn load() -> Option<Self> {
        // SAFETY: resolving from the current process is always valid.
        unsafe {
            #[cfg(unix)]
            let lib = libloading::os::unix::Library::this();
            #[cfg(windows)]
            let lib = libloading::os::windows::Library::this().ok()?;
            let uv_run = *lib.get::<UvRunFn>(b"uv_run\0").ok()?;
            let uv_backend_timeout =
                *lib.get::<UvBackendTimeoutFn>(b"uv_backend_timeout\0").ok()?;
            Some(Self { uv_run, uv_backend_timeout })
        }
    }
}

struct LibuvHandler {
    uv_loop: *mut std::ffi::c_void,
    uv: UvFunctions,
}

// SAFETY: uv_loop is only ever accessed from the main thread, which is also
// the thread that drives the winit event loop and owns the V8 isolate.
unsafe impl Send for LibuvHandler {}

impl LibuvHandler {
    /// # Safety
    /// `uv_loop` must be a valid, non-null `uv_loop_t*` that outlives this handler.
    unsafe fn new(uv_loop: *mut std::ffi::c_void, uv: UvFunctions) -> Self {
        Self { uv_loop, uv }
    }
}

impl CustomApplicationHandler for LibuvHandler {
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) -> EventResult {
        // Tick libuv non-blocking before winit waits for input.
        unsafe { (self.uv.uv_run)(self.uv_loop, UV_RUN_NOWAIT) };

        // Wake winit when libuv's next timer is due so JS timers fire on time.
        // -1 means no pending libuv work — leave winit's own ControlFlow alone.
        let timeout_ms = unsafe { (self.uv.uv_backend_timeout)(self.uv_loop) };
        if timeout_ms == 0 {
            event_loop.set_control_flow(
                i_slint_backend_winit::winit::event_loop::ControlFlow::Poll,
            );
        } else if timeout_ms > 0 {
            event_loop.set_control_flow(
                i_slint_backend_winit::winit::event_loop::ControlFlow::wait_duration(
                    std::time::Duration::from_millis(timeout_ms as u64),
                ),
            );
        }

        EventResult::Propagate
    }
}

/// Register a [`CustomApplicationHandler`] that pumps libuv on every
/// winit iteration.
///
/// Called from the `node-slint` runner *before* the user script is loaded.
/// On platforms where the `uv_prepare`-based path works (Linux/macOS) this
/// is unnecessary but harmless — the prepare callback drives slint and the
/// custom handler simply runs `uv_run(NOWAIT)` redundantly on each winit
/// wakeup.
///
/// Returns an error if no winit backend is selected or the platform was
/// already initialized.
#[napi]
pub fn register_winit_libuv_handler(uv_loop_ptr: i64) -> napi::Result<()> {
    if uv_loop_ptr == 0 {
        return Err(napi::Error::from_reason("uv_loop_ptr is null"));
    }

    let uv = UvFunctions::load().ok_or_else(|| {
        napi::Error::from_reason(
            "libuv symbols not found in host process; \
             register_winit_libuv_handler() requires the node-slint runner",
        )
    })?;

    let handler = unsafe { LibuvHandler::new(uv_loop_ptr as *mut std::ffi::c_void, uv) };

    i_slint_backend_selector::api::BackendSelector::new()
        .with_winit_custom_application_handler(handler)
        .select()
        .map_err(|e: i_slint_core::platform::PlatformError| {
            napi::Error::from_reason(e.to_string())
        })?;

    crate::WINIT_LIBUV_HANDLER_REGISTERED.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

/// True if [`register_winit_libuv_handler`] succeeded.
///
/// Used by JS to decide whether to take the blocking
/// [`run_event_loop_blocking`] path or fall back to polling.
#[napi]
pub fn has_winit_libuv_integration() -> bool {
    crate::WINIT_LIBUV_HANDLER_REGISTERED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Run the Slint event loop synchronously, blocking until the loop exits.
///
/// libuv is ticked from `about_to_wait` via the handler registered by
/// [`register_winit_libuv_handler`].
#[napi]
pub fn run_event_loop_blocking() -> napi::Result<()> {
    i_slint_backend_selector::with_platform(|b| b.run_event_loop())
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}
