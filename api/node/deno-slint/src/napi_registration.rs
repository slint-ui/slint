// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! In-process NAPI module registration for slint-node.
//!
//! deno-slint links slint-node as an rlib, which puts the
//! `napi_register_module_v1` symbol in the binary.  We re-export it
//! under a unique name (`slint_napi_register_module_v1`) and build a
//! tiny stub `.so` that forwards to it via `dlsym(RTLD_DEFAULT, ...)`.
//! Deno's `op_napi_open` dlopen's the stub, finds the forwarding
//! `napi_register_module_v1`, and calls it -- registering slint-node's
//! NAPI module in-process without a separate `.node` addon.

use deno_runtime::deno_napi::MODULE_TO_REGISTER;
use deno_runtime::worker::MainWorker;

/// Reference a slint-node symbol so the linker pulls in the compilation
/// unit containing the `napi_register_module_v1` symbol.
pub fn _force_slint_node_link() {
    std::hint::black_box(slint_node::init_testing as fn());
}

// Re-export napi_register_module_v1 under a unique name so the stub .so
// can call it via dlsym(RTLD_DEFAULT, ...) without recursion.
unsafe extern "C" {
    fn napi_register_module_v1(
        env: *mut std::ffi::c_void,
        exports: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_napi_register_module_v1(
    env: *mut std::ffi::c_void,
    exports: *mut std::ffi::c_void,
) -> *mut std::ffi::c_void {
    unsafe { napi_register_module_v1(env, exports) }
}

/// Save `MODULE_TO_REGISTER` before snapshot creation (which may clear it).
pub fn save_module_to_register() -> Option<*const std::ffi::c_void> {
    MODULE_TO_REGISTER.with(|cell| {
        let val = cell.borrow_mut().take();
        val.map(|ptr| ptr as *const std::ffi::c_void)
    })
}

/// Restore `MODULE_TO_REGISTER` after snapshot creation.
pub fn restore_module_to_register(saved: Option<*const std::ffi::c_void>) {
    if let Some(ptr) = saved {
        MODULE_TO_REGISTER.with(|cell| {
            *cell.borrow_mut() = Some(ptr as _);
        });
    }
}

/// Create a stub shared library that forwards `napi_register_module_v1`
/// to `slint_napi_register_module_v1` in the main binary.
fn create_stub_so() -> std::path::PathBuf {
    let path = std::env::temp_dir().join("deno-slint-stub.so");
    if path.exists() {
        return path;
    }
    let c_src = r#"
        #include <dlfcn.h>
        typedef void* napi_env;
        typedef void* napi_value;
        typedef napi_value (*register_func)(napi_env, napi_value);
        napi_value napi_register_module_v1(napi_env env, napi_value exports) {
            register_func f = (register_func)dlsym(RTLD_DEFAULT,
                "slint_napi_register_module_v1");
            if (!f) return exports;
            return f(env, exports);
        }
    "#;
    let c_path = std::env::temp_dir().join("deno-slint-stub.c");
    std::fs::write(&c_path, c_src).expect("failed to write stub.c");
    let status = std::process::Command::new("cc")
        .args(["-shared", "-fPIC", "-o"])
        .arg(&path)
        .arg(&c_path)
        .status()
        .expect("failed to run cc");
    assert!(status.success(), "failed to compile stub .so");
    let _ = std::fs::remove_file(c_path);
    path
}

/// Register slint-node's NAPI module in-process and store the exports
/// on `globalThis.__slintNapiExports`.
///
/// Calls deno's `op_napi_open` with a stub `.so` whose
/// `napi_register_module_v1` forwards to the binary's copy.
pub fn register_slint_napi(worker: &mut MainWorker) {
    let stub_path = create_stub_so();

    let script = format!(
        r#"
        (function() {{
            const {{ op_napi_open }} = Deno[Deno.internal].core.ops;
            function noop() {{}}
            const buf = globalThis.Buffer;
            globalThis.__slintNapiExports = op_napi_open(
                String.raw`{stub_path}`,
                globalThis,
                buf ? buf.from.bind(buf) : noop,
                globalThis.reportError ?? noop,
                noop, noop, noop, noop
            );
        }})();
        "#,
        stub_path = stub_path.display()
    );

    worker
        .execute_script("[deno-slint:napi-register]", script.into())
        .expect("failed to register slint-node NAPI module");
}
