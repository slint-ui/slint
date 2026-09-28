// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `show()`/`hide()` on a SystemTrayIcon-rooted component must set the `visible`
//! property of the native item, like the generated Rust/C++ code does. The item
//! property is not part of the component's public property API, so going through
//! `set_property` fails: the component below intentionally does not redeclare
//! `visible`.

use slint_interpreter::{ComponentHandle, Value};

#[test]
fn tray_show_hide_sets_item_visible() {
    i_slint_backend_testing::init_no_event_loop();

    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_style("fluent".into());
    let code = r#"
        export component TestCase inherits SystemTrayIcon {
            tooltip: "Test tray";
            out property <bool> shown: self.visible;
        }
    "#;
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/test.slint");
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), path.into()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("TestCase").unwrap();

    let instance = definition.create().unwrap();
    instance.show().unwrap();
    assert_eq!(instance.get_property("shown").unwrap(), Value::Bool(true));
    instance.hide().unwrap();
    assert_eq!(instance.get_property("shown").unwrap(), Value::Bool(false));
}
