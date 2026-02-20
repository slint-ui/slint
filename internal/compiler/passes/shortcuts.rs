// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{
    collections::{HashMap, hash_map::Entry},
    rc::Rc,
};

use crate::{
    diagnostics::BuildDiagnostics,
    expression_tree::Expression,
    object_tree::{Component, recurse_elem_including_sub_components},
};

pub fn warn_duplicates(component: &Rc<Component>, diagnostics: &mut BuildDiagnostics) {
    recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
        let elem = elem.borrow();
        let is_focus_scope =
            elem.builtin_type().map(|builtin| builtin.name == "FocusScope").unwrap_or_default();
        if is_focus_scope {
            let shortcuts = elem
                .children
                .iter()
                .filter_map(|child| {
                    let child = child.borrow();
                    if child.builtin_type().map(|b| b.name == "Shortcut").unwrap_or_default() {
                        Some(child)
                    } else {
                        None
                    }
                })
                .filter_map(|shortcut| {
                    let Some(keys_expr) = shortcut.bindings.get("keys") else {
                        return None;
                    };
                    let keys_expr = keys_expr.borrow();
                    if let Expression::KeyboardShortcut(ref keys) = keys_expr.expression {
                        Some((keys.clone(), keys_expr.clone()))
                    } else {
                        None
                    }
                });

            let mut seen_shortcuts = HashMap::new();
            for (shortcut, span) in shortcuts {
                match seen_shortcuts.entry(shortcut.clone()) {
                    Entry::Vacant(entry) => {
                        entry.insert(span);
                    }
                    Entry::Occupied(first) => {
                        diagnostics.push_warning(
                            "This `Shortcut` element has the same keys as an existing shortcut - it is undefined which shortcut activates".into(),
                            &span,
                        );
                        diagnostics
                            .push_note("First duplicate Shorcut defined here".into(), first.get());
                    }
                }
            }
        }
    });
}
