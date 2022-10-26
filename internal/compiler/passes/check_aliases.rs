// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Verify that aliases have proper default values

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::Expression;
use crate::langtype::ElementType;
use crate::object_tree::{Component, ElementRc};
use std::rc::Rc;

pub fn check_aliases(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    crate::object_tree::recurse_elem_including_sub_components(&component, &(), &mut |elem, _| {
        let base = if let ElementType::Component(base) = &elem.borrow().base_type {
            base.clone()
        } else {
            return;
        };
        for (prop, b) in &elem.borrow().bindings {
            if b.borrow().two_way_bindings.is_empty() {
                continue;
            }
            if let Some(lhs_prio) = explicit_binding_priority(&base.root_element, prop) {
                for nr in &b.borrow().two_way_bindings {
                    if explicit_binding_priority(&nr.element(), nr.name())
                        .map_or(true, |rhs_prio| rhs_prio > lhs_prio.saturating_add(1))
                    {
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
        }
    });
}

/// Return whether the property has an actual binding value set,
/// in that case return its priority
fn explicit_binding_priority(elem: &ElementRc, name: &str) -> Option<i32> {
    if let Some(b) = elem.borrow().bindings.get(name) {
        if !matches!(b.borrow().expression, Expression::Invalid) {
            Some(b.borrow().priority)
        } else {
            for nr in &b.borrow().two_way_bindings {
                if let Some(p) = explicit_binding_priority(&nr.element(), nr.name()) {
                    return Some(p.saturating_add(b.borrow().priority) - 1);
                }
            }
            None
        }
    } else if let ElementType::Component(base) = &elem.borrow().base_type {
        explicit_binding_priority(&base.root_element, name).map(|p| p.saturating_add(1))
    } else {
        None
    }
}
