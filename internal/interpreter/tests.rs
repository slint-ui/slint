// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::ComponentHandle;

fn set_global_log_message_handler(
    handler: Option<i_slint_core::debug_log::LogMessageHandler>,
) -> Option<i_slint_core::debug_log::LogMessageHandler> {
    i_slint_backend_selector::with_global_context(|ctx| ctx.set_log_message_handler(handler))
        .unwrap()
}

#[cfg(feature = "internal")]
#[test]
fn reuse_window() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{Compiler, SharedString, Value};
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
fn context_debug_handler_overrides_platform() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Compiler;
    use std::cell::RefCell;
    use std::rc::Rc;

    let captured = Rc::new(RefCell::new(Vec::new()));
    let previous = set_global_log_message_handler(Some(Box::new({
        let captured = captured.clone();
        move |message: i_slint_core::debug_log::LogMessage<'_>| {
            captured.borrow_mut().push(message.message_arguments().to_string());
        }
    })));

    let code = r#"
        export component MainWindow inherits Window {
            init => { debug("from component"); }
        }
    "#;
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("MainWindow").unwrap();
    let instance = definition.create().unwrap();

    assert_eq!(captured.borrow().as_slice(), ["from component"]);
    assert!(
        i_slint_backend_testing::access_testing_window(
            instance.window(),
            |w: &i_slint_backend_testing::TestingWindow| w.take_debug_log(),
        )
        .is_empty()
    );

    set_global_log_message_handler(previous);
}

#[test]
fn platform_debug_handler_is_fallback() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Compiler;

    let code = r#"
        export component MainWindow inherits Window {
            init => { debug("from component"); }
        }
    "#;
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("MainWindow").unwrap();
    let instance = definition.create().unwrap();

    assert_eq!(
        i_slint_backend_testing::access_testing_window(
            instance.window(),
            |w: &i_slint_backend_testing::TestingWindow| w.take_debug_log(),
        ),
        ["from component"]
    );
}

#[test]
fn global_debug_messages_use_context_handler() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Compiler;
    use std::cell::RefCell;
    use std::rc::Rc;

    let captured = Rc::new(RefCell::new(Vec::new()));
    let previous = set_global_log_message_handler(Some(Box::new({
        let captured = captured.clone();
        move |message: i_slint_core::debug_log::LogMessage<'_>| {
            captured.borrow_mut().push(message.message_arguments().to_string());
        }
    })));

    let code = r#"
        export global Logic {
            in-out property<string> greeting: {
                debug("from global");
                "hello"
            };
        }

        export component MainWindow inherits Window { }
    "#;
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("MainWindow").unwrap();
    let instance = definition.create().unwrap();

    assert_eq!(
        instance.get_global_property("Logic", "greeting").unwrap(),
        crate::Value::from(crate::SharedString::from("hello"))
    );
    assert_eq!(captured.borrow().as_slice(), ["from global"]);
    assert!(
        i_slint_backend_testing::access_testing_window(
            instance.window(),
            |w: &i_slint_backend_testing::TestingWindow| w.take_debug_log(),
        )
        .is_empty()
    );

    set_global_log_message_handler(previous);
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

#[test]
fn popup_is_open() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{Compiler, ComponentHandle, Value};
    // `Dropdown` is used twice, so it stays a real sub-component and gets fully inlined, which moves
    // the synthesized `is-open` property to the parent's root with a mangled name -- exercising the
    // `move_declarations` fixup path. `popup.is-open` is read from `open` inside the sub-component.
    let code = r#"
component Dropdown inherits Rectangle {
    width: 40px;
    height: 20px;
    out property <bool> open: popup.is-open;
    callback do-show;
    callback do-close;
    do-show => { popup.show(); }
    do-close => { popup.close(); }
    popup := PopupWindow {
        close-policy: no-auto-close;
        x: 0; y: 20px; width: 40px; height: 40px;
        Rectangle { background: blue; }
    }
}
export component TestCase {
    width: 300px;
    height: 300px;
    out property <bool> a-open: a.open;
    out property <bool> a2-open: a2.open;
    callback show-a;
    callback close-a;
    callback show-a2;
    show-a => { a.do-show(); }
    close-a => { a.do-close(); }
    show-a2 => { a2.do-show(); }
    a := Dropdown { x: 0; y: 0; }
    a2 := Dropdown { x: 50px; y: 0; }
}
    "#;
    let compiler = Compiler::default();
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("TestCase").unwrap();
    let instance = definition.create().unwrap();
    let _ = instance.window(); // ensure window

    assert_eq!(instance.get_property("a-open").unwrap(), Value::from(false), "a before show");
    assert_eq!(instance.get_property("a2-open").unwrap(), Value::from(false), "a2 before show");

    // Showing `a` flips only `a`'s is-open to true.
    instance.invoke("show-a", &[]).unwrap();
    assert_eq!(instance.get_property("a-open").unwrap(), Value::from(true), "a after show");
    assert_eq!(instance.get_property("a2-open").unwrap(), Value::from(false), "a2 unaffected");

    // A second independent instance opens on its own.
    instance.invoke("show-a2", &[]).unwrap();
    assert_eq!(instance.get_property("a2-open").unwrap(), Value::from(true), "a2 after show");

    // Programmatic close() drops the `PopupWindow`, whose `Drop` impl resets is-open to false.
    instance.invoke("close-a", &[]).unwrap();
    assert_eq!(instance.get_property("a-open").unwrap(), Value::from(false), "a after close");
}
