/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Remove the rectangles that serves no purposes
//!
//! Rectangles which do not draw anything and have no x or y don't need to be in
//! the item tree, we can just remove them.

use crate::{langtype::Type, object_tree::*};
use std::rc::Rc;

pub fn optimize_useless_rectangles(root_component: &Rc<Component>) {
    recurse_elem_including_sub_components(root_component, &(), &mut |parent, _| {
        let mut parent = parent.borrow_mut();
        let children = std::mem::take(&mut parent.children);

        for elem in children {
            if !can_optimize(&elem) {
                parent.children.push(elem);
                continue;
            }

            parent.children.extend(std::mem::take(&mut elem.borrow_mut().children));

            parent
                .enclosing_component
                .upgrade()
                .unwrap()
                .optimized_elements
                .borrow_mut()
                .push(elem);
        }
    });
}

/// Check that this is a element we can optimize
fn can_optimize(elem: &ElementRc) -> bool {
    let e = elem.borrow();
    if e.is_flickable_viewport || e.child_of_layout {
        return false;
    };

    let base_type = match &e.base_type {
        Type::Builtin(base_type) if base_type.name == "Rectangle" => base_type,
        _ => return false,
    };

    // Check that no Rectangle property other than height and width are set
    let analysis = e.property_analysis.borrow();
    e.bindings
        .keys()
        .chain(analysis.iter().filter(|(_, v)| v.is_set).map(|(k, _)| k))
        .filter(|k| !matches!(k.as_str(), "height" | "width"))
        .filter(|k| {
            !e.property_declarations.contains_key(*k) && base_type.properties.contains_key(*k)
        })
        .next()
        .is_none()
}
