/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

//! Passes that fills the root component used_global

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::NamedReference;
use crate::object_tree::*;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Fill the root_component's used_globals
pub fn collect_globals(root_component: &Rc<Component>, _diag: &mut BuildDiagnostics) {
    let mut hash = BTreeMap::<String, Rc<Component>>::new();
    recurse_elem_including_sub_components_no_borrow(
        &root_component.root_element,
        &(),
        &mut |elem, _| {
            visit_all_named_references(elem, |nr: &mut NamedReference| {
                let element = nr.element.upgrade().unwrap();
                let global_component = element.borrow().enclosing_component.upgrade().unwrap();
                if global_component.is_global() {
                    hash.insert(global_component.id.clone(), global_component.clone());
                }
            })
        },
    );
    *root_component.used_global.borrow_mut() = hash.into_iter().map(|(_, v)| v).collect();
}
