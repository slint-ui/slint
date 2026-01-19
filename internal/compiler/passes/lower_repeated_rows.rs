// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass turns the repeated Row elements (of grid layouts) to Empty elements
//! Non-repeated rows were already removed in lower_layout.rs
//! But repeated rows have to be kept (as a component, not as Empty elements) longer,
//! they're the enclosing component for their children.

use crate::object_tree::{self, Component};
use crate::typeregister::TypeRegister;
use std::rc::Rc;

pub(crate) fn lower_repeated_rows(component: &Rc<Component>, type_register: &TypeRegister) {
    object_tree::recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        let is_repeated_row = {
            if let Some(cell) = elem.borrow().grid_layout_cell.as_ref() {
                cell.borrow().child_items.is_some()
            } else {
                false
            }
        };

        if is_repeated_row {
            // Repeated Row in a grid layout
            elem.borrow_mut().base_type = type_register.empty_type();
        }
    });
}
