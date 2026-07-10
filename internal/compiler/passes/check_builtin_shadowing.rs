// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Diagnose declarations that shadow a builtin element member.
//!
//! A declaration may shadow a shadowable builtin member with a warning, but only as
//! long as no base component uses that member: inlining merges a base component's root
//! element into the inheriting element, and since references are looked up by name, the
//! base component's references to the builtin member would silently resolve to the
//! shadowing declaration instead. In that case, report an error.

use crate::diagnostics::BuildDiagnostics;
use crate::langtype::{ElementType, Type};
use crate::object_tree::{Component, Document, recurse_elem};
use crate::parser::SyntaxKind;
use std::rc::Rc;

pub fn check_builtin_shadowing(doc: &Document, diag: &mut BuildDiagnostics) {
    for component in &doc.inner_components {
        recurse_elem(&component.root_element, &(), &mut |elem, _| {
            let elem = elem.borrow();
            for (name, decl) in &elem.property_declarations {
                if !decl.shadows_builtin {
                    continue;
                }
                let Some(node) = &decl.node else { continue };
                let span =
                    node.child_node(SyntaxKind::DeclaredIdentifier).unwrap_or_else(|| node.clone());
                let builtin_kind = member_kind(&elem.base_type.lookup_property(name).property_type);

                // Look for a base component that uses the builtin member
                let mut base = elem.base_type.clone();
                let conflicting_base = loop {
                    let ElementType::Component(c) = base else { break None };
                    base = c.root_element.borrow().base_type.clone();
                    if uses_member_on_root(&c, name) {
                        break Some(c);
                    }
                };

                if let Some(c) = conflicting_base {
                    diag.push_error(
                        format!(
                            "Cannot shadow the builtin {builtin_kind} '{name}' because it is used by the base component '{}'",
                            c.id
                        ),
                        &span,
                    );
                } else {
                    diag.push_warning(
                        format!("'{name}' shadows the builtin {builtin_kind} of the same name"),
                        &span,
                    );
                }
            }
        });
    }
}

/// True if the component's own code uses `name` on its root element
fn uses_member_on_root(component: &Rc<Component>, name: &str) -> bool {
    let root = component.root_element.borrow();
    // bindings and change callbacks refer to the property by name,
    // everything else goes through a NamedReference
    root.named_references.is_referenced(name)
        || root.bindings.contains_key(name)
        || root.change_callbacks.contains_key(name)
}

/// The kind of element member that a type represents, for use in diagnostics
fn member_kind(ty: &Type) -> &'static str {
    match ty {
        Type::Callback { .. } => "callback",
        Type::Function { .. } => "function",
        _ => "property",
    }
}
