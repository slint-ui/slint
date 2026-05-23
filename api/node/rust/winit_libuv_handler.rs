// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Slint-driven event loop for the `node-slint` runner.
//!
//! The runner starts winit's event loop *before* any JavaScript runs.
//! Each iteration's `about_to_wait` callback:
//!
//! 1. On the first call, runs the bootstrap JS via `node::LoadEnvironment`.
//!    The bootstrap patches `Module._load` to redirect `slint-ui` to the
//!    in-process napi module and `import()`s the user script.
//! 2. Ticks libuv with `uv_run(NOWAIT)`.  All JavaScript work — user
//!    script execution, timer callbacks, promise resolutions, …  — runs
//!    inside this call.  Nothing nested calls `slint::run_event_loop`,
//!    so `uv_run` is never re-entered.
//! 3. Fires any pending JS quit callbacks if `quitEventLoop()` set the
//!    flag.
//! 4. Exits winit when libuv has no more work.
//!
//! `slint.runEventLoop()` in the TS layer returns a Promise that resolves
//! through the quit-callback hook, so user code awaiting it resumes the
//! same way as under the existing approach D path.

use i_slint_backend_winit::winit::event_loop::{ActiveEventLoop, ControlFlow};
use i_slint_backend_winit::{CustomApplicationHandler, EventResult};
use napi::Env;
use napi::bindgen_prelude::*;
use std::cell::{Cell, RefCell};
use std::ffi::{CString, c_char, c_int};
use std::time::Duration;

const UV_RUN_NOWAIT: c_int = 2;

type UvRunFn = unsafe extern "C" fn(loop_: *mut std::ffi::c_void, mode: c_int) -> c_int;
type UvBackendTimeoutFn = unsafe extern "C" fn(loop_: *mut std::ffi::c_void) -> c_int;
type UvLoopAliveFn = unsafe extern "C" fn(loop_: *mut std::ffi::c_void) -> c_int;

struct UvFunctions {
    uv_run: UvRunFn,
    uv_backend_timeout: UvBackendTimeoutFn,
    uv_loop_alive: UvLoopAliveFn,
}

impl UvFunctions {
    fn load() -> Option<Self> {
        // SAFETY: resolving from the current process is always valid.
        unsafe {
            #[cfg(unix)]
            let lib = libloading::os::unix::Library::this();
            #[cfg(windows)]
            let lib = libloading::os::windows::Library::this().ok()?;
            Some(Self {
                uv_run: *lib.get::<UvRunFn>(b"uv_run\0").ok()?,
                uv_backend_timeout: *lib
                    .get::<UvBackendTimeoutFn>(b"uv_backend_timeout\0")
                    .ok()?,
                uv_loop_alive: *lib.get::<UvLoopAliveFn>(b"uv_loop_alive\0").ok()?,
            })
        }
    }
}

unsafe extern "C" {
    /// Implemented by the `node-slint` runner's C++ shim.  Calls
    /// `node::LoadEnvironment(env, script)` on the V8 isolate the
    /// runner is currently inside.
    fn node_slint_load_environment(env: *mut std::ffi::c_void, script: *const c_char);
}

struct QuitCb {
    env: Env,
    cb: FunctionRef<crate::DynArgs, Unknown<'static>>,
}

struct State {
    uv_loop: *mut std::ffi::c_void,
    node_env: *mut std::ffi::c_void,
    bootstrap_js: CString,
    uv: UvFunctions,

    bootstrap_done: Cell<bool>,
    /// JS called `quitEventLoop()` and we haven't drained the callbacks yet.
    quit_pending: Cell<bool>,
    /// Quit callbacks have already fired; future `quitEventLoop` calls are no-ops.
    quit_complete: Cell<bool>,
    /// JS asked for a `runEventLoop()` Promise — exit only when quit is
    /// signalled, not when libuv merely drains.
    run_event_loop_active: Cell<bool>,
    quit_callbacks: RefCell<Vec<QuitCb>>,
}

thread_local! {
    static STATE: RefCell<Option<&'static State>> = const { RefCell::new(None) };
}

fn with_state<R>(f: impl FnOnce(&State) -> R) -> Option<R> {
    STATE.with(|s| s.borrow().map(f))
}

/// True if the `node-slint` runner has installed itself.
#[napi]
pub fn is_node_slint() -> bool {
    STATE.with(|s| s.borrow().is_some())
}

/// Register a JS callback to fire when the slint event loop is asked to
/// quit.  Called from the TS `runEventLoop()` to resolve its Promise.
#[napi]
pub fn node_slint_register_quit_callback(
    env: Env,
    callback: crate::DynFunction<'_>,
) -> napi::Result<()> {
    let cb = callback.create_ref()?;
    with_state(|state| {
        state.run_event_loop_active.set(true);
        state.quit_callbacks.borrow_mut().push(QuitCb { env, cb });
    })
    .ok_or_else(|| napi::Error::from_reason("node-slint runner is not active"))
}

/// Mark the slint event loop for exit.  Used in place of
/// `slint::quit_event_loop()` from the JS-facing `quitEventLoop` when
/// running under node-slint, so we can resolve pending Promises in JS
/// before winit actually exits.
pub(crate) fn request_quit() {
    let _ = with_state(|state| state.quit_pending.set(true));
}

struct LibuvHandler;

// SAFETY: the handler is created and consumed on the same thread that
// owns the V8 isolate and the libuv loop.  Slint's winit backend may
// require Send to move it into the event loop builder, but it is not
// actually used from another thread.
unsafe impl Send for LibuvHandler {}

impl CustomApplicationHandler for LibuvHandler {
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) -> EventResult {
        let Some(state) = STATE.with(|s| *s.borrow()) else {
            return EventResult::Propagate;
        };

        // First iteration: kick off Node's main bootstrap.  It runs
        // synchronously until its first await, then returns; the user
        // script's remaining work flows through the uv_run below.
        if !state.bootstrap_done.replace(true) {
            unsafe {
                node_slint_load_environment(state.node_env, state.bootstrap_js.as_ptr())
            };
        }

        // Pump libuv non-blocking.  JS callbacks (timers, promise
        // resolutions, module loads) run here on this same thread.
        unsafe { (state.uv.uv_run)(state.uv_loop, UV_RUN_NOWAIT) };

        // Fire JS quit callbacks once if quitEventLoop() was called.
        if state.quit_pending.replace(false) && !state.quit_complete.replace(true) {
            fire_quit_callbacks(state);
        }

        // Decide whether we're done.  Two conditions:
        //   * the JS quit completed and libuv has drained anything that
        //     ran after Promise resolution (console.log("after"), etc.);
        //   * the user never asked for runEventLoop and libuv is idle —
        //     same exit semantics as plain `node`.
        let alive = unsafe { (state.uv.uv_loop_alive)(state.uv_loop) };
        if alive == 0
            && (state.quit_complete.get() || !state.run_event_loop_active.get())
        {
            event_loop.exit();
            return EventResult::Propagate;
        }

        // Decide how long winit may sleep before its next wakeup.
        let timeout_ms = unsafe { (state.uv.uv_backend_timeout)(state.uv_loop) };
        if state.quit_complete.get() || timeout_ms == 0 {
            event_loop.set_control_flow(ControlFlow::Poll);
        } else if timeout_ms > 0 {
            event_loop.set_control_flow(ControlFlow::wait_duration(
                Duration::from_millis(timeout_ms as u64),
            ));
        }

        EventResult::Propagate
    }
}

fn fire_quit_callbacks(state: &State) {
    let drained: Vec<QuitCb> = state.quit_callbacks.borrow_mut().drain(..).collect();
    for QuitCb { env, cb } in drained {
        if let Ok(f) = cb.borrow_back(&env) {
            let _ = f.call(crate::DynArgs(vec![]));
        }
    }
}

/// Entry point invoked by the `node-slint` runner.  Installs the winit
/// handler, stashes the bootstrap script and libnode handles in
/// thread-local state, and runs slint's event loop until it exits.
///
/// Returns when winit's `run_event_loop` returns.  The runner is then
/// responsible for tearing Node down.
pub fn start_node_slint_event_loop(
    uv_loop_ptr: i64,
    node_env_ptr: i64,
    bootstrap_js: String,
) -> napi::Result<()> {
    if uv_loop_ptr == 0 || node_env_ptr == 0 {
        return Err(napi::Error::from_reason("uv_loop_ptr or env_ptr is null"));
    }
    let uv = UvFunctions::load().ok_or_else(|| {
        napi::Error::from_reason("libuv symbols not found in host process")
    })?;
    let bootstrap_js = CString::new(bootstrap_js)
        .map_err(|_| napi::Error::from_reason("bootstrap JS contains NUL"))?;

    let state: &'static State = Box::leak(Box::new(State {
        uv_loop: uv_loop_ptr as *mut _,
        node_env: node_env_ptr as *mut _,
        bootstrap_js,
        uv,
        bootstrap_done: Cell::new(false),
        quit_pending: Cell::new(false),
        quit_complete: Cell::new(false),
        run_event_loop_active: Cell::new(false),
        quit_callbacks: RefCell::new(Vec::new()),
    }));

    STATE.with(|s| *s.borrow_mut() = Some(state));

    i_slint_backend_selector::api::BackendSelector::new()
        .with_winit_custom_application_handler(LibuvHandler)
        .select()
        .map_err(|e: i_slint_core::platform::PlatformError| {
            napi::Error::from_reason(e.to_string())
        })?;

    i_slint_backend_selector::with_platform(|b| b.run_event_loop())
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}
