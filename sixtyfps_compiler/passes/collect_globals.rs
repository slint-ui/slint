/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

//! Passes that fills the root component used_types.globals

use by_address::ByAddress;

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::NamedReference;
use crate::langtype::Type;
use crate::object_tree::*;
use std::collections::HashSet;

/// Fill the root_component's used_types.globals
pub fn collect_globals(doc: &Document, _diag: &mut BuildDiagnostics) {
    let mut set = HashSet::new();
    for (_, ty) in doc.exports() {
        if let Type::Component(c) = ty {
            if c.is_global() {
                set.insert(ByAddress(c.clone()));
                c.exported_global.set(true);
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
    let mut used_types = doc.root_component.used_types.borrow_mut();
    used_types.globals = set.into_iter().map(|x| x.0).collect();
    used_types.globals.sort_by(|a, b| a.id.cmp(&b.id));
}
