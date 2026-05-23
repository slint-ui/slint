// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `node-slint` — a Node.js runner with statically linked Slint.
//!
//! Plain `node` polls the Slint event loop at 16 ms intervals on Windows
//! (and uses the `uv_prepare` integration on Unix).  This runner does the
//! same on Unix but also wires winit's `CustomApplicationHandler` on
//! Windows for 0-latency UI events when running the winit backend.
//!
//! ## Linking
//!
//! `slint-node` is depended on as a Rust rlib, so its
//! `napi_register_module_v1` ends up in this binary's exported symbols.
//! The bootstrap below tells Node's module loader to redirect
//! `require('slint-ui/rust-module.cjs')` to `process.dlopen(mod,
//! process.execPath)`, which dlopens this binary and finds that symbol.
//! Result: a single in-process copy of the slint NAPI code, no separate
//! `.node` addon to ship.

use std::ffi::{CString, c_char, c_int};

unsafe extern "C" {
    fn node_slint_run(
        argc: c_int,
        argv: *mut *mut c_char,
        bootstrap_js: *const c_char,
        before_user_script: Option<unsafe extern "C" fn(i64, *mut std::ffi::c_void)>,
        userdata: *mut std::ffi::c_void,
    ) -> c_int;
}

/// Force the linker to keep `napi_register_module_v1` (provided by the
/// statically linked slint-node rlib) so `process.dlopen(mod,
/// process.execPath)` from the bootstrap can find it.
///
/// Without an explicit reference here, the linker would treat the symbol
/// as dead code: nothing in this binary calls it (Node looks it up via
/// dlsym at runtime).
#[used]
#[cfg(not(target_os = "windows"))]
static _FORCE_NAPI_LINK: *const () =
    slint_node::slint_napi_register_module_v1 as *const ();

/// JS bootstrap.  Patches the `.node` extension loader so any
/// `require('…rust-module…')` resolves to this binary's in-process
/// napi module instead of dlopen'ing a separate `.node` file, then
/// imports the user script.
const BOOTSTRAP_JS: &str = r#"
const Module = require('module');
const path = require('path');
const { pathToFileURL } = require('url');

const orig_dot_node = Module._extensions['.node'];
Module._extensions['.node'] = function (mod, filename) {
    if (/(?:^|[\\/])rust-module\.(?:cjs|node)$/.test(filename) ||
        /slint-ui.*\.node$/.test(filename)) {
        // Load the running executable as a NAPI addon.  Node calls
        // napi_register_module_v1 which is provided by this binary (the
        // slint-node rlib was linked in statically).
        process.dlopen(mod, process.execPath);
        return;
    }
    return orig_dot_node.call(this, mod, filename);
};

if (process.argv.length < 2) {
    process.stderr.write('Usage: node-slint <script.js> [args...]\n');
    process.exit(1);
}
const script = path.resolve(process.argv[1]);
import(pathToFileURL(script).href);
"#;

unsafe extern "C" fn before_user_script(uv_loop_ptr: i64, _userdata: *mut std::ffi::c_void) {
    // Register the winit CustomApplicationHandler that pumps libuv inside
    // about_to_wait.  Only meaningful on Windows (where the uv_prepare path
    // doesn't work) and only when the winit backend is selected — the
    // backend selector silently ignores winit-specific options for other
    // backends.  Errors are surfaced to stderr but not fatal: JS will fall
    // back to either the uv_prepare path (Unix) or polling.
    #[cfg(feature = "backend-winit")]
    {
        if let Err(e) = slint_node::register_winit_libuv_handler(uv_loop_ptr) {
            eprintln!(
                "node-slint: register_winit_libuv_handler skipped: {}",
                e
            );
        }
    }
    let _ = uv_loop_ptr;
}

fn main() {
    // Convert process args into null-terminated C strings.
    let args: Vec<CString> = std::env::args()
        .map(|a| CString::new(a).expect("argv contains NUL"))
        .collect();
    let mut argv: Vec<*mut c_char> = args.iter().map(|c| c.as_ptr() as *mut _).collect();
    argv.push(std::ptr::null_mut());

    let bootstrap = CString::new(BOOTSTRAP_JS).expect("bootstrap contains NUL");

    let rc = unsafe {
        node_slint_run(
            args.len() as c_int,
            argv.as_mut_ptr(),
            bootstrap.as_ptr(),
            Some(before_user_script),
            std::ptr::null_mut(),
        )
    };
    std::process::exit(rc);
}
