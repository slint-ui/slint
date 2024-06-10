// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Remove the rectangles that serves no purposes
//!
//! Rectangles which do not draw anything and have no x or y don't need to be in
//! the item tree, we can just remove them.

use crate::langtype::ElementType;
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

            enclosing.optimized_elements.borrow_mut().push(elem);
        }
    });
}

/// Check that this is a element we can optimize
fn can_optimize(elem: &ElementRc) -> bool {
    let e = elem.borrow();
    if e.is_flickable_viewport || e.has_popup_child || e.is_component_placeholder {
        return false;
    };

    if e.child_of_layout {
        // The `LayoutItem` still has reference to this component, so we cannot remove it
        return false;
    }

    let base_type = match &e.base_type {
        ElementType::Builtin(base_type) if base_type.name == "Rectangle" => base_type,
        ElementType::Builtin(base_type) if base_type.native_class.class_name == "Empty" => {
            base_type
        }
        _ => return false,
    };

    let analysis = e.property_analysis.borrow();
    for coord in ["x", "y"] {
        if e.bindings.contains_key(coord) || analysis.get(coord).map_or(false, |a| a.is_set) {
            return false;
        }
    }
    if analysis.get("absolute-position").map_or(false, |a| a.is_read) {
        return false;
    }

    // Check that no Rectangle property are set
    !e.bindings.keys().chain(analysis.iter().filter(|(_, v)| v.is_set).map(|(k, _)| k)).any(|k| {
        !e.property_declarations.contains_key(k.as_str())
            && base_type.properties.contains_key(k.as_str())
    }) && e.accessibility_props.0.is_empty()
}
