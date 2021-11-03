/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Assign the Element::item_index on each elements

use std::cell::Cell;

/// The item indices are generated and assigned to the ElementRc's item_index for each
/// element in the component. The indices are local to the component.
///
/// For sub-components the structure of the tree becomes a little complicated, which is best
/// illustrated using an example:
/// ```60
/// SubCompo := Rectangle { Image {} }
/// MainCompo := Window {
///     TouchArea {}
///     SubCompo {}
///     Text {
///         Path {}
///     }
/// }
/// ```
/// The item tree for `MainCompo` with its local indices is as follows:
/// 0: Window (children: 3, children_offset: 1, parent_index: 0)
/// 1: TouchArea (children: 0, children_offset: X, parent_index: 0)
/// 2: Rectangle (children: 1, children_offset: 4, parent_index: 0) // SubCompo's root element
/// 3: Text      (children: 1, children_offset: 5, parent_index: 0)
/// 4: Image     (children: 0, children_offset: X, parent_index: 2) // SubCompo's child(ren)
/// 5: Path      (children: 0, children_offset: X, parent_index: 3)
pub fn generate_item_indices(component: &std::rc::Rc<crate::object_tree::Component>) {
    // In order to create the local indices like in the above example (0-5) we use the same function
    // that is also used for building the item tree. It recurses into all sub-components, but we skip
    // them, by using a nesting level as the state parameter.
    // The immediate children of for example the Window element are emitted first. When a sub-component
    // is encountered (like `SubCompo`) the root element is emitted, but the children later. This simulates
    // the structure as if the SubCompo was inlined, but it also means that the local item indices must be
    // counted continuously.

    let current_item_index: Cell<usize> = Cell::new(0);
    crate::generator::build_item_tree(
        component,
        0,
        |level, _, item_rc, _, _| {
            let item = item_rc.borrow();
            if item.base_type == crate::langtype::Type::Void {
            } else {
                if *level == 0 {
                    if let crate::langtype::Type::Component(c) = &item.base_type {
                        if c.parent_element.upgrade().is_some() {
                            generate_item_indices(c);
                        }
                    }
                    item.item_index.set(current_item_index.get()).unwrap();
                }
                current_item_index.set(current_item_index.get() + 1);
            }
        },
        |level, _, item_rc| {
            if *level == 0 {
                let item = item_rc.borrow();
                item.item_index.set(current_item_index.get()).unwrap();
            }

            level + 1
        },
    );
    for p in component.popup_windows.borrow().iter() {
        generate_item_indices(&p.component)
    }
}
