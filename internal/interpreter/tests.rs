// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore dontcrash

#[allow(unused_imports)]
use i_slint_core::api::ComponentHandle;

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

/// Run the eager instantiation pass, materializing repeaters and
/// conditionals — normally driven by the window before rendering and
/// event dispatch. Headless tests call it explicitly after creation and
/// after changing what a repeater depends on.
fn ensure_instantiated(instance: &crate::component::ComponentInstanceInner) {
    instance.0.ensure_instantiated();
}

/// Compile and create a runtime instance of a single component for tests.
fn llr_compile(code: &str, name: &str) -> crate::component::ComponentInstanceInner {
    let config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    let result = spin_on::spin_on(crate::component::build_from_source(
        code.into(),
        Default::default(),
        config,
    ));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.level() != i_slint_compiler::diagnostics::DiagnosticLevel::Error),
        "{:?}",
        result.diagnostics
    );
    result.components.get(name).expect("component should compile").create()
}

/// Nested sub-component composition.
#[test]
fn interpreter_sub_component() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            component Inner {
                in property <int> value: 5;
                out property <int> doubled: value * 2;
            }
            export component Outer {
                in-out property <int> v: 11;
                inner := Inner { value: root.v; }
                out property <int> seen: inner.doubled;
            }
        "#,
        "Outer",
    );

    assert_eq!(instance.get_property("seen"), Some(Value::Number(22.)));
    instance.set_property("v", Value::Number(7.)).unwrap();
    assert_eq!(instance.get_property("seen"), Some(Value::Number(14.)));
}

/// A callback declared on a sub-component can be invoked
/// via a conditional child's `init` handler. Without inlining, `if true`
/// expands to a repeater whose init must reach the parent sub-component's
/// callbacks table.
#[test]
fn interpreter_sub_component_callback_from_if() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            component Inner {
                callback dontcrash();
                dontcrash => {}
                VerticalLayout {
                    if true: Rectangle {
                        init => { dontcrash(); }
                    }
                }
            }
            export component Outer {
                property <bool> ok;
                inner := Inner {
                    dontcrash => { ok = true; }
                }
                out property <length> pw: inner.preferred-width;
                out property <bool> test: ok;
            }
        "#,
        "Outer",
    );
    // Exactly mirrors `callbacks/init_access_base_compo.slint`: the
    // instantiation pass initializes the nested conditional child (running
    // its `init` code) before the boolean is evaluated.
    ensure_instantiated(&instance);
    assert_eq!(instance.get_property("test"), Some(Value::Bool(true)));
}

/// A two-way binding from a repeated sub-component's property
/// up to a parent property. The parent's value must be preserved; the inner
/// component's default binding should not clobber it.
#[test]
fn interpreter_two_way_binding_repeated_priority() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            component Compo {
                preferred-height: 10px;
                in-out property <int> bar: 120;
            }
            export component TestCase {
                in-out property <int> override_bar: 22;
                force_instance := VerticalLayout {
                    if true : Compo { bar <=> root.override_bar; }
                }
                out property <bool> test: force_instance.preferred-height == 10px
                    && override_bar == 22;
            }
        "#,
        "TestCase",
    );
    ensure_instantiated(&instance);
    assert_eq!(instance.get_property("test"), Some(Value::Bool(true)));
}

/// A container component with `@children` must host outer
/// children at the placeholder slot even without full element inlining.
#[test]
fn interpreter_children_placeholder() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            component Container inherits HorizontalLayout {
                Rectangle { width: 50px; }
                @children
                Rectangle { width: 50px; }
            }
            export component TestCase inherits Window {
                width: 300px;
                height: 100px;
                Container {
                    target := Rectangle { width: 50px; }
                }
                out property <length> target_x: target.absolute-position.x;
                out property <bool> test: target_x == 50px;
            }
        "#,
        "TestCase",
    );
    assert_eq!(instance.get_property("test"), Some(Value::Bool(true)));
}

/// Nested @children where a wrapper component has
/// `@children` placed inside a nested element (not at the root).
#[test]
fn interpreter_nested_children_placeholder() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            component Fgh inherits HorizontalLayout {
                Rectangle { background: red; width: 100px; }
                Rectangle {
                    background: yellow;
                    GridLayout { @children }
                }
                Rectangle { background: gray; width: 100px; }
            }
            export component TestCase inherits Window {
                width: 300px;
                height: 100px;
                Fgh { target := Rectangle {} }
                out property <length> target_x: target.absolute-position.x;
                out property <bool> test: target_x == 100px;
            }
        "#,
        "TestCase",
    );
    assert_eq!(instance.get_property("test"), Some(Value::Bool(true)));
}

/// model_data and model_index propagation through repeaters.
#[test]
fn interpreter_model_index_propagation() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            export component MI inherits Window {
                in property <[int]> values: [10, 20, 30];
                out property <int> total;
                out property <int> hits;
                in-out property <length> echo: layout.preferred-height;
                layout := VerticalLayout {
                    for v[i] in values: Rectangle {
                        height: v * 1px;
                        init => { root.hits += 1; root.total += v; }
                    }
                }
            }
        "#,
        "MI",
    );
    // The instantiation pass materializes the repeater and runs each
    // row's `init` block.
    ensure_instantiated(&instance);
    assert_eq!(instance.get_property("hits"), Some(Value::Number(3.)));
    assert_eq!(instance.get_property("total"), Some(Value::Number(60.)));
}

/// Simple grid layout with `for` cells using explicit row/col.
#[test]
fn interpreter_grid_with_for_cells() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            export component G inherits Window {
                width: 200px;
                height: 200px;
                in property <[{r: int, c: int}]> cells: [
                    { r: 0, c: 0 },
                    { r: 0, c: 1 },
                    { r: 1, c: 0 },
                ];
                GridLayout {
                    for cell[i] in cells: Rectangle {
                        row: cell.r;
                        col: cell.c;
                        width: 50px;
                        height: 50px;
                    }
                    next := Rectangle { width: 50px; height: 50px; }
                }
                out property <int> next-row: next.row;
                out property <int> next-col: next.col;
            }
        "#,
        "G",
    );
    // After 3 cells at (0,0), (0,1), (1,0), `next` should auto-place at (1,1).
    ensure_instantiated(&instance);
    assert_eq!(instance.get_property("next-row"), Some(Value::Number(1.)));
    assert_eq!(instance.get_property("next-col"), Some(Value::Number(1.)));
}

/// Exercises `Cast { to: Type::PathData }` during binding install. The LLR
/// lowers `Path { MoveTo {..} … }` to a cast over an array of builtin-struct
/// literals, which the interpreter has to recover at eval time because
/// `Value::Struct` does not carry the source struct type name. Regression
/// for the panic `binding was of the wrong type` hit when rendering paths.
#[test]
fn interpreter_path_elements() {
    i_slint_backend_testing::init_no_event_loop();

    let code = r#"
        export component Drawing {
            out property <int> dummy: 1;
            Path {
                stroke: black;
                stroke-width: 1px;
                MoveTo { x: 10; y: 20; }
                LineTo { x: 30; y: 40; }
                QuadraticTo { x: 50; y: 60; control-x: 40; control-y: 30; }
                CubicTo { x: 70; y: 80; control-1-x: 50; control-1-y: 60; control-2-x: 60; control-2-y: 70; }
                ArcTo { x: 90; y: 100; radius-x: 5; radius-y: 5; }
                Close {}
            }
            Path {
                stroke: red;
                stroke-width: 1px;
                commands: "M 0 0 L 10 10 Z";
            }
        }
    "#;

    let config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    let result = spin_on::spin_on(crate::component::build_from_source(
        code.into(),
        Default::default(),
        config,
    ));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.level() != i_slint_compiler::diagnostics::DiagnosticLevel::Error),
        "{:?}",
        result.diagnostics
    );
    let def = result.components.get("Drawing").expect("Drawing should compile");
    // The rtti-wrapped binding closure only enforces the `PathData` type
    // check on the first `Property::get`, which happens during rendering —
    // not drivable from this harness. Creating the instance still exercises
    // the LLR walk where `cast_to_path_data` intercepts the expression;
    // end-to-end coverage comes from running an example through
    // `slint-viewer`.
    let instance = def.create();
    assert_eq!(instance.get_property("dummy"), Some(crate::Value::Number(1.)));
}

fn set_global_log_message_handler(
    handler: Option<i_slint_core::debug_log::LogMessageHandler>,
) -> Option<i_slint_core::debug_log::LogMessageHandler> {
    i_slint_backend_selector::with_global_context(|ctx| ctx.set_log_message_handler(handler))
        .unwrap()
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

/// `root_component()` must return the component the definition was built
/// from, not a same-named component from another loaded document.
#[cfg(feature = "internal")]
#[test]
fn root_component_resolves_to_the_right_document() {
    i_slint_backend_testing::init_no_event_loop();
    let mut compiler = crate::Compiler::default();
    compiler.set_file_loader(|path| {
        let path = path.to_path_buf();
        Box::pin(async move {
            (path.file_name().and_then(|n| n.to_str()) == Some("lib.slint")).then(|| {
                Ok("component App inherits Rectangle { }\nexport component Helper inherits Rectangle { App {} }".to_owned())
            })
        })
    });
    let result = spin_on::spin_on(
        compiler.build_from_source(
            r#"
            import { Helper } from "lib.slint";
            export component App inherits Window {
                width: 123px;
                Helper { }
            }
        "#
            .into(),
            std::path::PathBuf::from("main.slint"),
        ),
    );
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("App").unwrap();
    let root = definition.root_component();
    assert_eq!(
        root.root_element.borrow().debug.first().unwrap().node.source_file.path(),
        std::path::Path::new("main.slint")
    );
}

#[test]
fn accent_color_reachable_from_global() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Compiler;
    // The style palette reads the accent color in a global, which has no
    // enclosing sub-component to reach the window from.
    let code = r#"
        import { Palette } from "std-widgets.slint";
        export component App inherits Window {
            out property <brush> accent: Palette.accent-background;
        }
    "#;
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let instance = result.component("App").unwrap().create().unwrap();

    let before = instance.get_property("accent").unwrap();
    i_slint_core::context::with_global_context(
        || panic!("context should already be initialized"),
        |ctx| ctx.set_accent_color(i_slint_core::Color::from_argb_u8(255, 255, 0, 0)),
    )
    .unwrap();
    let after = instance.get_property("accent").unwrap();
    assert_ne!(before, after, "accent-background should follow the system accent color");
}
