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

pub fn mark_library_globals(doc: &Document) {
    let mut used_types = doc.used_types.borrow_mut();
    used_types.globals.clone().iter().for_each(|component| {
        if let Some(library_info) = doc.library_exports.get(component.id.as_str()) {
            component.from_library.set(true);
            // Properties on a library-imported global may be written by the
            // library's own components or by its host Rust/C++ code via the
            // public API. The consumer document has no visibility into those
            // writes, so mark every non-`Input` property as `is_set_externally`
            // — this is the single source of truth that downstream passes
            // (`NamedReference::is_constant`, `inline_simple_expressions`,
            // etc.) already consult to decide whether a value is fixed at
            // compile time.
            let root = component.root_element.borrow();
            let mut analysis = root.property_analysis.borrow_mut();
            for (name, decl) in &root.property_declarations {
                if decl.visibility != PropertyVisibility::Input {
                    analysis.entry(name.clone()).or_default().is_set_externally = true;
                }
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

    /// `mark_library_globals` must record that non-`Input` properties of a
    /// library-imported global can be set externally. The flag is the single
    /// source of truth that downstream passes (`NamedReference::is_constant`,
    /// the LLR `inline_simple_expressions` pass, etc.) consult to decide
    /// whether a value is fixed at compile time, so without it those passes
    /// would silently drop bindings that read from the library global.
    ///
    /// Regression test for the bug where a binding such as
    /// `text: "\{LibGlobal.value}"` in the consumer was inlined to the
    /// global's compile-time default.
    #[test]
    fn mark_library_globals_marks_properties_as_externally_set() {
        let mut compiler_config =
            crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
        compiler_config.style = Some("fluent".into());
        let mut diag = crate::diagnostics::BuildDiagnostics::default();
        let doc_node = crate::parser::parse(
            r#"
export global LibGlobal {
    in-out property <int> value: 0;
    in property <bool> input-only: false;
}
export component App {
    out property <string> text-out: "\{LibGlobal.value}";
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

        // Simulate cross-module compilation: pretend `LibGlobal` was imported
        // from a library by adding it to `library_exports`, then re-run
        // `mark_library_globals`. Also clear `expose_in_public_api` (which
        // is not set on properties imported from another module) so the
        // existing `is_constant_impl` short-circuit on it doesn't mask the
        // behavior we're trying to verify.
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
        assert!(
            analysis.get("value").is_some_and(|a| a.is_set_externally),
            "in-out property of a library global must be marked is_set_externally"
        );
        assert!(
            !analysis.get("input-only").is_some_and(|a| a.is_set_externally),
            "input-only property must not be marked is_set_externally — the consumer is its writer"
        );
        drop(analysis);
        drop(root);

        assert!(
            !NamedReference::new(&global.root_element, "value".into()).is_constant(),
            "value on a library global must never be constant"
        );
    }
}
