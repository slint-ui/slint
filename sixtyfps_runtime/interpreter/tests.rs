// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

#[test]
fn reuse_window() {
    sixtyfps_rendering_backend_testing::init();
    use crate::{ComponentCompiler, SharedString, Value};
    let code = r#"
        MainWindow := Window {
            property<string> text_text: "foo";
            property<string> text_alias: input.text;
            input := TextInput {
                text:  enabled ? text_text : text_text;
            }
        }
    "#;
    let handle = {
        let mut compiler = ComponentCompiler::default();
        compiler.set_style("fluent".into());
        let definition =
            spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
        assert!(compiler.diagnostics().is_empty(), "{:?}", compiler.diagnostics());
        let instance = definition.unwrap().create();
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
