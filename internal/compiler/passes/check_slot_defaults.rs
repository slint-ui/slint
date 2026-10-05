// Copyright © 2026 Slint Developers
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;

use by_address::ByAddress;

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::object_tree::{Document, recurse_elem_no_borrow, visit_all_named_references_in_element};

pub fn check_slot_defaults(doc: &Document, diag: &mut BuildDiagnostics) {
    for component in &doc.inner_components {
        let mut owners = HashMap::new();
        for (name, point) in component.child_insertion_points.borrow().iter() {
            let parent = point.parent.borrow();
            for child in parent
                .children
                .iter()
                .skip(point.insertion_index)
                .take(point.default_children_count)
            {
                recurse_elem_no_borrow(child, &(), &mut |element, _| {
                    owners.insert(ByAddress(element.clone()), name.clone());
                });
            }
        }
        if owners.is_empty() {
            continue;
        }
        recurse_elem_no_borrow(&component.root_element, &(), &mut |element, _| {
            let owner = owners.get(&ByAddress(element.clone()));
            let mut invalid = Vec::new();
            let mut locations = Vec::new();
            for (_, binding) in element.borrow().bindings_including_synthetic() {
                let binding = binding.borrow();
                binding.value_expression().visit_recursive(&mut |expression| {
                    let target = match expression {
                        crate::expression_tree::Expression::PropertyReference(reference) => {
                            Some(reference.element())
                        }
                        crate::expression_tree::Expression::ElementReference(target) => {
                            target.upgrade()
                        }
                        crate::expression_tree::Expression::FunctionCall {
                            function:
                                crate::expression_tree::Callable::Function(reference)
                                | crate::expression_tree::Callable::Callback(reference),
                            ..
                        } => Some(reference.element()),
                        _ => None,
                    };
                    if let Some(target_owner) =
                        target.and_then(|target| owners.get(&ByAddress(target)))
                    {
                        if owner != Some(target_owner) {
                            locations.push((target_owner.clone(), binding.to_source_location()));
                        }
                    }
                });
            }
            visit_all_named_references_in_element(element, |reference| {
                if let Some(target_owner) = owners.get(&ByAddress(reference.element())) {
                    if owner != Some(target_owner) {
                        invalid.push(target_owner.clone());
                    }
                }
            });
            invalid.extend(locations.iter().map(|(name, _)| name.clone()));
            invalid.sort();
            invalid.dedup();
            for name in invalid {
                if let Some((_, location)) = locations.iter().find(|(target, _)| target == &name) {
                    diag.push_error(format!("Move references to the default content of slot '{name}' inside that content"), location);
                } else {
                    diag.push_error(format!("Move references to the default content of slot '{name}' inside that content"), &*element.borrow());
                }
            }
        });
    }
}
