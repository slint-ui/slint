// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Native event loop integration for `node-slint`.
//!
//! When running under the `node-slint` runner, winit owns the event loop and
//! libuv is ticked on each iteration via [`CustomApplicationHandler::about_to_wait`].
//! This replaces the 16 ms polling hack used when running under plain `node`.
//!
//! The libuv symbols (`uv_run`, `uv_loop_alive`, `uv_backend_timeout`) are
//! resolved at runtime from the host process so the NAPI addon can also be
//! loaded into runtimes that don't provide libuv (e.g. Deno).

use i_slint_backend_winit::winit::event_loop::ActiveEventLoop;
use i_slint_backend_winit::{CustomApplicationHandler, EventResult};

/// `UV_RUN_NOWAIT`: poll for I/O once but don't block if there are no pending callbacks.
const UV_RUN_NOWAIT: std::ffi::c_int = 2;

type UvRunFn =
    unsafe extern "C" fn(loop_: *mut std::ffi::c_void, mode: std::ffi::c_int) -> std::ffi::c_int;
type UvBackendTimeoutFn =
    unsafe extern "C" fn(loop_: *mut std::ffi::c_void) -> std::ffi::c_int;

/// Resolved libuv function pointers.
struct UvFunctions {
    uv_run: UvRunFn,
    uv_backend_timeout: UvBackendTimeoutFn,
}

impl UvFunctions {
    /// Returns `None` if any symbol is missing (e.g. running under Deno).
    fn load() -> Option<Self> {
        // SAFETY: loading from the current process is always valid.
        unsafe {
            let lib = libloading::os::unix::Library::this();
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

// SAFETY: The uv_loop pointer is only ever accessed from the main thread,
// which is the same thread that runs the winit event loop.
unsafe impl Send for LibuvHandler {}

impl LibuvHandler {
    /// # Safety
    ///
    /// `uv_loop` must be a valid, non-null `uv_loop_t*` that outlives this handler.
    unsafe fn new(uv_loop: *mut std::ffi::c_void, uv: UvFunctions) -> Self {
        Self { uv_loop, uv }
    }
}

impl CustomApplicationHandler for LibuvHandler {
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) -> EventResult {
        unsafe {
            (self.uv.uv_run)(self.uv_loop, UV_RUN_NOWAIT);
        }

        // Tell winit when to wake up for libuv's next timer.
        // -1 means no pending work — let winit and Slint's own timers decide.
        let timeout_ms = unsafe { (self.uv.uv_backend_timeout)(self.uv_loop) };
        if timeout_ms == 0 {
            event_loop
                .set_control_flow(i_slint_backend_winit::winit::event_loop::ControlFlow::Poll);
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

/// Initialize the Slint platform with a [`CustomApplicationHandler`] that ticks libuv.
///
/// Called from the `node-slint` C++ runner *before* the user script is loaded,
/// so that any subsequent Slint API calls from JS find the platform already set up.
#[napi]
pub fn init_platform(uv_loop_ptr: i64) -> napi::Result<()> {
    let uv = UvFunctions::load().ok_or_else(|| {
        napi::Error::from_reason(
            "libuv symbols not found in host process; \
             initPlatform() requires the node-slint runner",
        )
    })?;

    let handler = unsafe { LibuvHandler::new(uv_loop_ptr as *mut std::ffi::c_void, uv) };

    i_slint_backend_selector::api::BackendSelector::new()
        .with_winit_custom_application_handler(handler)
        .select()
        .map_err(|e: i_slint_core::platform::PlatformError| {
            napi::Error::from_reason(e.to_string())
        })
}

/// Run the Slint event loop (blocking).
///
/// Under `node-slint`, winit is the primary event loop and libuv is ticked
/// inside `about_to_wait`. This call blocks until the event loop exits
/// (all windows closed or `quitEventLoop()` called).
#[napi]
pub fn run_event_loop_native() -> napi::Result<()> {
    i_slint_backend_selector::with_platform(|b| b.run_event_loop())
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}
