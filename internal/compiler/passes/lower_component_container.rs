// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use crate::diagnostics::BuildDiagnostics;
use crate::object_tree::*;
use std::rc::Rc;

pub fn lower_component_container(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    recurse_elem_including_sub_components_no_borrow(component, &None, &mut |elem, _| {
        if matches!(&elem.borrow().builtin_type(), Some(b) if b.name == "ComponentContainer") {
            diagnose_component_container(elem, diag);
        }
        Some(elem.clone())
    })
}

fn diagnose_component_container(element: &ElementRc, diag: &mut BuildDiagnostics) {
    if !element.borrow().children.is_empty() {
        diag.push_error("ComponentContainers may not have children".into(), &*element.borrow());
        return;
    }
}
