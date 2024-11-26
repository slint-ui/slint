// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Passes that fills the root component used_types.globals

use by_address::ByAddress;

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::NamedReference;
use crate::object_tree::*;
use std::collections::HashSet;
use std::rc::Rc;

/// Fill the root_component's used_types.globals
pub fn collect_globals(doc: &Document, _diag: &mut BuildDiagnostics) {
    doc.used_types.borrow_mut().globals.clear();
    let mut set = HashSet::new();
    let mut sorted_globals = vec![];
    for (_, ty) in &*doc.exports {
        if let Some(c) = ty.as_ref().left() {
            if c.is_global() && set.insert(ByAddress(c.clone())) {
                collect_in_component(c, &mut set, &mut sorted_globals);
                sorted_globals.push(c.clone());
            }
        }
    }
    doc.visit_all_used_components(|component| {
        collect_in_component(component, &mut set, &mut sorted_globals)
    });

    doc.used_types.borrow_mut().globals = sorted_globals;
}

fn collect_in_component(
    component: &Rc<Component>,
    global_set: &mut HashSet<ByAddress<Rc<Component>>>,
    sorted_globals: &mut Vec<Rc<Component>>,
) {
    let mut maybe_collect_global = |nr: &mut NamedReference| {
        let element = nr.element();
        let global_component = element.borrow().enclosing_component.upgrade().unwrap();
        if global_component.is_global() && global_set.insert(ByAddress(global_component.clone())) {
            collect_in_component(&global_component, global_set, sorted_globals);
            sorted_globals.push(global_component);
        }
    };
    visit_all_named_references(component, &mut maybe_collect_global);
}
