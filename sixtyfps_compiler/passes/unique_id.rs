/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use crate::diagnostics::BuildDiagnostics;
use crate::langtype::Type;
use crate::object_tree::*;
use std::collections::HashMap;
use std::rc::Rc;

/// This pass make sure that the id of the elements are unique
///
/// It currently does so by adding a number to the existing id
pub fn assign_unique_id(component: &Rc<Component>) {
    let mut count = 0;
    recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
        count += 1;
        let mut elem_mut = elem.borrow_mut();
        let old_id = if !elem_mut.id.is_empty() {
            elem_mut.id.clone()
        } else {
            elem_mut.base_type.to_string().to_ascii_lowercase()
        };
        elem_mut.id = format!("{}-{}", old_id, count);
    });

    rename_globals(component, count);
}

/// Give globals unique name
fn rename_globals(component: &Rc<Component>, mut count: u32) {
    for g in &component.used_types.borrow().globals {
        count += 1;
        let mut root = g.root_element.borrow_mut();
        if matches!(&root.base_type, Type::Builtin(_)) {
            // builtin global keeps its name
            root.id = g.id.clone();
        } else {
            root.id = format!("{}-{}", g.id, count);
        }
    }
}

/// Checks that all ids in the Component are unique
pub fn check_unique_id(doc: &Document, diag: &mut BuildDiagnostics) {
    for component in &doc.inner_components {
        check_unique_id_in_component(component, diag);
    }
}

fn check_unique_id_in_component(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    struct SeenId {
        element: ElementRc,
        warned: bool,
    }
    let mut seen_ids: HashMap<String, SeenId> = HashMap::new();

    recurse_elem(&component.root_element, &(), &mut |elem, _| {
        let elem_bor = elem.borrow();
        let id = &elem_bor.id;
        if !id.is_empty() {
            if let Some(other_loc) = seen_ids.get_mut(id) {
                debug_assert!(!Rc::ptr_eq(&other_loc.element, elem));
                let message = format!("duplicated element id '{}'. This used to be accepted in earlier version but will be an error in future versions", id);
                if !other_loc.warned {
                    diag.push_warning(message.clone(), &*other_loc.element.borrow());
                    other_loc.warned = true;
                }
                diag.push_warning(message, &*elem_bor);
            } else {
                seen_ids.insert(id.clone(), SeenId { element: elem.clone(), warned: false });
            }
        }
    })
}
