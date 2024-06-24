// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::diagnostics::BuildDiagnostics;
use crate::langtype::ElementType;
use crate::object_tree::*;
use crate::typeregister::TypeRegister;
use std::rc::Rc;

pub fn lower_component_container(
    component: &Rc<Component>,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    let empty_type = type_register.empty_type();

    recurse_elem_including_sub_components_no_borrow(component, &None, &mut |elem, _| {
        if matches!(&elem.borrow().builtin_type(), Some(b) if b.name == "ComponentContainer") {
            diagnose_component_container(elem, diag);
            process_component_container(elem, &empty_type);
        }
        Some(elem.clone())
    })
}

fn diagnose_component_container(element: &ElementRc, diag: &mut BuildDiagnostics) {
    if !element.borrow().children.is_empty() {
        diag.push_error("ComponentContainers may not have children".into(), &*element.borrow());
    }
}

fn process_component_container(element: &ElementRc, empty_type: &ElementType) {
    let mut elem = element.borrow_mut();

    let embedded_element = Element::make_rc(Element {
        base_type: empty_type.clone(),
        id: elem.id.clone(),
        debug: elem.debug.clone(),
        enclosing_component: elem.enclosing_component.clone(),
        default_fill_parent: (true, true),
        is_legacy_syntax: elem.is_legacy_syntax,
        inline_depth: elem.inline_depth,
        is_component_placeholder: true,
        ..Default::default()
    });

    elem.children.push(embedded_element);
}
