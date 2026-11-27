// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Interpreter behavior driven from host code through the public API:
//! callbacks, functions, external models, input dispatch and element
//! queries. Nothing here depends on how the interpreter is implemented.

use i_slint_core::model::{Model, ModelRc, VecModel};
use slint_interpreter::{Compiler, ComponentHandle, ComponentInstance, Value};
use std::rc::Rc;

fn compile(code: &str, name: &str) -> ComponentInstance {
    static DEBUG_INFO: std::sync::Once = std::sync::Once::new();
    // SAFETY: the other tests wait on the `Once` before they read the environment.
    DEBUG_INFO.call_once(|| unsafe { std::env::set_var("SLINT_EMIT_DEBUG_INFO", "1") });
    i_slint_backend_testing::init_no_event_loop();
    let result = i_slint_compiler::poll_ready(
        Compiler::default().build_from_source(code.into(), Default::default()),
    );
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

#[cfg(feature = "internal")]
fn compile_collection_case(name: &str, component: &str) -> ComponentInstance {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cases/component_slots")
        .join(name);
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    compiler.compiler_configuration(i_slint_core::InternalToken).debug_info = true;
    compiler.compiler_configuration(i_slint_core::InternalToken).experimental_features =
        i_slint_compiler::ExperimentalFeatures::all();
    let result = i_slint_compiler::poll_ready(compiler.build_from_path(path));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    result.component(component).unwrap().create().unwrap()
}

#[cfg(feature = "internal")]
#[test]
fn indexed_collection_reads_follow_model_edits() {
    i_slint_backend_testing::init_no_event_loop();
    let indexed = compile_collection_case("slot_collection_indexed.slint", "TestCase");
    assert_eq!(indexed.get_property("test").unwrap(), Value::Bool(true));
    indexed.set_property("second-enabled", Value::Bool(true)).unwrap();
    assert_eq!(indexed.get_property("static-enabled").unwrap(), Value::Bool(true));
    assert_eq!(indexed.get_property("repeated-enabled").unwrap(), Value::Bool(true));
    let names = Rc::new(VecModel::from(vec![Value::String("X".into()), Value::String("Y".into())]));
    indexed.set_property("names", Value::Model(names.clone().into())).unwrap();
    indexed.set_property("index", Value::Number(0.)).unwrap();
    assert_eq!(indexed.get_property("repeated-title").unwrap(), Value::String("X".into()));
    names.remove(0);
    assert_eq!(indexed.get_property("repeated-title").unwrap(), Value::String("Y".into()));
    indexed.set_property("index", Value::Number(-1.)).unwrap();
    assert_eq!(indexed.get_property("static-title").unwrap(), Value::String("".into()));
    assert_eq!(indexed.get_property("repeated-title").unwrap(), Value::String("".into()));
}

#[cfg(feature = "internal")]
#[test]
fn repeated_collection_views_follow_page_instances() {
    use i_slint_backend_testing::{AccessibleRole, ElementHandle};
    i_slint_backend_testing::init_no_event_loop();
    let focused = compile_collection_case("slot_collection_dynamic_views.slint", "FocusTest");
    for index in [1, 0] {
        ElementHandle::find_by_accessible_label(&focused, &format!("Focus {index}"))
            .find(|element| element.accessible_role() == Some(AccessibleRole::Button))
            .unwrap()
            .invoke_accessible_default_action();
        assert_eq!(focused.get_property("focused-index").unwrap(), Value::Number(index as f64));
    }
    let instance = compile_collection_case("slot_collection_dynamic_views.slint", "TestCase");
    let button = |label: &str| {
        ElementHandle::find_by_accessible_label(&instance, label)
            .find(|element| element.accessible_role() == Some(AccessibleRole::Button))
            .unwrap()
    };
    button("Change B: 0").invoke_accessible_default_action();
    button("Change B: 1");
    button("Change D: 0").invoke_accessible_default_action();
    button("Change D: 1").invoke_accessible_default_action();
    assert_eq!(instance.get_property("pings").unwrap(), Value::Number(3.));
    button("Change A: 0");
    button("Change C: 0");
    button("Reset B").invoke_accessible_default_action();
    button("Reset D").invoke_accessible_default_action();
    button("Change B: 0");
    button("Change D: 0");
    button("Proxy B: 0").invoke_accessible_default_action();
    button("Change B: 10");
    button("Proxy D: 0");
    button("Reset B").invoke_accessible_default_action();
    button("Proxy B: 0");
    instance.invoke("insert-first", &[]).unwrap();
    button("Proxy B: 0").invoke_accessible_default_action();
    button("Change B: 10");
    button("Change A: 0");
    button("Change Zero: 0");
    instance.invoke("remove-first", &[]).unwrap();
    button("Proxy B: 10").invoke_accessible_default_action();
    button("Change B: 20");
    button("Reset B").invoke_accessible_default_action();
    button("Proxy B: 0");
    button("Animate B").invoke_accessible_default_action();
    i_slint_backend_testing::mock_elapsed_time(100);
    assert!(ElementHandle::find_by_accessible_label(&instance, "Progress B: 50").next().is_some());
    i_slint_backend_testing::mock_elapsed_time(100);
    assert!(ElementHandle::find_by_accessible_label(&instance, "Progress B: 100").next().is_some());

    {
        for mode in 0..3 {
            let instance =
                compile_collection_case("slot_collection_dynamic_views.slint", "LifetimeTest");
            let names =
                Rc::new(VecModel::from(vec![Value::String("A".into()), Value::String("B".into())]));
            instance.set_property("names", Value::Model(names.clone().into())).unwrap();
            let weak = instance.as_weak();
            instance
                .set_callback("mutate", move |_| {
                    let instance = weak.upgrade().unwrap();
                    match mode {
                        0 => {
                            names.remove(0);
                        }
                        1 => {
                            let replacement =
                                Rc::new(VecModel::from(vec![Value::String("Replacement".into())]));
                            instance
                                .set_property("names", Value::Model(replacement.into()))
                                .unwrap();
                        }
                        _ => instance.set_property("show-group", Value::Bool(false)).unwrap(),
                    }
                    ElementHandle::find_by_accessible_label(&instance, "Change B: 100").next();
                    Value::Void
                })
                .unwrap();
            ElementHandle::find_by_accessible_label(&instance, "Mutate A")
                .next()
                .unwrap()
                .invoke_accessible_default_action();
            assert_eq!(
                instance.get_global_property("CollectionProbe", "returned").unwrap(),
                Value::Number(1.)
            );
            assert_eq!(
                instance.get_global_property("CollectionProbe", "after").unwrap(),
                Value::Number(0.)
            );
            assert_eq!(
                instance.get_global_property("CollectionProbe", "after-call").unwrap(),
                Value::Number(0.)
            );
            assert_eq!(
                instance.get_global_property("CollectionProbe", "completed").unwrap(),
                Value::Number(1.)
            );
            if mode == 0 {
                assert!(
                    ElementHandle::find_by_accessible_label(&instance, "Change B: 100")
                        .next()
                        .is_some()
                );
            }
        }
    }
    let instance =
        compile_collection_case("slot_collection_dynamic_views.slint", "VirtualizedTest");
    let layout = || {
        instance.window().dispatch_event(i_slint_core::platform::WindowEvent::PointerMoved {
            position: i_slint_core::api::LogicalPosition::new(1., 1.),
        })
    };
    let button = |label: &str| {
        ElementHandle::find_by_accessible_label(&instance, label)
            .find(|element| element.accessible_role() == Some(AccessibleRole::Button))
            .unwrap()
    };
    layout();
    let Value::Number(count) = instance.get_property("constructions").unwrap() else { panic!() };
    assert!(count > 0. && count < 200.);
    button("Change 0:B: 0").invoke_accessible_default_action();
    button("Change 0:B: 1");
    instance.invoke("scroll-to", &[Value::Number(80.)]).unwrap();
    layout();
    button("Change 80:B: 0").invoke_accessible_default_action();
    button("Change 80:B: 1");
    let Value::Number(count) = instance.get_property("constructions").unwrap() else { panic!() };
    assert!(count < 200.);
    instance.invoke("scroll-to", &[Value::Number(0.)]).unwrap();
    layout();
    button("Change 0:B: 0");
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

/// With debug info, every item must report its element infos,
/// including the items the compiler synthesizes, such as the
/// wrapper for a `visible` binding. The code generators do the same. Otherwise
/// queries warn about missing debug info while walking the tree.
#[test]
fn every_item_has_element_infos() {
    use i_slint_core::items::ItemRc;

    let instance = compile(
        r#"
            import { CheckBox, LineEdit } from "std-widgets.slint";
            export component TestCase inherits Window {
                in property <bool> show-icon: true;
                VerticalLayout {
                    LineEdit {}
                    CheckBox { text: "Check"; }
                    icon := Rectangle { visible: root.show-icon; }
                }
            }
        "#,
        "TestCase",
    );

    fn visit(item: ItemRc, missing: &mut Vec<u32>) {
        if item.element_count().is_none() {
            missing.push(item.index());
        }
        let mut child = item.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            visit(c, missing);
        }
    }

    let item_tree = i_slint_core::window::WindowInner::from_pub(instance.window()).component();
    let mut missing = Vec::new();
    visit(ItemRc::new_root(item_tree), &mut missing);
    assert_eq!(missing, Vec::<u32>::new(), "items without element infos");

    let icons =
        i_slint_backend_testing::ElementHandle::find_by_element_id(&instance, "TestCase::icon");
    assert_eq!(icons.count(), 1);
}
