// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pass that check that the public api is ok and mark the property as exposed

use std::rc::Rc;

use crate::diagnostics::{BuildDiagnostics, DiagnosticLevel};
use crate::object_tree::{Component, Document, PropertyVisibility};
use crate::CompilerConfiguration;
use itertools::Either;

pub fn check_public_api(
    doc: &mut Document,
    config: &CompilerConfiguration,
    diag: &mut BuildDiagnostics,
) {
    let last = doc.last_exported_component();

    match &config.components_to_generate {
        crate::ComponentsToGenerate::AllExportedWindows => doc.exports.retain(|export| {
            // Warn about exported non-window (and remove them from the export unless it's the last for compatibility)
            if let Either::Left(c) = &export.1 {
                if !c.is_global() && !super::ensure_window::inherits_window(c) {
                    let is_last = last.as_ref().is_some_and(|last| !Rc::ptr_eq(last, c));
                    if is_last {
                        diag.push_warning(format!("Exported component '{}' doesn't inherit Window. No code will be generated for it", export.0.name), &export.0.name_ident);
                        return false;
                    } else {
                        diag.push_warning(format!("Exported component '{}' doesn't inherit Window. This is deprecated", export.0.name), &export.0.name_ident);
                    }
                }
            }
            true
        }),
        // Only keep the last component if there is one
        crate::ComponentsToGenerate::LastComponent => doc.exports.retain(|export| {
            if let Either::Left(c) = &export.1 {
                c.is_global() || last.as_ref().map_or(true, |last| Rc::ptr_eq(last, c))
            } else {
                true
            } }),
        // Only keep the component with the given name
        crate::ComponentsToGenerate::ComponentWithName(name) => doc.exports.retain(|export| {
            if let Either::Left(c) = &export.1 {
                c.is_global() || &c.id == name
            } else {
                true
            }
        }),
    }

    for c in doc.exported_roots() {
        check_public_api_component(&c, diag);
    }
    for (export_name, e) in &*doc.exports {
        if let Some(c) = e.as_ref().left() {
            if c.is_global() {
                // This global will become part of the public API.
                c.exported_global_names.borrow_mut().push(export_name.clone());
                check_public_api_component(c, diag)
            }
        }
    }
}

fn check_public_api_component(root_component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    let mut root_elem = root_component.root_element.borrow_mut();
    let root_elem = &mut *root_elem;
    let mut pa = root_elem.property_analysis.borrow_mut();
    root_elem.property_declarations.iter_mut().for_each(|(n, d)| {
        if d.property_type.ok_for_public_api() {
            if d.visibility == PropertyVisibility::Private {
                root_component.private_properties.borrow_mut().push((n.clone(), d.property_type.clone()));
            } else {
                d.expose_in_public_api = true;
                if d.visibility != PropertyVisibility::Output {
                    pa.entry(n.to_string()).or_default().is_set = true;
                }
            }
        } else {
            diag.push_diagnostic(
                 format!("Properties of type {} are not supported yet for public API. The property will not be exposed", d.property_type),
                 &d.type_node(),
                 DiagnosticLevel::Warning
            );
        }
    });
}
