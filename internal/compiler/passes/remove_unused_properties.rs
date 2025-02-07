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
                let mut elem = elem.borrow_mut();
                let mut to_remove = HashSet::new();
                for (prop, decl) in &elem.property_declarations {
                    if !decl.expose_in_public_api
                        && !elem.named_references.is_referenced(prop)
                        && !elem.property_analysis.borrow().get(prop).is_some_and(|v| v.is_used())
                        && !elem.change_callbacks.contains_key(prop)
                    {
                        to_remove.insert(prop.to_owned());
                    }
                }
                for x in &to_remove {
                    elem.property_declarations.remove(x);
                    elem.property_analysis.borrow_mut().remove(x);
                    elem.bindings.remove(x);
                }
                // Remove changed callbacks over properties that are not materialized as they are not used
                let mut change_callbacks = std::mem::take(&mut elem.change_callbacks);
                change_callbacks.retain(|prop, _| {
                    super::materialize_fake_properties::has_declared_property(&elem, prop)
                });
                elem.change_callbacks = change_callbacks;
            },
        );
    }
    doc.visit_all_used_components(|component| recurse_remove_unused_properties(component))
}
