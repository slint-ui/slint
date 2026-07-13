// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass fills the root component used_types.globals

#![allow(clippy::mutable_key_type)] // NamedReference/Component keys intentionally use interior mutability

use by_address::ByAddress;
use smol_str::format_smolstr;

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::NamedReference;
use crate::object_tree::*;
use std::collections::HashSet;
use std::rc::Rc;

/// Fill the root_component's used_types.globals
pub fn collect_globals(doc: &Document, _diag: &mut BuildDiagnostics) {
    doc.used_types.borrow_mut().globals.clear();
    let mut set = HashSet::new();
    let mut sorted_globals = Vec::new();
    for (_, ty) in &*doc.exports {
        if let Some(c) = ty.as_ref().left()
            && c.is_global()
            && set.insert(ByAddress(c.clone()))
        {
            collect_in_component(c, &mut set, &mut sorted_globals);
            sorted_globals.push(c.clone());
        }
    }
    doc.visit_all_used_components(|component| {
        collect_in_component(component, &mut set, &mut sorted_globals)
    });

    doc.used_types.borrow_mut().globals = sorted_globals;
}

/// Properties of library-imported globals may be modified by the library at runtime via bindings or
/// public API. Mark the properties as externally set so that they do not get treated as constants
/// by future passes.
/// Additionally register the globals as imported from external libraries.
pub fn mark_library_globals(doc: &Document) {
    let mut used_types = doc.used_types.borrow_mut();
    used_types.globals.clone().iter().for_each(|component| {
        if let Some(library_info) = doc.library_exports.get(component.id.as_str()) {
            component.from_library.set(true);
            let root = component.root_element.borrow();
            let mut analysis = root.property_analysis.borrow_mut();
            for name in root.property_declarations.keys() {
                let entry = analysis.entry(name.clone()).or_default();
                entry.is_set_externally = true;
                entry.is_read_externally = true;
            }
            used_types.library_types_imports.push((component.id.clone(), library_info.clone()));
            used_types
                .library_types_imports
                .push((format_smolstr!("Inner{}", component.id.clone()), library_info.clone()));
        }
    });
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks that `mark_library_globals` sets `is_set_externally` and
    /// `is_read_externally` on every property of a library-imported global,
    /// regardless of visibility.
    ///
    /// Regression for the bug where consumer bindings reading from a library global
    /// were inlined to the global's compile-time default.
    #[test]
    fn mark_library_globals_marks_properties_as_externally_used() {
        let mut compiler_config =
            crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
        compiler_config.style = Some("fluent".into());
        let mut diag = crate::diagnostics::BuildDiagnostics::default();
        let doc_node = crate::parser::parse(
            r#"
export global LibGlobal {
    in-out property <int> value: 0;
    in property <int> count: 5;
    out property <int> ready: 9;
}
export component App {
    out property <string> text-out: "\{LibGlobal.value} \{LibGlobal.count} \{LibGlobal.ready}";
}
"#
            .into(),
            Some(std::path::Path::new("test.slint")),
            &mut diag,
        );
        let (mut doc, diag, _) =
            spin_on::spin_on(crate::compile_syntax_node(doc_node, diag, compiler_config));
        assert!(!diag.has_errors(), "compile error: {:?}", diag.to_string_vec());

        let global = doc
            .used_types
            .borrow()
            .globals
            .iter()
            .find(|g| g.id == "LibGlobal")
            .expect("LibGlobal not found")
            .clone();

        // Simulate a library import by adding `LibGlobal` to `library_exports`
        // and re-running `mark_library_globals`. Clear `expose_in_public_api`
        // so the `is_constant_impl` short-circuit doesn't mask the result.
        doc.library_exports.insert(
            "LibGlobal".to_string(),
            crate::typeloader::LibraryInfo {
                name: "Lib".into(),
                package: "lib".into(),
                module: None,
                exports: Vec::new(),
            },
        );
        for (_, d) in global.root_element.borrow_mut().property_declarations.iter_mut() {
            d.expose_in_public_api = false;
        }
        global.root_element.borrow().property_analysis.borrow_mut().clear();

        mark_library_globals(&doc);

        let root = global.root_element.borrow();
        let analysis = root.property_analysis.borrow();
        // `private` is omitted — `remove_unused_properties` strips it before
        // we re-invoke `mark_library_globals`; `bapp` covers that path.
        for prop in ["value", "count", "ready"] {
            let a = analysis
                .get(prop)
                .unwrap_or_else(|| panic!("{prop}: no analysis entry for library global property"));
            assert!(
                a.is_set_externally,
                "{prop}: every property on a library global must be marked is_set_externally \
                 — the library or its host code may write it at runtime regardless of visibility"
            );
            assert!(
                a.is_read_externally,
                "{prop}: every property on a library global must be marked is_read_externally \
                 — the library's bindings or host code may read it"
            );
        }
        drop(analysis);
        drop(root);

        for prop in ["value", "count", "ready"] {
            assert!(
                !NamedReference::new(&global.root_element, prop.into()).is_constant(),
                "{prop} on a library global must never be constant"
            );
        }
    }
}
