// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::diagnostics::BuildDiagnostics;
use crate::langtype::ElementType;
use crate::object_tree::*;
use smol_str::{format_smolstr, SmolStr, ToSmolStr};
use std::collections::HashMap;
use std::rc::Rc;

/// This pass make sure that the id of the elements are unique
///
/// It currently does so by adding a number to the existing id
pub fn assign_unique_id(doc: &Document) {
    let mut count = 0;
    doc.visit_all_used_components(|component| {
        if !component.is_global() {
            assign_unique_id_in_component(component, &mut count)
        }
    });
    rename_globals(doc, count);
}

fn assign_unique_id_in_component(component: &Rc<Component>, count: &mut u32) {
    recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
        *count += 1;
        let mut elem_mut = elem.borrow_mut();
        let old_id = if !elem_mut.id.is_empty() {
            elem_mut.id.clone()
        } else {
            elem_mut.base_type.to_smolstr().to_ascii_lowercase().into()
        };
        elem_mut.id = format_smolstr!("{}-{}", old_id, count);

        let enclosing = elem_mut.enclosing_component.upgrade().unwrap();
        if Rc::ptr_eq(&elem, &enclosing.root_element) {
            for o in enclosing.optimized_elements.borrow().iter() {
                *count += 1;
                let mut elem_mut = o.borrow_mut();
                elem_mut.id = format_smolstr!("optimized-{}-{}", elem_mut.id, count);
            }
        }
    });
}

/// Give globals unique name
fn rename_globals(doc: &Document, mut count: u32) {
    for g in &doc.used_types.borrow().globals {
        count += 1;
        let mut root = g.root_element.borrow_mut();
        if matches!(&root.base_type, ElementType::Builtin(_)) {
            // builtin global keeps its name
            root.id.clone_from(&g.id);
        } else if let Some(s) = g.exported_global_names.borrow().first() {
            root.id = s.to_smolstr();
        } else {
            root.id = format_smolstr!("{}-{}", g.id, count);
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
        error_reported: bool,
    }
    let mut seen_ids: HashMap<SmolStr, SeenId> = HashMap::new();

    recurse_elem(&component.root_element, &(), &mut |elem, _| {
        let elem_bor = elem.borrow();
        let id = &elem_bor.id;
        if !id.is_empty() {
            if let Some(other_loc) = seen_ids.get_mut(id) {
                debug_assert!(!Rc::ptr_eq(&other_loc.element, elem));
                let message = format!("duplicated element id '{}'", id);
                if !other_loc.error_reported {
                    diag.push_error(message.clone(), &*other_loc.element.borrow());
                    other_loc.error_reported = true;
                }
                diag.push_error(message, &*elem_bor);
            } else {
                seen_ids
                    .insert(id.clone(), SeenId { element: elem.clone(), error_reported: false });
            }
        }
    })
}
