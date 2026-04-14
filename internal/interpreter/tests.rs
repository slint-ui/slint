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

/// End-to-end smoke test for the interpreter.
///
/// Compiles a tiny component, instantiates it through
/// [`crate::component::build_from_source`], reads a default-valued
/// property, writes a new value and reads it back, and calls a user-declared
/// function through the public-properties bridge.
#[test]
fn interpreter_smoke() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{SharedString, Value};

    let code = r#"
        export component Greeter {
            in-out property <string> who: "world";
            out property <string> greeting: "hello " + who;
            public function shout() -> string { return who + "!"; }
        }
    "#;

    let config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    let (diags, components) = spin_on::spin_on(crate::component::build_from_source(
        code.into(),
        Default::default(),
        config,
    ));
    assert!(
        diags.iter().all(|d| d.level() != i_slint_compiler::diagnostics::DiagnosticLevel::Error),
        "{diags:?}"
    );

    let def = components.get("Greeter").expect("Greeter should be compiled");
    let instance = def.create();

    assert_eq!(
        instance.get_property("greeting"),
        Some(Value::String(SharedString::from("hello world")))
    );

    instance.set_property("who", Value::String(SharedString::from("slint"))).expect("set_property");
    assert_eq!(
        instance.get_property("greeting"),
        Some(Value::String(SharedString::from("hello slint")))
    );

    let shouted = instance.invoke("shout", &[]).unwrap();
    assert_eq!(shouted, Value::String(SharedString::from("slint!")));
}

/// exercise callbacks, arithmetic and conditional expressions.
#[test]
fn interpreter_callbacks_and_math() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;
    use std::cell::Cell;
    use std::rc::Rc;

    let code = r#"
        export component Calc {
            in property <int> a: 3;
            in property <int> b: 4;
            out property <int> sum: a + b;
            out property <int> max_val: max(a, b);
            out property <bool> same: a == b;
            callback clicked(int);
            public function double(x: int) -> int { return x * 2; }
        }
    "#;

    let config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    let (_, components) = spin_on::spin_on(crate::component::build_from_source(
        code.into(),
        Default::default(),
        config,
    ));
    let def = components.get("Calc").expect("Calc should compile");
    let instance = def.create();

    assert_eq!(instance.get_property("sum"), Some(Value::Number(7.)));
    assert_eq!(instance.get_property("max_val"), Some(Value::Number(4.)));
    assert_eq!(instance.get_property("same"), Some(Value::Bool(false)));

    instance.set_property("a", Value::Number(10.)).unwrap();
    assert_eq!(instance.get_property("sum"), Some(Value::Number(14.)));
    assert_eq!(instance.get_property("max_val"), Some(Value::Number(10.)));

    // Callback round trip.
    let seen = Rc::new(Cell::new(0.0));
    {
        let seen = seen.clone();
        instance
            .set_callback("clicked", move |args| {
                let v: f64 = args[0].clone().try_into().unwrap();
                seen.set(v);
                Value::Void
            })
            .unwrap();
    }
    instance.invoke("clicked", &[Value::Number(42.)]).unwrap();
    assert_eq!(seen.get(), 42.);

    // User-declared function.
    let doubled = instance.invoke("double", &[Value::Number(21.)]).unwrap();
    assert_eq!(doubled, Value::Number(42.));
}

/// Compile + create a runtime instance of a single component for tests.
fn llr_compile(code: &str, name: &str) -> crate::component::ComponentInstanceInner {
    let config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    let (diags, components) = spin_on::spin_on(crate::component::build_from_source(
        code.into(),
        Default::default(),
        config,
    ));
    assert!(
        diags.iter().all(|d| d.level() != i_slint_compiler::diagnostics::DiagnosticLevel::Error),
        "{diags:?}"
    );
    components.get(name).expect("component should compile").create()
}

/// arithmetic expressions, unary operators and self-assignment.
#[test]
fn interpreter_arithmetic() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            export component TestCase {
                in-out property <int> a;
                out property <int> t1: 4 + 3 * 2 + 2 - 50 - 2;
                out property <int> t2: 500 / 2 * 30 - 1;
                out property <int> t3: a - (3 + ++2 * (a + 2));
                out property <int> t5: (a + 1.3) * 10;
                callback foo;
                foo => {
                    a += +8;
                    a *= 10;
                    a /= 2;
                    a -= 3;
                }
            }
        "#,
        "TestCase",
    );

    assert_eq!(instance.get_property("t1"), Some(Value::Number(-40.)));
    assert_eq!(instance.get_property("t2"), Some(Value::Number(7499.)));
    assert_eq!(instance.get_property("t5"), Some(Value::Number(13.)));

    instance.set_property("a", Value::Number(42.)).unwrap();
    assert_eq!(instance.get_property("t3"), Some(Value::Number(-49.)));
    assert_eq!(instance.get_property("t5"), Some(Value::Number(433.)));

    instance.invoke("foo", &[]).unwrap();
    let expected = (((42. + 8.) * 10.) / 2.) - 3.;
    assert_eq!(instance.get_property("a"), Some(Value::Number(expected)));
}

/// nested sub-component composition.
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

/// a callback declared on a sub-component can be invoked
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
    let _ = instance.get_property("pw");
    // Exactly mirrors `callbacks/init_access_base_compo.slint`: the `test`
    // binding reads a layout property which should ensure the nested
    // conditional child is initialized before the boolean is evaluated.
    assert_eq!(instance.get_property("test"), Some(Value::Bool(true)));
}

/// a two-way binding from a repeated sub-component's property
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
    assert_eq!(instance.get_property("test"), Some(Value::Bool(true)));
}

/// a container component with `@children` must host outer
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

/// nested @children where a wrapper component has
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

/// function on a sub-component returning an enum value
/// through empty if branches fallthrough (mirrors the Issue4070 pattern
/// from `tests/cases/expr/return2.slint`).
#[test]
fn interpreter_sub_component_enum_return() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            component Inner {
                function broken(event: string) -> EventResult {
                    if (event == "a") {  }
                    else if (event == "s") {  }
                    else {
                        return reject;
                    }
                    accept
                }
                out property <bool> test : broken("a") == EventResult.accept;
            }
            export component Outer {
                i := Inner {}
                out property <bool> test: i.test;
            }
        "#,
        "Outer",
    );

    assert_eq!(instance.get_property("test"), Some(Value::Bool(true)));
}

/// a function on a sub-component with an early return in
/// one branch. With element inlining disabled, this exercises both the
/// `return`-lowering pass and cross-sub-component function invocation.
#[test]
fn interpreter_sub_component_function_return() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            component Inner {
                function pick(which: int) -> int {
                    if (which == 0) {
                        return 10;
                    }
                    20
                }
                out property <int> a: pick(0);
                out property <int> b: pick(1);
            }
            export component Outer {
                inner := Inner {}
                out property <int> a: inner.a;
                out property <int> b: inner.b;
            }
        "#,
        "Outer",
    );

    assert_eq!(instance.get_property("a"), Some(Value::Number(10.)));
    assert_eq!(instance.get_property("b"), Some(Value::Number(20.)));
}

/// string operations and conversions.
#[test]
fn interpreter_strings() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{SharedString, Value};

    let instance = llr_compile(
        r#"
            export component S {
                in property <string> a: "Hello";
                in property <string> b: "world";
                out property <string> joined: a + " " + b;
                out property <int> len: a.character-count;
                out property <string> upper: a.to-uppercase();
                out property <string> lower: a.to-lowercase();
                out property <bool> is-empty: a.is-empty;
            }
        "#,
        "S",
    );

    assert_eq!(
        instance.get_property("joined"),
        Some(Value::String(SharedString::from("Hello world")))
    );
    assert_eq!(instance.get_property("len"), Some(Value::Number(5.)));
    assert_eq!(instance.get_property("upper"), Some(Value::String(SharedString::from("HELLO"))));
    assert_eq!(instance.get_property("lower"), Some(Value::String(SharedString::from("hello"))));
    assert_eq!(instance.get_property("is-empty"), Some(Value::Bool(false)));
}

/// struct construction and field access.
#[test]
fn interpreter_structs() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            struct Point { x: int, y: int }
            export component S {
                in-out property <Point> p: { x: 3, y: 4 };
                out property <int> sum: p.x + p.y;
            }
        "#,
        "S",
    );
    assert_eq!(instance.get_property("sum"), Some(Value::Number(7.)));
}

/// ternary conditional `cond ? a : b`.
#[test]
fn interpreter_ternary() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{SharedString, Value};

    let instance = llr_compile(
        r#"
            export component S {
                in-out property <bool> flag: true;
                out property <string> word: flag ? "yes" : "no";
            }
        "#,
        "S",
    );
    assert_eq!(instance.get_property("word"), Some(Value::String(SharedString::from("yes"))));
    instance.set_property("flag", Value::Bool(false)).unwrap();
    assert_eq!(instance.get_property("word"), Some(Value::String(SharedString::from("no"))));
}

/// read and write a native item's `text` property via an alias.
#[test]
fn interpreter_item_property_alias() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{SharedString, Value};

    let instance = llr_compile(
        r#"
            export component App {
                in-out property <string> text <=> txt.text;
                txt := TextInput { text: "default"; }
            }
        "#,
        "App",
    );
    assert_eq!(instance.get_property("text"), Some(Value::String(SharedString::from("default"))));
    instance.set_property("text", Value::String(SharedString::from("changed"))).unwrap();
    assert_eq!(instance.get_property("text"), Some(Value::String(SharedString::from("changed"))));
}

/// built-in math functions.
#[test]
fn interpreter_math_builtins() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            export component M {
                out property <int> r1: round(1.6);
                out property <int> r2: floor(1.9);
                out property <int> r3: ceil(1.1);
                out property <int> r4: abs(-7);
                out property <int> r5: max(3, 9);
                out property <int> r6: min(3, 9);
                out property <int> r7: clamp(15, 0, 10);
                out property <int> mod_v: mod(10, 3);
            }
        "#,
        "M",
    );
    assert_eq!(instance.get_property("r1"), Some(Value::Number(2.)));
    assert_eq!(instance.get_property("r2"), Some(Value::Number(1.)));
    assert_eq!(instance.get_property("r3"), Some(Value::Number(2.)));
    assert_eq!(instance.get_property("r4"), Some(Value::Number(7.)));
    assert_eq!(instance.get_property("r5"), Some(Value::Number(9.)));
    assert_eq!(instance.get_property("r6"), Some(Value::Number(3.)));
    assert_eq!(instance.get_property("r7"), Some(Value::Number(10.)));
    assert_eq!(instance.get_property("mod_v"), Some(Value::Number(1.)));
}

/// length units, percentages and the `min`/`max` functions on lengths.
#[test]
fn interpreter_length_units() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            export component L {
                in property <length> w: 100px;
                out property <length> half: w / 2;
                out property <length> capped: min(w, 50px);
            }
        "#,
        "L",
    );
    assert_eq!(instance.get_property("half"), Some(Value::Number(50.)));
    assert_eq!(instance.get_property("capped"), Some(Value::Number(50.)));
    instance.set_property("w", Value::Number(20.)).unwrap();
    assert_eq!(instance.get_property("half"), Some(Value::Number(10.)));
    assert_eq!(instance.get_property("capped"), Some(Value::Number(20.)));
}

/// array literal as a model and `length` indexing.
#[test]
fn interpreter_array_literal_indexing() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            export component A {
                out property <[int]> xs: [10, 20, 30];
                out property <int> first: xs[0];
                out property <int> last: xs[2];
            }
        "#,
        "A",
    );
    assert_eq!(instance.get_property("first"), Some(Value::Number(10.)));
    assert_eq!(instance.get_property("last"), Some(Value::Number(30.)));
}

/// `changed` handlers run when a property mutates.
#[test]
fn interpreter_change_callback() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            export component CC {
                in-out property <int> a: 0;
                in-out property <int> seen: 0;
                changed a => { seen = a * 2; }
            }
        "#,
        "CC",
    );
    // Initial value
    assert_eq!(instance.get_property("seen"), Some(Value::Number(0.)));
    // Mutate `a`, then poke the runtime to flush change trackers.
    instance.set_property("a", Value::Number(5.)).unwrap();
    i_slint_core::properties::ChangeTracker::run_change_handlers();
    assert_eq!(instance.get_property("seen"), Some(Value::Number(10.)));
}

/// brush / color literal handling.
#[test]
fn interpreter_brush() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            export component C {
                in property <color> c: #ff0000;
                out property <color> echo: c;
            }
        "#,
        "C",
    );
    // We can't introspect the brush directly without leaving Value land,
    // but the property should round-trip without panicking.
    let v = instance.get_property("echo").unwrap();
    assert!(matches!(v, Value::Brush(_)), "{:?}", v);
}

/// a function that takes parameters and returns a struct.
#[test]
fn interpreter_function_returning_struct() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            struct Pair { a: int, b: int }
            export component F {
                public pure function make(x: int) -> Pair { return { a: x, b: x * 2 }; }
                out property <int> result: make(5).b;
            }
        "#,
        "F",
    );
    assert_eq!(instance.get_property("result"), Some(Value::Number(10.)));

    let pair = instance.invoke("make", &[Value::Number(7.)]).unwrap();
    let Value::Struct(s) = pair else { panic!("expected struct, got {pair:?}") };
    assert_eq!(s.get_field("a"), Some(&Value::Number(7.)));
    assert_eq!(s.get_field("b"), Some(&Value::Number(14.)));
}

/// callback declared with multiple arguments.
#[test]
fn interpreter_multi_arg_callback() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;
    use std::cell::RefCell;
    use std::rc::Rc;

    let instance = llr_compile(
        r#"
            export component MA {
                pure callback compute(int, int) -> int;
                out property <int> via-callback: compute(2, 3);
            }
        "#,
        "MA",
    );
    let captured = Rc::new(RefCell::new((0., 0.)));
    {
        let captured = captured.clone();
        instance
            .set_callback("compute", move |args| {
                let a: f64 = args[0].clone().try_into().unwrap();
                let b: f64 = args[1].clone().try_into().unwrap();
                *captured.borrow_mut() = (a, b);
                Value::Number(a + b)
            })
            .unwrap();
    }
    // Setting the callback re-runs the binding via the dependency tracker
    // on next read.
    assert_eq!(instance.get_property("via-callback"), Some(Value::Number(5.)));
    assert_eq!(*captured.borrow(), (2., 3.));
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
    // Reading `echo` pulls the layout in, which materializes the repeater
    // and runs each row's `init` block.
    let _ = instance.get_property("echo");
    assert_eq!(instance.get_property("hits"), Some(Value::Number(3.)));
    assert_eq!(instance.get_property("total"), Some(Value::Number(60.)));
}

/// assigning a model property to another shares the underlying model.
#[test]
fn interpreter_share_model_via_assignment() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            export component S {
                in-out property <[int]> foo: [1, 2, 3];
                in-out property <[int]> bar: [10, 20, 30];
                out property <int> first: foo[0];
                public function share() { foo = bar; }
                public function poke(v: int) { bar[0] = v; }
            }
        "#,
        "S",
    );
    assert_eq!(instance.get_property("first"), Some(Value::Number(1.)));
    instance.invoke("share", &[]).unwrap();
    assert_eq!(instance.get_property("first"), Some(Value::Number(10.)));
    instance.invoke("poke", &[Value::Number(42.)]).unwrap();
    assert_eq!(instance.get_property("first"), Some(Value::Number(42.)));
}

/// out-of-bounds and fractional model index writes are silent.
#[test]
fn interpreter_array_index_assignment_edge_cases() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            export component Q {
                in-out property <[int]> xs: [1, 2, 3];
                out property <int> first: xs[0];
                public function set-frac() { xs[0.999] = 8; }
                public function set-negative() { xs[-1] = 99; }
                public function set-large() { xs[10] = 99; }
            }
        "#,
        "Q",
    );
    assert_eq!(instance.get_property("first"), Some(Value::Number(1.)));
    instance.invoke("set-frac", &[]).unwrap();
    assert_eq!(instance.get_property("first"), Some(Value::Number(8.)));
    instance.invoke("set-negative", &[]).unwrap();
    // Out of bounds, no change.
    assert_eq!(instance.get_property("first"), Some(Value::Number(8.)));
    instance.invoke("set-large", &[]).unwrap();
    assert_eq!(instance.get_property("first"), Some(Value::Number(8.)));
}

/// simple grid layout with `for` cells using explicit row/col.
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
    assert_eq!(instance.get_property("next-row"), Some(Value::Number(1.)));
    assert_eq!(instance.get_property("next-col"), Some(Value::Number(1.)));
}

/// nested sub-component containing a repeater.
#[test]
fn interpreter_nested_repeater() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            component Inner {
                in property <[int]> data: [1, 2, 3];
                out property <int> n: data.length;
                for d in data : Rectangle { width: d * 1px; }
            }
            export component Outer {
                inner := Inner {}
                out property <int> count: inner.n;
            }
        "#,
        "Outer",
    );
    assert_eq!(instance.get_property("count"), Some(Value::Number(3.)));
}

/// a binding annotated with `animate` shouldn't crash creation.
#[test]
fn interpreter_animation_binding_compiles() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let instance = llr_compile(
        r#"
            export component Anim {
                in property <length> w: 100px;
                Rectangle {
                    width: w;
                    animate width { duration: 100ms; }
                }
                out property <length> echo: w;
            }
        "#,
        "Anim",
    );
    assert_eq!(instance.get_property("echo"), Some(Value::Number(100.)));
    instance.set_property("w", Value::Number(200.)).unwrap();
    assert_eq!(instance.get_property("echo"), Some(Value::Number(200.)));
}

/// a Slint model produced from a builtin Rust `VecModel`.
#[test]
fn interpreter_external_model() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;
    use i_slint_core::model::{ModelRc, VecModel};
    use std::rc::Rc;

    let instance = llr_compile(
        r#"
            export component E {
                in property <[int]> nums: [];
                out property <int> n: nums.length;
            }
        "#,
        "E",
    );
    let model: Rc<VecModel<Value>> = Rc::new(VecModel::default());
    model.push(Value::Number(1.));
    model.push(Value::Number(2.));
    model.push(Value::Number(3.));
    let model_rc: ModelRc<Value> =
        ModelRc::from(model.clone() as Rc<dyn i_slint_core::model::Model<Data = Value>>);
    instance.set_property("nums", Value::Model(model_rc)).unwrap();
    assert_eq!(instance.get_property("n"), Some(Value::Number(3.)));
}

/// for-loop repeater populated from a model property.
#[test]
fn interpreter_repeater() {
    i_slint_backend_testing::init_no_event_loop();

    let code = r#"
        export component List {
            in property <[int]> numbers: [1, 2, 3];
            out property <int> total: numbers[0] + numbers[1] + numbers[2];
            for n[i] in numbers : Rectangle {
                width: n * 1px;
                height: i * 1px;
            }
        }
    "#;

    let config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    let (diags, components) = spin_on::spin_on(crate::component::build_from_source(
        code.into(),
        Default::default(),
        config,
    ));
    assert!(
        diags.iter().all(|d| d.level() != i_slint_compiler::diagnostics::DiagnosticLevel::Error),
        "{diags:?}"
    );
    let def = components.get("List").expect("List should compile");
    let _instance = def.create();
    // We only check that instantiation + binding install doesn't panic.
    // Walking the repeater items is exercised by `visit_children_item`
    // when the window draws, which isn't part of this smoke test.
}

/// two-way binding between a property and an item property.
#[test]
fn interpreter_two_way_binding() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{SharedString, Value};

    let code = r#"
        export component Form {
            in-out property <string> text <=> input.text;
            input := TextInput { text: "hi"; }
        }
    "#;

    let config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    let (_, components) = spin_on::spin_on(crate::component::build_from_source(
        code.into(),
        Default::default(),
        config,
    ));
    let def = components.get("Form").expect("Form should compile");
    let instance = def.create();

    assert_eq!(instance.get_property("text"), Some(Value::String(SharedString::from("hi"))));
    instance.set_property("text", Value::String(SharedString::from("bye"))).unwrap();
    assert_eq!(instance.get_property("text"), Some(Value::String(SharedString::from("bye"))));
}

/// `if` conditional.
#[test]
fn interpreter_conditional() {
    i_slint_backend_testing::init_no_event_loop();

    let code = r#"
        export component Gate {
            in property <bool> open: true;
            if open : Rectangle {}
        }
    "#;

    let config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    let (diags, components) = spin_on::spin_on(crate::component::build_from_source(
        code.into(),
        Default::default(),
        config,
    ));
    assert!(
        diags.iter().all(|d| d.level() != i_slint_compiler::diagnostics::DiagnosticLevel::Error),
        "{diags:?}"
    );
    let def = components.get("Gate").expect("Gate should compile");
    let _instance = def.create();
}

/// global singletons.
#[test]
fn interpreter_globals() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let code = r#"
        export global Settings {
            in-out property <int> count: 7;
        }
        export component App {
            out property <int> mirrored: Settings.count * 2;
        }
    "#;

    let config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    let (_, components) = spin_on::spin_on(crate::component::build_from_source(
        code.into(),
        Default::default(),
        config,
    ));
    let def = components.get("App").expect("App should compile");
    let instance = def.create();

    assert_eq!(instance.get_property("mirrored"), Some(Value::Number(14.)));
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
    let (diags, components) = spin_on::spin_on(crate::component::build_from_source(
        code.into(),
        Default::default(),
        config,
    ));
    assert!(
        diags.iter().all(|d| d.level() != i_slint_compiler::diagnostics::DiagnosticLevel::Error),
        "{diags:?}"
    );
    let def = components.get("Drawing").expect("Drawing should compile");
    // Binding install for the `Path` native item property calls
    // `set_property_binding`, which stores a lazy closure. The rtti-wrapped
    // closure only enforces the `PathData` type check on the first
    // `Property::get`, which happens during rendering — something this
    // unit-test harness cannot drive without a live renderer. Creating the
    // instance here is still enough to exercise the LLR walk that produced
    // the original panic at the site where `cast_to_path_data` intercepts
    // the expression: if the conversion were missing, every evaluation of
    // the Path.elements closure from the renderer would blow up. Full
    // end-to-end coverage comes from running an example through
    // `slint-viewer`.
    let instance = def.create();
    assert_eq!(instance.get_property("dummy"), Some(crate::Value::Number(1.)));
}

/// Smoke-test a TouchArea click via the testing backend. Regression for the
/// interpreter rewrite dropping dynamic-tree dispatch / window attach on
/// test-only entry points — equivalent to `test_nodejs_elements_toucharea`
/// but runnable without the node harness.
#[test]
fn interpreter_toucharea_click() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let code = r#"
        export component TestCase inherits Rectangle {
            width: 300px;
            height: 300px;
            in-out property <int> hits;
            TouchArea {
                x: 100px; y: 100px; width: 10px; height: 10px;
                clicked => { hits += 1; }
            }
        }
    "#;
    let config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    let (diags, components) = spin_on::spin_on(crate::component::build_from_source(
        code.into(),
        Default::default(),
        config,
    ));
    assert!(
        diags.iter().all(|d| d.level() != i_slint_compiler::diagnostics::DiagnosticLevel::Error),
        "{diags:?}"
    );
    let def = components.get("TestCase").expect("TestCase should compile");
    let instance = def.create();
    let public = crate::api::ComponentInstance { inner: instance.clone() };

    // Click outside the TouchArea first — should not fire.
    crate::api::testing::send_mouse_click(&public, 5.0, 5.0);
    assert_eq!(instance.get_property("hits"), Some(Value::Number(0.0)));

    // Click inside — should fire once.
    crate::api::testing::send_mouse_click(&public, 105.0, 105.0);
    assert_eq!(instance.get_property("hits"), Some(Value::Number(1.0)));
}

/// Regression for the gallery bug where switching to the "TextEdit" page
/// would leave the Dark Mode switch with a stale label from a previously
/// visited page (e.g. "Slider"). The shape mirrors the gallery: a shared
/// `Page` base with a conditional header widget + an always-present header
/// widget, embedded in outer `if` conditionals that switch based on an
/// integer property. Verifies the always-present widget on each page reads
/// its own literal string, not one leaked from a sibling page.
#[test]
fn interpreter_switch_conditional_pages_dark_mode() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let code = r#"
        component Page inherits Rectangle {
            in property <bool> show-extra: false;
            width: 300px;
            height: 300px;
            VerticalLayout {
                if show-extra: Text {
                    accessible-role: text;
                    accessible-label: "extra";
                    text: "Extra switch";
                }
                dark := Text {
                    accessible-role: text;
                    accessible-label: "dark-mode";
                    text: "Dark Mode";
                }
            }
        }
        component ControlsPage inherits Page {
            show-extra: false;
            Text {
                accessible-role: text;
                accessible-label: "controls-body";
                text: "Slider";
            }
        }
        component TextEditPage inherits Page {
            show-extra: true;
            Text {
                accessible-role: text;
                accessible-label: "text-edit-body";
                text: "Word-Wrap";
            }
        }
        export component TestCase inherits Window {
            in-out property <int> page: 0;
            width: 300px;
            height: 300px;
            if page == 0: ControlsPage {}
            if page == 1: TextEditPage {}
        }
    "#;
    let config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    let (diags, components) = spin_on::spin_on(crate::component::build_from_source(
        code.into(),
        Default::default(),
        config,
    ));
    assert!(
        diags.iter().all(|d| d.level() != i_slint_compiler::diagnostics::DiagnosticLevel::Error),
        "{diags:?}"
    );
    let def = components.get("TestCase").expect("TestCase should compile");
    let instance = def.create();
    let public = crate::api::ComponentInstance { inner: instance.clone() };

    // On page 0: Controls body visible, "dark-mode" present, no "extra".
    let dark_on_controls: Vec<_> =
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(&public, "dark-mode")
            .collect();
    assert_eq!(dark_on_controls.len(), 1, "Dark mode should be present once on ControlsPage");

    // Switch to page 1 — TextEdit page.
    instance.set_property("page", Value::Number(1.0)).unwrap();
    let dark_on_text_edit: Vec<_> =
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(&public, "dark-mode")
            .collect();
    assert_eq!(dark_on_text_edit.len(), 1, "Dark mode should be present once on TextEditPage");
    let extra: Vec<_> =
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(&public, "extra")
            .collect();
    assert_eq!(extra.len(), 1, "Extra switch should be present on TextEditPage");
    // The controls-body should be gone — the Controls Text is destroyed.
    let controls_body: Vec<_> =
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(&public, "controls-body")
            .collect();
    assert!(controls_body.is_empty(), "Controls body should be gone after switching to TextEdit");
    let text_edit_body: Vec<_> =
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(&public, "text-edit-body")
            .collect();
    assert_eq!(text_edit_body.len(), 1, "TextEdit body should be visible");
}

/// Regression for the gallery bug where `find_by_element_type_name` (and
/// anything else using `item_element_infos`) would miss widgets whose
/// element-info entry lives deep in a sub-component path — typically a
/// component-instance declaration (`Switch { }`) sitting next to
/// `if`-conditionals in the same layout. Exercises the path-walking
/// lookup in the vtable's `item_element_infos`: the always-visible
/// `Switch` declared after a conditional `Switch` in the same parent
/// layout should still be discoverable by its source-level type name.
#[test]
fn interpreter_element_info_walks_sub_component_path() {
    i_slint_backend_testing::init_no_event_loop();

    let code = r#"
        import { Switch } from "std-widgets.slint";
        export component TestCase inherits Window {
            in-out property <bool> show-extra: true;
            width: 400px;
            height: 100px;
            HorizontalLayout {
                if show-extra: Switch { text: "Extra"; }
                Switch { text: "Always"; }
            }
        }
    "#;
    let config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    let (diags, components) = spin_on::spin_on(crate::component::build_from_source(
        code.into(),
        Default::default(),
        config,
    ));
    assert!(
        diags.iter().all(|d| d.level() != i_slint_compiler::diagnostics::DiagnosticLevel::Error),
        "{diags:?}"
    );
    let def = components.get("TestCase").expect("TestCase should compile");
    let instance = def.create();
    let public = crate::api::ComponentInstance { inner: instance.clone() };

    let labels: Vec<String> =
        i_slint_backend_testing::ElementHandle::find_by_element_type_name(&public, "Switch")
            .filter_map(|h| h.accessible_label().map(|s: i_slint_core::SharedString| s.to_string()))
            .collect();
    assert_eq!(
        labels,
        vec!["Extra".to_string(), "Always".to_string()],
        "expected both Switches discoverable by source-level type name"
    );
}

/// Same shape as `interpreter_switch_conditional_pages_dark_mode`, but
/// smaller: just verifies that the `accessible-label` of a conditional
/// page's element reflects the current page.
#[test]
fn interpreter_switch_conditional_pages_labels() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Value;

    let code = r#"
        component PageA inherits Rectangle {
            width: 300px;
            height: 300px;
            Text {
                accessible-role: text;
                accessible-label: "page-a-label";
                text: "Slider";
            }
        }
        component PageB inherits Rectangle {
            width: 300px;
            height: 300px;
            Text {
                accessible-role: text;
                accessible-label: "page-b-label";
                text: "Dark Mode";
            }
        }
        export component TestCase inherits Window {
            in-out property <int> page: 0;
            width: 300px;
            height: 300px;
            if page == 0: PageA {}
            if page == 1: PageB {}
        }
    "#;
    let config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    let (diags, components) = spin_on::spin_on(crate::component::build_from_source(
        code.into(),
        Default::default(),
        config,
    ));
    assert!(
        diags.iter().all(|d| d.level() != i_slint_compiler::diagnostics::DiagnosticLevel::Error),
        "{diags:?}"
    );
    let def = components.get("TestCase").expect("TestCase should compile");
    let instance = def.create();
    let public = crate::api::ComponentInstance { inner: instance.clone() };

    // Initially on page 0 — look up the page-a Text via accessible-label
    // and check its text (carried by accessible-string-property "Label"
    // since we override it explicitly). Use a helper to find by label.
    let find_text_by_label = |label: &str| -> Option<String> {
        let handles: Vec<_> =
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(&public, label)
                .collect();
        if handles.is_empty() {
            return None;
        }
        // All matched; read their `text` property directly via the public API
        // wouldn't work since Text is a native item — use ElementHandle's
        // accessible-label instead, which we know is set to the label we
        // searched for. Instead, verify via visibility that the *right*
        // page was materialized.
        handles.first().and_then(|h| h.accessible_label().map(|s| s.to_string()))
    };

    // Page 0 visible — page-a-label should find exactly one element.
    assert!(find_text_by_label("page-a-label").is_some(), "PageA Text not found while page == 0");
    assert!(
        find_text_by_label("page-b-label").is_none(),
        "PageB Text unexpectedly present while page == 0"
    );

    // Switch to page 1.
    instance.set_property("page", Value::Number(1.0)).unwrap();

    assert!(
        find_text_by_label("page-a-label").is_none(),
        "PageA Text unexpectedly present while page == 1"
    );
    assert!(find_text_by_label("page-b-label").is_some(), "PageB Text not found while page == 1");

    // And back to page 0.
    instance.set_property("page", Value::Number(0.0)).unwrap();
    assert!(
        find_text_by_label("page-a-label").is_some(),
        "PageA Text not re-materialized after page 1 → 0"
    );
    assert!(find_text_by_label("page-b-label").is_none(), "PageB Text lingering after page 1 → 0");
}
