// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Rejects references into the default content of a slot from outside that content,
//! since a caller can replace the content.

use std::collections::{BTreeSet, HashMap, HashSet};

use by_address::ByAddress;
use smol_str::SmolStr;

use crate::diagnostics::{BuildDiagnostics, SourceLocation, Spanned};
use crate::expression_tree::Expression;
use crate::object_tree::{
    Document, ElementRc, recurse_elem_no_borrow, visit_all_named_references_in_element,
    visit_element_expressions_with_location,
};

#[allow(clippy::mutable_key_type)]
pub fn check_slot_defaults(doc: &Document, diag: &mut BuildDiagnostics) {
    for component in &doc.inner_components {
        let mut owners = HashMap::new();
        let mut default_roots = Vec::new();
        recurse_elem_no_borrow(
            &component.root_element,
            &None,
            &mut |element, owner: &Option<SmolStr>| {
                let slot_default = element.borrow().slot_default.clone();
                if slot_default.is_some() {
                    default_roots.push(element.clone());
                }
                let owner = slot_default.or_else(|| owner.clone());
                if let Some(owner) = &owner {
                    owners.insert(ByAddress(element.clone()), owner.clone());
                }
                owner
            },
        );
        if owners.is_empty() {
            continue;
        }
        let mut defaults_kept_for_inlining = BTreeSet::new();
        recurse_elem_no_borrow(&component.root_element, &(), &mut |element, _| {
            let owner = owners.get(&ByAddress(element.clone()));
            let foreign_owner = |target: &ElementRc| {
                owners.get(&ByAddress(target.clone())).filter(|target| Some(*target) != owner)
            };
            let message = |name: &SmolStr| {
                format!(
                    "Move references to the default content of slot '{name}' inside that content"
                )
            };
            let location = element.borrow().to_source_location();
            let mut references = element_references(element, &location);
            let referenced: HashSet<_> =
                references.iter().map(|(target, _)| ByAddress(target.clone())).collect();
            visit_all_named_references_in_element(element, |reference| {
                let target = reference.element();
                if !referenced.contains(&ByAddress(target.clone())) {
                    references.push((target, location.clone()));
                }
            });

            let mut reported = BTreeSet::new();
            for (target, location) in &references {
                if let Some(name) = foreign_owner(target)
                    && reported.insert((name.clone(), location.span.offset, location.span.length))
                {
                    diag.push_error(message(name), location);
                    defaults_kept_for_inlining.insert(name.clone());
                }
            }
        });
        for root in default_roots {
            let mut root = root.borrow_mut();
            if root
                .slot_default
                .as_ref()
                .is_some_and(|name| defaults_kept_for_inlining.contains(name))
            {
                root.slot_default = None;
            }
        }
    }
}

fn element_references(
    element: &ElementRc,
    fallback: &SourceLocation,
) -> Vec<(ElementRc, SourceLocation)> {
    let mut references = Vec::new();
    visit_element_expressions_with_location(element, fallback, &mut |expression, location| {
        let target = match expression {
            Expression::ElementReference(target) => target.upgrade(),
            _ => expression.referenced_property().map(|reference| reference.element()),
        };
        references.extend(target.map(|target| (target, location.clone())));
    });
    references
}
