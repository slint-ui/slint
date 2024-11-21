// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Passes that fills the root component used_types.sub_components

use by_address::ByAddress;

use crate::langtype::ElementType;
use crate::object_tree::*;
use std::collections::HashSet;
use std::rc::Rc;

/// Fill the root_component's used_types.sub_components
pub fn collect_subcomponents(doc: &Document) {
    let mut result = vec![];
    let mut hash = HashSet::new();
    for component in doc.exported_roots().chain(doc.popup_menu_impl.iter().cloned()) {
        collect_subcomponents_recursive(&component, &mut result, &mut hash);
    }
    doc.used_types.borrow_mut().sub_components = result;
}

fn collect_subcomponents_recursive(
    component: &Rc<Component>,
    result: &mut Vec<Rc<Component>>,
    hash: &mut HashSet<ByAddress<Rc<Component>>>,
) {
    hash.insert(ByAddress(component.clone()));
    recurse_elem(&component.root_element, &(), &mut |elem: &ElementRc, &()| {
        let base_comp = match &elem.borrow().base_type {
            ElementType::Component(base_comp) => {
                if hash.contains(&ByAddress(base_comp.clone())) {
                    return;
                }
                base_comp.clone()
            }
            _ => return,
        };
        collect_subcomponents_recursive(&base_comp, result, hash);
        if base_comp.parent_element.upgrade().is_some() {
            // This is not a sub-component, but is a repeated component
            return;
        }
        result.push(base_comp);
    });
    for popup in component.popup_windows.borrow().iter() {
        collect_subcomponents_recursive(&popup.component, result, hash);
    }
}
