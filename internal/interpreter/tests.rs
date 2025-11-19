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

#[test]
fn set_wrong_struct() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{Compiler, Struct, Value};
    let code = r#"
struct TimeStruct {
    Clock:              string,
    Enabled:            bool,
}

export global Device {
    in-out property <TimeStruct> Time: { Clock: "11:37", Enabled: true };
}

export component Clock {
    ta := TouchArea {
        enabled: Device.Time.Enabled;
    }
    out property <bool> ta_enabled: ta.enabled;
    out property <string> time: Device.Time.Clock;
}
    "#;
    let compiler = Compiler::default();
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("Clock").unwrap();
    let instance = definition.create().unwrap();
    assert_eq!(instance.get_property("ta_enabled").unwrap(), Value::from(true));
    assert_eq!(instance.get_property("time").unwrap(), Value::String("11:37".into()));
    // This only has "Clock" but no "Enabled"
    instance
        .set_global_property(
            "Device",
            "Time",
            Struct::from_iter([("Clock".into(), Value::String("10:37".into()))]).into(),
        )
        .unwrap();
    assert_eq!(instance.get_property("ta_enabled").unwrap(), Value::from(false));
    assert_eq!(instance.get_property("time").unwrap(), Value::String("10:37".into()));

    // Setting a struct with wrong fields leads to an error
    assert_eq!(
        instance.set_global_property(
            "Device",
            "Time",
            Struct::from_iter([("Broken".into(), Value::Number(12.1))]).into(),
        ),
        Err(crate::SetPropertyError::WrongType)
    );
    assert_eq!(instance.get_property("ta_enabled").unwrap(), Value::from(false));
    assert_eq!(instance.get_property("time").unwrap(), Value::String("10:37".into()));
}
