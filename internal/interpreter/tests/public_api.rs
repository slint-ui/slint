// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Interpreter behavior driven from host code through the public API:
//! callbacks, functions, external models, input dispatch and element
//! queries. Nothing here depends on how the interpreter is implemented.

use i_slint_core::model::{Model, ModelRc, VecModel};
use slint_interpreter::{Compiler, ComponentHandle, ComponentInstance, Value};
use std::rc::Rc;

fn compile(code: &str, name: &str) -> ComponentInstance {
    i_slint_backend_testing::init_no_event_loop();
    let result =
        spin_on::spin_on(Compiler::default().build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    result.component(name).expect("component should compile").create().unwrap()
}

fn send_mouse_click(instance: &ComponentInstance, x: f32, y: f32) {
    let adapter = i_slint_core::window::WindowInner::from_pub(instance.window()).window_adapter();
    i_slint_backend_testing::testing_backend::send_mouse_click(x, y, &adapter);
}

/// Callbacks with arguments set from the host, and invoking a function that
/// returns a struct.
#[test]
fn callbacks_and_functions() {
    let instance = compile(
        r#"
            struct Pair { a: int, b: int }
            export component TestCase {
                pure callback compute(int, int) -> int;
                out property <int> via-callback: compute(2, 3);
                public pure function make(x: int) -> Pair { return { a: x, b: x * 2 }; }
            }
        "#,
        "TestCase",
    );

    let captured = Rc::new(std::cell::RefCell::new((0., 0.)));
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
    assert_eq!(instance.get_property("via-callback").unwrap(), Value::Number(5.));
    assert_eq!(*captured.borrow(), (2., 3.));

    let pair = instance.invoke("make", &[Value::Number(7.)]).unwrap();
    let Value::Struct(s) = pair else { panic!("expected struct, got {pair:?}") };
    assert_eq!(s.get_field("a"), Some(&Value::Number(7.)));
    assert_eq!(s.get_field("b"), Some(&Value::Number(14.)));
}

/// A model created on the host side is observed by the component.
#[test]
fn external_model() {
    let instance = compile(
        r#"
            export component TestCase {
                in property <[int]> nums: [];
                out property <int> n: nums.length;
                out property <int> first: nums[0];
            }
        "#,
        "TestCase",
    );

    let model: Rc<VecModel<Value>> = Rc::new(VecModel::default());
    model.push(Value::Number(1.));
    model.push(Value::Number(2.));
    instance.set_property("nums", Value::Model(ModelRc::from(model.clone()))).unwrap();
    assert_eq!(instance.get_property("n").unwrap(), Value::Number(2.));
    assert_eq!(instance.get_property("first").unwrap(), Value::Number(1.));

    model.push(Value::Number(3.));
    assert_eq!(instance.get_property("n").unwrap(), Value::Number(3.));
    model.set_row_data(0, Value::Number(42.));
    assert_eq!(instance.get_property("first").unwrap(), Value::Number(42.));
}

/// Assigning a model property to another shares the underlying model.
#[test]
fn share_model_via_assignment() {
    let instance = compile(
        r#"
            export component TestCase {
                in-out property <[int]> foo: [1, 2, 3];
                in-out property <[int]> bar: [10, 20, 30];
                out property <int> first: foo[0];
                public function share() { foo = bar; }
                public function poke(v: int) { bar[0] = v; }
            }
        "#,
        "TestCase",
    );
    assert_eq!(instance.get_property("first").unwrap(), Value::Number(1.));
    instance.invoke("share", &[]).unwrap();
    assert_eq!(instance.get_property("first").unwrap(), Value::Number(10.));
    instance.invoke("poke", &[Value::Number(42.)]).unwrap();
    assert_eq!(instance.get_property("first").unwrap(), Value::Number(42.));
}

/// Out-of-bounds and fractional model index writes are silent.
#[test]
fn model_index_write_edge_cases() {
    let instance = compile(
        r#"
            export component TestCase {
                in-out property <[int]> xs: [1, 2, 3];
                out property <int> first: xs[0];
                public function set-frac() { xs[0.999] = 8; }
                public function set-negative() { xs[-1] = 99; }
                public function set-large() { xs[10] = 99; }
            }
        "#,
        "TestCase",
    );
    assert_eq!(instance.get_property("first").unwrap(), Value::Number(1.));
    instance.invoke("set-frac", &[]).unwrap();
    assert_eq!(instance.get_property("first").unwrap(), Value::Number(8.));
    instance.invoke("set-negative", &[]).unwrap();
    assert_eq!(instance.get_property("first").unwrap(), Value::Number(8.));
    instance.invoke("set-large", &[]).unwrap();
    assert_eq!(instance.get_property("first").unwrap(), Value::Number(8.));
}

/// A click dispatched through the testing backend reaches a TouchArea.
#[test]
fn toucharea_click() {
    let instance = compile(
        r#"
            export component TestCase inherits Rectangle {
                width: 300px;
                height: 300px;
                in-out property <int> hits;
                TouchArea {
                    x: 100px; y: 100px; width: 10px; height: 10px;
                    clicked => { hits += 1; }
                }
            }
        "#,
        "TestCase",
    );

    send_mouse_click(&instance, 5.0, 5.0);
    assert_eq!(instance.get_property("hits").unwrap(), Value::Number(0.));
    send_mouse_click(&instance, 105.0, 105.0);
    assert_eq!(instance.get_property("hits").unwrap(), Value::Number(1.));
}

/// A two-way binding into model data must not write identical values back
/// to the model — host observers would see spurious `row_changed`
/// notifications.
#[test]
fn two_way_to_model_skips_identical_writes() {
    use i_slint_core::model::{ModelNotify, ModelTracker};

    struct CountingModel {
        rows: std::cell::RefCell<Vec<Value>>,
        notify: ModelNotify,
        sets: std::cell::Cell<usize>,
    }
    impl Model for CountingModel {
        type Data = Value;
        fn row_count(&self) -> usize {
            self.rows.borrow().len()
        }
        fn row_data(&self, row: usize) -> Option<Value> {
            self.rows.borrow().get(row).cloned()
        }
        fn set_row_data(&self, row: usize, data: Value) {
            self.sets.set(self.sets.get() + 1);
            self.rows.borrow_mut()[row] = data;
            self.notify.row_changed(row);
        }
        fn model_tracker(&self) -> &dyn ModelTracker {
            &self.notify
        }
    }

    let instance = compile(
        r#"
            export component TestCase inherits Rectangle {
                width: 300px;
                height: 300px;
                in property <[{v: int}]> model;
                for data in model: Rectangle {
                    property <int> val <=> data.v;
                    TouchArea {
                        x: 0; y: 0; width: 100px; height: 100px;
                        clicked => { val = 42; }
                    }
                    TouchArea {
                        x: 100px; y: 0; width: 100px; height: 100px;
                        clicked => { val += 1; }
                    }
                }
            }
        "#,
        "TestCase",
    );

    let row: slint_interpreter::Struct =
        [("v".to_string(), Value::Number(42.))].into_iter().collect();
    let model = Rc::new(CountingModel {
        rows: std::cell::RefCell::new(vec![Value::Struct(row)]),
        notify: ModelNotify::default(),
        sets: std::cell::Cell::new(0),
    });
    instance.set_property("model", Value::Model(ModelRc::from(model.clone()))).unwrap();

    // Writing the value the row already holds must not reach the model.
    send_mouse_click(&instance, 50.0, 50.0);
    assert_eq!(model.sets.get(), 0);

    // A different value must.
    send_mouse_click(&instance, 150.0, 50.0);
    assert_eq!(model.sets.get(), 1);
    assert_eq!(
        model.rows.borrow()[0],
        Value::Struct([("v".to_string(), Value::Number(43.))].into_iter().collect())
    );
}

/// Conditional pages switched by an integer property: element queries must
/// see only the current page's elements, in every direction of the switch.
#[test]
fn conditional_pages_element_queries() {
    use i_slint_backend_testing::ElementHandle;

    let instance = compile(
        r#"
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
                    Text {
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
        "#,
        "TestCase",
    );

    let count = |label: &str| ElementHandle::find_by_accessible_label(&instance, label).count();

    assert_eq!(count("dark-mode"), 1);
    assert_eq!(count("controls-body"), 1);
    assert_eq!(count("extra"), 0);

    instance.set_property("page", Value::Number(1.)).unwrap();
    assert_eq!(count("dark-mode"), 1);
    assert_eq!(count("extra"), 1);
    assert_eq!(count("controls-body"), 0);
    assert_eq!(count("text-edit-body"), 1);

    instance.set_property("page", Value::Number(0.)).unwrap();
    assert_eq!(count("controls-body"), 1);
    assert_eq!(count("text-edit-body"), 0);
}

/// A widget declared after a conditional sibling must still be
/// discoverable by its source-level type name.
#[test]
fn find_widget_after_conditional_sibling() {
    use i_slint_backend_testing::ElementHandle;

    let instance = compile(
        r#"
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
        "#,
        "TestCase",
    );

    let labels: Vec<String> = ElementHandle::find_by_element_type_name(&instance, "Switch")
        .filter_map(|h| h.accessible_label().map(|s| s.to_string()))
        .collect();
    assert_eq!(labels, vec!["Extra".to_string(), "Always".to_string()]);
}
