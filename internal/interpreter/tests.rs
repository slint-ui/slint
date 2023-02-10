// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#[test]
fn reuse_window() {
    i_slint_backend_testing::init();
    use crate::{ComponentCompiler, ComponentHandle, SharedString, Value};
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
        let mut compiler = ComponentCompiler::default();
        compiler.set_style("fluent".into());
        let definition =
            spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
        assert!(compiler.diagnostics().is_empty(), "{:?}", compiler.diagnostics());
        let instance = definition.unwrap().create().unwrap();
        assert_eq!(
            instance.get_property("text_alias").unwrap(),
            Value::from(SharedString::from("foo"))
        );
        instance
    };

    let _handle2 = {
        let mut compiler = ComponentCompiler::default();
        compiler.set_style("fluent".into());
        let definition =
            spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
        assert!(compiler.diagnostics().is_empty(), "{:?}", compiler.diagnostics());
        let instance = definition.unwrap().create_with_existing_window(handle.window());
        drop(handle);
        assert_eq!(
            instance.get_property("text_alias").unwrap(),
            Value::from(SharedString::from("foo"))
        );
        instance
    };
}
