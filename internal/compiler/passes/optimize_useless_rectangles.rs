// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Remove the rectangles that serves no purposes
//!
//! Rectangles which do not draw anything and have no x or y don't need to be in
//! the item tree, we can just remove them.

use crate::element_type::ElementType;
use crate::object_tree::*;
use std::rc::Rc;

pub fn optimize_useless_rectangles(root_component: &Rc<Component>) {
    recurse_elem_including_sub_components(root_component, &(), &mut |parent_, _| {
        let mut parent = parent_.borrow_mut();
        let children = std::mem::take(&mut parent.children);

        for elem in children {
            if !can_optimize(&elem) {
                parent.children.push(elem);
                continue;
            }

            parent.children.extend(std::mem::take(&mut elem.borrow_mut().children));
            if let Some(last) = parent.debug.last_mut() {
                last.element_boundary = true;
            }
            parent.debug.extend(std::mem::take(&mut elem.borrow_mut().debug));

            let enclosing = parent.enclosing_component.upgrade().unwrap();

            for popup in enclosing.popup_windows.borrow_mut().iter_mut() {
                if Rc::ptr_eq(&popup.parent_element, &elem) {
                    // parent element is use for x/y, and the position of the removed element is 0,0
                    popup.parent_element = parent_.clone();
                }
            }

            enclosing
                .optimized_elements
                .borrow_mut()
                .push(OptimizedElement { element: elem, former_parent: Rc::downgrade(parent_) });
        }
    });
}

/// Check that this is a element we can optimize
fn can_optimize(elem: &ElementRc) -> bool {
    let element = elem.borrow();
    if element.is_flickable_content || element.has_popup_child || element.is_component_placeholder {
        return false;
    };

    if element.child_of_layout {
        // The `LayoutItem` still has reference to this component, so we cannot remove it
        return false;
    }

    let base_type = match &element.base_type {
        ElementType::Builtin(base_type) if base_type.name == "Rectangle" => base_type,
        ElementType::Builtin(base_type) if base_type.native_class.class_name == "Empty" => {
            base_type
        }
        _ => return false,
    };

    // Keep elements involved in dynamic z-ordering: children with a z value, and the
    // parent whose children are sorted (removing it would merge them into another z group)
    if element.z_order.is_some() || element.has_dynamic_z_order() {
        return false;
    }

    let analysis = element.property_analysis.borrow();
    for coord in ["x", "y"] {
        if element.binding(coord).is_some() || analysis.get(coord).is_some_and(|a| a.is_set) {
            return false;
        }
    }
    if analysis.get("absolute-position").is_some_and(|a| a.is_read) {
        return false;
    }
    // Read by `ensure-visible()`, which needs the element as an item
    if analysis.get("").is_some_and(|a| a.is_read) {
        return false;
    }

    // Check that no Rectangle property are set
    !element
        .real_bindings()
        .map(|(property_name, _)| property_name)
        .chain(analysis.iter().filter(|(_, v)| v.is_set).map(|(k, _)| k))
        .any(|property_name| {
            !element.property_declarations.contains_key(property_name.as_str())
                && base_type.properties.contains_key(property_name.as_str())
        })
        && element.accessibility_props.0.is_empty()
}

/// We cannot optimize out a rectangle if there is a ensure-visible function on it
#[test]
fn keeps_elements_that_ensure_visible_refers_to() {
    let mut compiler_config =
        crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    // `SLINT_EMIT_DEBUG_INFO`, which CI sets, would skip the pass under test.
    compiler_config.debug_info = false;
    let mut test_diags = crate::diagnostics::BuildDiagnostics::default();
    let doc_node = crate::parser::parse(
        r#"
export component Foo {
    public function reveal() {
        plain.ensure-visible(ScrollBehavior.smooth);
    }
    Flickable {
        plain := Rectangle { Text { text: "y"; } }
        removed := Rectangle { Text { text: "y"; } }
    }
}
"#
        .into(),
        Some(crate::source_path::SourcePath::new("test.slint")),
        &mut test_diags,
    );
    let (doc, diag, _) =
        spin_on::spin_on(crate::compile_syntax_node(doc_node, test_diags, compiler_config.clone()));
    assert!(!diag.has_errors(), "slint compile error {:#?}", diag.to_string_vec());
    crate::llr::lower_to_item_tree::lower_to_item_tree(&doc, &compiler_config);

    let foo = doc.inner_components.iter().find(|c| c.id == "Foo").unwrap();
    let mut ids = Vec::new();
    recurse_elem_including_sub_components(foo, &(), &mut |elem, _| {
        // `assign_unique_id` appends a suffix such as `-5`.
        ids.push(elem.borrow().id.split('-').next().unwrap().to_string());
    });
    assert!(ids.iter().any(|id| id == "plain"), "{ids:?}");
    assert!(!ids.iter().any(|id| id == "removed"), "{ids:?}");
}
