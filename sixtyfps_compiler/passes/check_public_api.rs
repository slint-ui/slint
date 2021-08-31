/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

//! Pass that check that the public api is ok and mark the property as exposed

use std::rc::Rc;

use crate::diagnostics::{BuildDiagnostics, DiagnosticLevel};
use crate::langtype::Type;
use crate::object_tree::{Component, Document};

pub fn check_public_api(doc: &Document, diag: &mut BuildDiagnostics) {
    check_public_api_component(&doc.root_component, diag);
    for (export_name, ty) in doc.exports() {
        if let Type::Component(c) = ty {
            if c.is_global() {
                // This global will become part of the public API.
                c.exported_global_names.borrow_mut().push(export_name.clone());
                check_public_api_component(c, diag)
            }
        }
    }
}

fn check_public_api_component(root_component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    root_component.root_element.borrow_mut().property_declarations.values_mut().for_each(|d| {
        if d.property_type.ok_for_public_api() {
            d.expose_in_public_api = true
        } else {
            diag.push_diagnostic(
                 format!("Properties of type {} are not supported yet for public API. The property will not be exposed", d.property_type),
                 &d.type_node(),
                 DiagnosticLevel::Warning
            );
        }
    });
}
