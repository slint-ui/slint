// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

//! Passes that fills the root component used_types.sub_components

use by_address::ByAddress;

use crate::langtype::ElementType;
use crate::object_tree::*;
use std::collections::HashSet;
use std::rc::Rc;

/// Fill the root_component's used_types.sub_components
pub fn collect_subcomponents(root_component: &Rc<Component>) {
    let mut result = vec![];
    let mut hash = HashSet::new();

    collect_subcomponents_recursive(root_component, &mut result, &mut hash);

    root_component.used_types.borrow_mut().sub_components = result;
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
        result.push(base_comp);
    });
}
