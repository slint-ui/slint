// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `Opacity::need_layer` on the item trees the compiler produces.
//! The wrappers injected for `visible` and the transform properties sit between the `Opacity` and the authored element (#13668).

use i_slint_backend_testing::ElementRoot;
use i_slint_core::items::{Clip, ItemRc, Opacity, Transform};
use slint_interpreter::{Compiler, ComponentInstance, Value};

fn compile(code: &str) -> ComponentInstance {
    i_slint_backend_testing::init_no_event_loop();
    let result =
        spin_on::spin_on(Compiler::default().build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    result.component("TestCase").expect("component should compile").create().unwrap()
}

/// The `Opacity` items directly below the root, in declaration order.
/// Repeated elements contribute one item per instance.
fn opacity_items(instance: &ComponentInstance) -> Vec<ItemRc> {
    let mut items = Vec::new();
    let mut child = ItemRc::new_root(instance.item_tree()).first_child();
    while let Some(item) = child {
        assert!(item.downcast::<Opacity>().is_some(), "every root child should be an Opacity");
        child = item.next_sibling();
        items.push(item);
    }
    items
}

/// The subtree below `item` as nested type names, with drawn items written as `Item`.
fn shape(item: &ItemRc) -> String {
    let kind = if item.downcast::<Opacity>().is_some() {
        "Opacity"
    } else if item.downcast::<Clip>().is_some() {
        "Clip"
    } else if item.downcast::<Transform>().is_some() {
        "Transform"
    } else {
        "Item"
    };
    let mut children = Vec::new();
    let mut child = item.first_child();
    while let Some(c) = child {
        children.push(shape(&c));
        child = c.next_sibling();
    }
    if children.is_empty() { kind.to_string() } else { format!("{kind}({})", children.join(", ")) }
}

#[test]
fn leaf_behind_injected_wrappers_needs_no_layer() {
    let instance = compile(
        r#"
            export component TestCase inherits Window {
                width: 100px;
                height: 100px;
                in property <bool> shown: true;
                plain := Rectangle { background: red; opacity: 0.5; }
                with-visible := Rectangle { background: red; opacity: 0.5; visible: root.shown; }
                with-rotation := Rectangle { background: red; opacity: 0.5; transform-rotation: 10deg; }
                with-both := Rectangle {
                    background: red;
                    opacity: 0.5;
                    visible: root.shown;
                    transform-rotation: 10deg;
                }
                group := Rectangle {
                    background: red;
                    opacity: 0.5;
                    visible: root.shown;
                    Rectangle { background: blue; }
                }
                for i in [1, 2]: Rectangle { background: red; opacity: 0.5; visible: root.shown; }
            }
        "#,
    );
    let items = opacity_items(&instance);
    let shapes: Vec<String> = items.iter().map(shape).collect();
    assert_eq!(
        shapes,
        [
            "Opacity(Item)",
            "Opacity(Clip(Item))",
            "Opacity(Transform(Item))",
            "Opacity(Clip(Transform(Item)))",
            "Opacity(Clip(Item(Item)))",
            "Opacity(Clip(Item))",
            "Opacity(Clip(Item))",
        ]
    );

    let need_layer = |opacity| -> Vec<bool> {
        items.iter().map(|item| Opacity::need_layer(item, opacity)).collect()
    };
    let expected = [false, false, false, false, true, false, false];
    assert_eq!(need_layer(0.5), expected);

    // Hiding flips the visibility Clip, and must not change the layer decision.
    let visibility_clip = items[1].first_child().unwrap().downcast::<Clip>().unwrap();
    assert!(visibility_clip.as_pin_ref().is_visibility_clip());
    assert!(!visibility_clip.as_pin_ref().clip());
    instance.set_property("shown", Value::Bool(false)).unwrap();
    assert!(visibility_clip.as_pin_ref().clip());
    assert_eq!(need_layer(0.5), expected);

    assert_eq!(need_layer(1.0), [false; 7]);
}
