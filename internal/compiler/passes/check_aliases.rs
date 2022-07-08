// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Verify that aliases have proper default values

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::Expression;
use crate::langtype::Type;
use crate::object_tree::{Component, ElementRc};
use std::rc::Rc;

pub fn check_aliases(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    crate::object_tree::recurse_elem_including_sub_components(&component, &(), &mut |elem, _| {
        let base = if let Type::Component(base) = &elem.borrow().base_type {
            base.clone()
        } else {
            return;
        };
        for (prop, b) in &elem.borrow().bindings {
            if b.borrow().two_way_bindings.is_empty() {
                continue;
            }
            if !has_default_binding(&base.root_element, prop) {
                continue;
            }
            for nr in &b.borrow().two_way_bindings {
                if !has_default_binding(&nr.element(), nr.name()) {
                    diag.push_warning(
                        format!(
r#"Two way binding between the property '{prop}' with a default value to the property '{nr:?}' without value.
The current behavior is to keep the value from the left-hand-side, but this behavior will change in the next version to always keep the right-hand-side value.
This may cause panic at runtime. See https://github.com/slint-ui/slint/issues/1394
To fix this warning, add a default value to the property '{nr:?}'"#,
            ),
             &b.borrow().span);
                }
            }
        }
    });
}

/// return whether the property has an actual default binding value set
fn has_default_binding(elem: &ElementRc, name: &str) -> bool {
    if let Some(b) = elem.borrow().bindings.get(name) {
        if !matches!(b.borrow().expression, Expression::Invalid) {
            true
        } else {
            for nr in &b.borrow().two_way_bindings {
                if has_default_binding(&nr.element(), nr.name()) {
                    return true;
                }
            }
            false
        }
    } else if let Type::Component(base) = &elem.borrow().base_type {
        has_default_binding(&base.root_element, name)
    } else {
        false
    }
}
