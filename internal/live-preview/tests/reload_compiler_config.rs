// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A `Compiler` is not `Send`, so the worker thread builds its own rather than
//! being handed one. Check that it builds it from what the caller passed, by
//! importing through an include path only the caller sets up.

#![cfg(feature = "live-component")]

#[path = "live_reload/harness.rs"]
mod harness;

use i_slint_live_preview::live_component::{Compiler, Value};

/// Resolves `lib.slint` only through the include path: it sits in a directory of
/// its own, so the import cannot resolve relative to this file.
const APP: &str = r#"
    import { Lib } from "lib.slint";
    export component App inherits Window {
        out property <int> value: Lib.base;
    }"#;

#[test]
fn the_worker_compiles_with_the_compiler_it_was_given() {
    let dir = harness::TestDir::new("compiler-config");
    let lib = dir.write("inc/lib.slint", "export global Lib { out property <int> base: 100; }");
    let file = dir.write("app.slint", APP);

    let include_path = lib.parent().unwrap().to_path_buf();
    let component = harness::start_with(
        move || {
            let mut compiler = Compiler::default();
            compiler.set_include_paths(vec![include_path.clone()]);
            compiler
        },
        &file,
    );

    assert_eq!(
        component.borrow().get_property("value"),
        Value::Number(100.),
        "a worker compiling with a default Compiler would not resolve the import"
    );
}
