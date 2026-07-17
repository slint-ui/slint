// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::{Value, dynamic_item_tree};

use smol_str::SmolStr;

use std::pin::Pin;

pub type DebugHookCallback = Box<dyn Fn(&str) -> Option<Value>>;

pub(crate) fn set_debug_hook_callback(
    component: Pin<&dynamic_item_tree::ItemTreeBox>,
    func: Option<DebugHookCallback>,
) {
    let Some(global_storage) = component.description().compiled_globals() else {
        return;
    };
    *(global_storage.debug_hook_callback.borrow_mut()) = func;
}

pub(crate) fn trigger_debug_hook(
    component_instance: &dynamic_item_tree::InstanceRef,
    id: SmolStr,
) -> Option<Value> {
    component_instance.description.compiled_globals().and_then(|global_storage| {
        let callback = global_storage.debug_hook_callback.borrow();
        callback.as_ref().and_then(|callback| callback(&id))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Compiler, ComponentInstance};
    use i_slint_compiler::object_tree::Element;
    use i_slint_core::{Property, graphics::ApproxEq};
    use std::{cell::RefCell, collections::HashMap, path::PathBuf, rc::Rc};

    fn compile_with_debug_hooks(code: &str) -> ComponentInstance {
        i_slint_backend_testing::init_no_event_loop();

        let mut compiler = Compiler::default();
        compiler.compiler_configuration(i_slint_core::InternalToken).debug_hooks =
            Some(std::hash::RandomState::new());
        let compile_result =
            spin_on::spin_on(compiler.build_from_source(code.to_string(), test_path()));
        assert!(!compile_result.has_errors(), "{:?}", compile_result.diagnostics);
        compile_result.components().next().unwrap().create().unwrap()
    }

    fn install_debug_hook_store(instance: &ComponentInstance) -> Store {
        let store: Store = Default::default();
        {
            let store = Rc::clone(&store);
            instance.set_debug_hook_callback(Some(Box::new(move |id: &str| -> Option<Value> {
                let mut m = (*store).borrow_mut();
                let p = m.entry(SmolStr::from(id)).or_insert_with(|| Box::pin(Property::new(None)));
                p.as_ref().get()
            })));
        }
        store
    }

    fn set_override(store: &Store, element_hash: u64, name: &str, value: Option<Value>) {
        let id = i_slint_compiler::passes::property_id(element_hash, &SmolStr::from(name));
        let mut store = (*store).borrow_mut();
        let override_property = store.entry(id).or_insert_with(|| Box::pin(Property::new(None)));
        (&**override_property).set(value);
    }

    // Make sure to not actually write this file, it's just a synthetic path
    fn test_path() -> PathBuf {
        PathBuf::from("/tmp/test.slint")
    }

    fn find_element(
        instance: &ComponentInstance,
        code: &str,
        search_term: &str,
    ) -> (Rc<RefCell<Element>>, u64) {
        let offset = code.find(search_term).unwrap() as u32;
        let (element, debug_index) = instance
            .element_node_at_source_code_position(&test_path(), offset)
            .first()
            .cloned()
            .expect("element resolved");
        let element_hash = element.borrow().debug[debug_index].element_hash;
        assert_ne!(element_hash, 0, "debug_hooks should populate element_hash");
        (element, element_hash)
    }

    // Editor-style override store + callback (must be installed before the first evaluation so
    // the hooked bindings register a dependency on the override properties while still `None`).
    type Store = Rc<RefCell<HashMap<SmolStr, Pin<Box<Property<Option<Value>>>>>>>;

    // Validates the "live drag" mechanism the visual editor relies on: with `debug_hooks`
    // enabled, a `set_debug_hook_callback` that reads a per-id `Property<Option<Value>>` lets the
    // editor reactively override a property's value (and revert it) without touching the source.
    // Setting the override property re-evaluates the hooked binding via Slint's dependency tracker.
    #[test]
    fn debug_hook_live_override() {
        let code = r#"
export component Win inherits Window {
    width: 300px;
    height: 300px;
    rect := Rectangle {
        x: 10px;
        y: 20px;
        width: 30px;
        height: 40px;
    }
}"#;

        let instance = compile_with_debug_hooks(code);

        let (element, element_hash) = find_element(&instance, code, "Rectangle");

        let store = install_debug_hook_store(&instance);

        let base = instance.element_positions(&element).first().expect("geometry").rect;

        set_override(&store, element_hash, "x", Some(Value::Number(100.0)));
        set_override(&store, element_hash, "width", Some(Value::Number(70.0)));
        let after = instance.element_positions(&element).first().expect("geometry").rect;
        assert!(
            after.origin.x.approx_eq(&(base.origin.x + 90.0)),
            "x override should shift the element by 90px (base {}, after {})",
            base.origin.x,
            after.origin.x
        );
        assert!(
            after.size.width.approx_eq(&(base.size.width + 40.0)),
            "width override should grow the element by 40px (base {}, after {})",
            base.size.width,
            after.size.width
        );

        set_override(&store, element_hash, "x", None);
        set_override(&store, element_hash, "width", None);
        let reverted = instance.element_positions(&element).first().expect("geometry").rect;
        assert!(reverted.origin.x.approx_eq(&base.origin.x), "x should revert");
        assert!(reverted.size.width.approx_eq(&base.size.width), "width should revert");
    }

    // Component-instance elements are hooked too: their unbound properties get synthetic hooks
    // that must be upgraded with the definition's default bindings during inlining (keeping the
    // *instance* element's hook id). Verifies that the defaults are preserved (regression: they
    // used to be clobbered, rendering repeated items transparent) and that instance properties
    // are live-overridable through the hook callback.
    #[test]
    fn debug_hook_component_instance_override() {
        let code = r#"
component Sub inherits Rectangle {
    in property <color> tint: blue;
    background: tint;
}
export component Win inherits Window {
    width: 300px;
    height: 300px;
    sub := Sub { x: 10px; y: 20px; width: 50px; height: 50px; }
    for _idx in 2: Sub { width: 10px; height: 10px; }
    out property <brush> sub-background: sub.background;
}"#;
        let instance = compile_with_debug_hooks(code);

        let store = install_debug_hook_store(&instance);

        let blue = Value::Brush(i_slint_core::Brush::SolidColor(i_slint_core::Color::from_rgb_u8(
            0, 0, 255,
        )));
        assert_eq!(instance.get_property("sub-background").unwrap(), blue);

        let (element, element_hash) = find_element(&instance, code, "Sub {");

        let red = Value::Brush(i_slint_core::Brush::SolidColor(i_slint_core::Color::from_rgb_u8(
            255, 0, 0,
        )));
        set_override(&store, element_hash, "background", Some(red.clone()));
        assert_eq!(instance.get_property("sub-background").unwrap(), red);
        set_override(&store, element_hash, "background", None);
        assert_eq!(instance.get_property("sub-background").unwrap(), blue);

        let base = instance.element_positions(&element).first().expect("geometry").rect;
        set_override(&store, element_hash, "x", Some(Value::Number(110.0)));
        let after = instance.element_positions(&element).first().expect("geometry").rect;
        assert!(
            (after.origin.x - base.origin.x - 100.0).abs() < 0.5,
            "x override should shift the instance by 100px (base {}, after {})",
            base.origin.x,
            after.origin.x
        );
        set_override(&store, element_hash, "x", None);

        // Overriding the injected transform-rotation hook must be possible (the Transform
        // wrapper element is reified around the instance) and must not affect the geometry.
        set_override(&store, element_hash, "transform-rotation", Some(Value::Number(45.0)));
        let rotated = instance.element_positions(&element).first().expect("geometry").rect;
        assert!(
            (rotated.origin.x - base.origin.x).abs() < 0.5,
            "rotation must not move the origin"
        );
        set_override(&store, element_hash, "transform-rotation", None);
    }

    // Regression test: debug hooks inject bindings for properties the element may not have
    // natively (geometry, transform-rotation). Every injected binding must end up on a property
    // that actually exists at runtime, for every kind of element — otherwise instantiation
    // aborts with "unknown property ... in ...". Exercise the special cases: the root element,
    // plain items, elements that become component roots later (PopupWindow — plain or through
    // component inheritance), non-item types (Timer), repeated and conditional elements,
    // layouts, menus, style widgets, and tooltips with custom content.
    #[test]
    fn debug_hooks_instantiate_special_elements() {
        let code = r#"
import { Button } from "std-widgets.slint";

component MyPopup inherits PopupWindow {
    Rectangle { background: yellow; }
}

export component Win inherits Window {
    width: 300px;
    height: 300px;

    MenuBar {
        Menu {
            title: "File";
            MenuItem { title: "Quit"; }
        }
    }

    rect := Rectangle {
        rotated := Rectangle { transform-rotation: 45deg; }
        scaled := Rectangle { transform-scale: 150%; }
        plain := Rectangle { }
    }

    covered := Rectangle {
        Tooltip {
            Rectangle { background: #222; }
        }
    }

    Button { text: "a widget"; }

    popup := PopupWindow {
        Text { text: "popup content"; }
    }
    my-popup := MyPopup { }
    callback show-the-popups();
    show-the-popups() => { popup.show(); my-popup.show(); }

    Timer { interval: 1s; running: false; }

    for _ in 3: Rectangle { width: 10px; }
    if true: Rectangle { height: 5px; }

    VerticalLayout {
        Rectangle { }
    }
}"#;

        let instance = compile_with_debug_hooks(code);

        // Showing the popups instantiates the popup components (their bindings are only set up then).
        instance.invoke("show-the-popups", &[]).unwrap();
    }

    // Enabling debug_hooks now also materializes hooked default bindings for unbound properties.
    // This must NOT change the rendered result when no override is set: wrapping default-geometry
    // bindings must preserve fill/implicit sizing, and injecting the type-default for unbound props
    // must equal their unbound value (e.g. the font sentinel that drives Window inheritance).
    #[test]
    fn debug_hooks_preserve_geometry() {
        i_slint_backend_testing::init_no_event_loop();

        let code = r#"
export component Win inherits Window {
    width: 300px;
    height: 200px;
    rect := Rectangle { }            // no explicit geometry -> fills the parent
    txt := Text { text: "Hello"; }   // implicit (font-dependent) size, inherited font
}"#;
        let geometries = |debug_hooks: bool| -> Vec<(f32, f32, f32, f32)> {
            let mut compiler = Compiler::default();
            if debug_hooks {
                compiler.compiler_configuration(i_slint_core::InternalToken).debug_hooks =
                    Some(std::hash::RandomState::new());
            }
            let r = spin_on::spin_on(compiler.build_from_source(code.to_string(), test_path()));
            assert!(!r.has_errors(), "{:?}", r.diagnostics);
            let instance = r.components().next().unwrap().create().unwrap();
            [code.find("Rectangle").unwrap(), code.find("Text").unwrap()]
                .into_iter()
                .map(|off| {
                    let (elem, _) = instance
                        .element_node_at_source_code_position(&test_path(), off as u32)
                        .first()
                        .cloned()
                        .expect("element");
                    let g = instance.element_positions(&elem).first().expect("geometry").rect;
                    (g.origin.x, g.origin.y, g.size.width, g.size.height)
                })
                .collect()
        };

        let without = geometries(false);
        let with = geometries(true);
        for (a, b) in without.iter().zip(with.iter()) {
            assert!(
                (a.0 - b.0).abs() < 0.5
                    && (a.1 - b.1).abs() < 0.5
                    && (a.2 - b.2).abs() < 0.5
                    && (a.3 - b.3).abs() < 0.5,
                "geometry differs with vs without debug_hooks: {a:?} vs {b:?}"
            );
        }
        // Sanity: the Rectangle actually filled the 300x200 window (so we know we compared real sizes).
        assert!((with[0].2 - 300.0).abs() < 0.5 && (with[0].3 - 200.0).abs() < 0.5);
    }
}
