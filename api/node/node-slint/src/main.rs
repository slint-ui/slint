// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `node-slint` — a Node.js runner with statically linked Slint.
//!
//! Unlike plain `node`, slint owns the event loop.  Winit's
//! `run_event_loop` is the outermost loop; libuv is pumped from
//! `about_to_wait`.  `slint.runEventLoop()` from user JS becomes a thin
//! shim that returns a Promise resolved when `quitEventLoop()` is
//! called.
//!
//! ## Linking
//!
//! `slint-node` is depended on as a Rust rlib so its
//! `napi_register_module_v1` ends up in this binary's exported symbols.
//! The bootstrap below tells Node's module loader to redirect anything
//! that would load slint's native binary to `process.dlopen(mod,
//! process.execPath)`, which dlopens this binary and finds that symbol.

use std::ffi::{CString, c_char, c_int, c_void};

unsafe extern "C" {
    fn node_slint_run(
        argc: c_int,
        argv: *mut *mut c_char,
        body: Option<unsafe extern "C" fn(i64, i64, *mut c_void)>,
        userdata: *mut c_void,
    ) -> c_int;
}

/// Force the linker to keep `napi_register_module_v1` (provided by the
/// statically linked slint-node rlib).  `process.dlopen(mod,
/// process.execPath)` resolves it via `dlsym` at runtime.
#[cfg(not(target_os = "windows"))]
fn force_napi_link() {
    std::hint::black_box(slint_node::slint_napi_register_module_v1 as *const ());
}

/// JS bootstrap.  Intercepts `Module._load` so any require that would
/// reach the slint native binary returns this binary's in-process napi
/// module instead.  Then dynamic-imports the user script.
const BOOTSTRAP_JS: &str = r#"
const Module = require('module');
const path = require('path');
const { pathToFileURL } = require('url');

const interceptPattern =
    /(?:^|[\\/])rust-module\.(?:cjs|node)$|slint-ui.*\.node$/;

let cachedNapi = null;
function loadInProcess() {
    if (cachedNapi === null) {
        const mod = { exports: {} };
        process.dlopen(mod, process.execPath);
        cachedNapi = mod.exports;
    }
    return cachedNapi;
}

const origLoad = Module._load;
Module._load = function (request, parent, isMain) {
    if (interceptPattern.test(request)) {
        return loadInProcess();
    }
    if (parent && parent.filename &&
        /rust-module\.(?:cjs|node)$/.test(parent.filename)) {
        return loadInProcess();
    }
    return origLoad.call(this, request, parent, isMain);
};

if (process.argv.length < 2) {
    process.stderr.write('Usage: node-slint <script.js> [args...]\n');
    process.exit(1);
}
const script = path.resolve(process.argv[1]);
import(pathToFileURL(script).href);
"#;

/// Called by the C++ shim with V8 scopes active.  Registers the winit
/// custom handler and starts slint's event loop; returns when it exits.
unsafe extern "C" fn body(uv_loop_ptr: i64, node_env_ptr: i64, _userdata: *mut c_void) {
    #[cfg(feature = "backend-winit")]
    {
        if let Err(e) = slint_node::start_node_slint_event_loop(
            uv_loop_ptr,
            node_env_ptr,
            BOOTSTRAP_JS.to_string(),
        ) {
            eprintln!("node-slint: {}", e);
        }
    }
    #[cfg(not(feature = "backend-winit"))]
    {
        let _ = (uv_loop_ptr, node_env_ptr);
        eprintln!("node-slint: built without backend-winit; cannot start event loop");
    }
}

fn main() {
    #[cfg(not(target_os = "windows"))]
    force_napi_link();

    let args: Vec<CString> = std::env::args()
        .map(|a| CString::new(a).expect("argv contains NUL"))
        .collect();
    let mut argv: Vec<*mut c_char> = args.iter().map(|c| c.as_ptr() as *mut _).collect();
    argv.push(std::ptr::null_mut());

    let rc = unsafe {
        node_slint_run(
            args.len() as c_int,
            argv.as_mut_ptr(),
            Some(body),
            std::ptr::null_mut(),
        )
    };
    std::process::exit(rc);
}
