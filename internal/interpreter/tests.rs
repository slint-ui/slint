// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(feature = "internal")]
#[test]
fn reuse_window() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{Compiler, ComponentHandle, SharedString, Value};
    let code = r#"
        export component MainWindow inherits Window {
            in-out property<string> text_text: "foo";
            in-out property<string> text_alias: input.text;
            input := TextInput {
                text:  self.enabled ? text_text : text_text;
            }
        }
    "#;
    let handle = {
        let mut compiler = Compiler::default();
        compiler.set_style("fluent".into());
        let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
        assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
        let definition = result.component("MainWindow").unwrap();
        let instance = definition.create().unwrap();
        assert_eq!(
            instance.get_property("text_alias").unwrap(),
            Value::from(SharedString::from("foo"))
        );
        instance
    };

    let _handle2 = {
        let mut compiler = Compiler::default();
        compiler.set_style("fluent".into());
        let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
        assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
        let definition = result.component("MainWindow").unwrap();
        let instance = definition.create_with_existing_window(handle.window()).unwrap();
        drop(handle);
        assert_eq!(
            instance.get_property("text_alias").unwrap(),
            Value::from(SharedString::from("foo"))
        );
        instance
    };
}
