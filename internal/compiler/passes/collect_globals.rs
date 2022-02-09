// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Passes that fills the root component used_types.globals

use by_address::ByAddress;

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::NamedReference;
use crate::langtype::Type;
use crate::object_tree::*;
use std::collections::HashSet;

/// Fill the root_component's used_types.globals
pub fn collect_globals(doc: &Document, _diag: &mut BuildDiagnostics) {
    doc.root_component.used_types.borrow_mut().globals.clear();
    let mut set = HashSet::new();
    for (_, ty) in doc.exports() {
        if let Type::Component(c) = ty {
            if c.is_global() {
                set.insert(ByAddress(c.clone()));
            }
        }
    }
    let mut maybe_collect_global = |nr: &mut NamedReference| {
        let element = nr.element();
        let global_component = element.borrow().enclosing_component.upgrade().unwrap();
        if global_component.is_global() {
            set.insert(ByAddress(global_component));
        }
    };
    visit_all_named_references(&doc.root_component, &mut maybe_collect_global);
    for component in &doc.root_component.used_types.borrow().sub_components {
        visit_all_named_references(component, &mut maybe_collect_global);
    }
    let mut used_types = doc.root_component.used_types.borrow_mut();
    used_types.globals = set.into_iter().map(|x| x.0).collect();
    used_types.globals.sort_by(|a, b| a.id.cmp(&b.id));
}
