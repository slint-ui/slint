// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pass that lowers synthetic `accessible-*` properties

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{Expression, NamedReference};
use crate::langtype::EnumerationValue;
use crate::object_tree::{Component, ElementRc};

use smol_str::SmolStr;
use std::rc::Rc;

pub fn lower_accessibility_properties(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    crate::object_tree::recurse_elem_including_sub_components_no_borrow(
        component,
        &(),
        &mut |elem, _| {
            if elem.borrow().repeated.is_some() {
                return;
            };
            apply_builtin(elem);
            let accessible_role_set = match elem.borrow().bindings.get("accessible-role") {
                Some(role) => {
                    // Check if accessible-role is set to none (the optional literal) or a valid enum value
                    match super::ignore_debug_hooks(&role.borrow().expression) {
                        Expression::NoneValue => {
                            return; // No accessible role set
                        }
                        // Handle none potentially wrapped in a Cast to Optional(AccessibleRole)
                        Expression::Cast { from, .. } if matches!(from.as_ref(), Expression::NoneValue) => {
                            return; // No accessible role set
                        }
                        Expression::EnumerationValue(val) => {
                            debug_assert_eq!(val.enumeration.name, "AccessibleRole");
                        }
                        Expression::Cast { from, .. }
                            if matches!(from.as_ref(), Expression::EnumerationValue(_)) =>
                        {
                            // EnumerationValue wrapped in Cast to Optional(AccessibleRole)
                        }
                        expr => {
                            // Check if it's a const expression at least (better error)
                            if !expr.is_constant(None) {
                                diag.push_error(
                                    "The `accessible-role` property must be a constant expression".into(),
                                    &*role.borrow(),
                                );
                            }
                        }
                    }
                    true
                }
                // maybe it was set on the parent
                None => elem.borrow().is_binding_set("accessible-role", false),
            };

            for prop_name in crate::typeregister::reserved_accessibility_properties()
                .map(|x| x.0)
                .chain(std::iter::once("accessible-role"))
            {
                if accessible_role_set {
                    if elem.borrow().is_binding_set(prop_name, false) {
                        let nr = NamedReference::new(elem, SmolStr::new_static(prop_name));
                        elem.borrow_mut().accessibility_props.0.insert(prop_name.into(), nr);
                    }
                } else if let Some(b) = elem.borrow().bindings.get(prop_name) {
                    diag.push_error(
                        format!("The `{prop_name}` property can only be set in combination to `accessible-role`"),
                        &*b.borrow(),
                    );
                }
            }
        },
    )
}

fn apply_builtin(e: &ElementRc) {
    let bty = if let Some(bty) = e.borrow().builtin_type() { bty } else { return };
    if bty.name == "Text" {
        e.borrow_mut().set_binding_if_not_set("accessible-role".into(), || {
            let enum_ty = crate::typeregister::BUILTIN.with(|e| e.enums.AccessibleRole.clone());
            Expression::EnumerationValue(EnumerationValue {
                value: enum_ty.values.iter().position(|v| v == "text").unwrap(),
                enumeration: enum_ty,
            })
        });
        let text_prop = NamedReference::new(e, SmolStr::new_static("text"));
        e.borrow_mut().set_binding_if_not_set("accessible-label".into(), || {
            Expression::PropertyReference(text_prop)
        });
    } else if bty.name == "TextInput" {
        e.borrow_mut().set_binding_if_not_set("accessible-role".into(), || {
            let enum_ty = crate::typeregister::BUILTIN.with(|e| e.enums.AccessibleRole.clone());
            Expression::EnumerationValue(EnumerationValue {
                value: enum_ty.values.iter().position(|v| v == "text-input").unwrap(),
                enumeration: enum_ty,
            })
        });
        let text_prop = NamedReference::new(e, SmolStr::new_static("text"));
        e.borrow_mut().set_binding_if_not_set("accessible-value".into(), || {
            Expression::PropertyReference(text_prop)
        });
        let enabled_prop = NamedReference::new(e, SmolStr::new_static("enabled"));
        e.borrow_mut().set_binding_if_not_set("accessible-enabled".into(), || {
            Expression::PropertyReference(enabled_prop)
        });
        let read_only_prop = NamedReference::new(e, SmolStr::new_static("read-only"));
        e.borrow_mut().set_binding_if_not_set("accessible-read-only".into(), || {
            Expression::PropertyReference(read_only_prop)
        });
    } else if bty.name == "Image" {
        e.borrow_mut().set_binding_if_not_set("accessible-role".into(), || {
            let enum_ty = crate::typeregister::BUILTIN.with(|e| e.enums.AccessibleRole.clone());
            Expression::EnumerationValue(EnumerationValue {
                value: enum_ty.values.iter().position(|v| v == "image").unwrap(),
                enumeration: enum_ty,
            })
        });
    }
}
