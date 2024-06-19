// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Remove the properties which are not used

use crate::object_tree::{Component, Document};
use std::collections::HashSet;

pub fn remove_unused_properties(doc: &Document) {
    fn recurse_remove_unused_properties(component: &Component) {
        crate::object_tree::recurse_elem_including_sub_components_no_borrow(
            component,
            &(),
            &mut |elem, _| {
                let mut to_remove = HashSet::new();
                for (prop, decl) in &elem.borrow().property_declarations {
                    if !decl.expose_in_public_api
                        && !elem.borrow().named_references.is_referenced(prop)
                        && !elem
                            .borrow()
                            .property_analysis
                            .borrow()
                            .get(prop)
                            .map_or(false, |v| v.is_used())
                    {
                        to_remove.insert(prop.to_owned());
                    }
                }
                let mut elem = elem.borrow_mut();
                for x in &to_remove {
                    elem.property_declarations.remove(x);
                    elem.property_analysis.borrow_mut().remove(x);
                    elem.bindings.remove(x);
                    elem.change_callbacks.remove(x);
                }
            },
        );
    }
    doc.visit_all_used_components(|component| recurse_remove_unused_properties(component))
}
