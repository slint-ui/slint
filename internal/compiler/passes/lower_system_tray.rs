// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{BuiltinFunction, Expression, NamedReference};
use crate::langtype::ElementType;
use crate::object_tree::*;
use smol_str::SmolStr;
use std::rc::Rc;

pub fn lower_system_tray(doc: &mut Document, _diag: &mut BuildDiagnostics) {
    doc.visit_all_used_components(|component| {
        let root_element = component.root_element.clone();
        if !matches!(
            &root_element.borrow().base_type,
            ElementType::Builtin(b) if b.name == "SystemTray"
        ) {
            return;
        }
        let menu_property = NamedReference::new(&root_element, SmolStr::new_static("menu"));
        let setup_tray = Expression::FunctionCall {
            function: BuiltinFunction::SetupSystemTray.into(),
            arguments: vec![
                Expression::PropertyReference(menu_property),
                Expression::ElementReference(Rc::downgrade(&root_element)),
            ],
            source_location: Some(root_element.borrow().to_source_location()),
        };
        component.init_code.borrow_mut().constructor_code.push(setup_tray);
    });
}
