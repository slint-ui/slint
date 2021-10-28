/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Assign the Element::item_index on each elements
pub fn generate_item_indices(component: &std::rc::Rc<crate::object_tree::Component>) {
    let mut current_item_index: usize = 0;
    crate::object_tree::recurse_elem_level_order(&component.root_element, &mut |item_rc| {
        let item = item_rc.borrow();
        if item.base_type == crate::langtype::Type::Void {
        } else {
            if let crate::langtype::Type::Component(c) = &item.base_type {
                if c.parent_element.upgrade().is_some() {
                    generate_item_indices(c);
                }
            }
            item.item_index.set(current_item_index).unwrap();
            current_item_index += crate::generator::item_tree_element_size(item_rc);
        }
    });
    for p in component.popup_windows.borrow().iter() {
        generate_item_indices(&p.component)
    }
}
