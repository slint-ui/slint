// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Pass that lowers synthetic `accessible-*` properties

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{Expression, NamedReference};
use crate::object_tree::Component;

use std::rc::Rc;

pub fn lower_accessibility_properties(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    crate::object_tree::recurse_elem_including_sub_components_no_borrow(
        component,
        &(),
        &mut |elem, _| {
            let accessible_role_set = match elem.borrow().bindings.get("accessible-role") {
                Some(role) => {
                    if let Expression::EnumerationValue(val) = &role.borrow().expression {
                        debug_assert_eq!(val.enumeration.name, "AccessibleRole");
                        debug_assert_eq!(val.enumeration.values[0], "none");
                        if val.value == 0 {
                            return;
                        }
                    } else {
                        diag.push_error(
                            "The `accessible-role` property must be a constant expression".into(),
                            &*role.borrow(),
                        );
                    }
                    true
                }
                // maybe it was set on the parent
                None => elem.borrow().is_binding_set("accessible-role", false),
            };

            for prop_name in crate::typeregister::RESERVED_ACCESSIBILITY_PROPERTIES
                .iter()
                .map(|x| x.0)
                .chain(std::iter::once("accessible-role"))
            {
                if accessible_role_set {
                    if elem.borrow().is_binding_set(prop_name, true) {
                        let nr = NamedReference::new(elem, prop_name);
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
