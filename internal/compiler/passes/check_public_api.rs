// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pass that check that the public api is ok and mark the property as exposed

use std::rc::Rc;

use crate::diagnostics::{BuildDiagnostics, DiagnosticLevel};
use crate::langtype::ElementType;
use crate::object_tree::{Component, Document, ExportedName, PropertyVisibility};
use crate::{CompilerConfiguration, ComponentSelection};
use itertools::Either;

pub fn check_public_api(
    doc: &mut Document,
    config: &CompilerConfiguration,
    diag: &mut BuildDiagnostics,
) {
    let last = doc.last_exported_component();

    if last.is_none() && !matches!(&config.components_to_generate, ComponentSelection::Named(_)) {
        let last_imported = doc
            .node
            .as_ref()
            .and_then(|n| {
                let import_node = n.ImportSpecifier().last()?;
                let import = crate::typeloader::ImportedName::extract_imported_names(&import_node.ImportIdentifierList()?).last()?;
                let ElementType::Component(c) = doc.local_registry.lookup_element(&import.internal_name).ok()? else { return None };
                diag.push_warning(format!("No component is exported. The last imported component '{}' will be used. This is deprecated", import.internal_name), &import_node);
                let exported_name = ExportedName{ name: import.internal_name, name_ident: import_node.into() };
                Some((exported_name, Either::Left(c)))
            });
        doc.exports.add_reexports(last_imported, diag);
    }

    match &config.components_to_generate {
        ComponentSelection::ExportedWindows => doc.exports.retain(|export| {
            // Warn about exported non-window (and remove them from the export unless it's the last for compatibility)
            if let Either::Left(c) = &export.1
                && !c.is_global() && !super::windows::inherits_window(c) {
                    let is_last = last.as_ref().is_some_and(|last| !Rc::ptr_eq(last, c));
                    if is_last {
                        diag.push_warning(format!("Exported component '{}' doesn't inherit Window. No code will be generated for it", export.0.name), &export.0.name_ident);
                        return false;
                    } else if config.library_name.is_none () {
                        diag.push_warning(format!("Exported component '{}' doesn't inherit Window. This is deprecated", export.0.name), &export.0.name_ident);
                    }
                }
            true
        }),
        // Only keep the last component if there is one
        ComponentSelection::LastExported => doc.exports.retain(|export| {
            if let Either::Left(c) = &export.1 {
                c.is_global() || last.as_ref().is_none_or(|last| Rc::ptr_eq(last, c))
            } else {
                true
            }
        }),
        // Only keep the component with the given name
        ComponentSelection::Named(name) => {
            doc.exports.retain(|export| {
                if let Either::Left(c) = &export.1 {
                    c.is_global() || c.id == name
                } else {
                    true
                }
            });
            if doc.last_exported_component().is_none() {
                // We maybe requested to preview a non-exported component.
                if let Ok(ElementType::Component(c)) = doc.local_registry.lookup_element(name)
                    && let Some(name_ident) = c.node.as_ref().map(|n| n.DeclaredIdentifier().into()) {
                        doc.exports.add_reexports(
                            [(ExportedName{ name: name.into(), name_ident }, Either::Left(c))],
                            diag,
                        );
                    }
            }
        },
    }

    for c in doc.exported_roots() {
        check_public_api_component(&c, diag);
    }
    for (export_name, e) in &*doc.exports {
        if let Some(c) = e.as_ref().left()
            && c.is_global()
        {
            // This global will become part of the public API.
            c.exported_global_names.borrow_mut().push(export_name.clone());
            check_public_api_component(c, diag)
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
                    pa.entry(n.clone()).or_default().is_set = true;
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

/// After inlining, interface API inherited from base components may have been added to exported roots with
/// `is_from_interface` set but `expose_in_public_api` still false. This function marks them for the public API.
pub fn expose_inherited_interface_properties(doc: &Document) {
    for c in doc.exported_roots() {
        let mut root_elem = c.root_element.borrow_mut();
        let root_elem = &mut *root_elem;
        let mut pa = root_elem.property_analysis.borrow_mut();
        for (n, d) in root_elem.property_declarations.iter_mut() {
            if d.is_from_interface
                && !d.expose_in_public_api
                && d.property_type.ok_for_public_api()
                && d.visibility != PropertyVisibility::Private
            {
                d.expose_in_public_api = true;
                if d.visibility != PropertyVisibility::Output {
                    pa.entry(n.clone()).or_default().is_set = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {

    /// Verify that when a  component implements an interface, the exported component exposes only
    /// the interface API (properties, callbacks, functions) and not other non-interface properties
    /// from the base.
    fn component_exposes_only_interface_api_impl(slint: &str) {
        let mut compiler_config =
            crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
        compiler_config.enable_experimental = true;
        compiler_config.style = Some("fluent".into());
        let mut test_diags = crate::diagnostics::BuildDiagnostics::default();
        test_diags.enable_experimental = true;
        let doc_node =
            crate::parser::parse(slint.into(), Some(std::path::Path::new("TEST")), &mut test_diags);
        let (doc, diag, _) =
            spin_on::spin_on(crate::compile_syntax_node(doc_node, test_diags, compiler_config));
        assert!(!diag.has_errors(), "compile errors: {:#?}", diag.to_string_vec());

        let cu = crate::llr::lower_to_item_tree::lower_to_item_tree(
            &doc,
            &crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter),
        );
        assert_eq!(cu.public_components.len(), 1);
        let props: Vec<&str> =
            cu.public_components[0].public_properties.iter().map(|p| p.name.as_str()).collect();

        // Interface properties/callbacks/functions must be exposed
        assert!(
            props.contains(&"text"),
            "interface property 'text' should be in public API, got: {props:?}"
        );
        assert!(
            props.contains(&"enabled"),
            "interface property 'enabled' should be in public API, got: {props:?}"
        );
        assert!(
            props.contains(&"checked"),
            "interface callback 'checked' should be in public API, got: {props:?}"
        );
        assert!(
            props.contains(&"length"),
            "interface function 'length' should be in public API, got: {props:?}"
        );

        // TestCase's own public property must be exposed
        assert!(props.contains(&"test"), "'test' should be in public API, got: {props:?}");
        assert!(
            props.contains(&"test-case-callback"),
            "'test-case-callback' should be in public API, got: {props:?}"
        );
        assert!(
            props.contains(&"test-case-function"),
            "'test-case-function' should be in public API, got: {props:?}"
        );

        // Non-interface properties from Base must NOT be exposed
        assert!(
            !props.contains(&"base-unrelated-property"),
            "'base-unrelated-property' should NOT be in public API, got: {props:?}"
        );
        assert!(
            !props.contains(&"base-unrelated-callback"),
            "'base-unrelated-callback' should NOT be in public API, got: {props:?}"
        );
        assert!(
            !props.contains(&"base-unrelated-function"),
            "'base-unrelated-function' should NOT be in public API, got: {props:?}"
        );
    }

    #[test]
    fn derived_component_exposes_only_interface_api() {
        let slint = r#"
interface MyInterface {
    in-out property <string> text: "Hello";
    out property <bool> enabled: true;
    public pure function length(text: string) -> int;
    callback checked();
}

component Base implements MyInterface {
    public pure function length(text: string) -> int {
        text.character-count
    }
    out property <int> base-unrelated-property: 42;
    callback base-unrelated-callback();
    public pure function base-unrelated-function() -> int {
        42
    }
}

export component TestCase inherits Base {
    out property <bool> test: true;
    callback test-case-callback();
    public pure function test-case-function() -> int {
        42
    }
}
"#;

        component_exposes_only_interface_api_impl(slint);
    }

    #[test]
    fn derived_component_with_implementation_exposes_only_interface_api() {
        let slint = r#"
interface MyInterface {
    in-out property <string> text: "Hello";
    out property <bool> enabled: true;
    public pure function length(text: string) -> int;
    callback checked();
}

component Impl {
    in-out property <string> text: "Hello";
    out property <bool> enabled: true;
    public pure function length(text: string) -> int {
        text.character-count
    }
    callback checked();

    out property <int> base-unrelated-property: 42;
    callback base-unrelated-callback();
    public pure function base-unrelated-function() -> int {
        42
    }

    @children
}

component Base implements MyInterface inherits Impl {
}

export component TestCase inherits Base {
    out property <bool> test: true;
    callback test-case-callback();
    public pure function test-case-function() -> int {
        42
    }
}
"#;

        component_exposes_only_interface_api_impl(slint);
    }

    #[test]
    fn component_exposes_only_interface_api_via_uses() {
        let slint = r#"
interface MyInterface {
    in-out property <string> text: "Hello";
    out property <bool> enabled: true;
    public pure function length(text: string) -> int;
    callback checked();
}

component Base implements MyInterface {
    public pure function length(text: string) -> int {
        text.character-count
    }
    out property <int> base-unrelated-property: 42;
    callback base-unrelated-callback();
    public pure function base-unrelated-function() -> int {
        42
    }
}

export component TestCase uses { MyInterface from base } {
    out property <bool> test: true;
    callback test-case-callback();
    public pure function test-case-function() -> int {
        42
    }

    base := Base { }
}
"#;

        component_exposes_only_interface_api_impl(slint);
    }
}
