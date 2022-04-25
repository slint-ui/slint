// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Pass that lowers synthetic `accessible-*` properties

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::NamedReference;
use crate::object_tree::Component;
use crate::typeregister::TypeRegister;

use std::rc::Rc;

pub fn lower_accessibility_properties(component: &Rc<Component>, _diag: &mut BuildDiagnostics) {
    crate::object_tree::recurse_elem_including_sub_components_no_borrow(
        component,
        &(),
        &mut |elem, _| {
            for prop_name in crate::typeregister::RESERVED_ACCESSIBILITY_PROPERTIES
                .iter()
                .map(|x| x.0)
                .chain(std::iter::once("accessible-role"))
            {
                if elem.borrow().is_binding_set(prop_name, true) {
                    let nr = NamedReference::new(elem, prop_name);
                    elem.borrow_mut().accessibility_props.0.insert(prop_name.into(), nr);
                }
            }
        },
    )
}
