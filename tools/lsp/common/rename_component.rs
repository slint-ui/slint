// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::Path;

use crate::{common, util};

use i_slint_compiler::diagnostics::Spanned;
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, TextSize};
use lsp_types::Url;
use smol_str::SmolStr;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

fn main_identifier(
    input: &i_slint_compiler::parser::SyntaxNode,
) -> Option<i_slint_compiler::parser::SyntaxToken> {
    input.child_token(SyntaxKind::Identifier)
}

fn is_symbol_name_exported(document_node: &syntax_nodes::Document, type_name: &SmolStr) -> bool {
    for export in document_node.ExportsList() {
        for specifier in export.ExportSpecifier() {
            let export_name = specifier
                .ExportName()
                .as_ref()
                .and_then(|sn| i_slint_compiler::parser::identifier_text(sn));
            let export_id =
                i_slint_compiler::parser::identifier_text(&specifier.ExportIdentifier());
            if export_name.as_ref() == Some(type_name)
                || (export_name.is_none() && export_id.as_ref() == Some(type_name))
            {
                return true;
            }
        }
        if let Some(component) = export.Component() {
            if i_slint_compiler::parser::identifier_text(&component.DeclaredIdentifier())
                .unwrap_or_default()
                == *type_name
            {
                return true;
            }
        }
        for structs in export.StructDeclaration() {
            if i_slint_compiler::parser::identifier_text(&structs.DeclaredIdentifier())
                .unwrap_or_default()
                == *type_name
            {
                return true;
            }
        }
        for enums in export.EnumDeclaration() {
            if i_slint_compiler::parser::identifier_text(&enums.DeclaredIdentifier())
                .unwrap_or_default()
                == *type_name
            {
                return true;
            }
        }
    }

    false
}

fn replace_in_all_elements(
    document_cache: &common::DocumentCache,
    element: &syntax_nodes::Element,
    action: &mut dyn FnMut(&syntax_nodes::Element, &mut Vec<common::SingleTextEdit>),
    edits: &mut Vec<common::SingleTextEdit>,
) {
    // HACK: We inject an ignored component into the live preview. Do not
    //       Generate changes for that -- it does not really exist.
    //
    // The proper fix for both is to enhance the slint interpreter to accept
    // the previewed component via API, so that the entire _SLINT_LivePreview
    // hack becomes unnecessary.
    if common::is_element_node_ignored(element) {
        return;
    }

    action(element, edits);

    for c in element.children() {
        match c.kind() {
            SyntaxKind::SubElement => {
                let e: syntax_nodes::SubElement = c.into();
                replace_in_all_elements(document_cache, &e.Element(), action, edits);
            }
            SyntaxKind::RepeatedElement => {
                let e: syntax_nodes::RepeatedElement = c.into();
                replace_in_all_elements(document_cache, &e.SubElement().Element(), action, edits);
            }
            SyntaxKind::ConditionalElement => {
                let e: syntax_nodes::ConditionalElement = c.into();
                replace_in_all_elements(document_cache, &e.SubElement().Element(), action, edits);
            }
            _ => { /* do nothing */ }
        }
    }
}

fn replace_element_types(
    document_cache: &common::DocumentCache,
    element: &syntax_nodes::Element,
    old_type: &SmolStr,
    new_type: &str,
    edits: &mut Vec<common::SingleTextEdit>,
) {
    replace_in_all_elements(
        document_cache,
        element,
        &mut |element, edits| {
            if let Some(name) = element.QualifiedName().and_then(|qn| main_identifier(&qn)) {
                if i_slint_compiler::parser::normalize_identifier(name.text()) == *old_type {
                    edits.push(
                        common::SingleTextEdit::from_path(
                            document_cache,
                            element.source_file.path(),
                            lsp_types::TextEdit {
                                range: util::token_to_lsp_range(&name),
                                new_text: new_type.to_string(),
                            },
                        )
                        .expect("URL conversion can not fail here"),
                    )
                }
            }
        },
        edits,
    )
}

fn fix_imports(
    document_cache: &common::DocumentCache,
    exporter_path: &Path,
    old_type: &SmolStr,
    new_type: &str,
    fixup_local_use: &dyn Fn(
        &common::DocumentCache,
        &syntax_nodes::Document,
        &i_slint_compiler::parser::TextRange,
        &SmolStr,
        &str,
        &mut Vec<common::SingleTextEdit>,
    ),
    edits: &mut Vec<common::SingleTextEdit>,
) {
    let Ok(exporter_url) = Url::from_file_path(exporter_path) else {
        return;
    };
    for (url, doc) in document_cache.all_url_documents() {
        if url.scheme() == "builtin" || url.path() == exporter_url.path() {
            continue;
        }

        fix_import_in_document(
            document_cache,
            doc,
            exporter_path,
            old_type,
            new_type,
            fixup_local_use,
            edits,
        );
    }
}

fn fix_import_in_document(
    document_cache: &common::DocumentCache,
    document_node: &syntax_nodes::Document,
    exporter_path: &Path,
    old_type: &SmolStr,
    new_type: &str,
    fixup_local_use: &dyn Fn(
        &common::DocumentCache,
        &syntax_nodes::Document,
        &i_slint_compiler::parser::TextRange,
        &SmolStr,
        &str,
        &mut Vec<common::SingleTextEdit>,
    ),
    edits: &mut Vec<common::SingleTextEdit>,
) {
    let Some(document_directory) =
        document_node.source_file().and_then(|sf| sf.path().parent()).map(|p| p.to_owned())
    else {
        return;
    };

    for import_specifier in document_node.ImportSpecifier() {
        let import = import_specifier
            .child_token(SyntaxKind::StringLiteral)
            .map(|t| t.text().trim_matches('"').to_string())
            .unwrap_or_default();

        // Do not bother with the TypeLoader: It will check the FS, which we do not use:-/
        let import_path = i_slint_compiler::pathutils::clean_path(&document_directory.join(import));

        if import_path != exporter_path {
            continue;
        }

        let Some(list) = import_specifier.ImportIdentifierList() else {
            continue;
        };

        for identifier in list.ImportIdentifier() {
            let Some(external) = main_identifier(&identifier.ExternalName()) else {
                continue;
            };
            if i_slint_compiler::parser::normalize_identifier(external.text()) != *old_type {
                continue;
            }
            let Some(source_file) = external.source_file() else {
                continue;
            };

            edits.push(
                common::SingleTextEdit::from_path(
                    document_cache,
                    source_file.path(),
                    lsp_types::TextEdit {
                        range: util::token_to_lsp_range(&external),
                        new_text: new_type.to_string(),
                    },
                )
                .expect("URL conversion can not fail here"),
            );

            if let Some(internal) = identifier.InternalName().and_then(|i| main_identifier(&i)) {
                if i_slint_compiler::parser::normalize_identifier(&internal.text()) == *new_type {
                    // remove " as Foo" part, no need to change anything else though!
                    let start_position =
                        util::text_size_to_lsp_position(source_file, external.text_range().end());
                    let end_position =
                        util::text_size_to_lsp_position(source_file, identifier.text_range().end());
                    edits.push(
                        common::SingleTextEdit::from_path(
                            document_cache,
                            source_file.path(),
                            lsp_types::TextEdit {
                                range: lsp_types::Range::new(start_position, end_position),
                                new_text: String::new(),
                            },
                        )
                        .expect("URL conversion can not fail here"),
                    );
                }
                // Nothing else to change: We still use the old internal name.
                continue;
            }

            // Change exports
            fix_exports(document_cache, document_node, old_type, new_type, fixup_local_use, edits);

            // Change all local usages:
            fixup_local_use(
                document_cache,
                document_node,
                &document_node.text_range(),
                old_type,
                new_type,
                edits,
            );
        }
    }
}

fn fix_exports(
    document_cache: &common::DocumentCache,
    document_node: &syntax_nodes::Document,
    old_type: &SmolStr,
    new_type: &str,
    fixup_local_use: &dyn Fn(
        &common::DocumentCache,
        &syntax_nodes::Document,
        &i_slint_compiler::parser::TextRange,
        &SmolStr,
        &str,
        &mut Vec<common::SingleTextEdit>,
    ),
    edits: &mut Vec<common::SingleTextEdit>,
) {
    for export in document_node.ExportsList() {
        for specifier in export.ExportSpecifier() {
            let Some(identifier) = main_identifier(&specifier.ExportIdentifier()) else {
                continue;
            };

            if i_slint_compiler::parser::normalize_identifier(identifier.text()) == *old_type {
                let Some(source_file) = identifier.source_file() else {
                    continue;
                };

                edits.push(
                    common::SingleTextEdit::from_path(
                        document_cache,
                        source_file.path(),
                        lsp_types::TextEdit {
                            range: util::token_to_lsp_range(&identifier),
                            new_text: new_type.to_string(),
                        },
                    )
                    .expect("URL conversion can not fail here"),
                );

                let update_imports = if let Some(export_name) =
                    specifier.ExportName().and_then(|en| main_identifier(&en))
                {
                    // Remove "as Foo"
                    if export_name.text().to_string() == new_type {
                        let start_position = util::text_size_to_lsp_position(
                            source_file,
                            identifier.text_range().end(),
                        );
                        let end_position = util::text_size_to_lsp_position(
                            source_file,
                            export_name.text_range().end(),
                        );
                        edits.push(
                            common::SingleTextEdit::from_path(
                                document_cache,
                                source_file.path(),
                                lsp_types::TextEdit {
                                    range: lsp_types::Range::new(start_position, end_position),
                                    new_text: String::new(),
                                },
                            )
                            .expect("URL conversion can not fail here"),
                        );
                        true
                    } else {
                        false
                    }
                } else {
                    true
                };

                if update_imports {
                    let my_path = document_node.source_file.path();
                    fix_imports(
                        document_cache,
                        my_path,
                        old_type,
                        new_type,
                        fixup_local_use,
                        edits,
                    );
                }
            }
        }
    }
}

fn visit_document_components(
    document_node: &syntax_nodes::Document,
    action: &mut impl FnMut(&syntax_nodes::Component),
) {
    for component in document_node.Component() {
        action(&component);
    }
    for exported in document_node.ExportsList() {
        if let Some(component) = exported.Component() {
            action(&component);
        }
    }
}

fn visit_document_structs(
    document_node: &syntax_nodes::Document,
    action: &mut impl FnMut(&syntax_nodes::StructDeclaration),
) {
    for struct_decl in document_node.StructDeclaration() {
        action(&struct_decl);
    }
    for exported in document_node.ExportsList() {
        for struct_decl in exported.StructDeclaration() {
            action(&struct_decl);
        }
    }
}

fn declaration_validity_range(
    document_node: &syntax_nodes::Document,
    identifier: &syntax_nodes::DeclaredIdentifier,
) -> i_slint_compiler::parser::TextRange {
    let parent = identifier.parent().unwrap();
    let start = parent.last_token().unwrap().text_range().end() + TextSize::new(1);

    let mut token = parent.last_token().unwrap().next_token();
    let identifier_text = i_slint_compiler::parser::identifier_text(identifier).unwrap_or_default();

    while let Some(t) = &token {
        if t.kind() == SyntaxKind::Identifier {
            let new_parent = t.parent();
            if new_parent.kind() == SyntaxKind::DeclaredIdentifier
                && i_slint_compiler::parser::identifier_text(&new_parent).unwrap_or_default()
                    == identifier_text
            {
                let new_grand_parent = new_parent.parent().unwrap();
                match parent.kind() {
                    SyntaxKind::Component => {
                        if new_grand_parent.kind() == SyntaxKind::Component {
                            return i_slint_compiler::parser::TextRange::new(
                                start,
                                new_grand_parent.last_token().unwrap().text_range().end(),
                            );
                        }
                    }
                    SyntaxKind::EnumDeclaration | SyntaxKind::StructDeclaration => {
                        if [SyntaxKind::EnumDeclaration, SyntaxKind::StructDeclaration]
                            .contains(&new_grand_parent.kind())
                        {
                            return i_slint_compiler::parser::TextRange::new(
                                start,
                                new_grand_parent.last_token().unwrap().text_range().end(),
                            );
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
        token = t.next_token();
    }

    i_slint_compiler::parser::TextRange::new(start, document_node.text_range().end())
}

/// Rename the `DeclaredIdentifier` in a struct/component declaration
pub fn rename_identifier_from_declaration(
    document_cache: &common::DocumentCache,
    identifier: &syntax_nodes::DeclaredIdentifier,
    new_type: &str,
) -> crate::Result<lsp_types::WorkspaceEdit> {
    fn change_local_element_type(
        document_cache: &common::DocumentCache,
        document_node: &syntax_nodes::Document,
        validity_range: &i_slint_compiler::parser::TextRange,
        old_type: &SmolStr,
        new_type: &str,
        edits: &mut Vec<common::SingleTextEdit>,
    ) {
        visit_document_components(document_node, &mut move |component| {
            if validity_range.contains_range(component.text_range()) {
                replace_element_types(
                    document_cache,
                    &component.Element(),
                    old_type,
                    new_type,
                    edits,
                );
            }
        });
    }

    fn change_local_data_type(
        document_cache: &common::DocumentCache,
        document_node: &syntax_nodes::Document,
        validity_range: &i_slint_compiler::parser::TextRange,
        old_type: &SmolStr,
        new_type: &str,
        edits: &mut Vec<common::SingleTextEdit>,
    ) {
        visit_document_components(document_node, &mut |component| {
            if validity_range.contains_range(component.text_range()) {
                for qualified_name in
                    component.descendants().filter(|node| node.kind() == SyntaxKind::QualifiedName)
                {
                    if let Some(first_identifier) = main_identifier(&qualified_name) {
                        if i_slint_compiler::parser::normalize_identifier(first_identifier.text())
                            == *old_type
                        {
                            edits.push(
                                common::SingleTextEdit::from_path(
                                    document_cache,
                                    qualified_name.source_file.path(),
                                    lsp_types::TextEdit {
                                        range: util::token_to_lsp_range(&first_identifier),
                                        new_text: new_type.to_string(),
                                    },
                                )
                                .expect("URL conversion can not fail here"),
                            )
                        }
                    }
                }
            }
        });
        visit_document_structs(document_node, &mut |struct_decl| {
            if validity_range.contains_range(struct_decl.text_range()) {
                for qualified_name in struct_decl
                    .descendants()
                    .filter(|d| d.kind() == SyntaxKind::QualifiedName)
                    .map(|d| Into::<syntax_nodes::QualifiedName>::into(d))
                {
                    let identifier = main_identifier(&qualified_name).unwrap();
                    if i_slint_compiler::parser::normalize_identifier(identifier.text())
                        == *old_type
                    {
                        edits.push(
                            common::SingleTextEdit::from_path(
                                document_cache,
                                qualified_name.source_file.path(),
                                lsp_types::TextEdit {
                                    range: util::token_to_lsp_range(&identifier),
                                    new_text: new_type.to_string(),
                                },
                            )
                            .expect("URL conversion can not fail here"),
                        )
                    }
                }
            }
        });
    }

    let action: Option<
        &dyn Fn(
            &common::DocumentCache,
            &syntax_nodes::Document,
            &i_slint_compiler::parser::TextRange,
            &SmolStr,
            &str,
            &mut Vec<common::SingleTextEdit>,
        ),
    > = match identifier.parent().map(|p| p.kind()).unwrap_or(SyntaxKind::Error) {
        SyntaxKind::Component => Some(&change_local_element_type),
        SyntaxKind::EnumDeclaration | SyntaxKind::StructDeclaration => {
            Some(&change_local_data_type)
        }
        _ => None,
    };

    if let Some(action) = action {
        rename_declared_identifier(document_cache, identifier, new_type, action)
    } else {
        Err("Can not rename this identifier".into())
    }
}

/// Helper function to rename a `DeclaredIdentifier`.
fn rename_declared_identifier(
    document_cache: &common::DocumentCache,
    identifier: &syntax_nodes::DeclaredIdentifier,
    new_type: &str,
    fixup_local_use: &dyn Fn(
        &common::DocumentCache,
        &syntax_nodes::Document,
        &i_slint_compiler::parser::TextRange,
        &SmolStr,
        &str,
        &mut Vec<common::SingleTextEdit>,
    ),
) -> crate::Result<lsp_types::WorkspaceEdit> {
    let source_file = identifier.source_file().expect("Identifier had no source file");
    let document = document_cache
        .get_document_for_source_file(source_file)
        .expect("Identifier is in unknown document");

    let parent = identifier.parent().unwrap();

    if parent.kind() != SyntaxKind::Component
        && document.local_registry.lookup(new_type) != i_slint_compiler::langtype::Type::Invalid
    {
        return Err(format!("{new_type} is already a registered type").into());
    }
    if parent.kind() == SyntaxKind::Component
        && document.local_registry.lookup_element(new_type).is_ok()
    {
        return Err(format!("{new_type} is already a registered element").into());
    }

    let old_type = i_slint_compiler::parser::identifier_text(&identifier).unwrap();
    let normalized_new_type = i_slint_compiler::parser::normalize_identifier(new_type);

    if old_type == normalized_new_type {
        return Ok(lsp_types::WorkspaceEdit::default());
    }

    let parent = identifier.parent().expect("Identifier had no parent");
    debug_assert!([
        SyntaxKind::Component,
        SyntaxKind::EnumDeclaration,
        SyntaxKind::StructDeclaration
    ]
    .contains(&parent.kind()));

    let Some(document_node) = &document.node else {
        return Err("No document found".into());
    };

    let validity_range = declaration_validity_range(document_node, identifier);
    let mut edits = vec![];

    // Replace the identifier itself
    edits.push(
        common::SingleTextEdit::from_path(
            document_cache,
            source_file.path(),
            lsp_types::TextEdit {
                range: util::node_to_lsp_range(identifier),
                new_text: new_type.to_string(),
            },
        )
        .expect("URL conversion can not fail here"),
    );

    // Change all local usages:
    fixup_local_use(
        document_cache,
        document_node,
        &validity_range,
        &old_type,
        new_type,
        &mut edits,
    );

    // Change exports (if the type lives till the end of the document!)
    if validity_range.end() == document_node.text_range().end() {
        fix_exports(
            document_cache,
            document_node,
            &old_type,
            new_type,
            fixup_local_use,
            &mut edits,
        );

        if is_symbol_name_exported(document_node, &old_type) {
            let my_path = source_file.path();

            fix_imports(document_cache, my_path, &old_type, new_type, fixup_local_use, &mut edits);
        }
    }

    Ok(common::create_workspace_edit_from_single_text_edits(edits))
}

#[cfg(all(test, feature = "preview-engine"))]
mod tests {
    use lsp_types::Url;

    use super::*;

    use std::collections::HashMap;

    use crate::common::test;
    use crate::common::text_edit;
    use crate::preview;

    /// Find the identifier that belongs to a struct of the given `name` in the `document`
    fn find_struct_identifiers(
        document: &syntax_nodes::Document,
        name: &str,
    ) -> Vec<syntax_nodes::DeclaredIdentifier> {
        let mut result = vec![];
        let name = i_slint_compiler::parser::normalize_identifier(name);

        for el in document.ExportsList() {
            for st in el.StructDeclaration() {
                let identifier = st.DeclaredIdentifier();
                if i_slint_compiler::parser::normalize_identifier(&identifier.text().to_string())
                    == name
                {
                    result.push(identifier);
                }
            }
        }

        for st in document.StructDeclaration() {
            let identifier = st.DeclaredIdentifier();
            if i_slint_compiler::parser::normalize_identifier(&identifier.text().to_string())
                == name
            {
                result.push(identifier);
            }
        }

        result.sort_by_key(|i| i.text_range().start());
        result
    }

    /// Find the identifier that belongs to a struct of the given `name` in the `document`
    fn find_enum_identifiers(
        document: &syntax_nodes::Document,
        name: &str,
    ) -> Vec<syntax_nodes::DeclaredIdentifier> {
        let mut result = vec![];

        for el in document.ExportsList() {
            for en in el.EnumDeclaration() {
                let identifier = en.DeclaredIdentifier();
                if i_slint_compiler::parser::normalize_identifier(&identifier.text().to_string())
                    == name
                {
                    result.push(identifier);
                }
            }
        }

        for en in document.EnumDeclaration() {
            let identifier = en.DeclaredIdentifier();
            if identifier.text() == name {
                result.push(identifier);
            }
        }

        result.sort_by_key(|i| i.text_range().start());
        result
    }

    #[track_caller]
    fn compile_test_changes(
        document_cache: &common::DocumentCache,
        edit: &lsp_types::WorkspaceEdit,
        allow_warnings: bool,
    ) -> Vec<text_edit::EditedText> {
        eprintln!("Edit:");
        for it in text_edit::EditIterator::new(edit) {
            eprintln!("   {} => {:?}", it.0.uri.to_string(), it.1);
        }
        eprintln!("*** All edits reported ***");

        let changed_text = text_edit::apply_workspace_edit(&document_cache, &edit).unwrap();
        assert!(!changed_text.is_empty()); // there was a change!

        eprintln!("After changes were applied:");
        for ct in &changed_text {
            eprintln!("File {}:", ct.url.to_string());
            for (count, line) in ct.contents.split('\n').enumerate() {
                eprintln!("    {:3}: {line}", count + 1);
            }
            eprintln!("=========");
        }
        eprintln!("*** All changes reported ***");

        let code = {
            let mut map: HashMap<Url, String> = document_cache
                .all_url_documents()
                .map(|(url, dn)| (url, dn.source_file.as_ref()))
                .map(|(url, sf)| (url, sf.source().unwrap().to_string()))
                .collect();
            for ct in &changed_text {
                map.insert(ct.url.clone(), ct.contents.clone());
            }
            map
        };

        // changed code compiles fine:
        let _ = test::recompile_test_with_sources("fluent", code, allow_warnings);

        changed_text
    }

    #[test]
    fn test_rename_component_from_definition_ok() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
component Foo { /* Foo 1 */  }

export { Foo }

enum Xyz { Foo, Bar }

struct Abc { Foo: Xyz }

component Baz {
    Foo /* <- TEST_ME_1 */ { }
}

component Foo /* Foo 2 */ {
    Foo /* <- TEST_ME_2 */ { }
}

export component Bar {
    Foo /* <- TEST_ME_3 */ { }
    Rectangle {
        Foo /* <- TEST_ME_4 */ { }
        Foo := Baz { }
    }

    if true: Rectangle {
        Foo /* <- TEST_ME_5 */ { }
    }

    if false: Rectangle {
        Foo /* <- TEST_ME_6 */ { }
    }

    function Foo(Foo: int) { Foo + 1; }
    function F() { self.Foo(42); }

    for i in [1, 2, 3]: Foo /* <- TEST_ME_7 */ { }
}
                "#
                .to_string(),
            )]),
            true, // Component `Foo` is replacing a component with the same name
        );

        let doc = document_cache.get_document_by_path(&test::main_test_file_name()).unwrap();

        let identifiers = preview::find_component_identifiers(doc.node.as_ref().unwrap(), "Foo");
        assert_eq!(identifiers.len(), 2);

        let edit =
            rename_identifier_from_declaration(&document_cache, &identifiers[0], "XxxYyyZzz")
                .unwrap();

        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);

        assert!(edited_text[0].contents.contains("component XxxYyyZzz { /* Foo 1 "));
        // The *last* Foo gets exported!
        assert!(edited_text[0].contents.contains("export { Foo }"));
        assert!(edited_text[0].contents.contains("enum Xyz { Foo,"));
        assert!(edited_text[0].contents.contains("struct Abc { Foo:"));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* <- TEST_ME_1 "));
        assert!(edited_text[0].contents.contains("component Foo /* Foo 2 "));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* <- TEST_ME_2 "));
        assert!(edited_text[0].contents.contains("Foo /* <- TEST_ME_3 "));
        assert!(edited_text[0].contents.contains("Foo /* <- TEST_ME_4 "));
        assert!(edited_text[0].contents.contains("Foo := Baz {"));
        assert!(edited_text[0].contents.contains("Foo /* <- TEST_ME_5 "));
        assert!(edited_text[0].contents.contains("Foo /* <- TEST_ME_6 "));
        assert!(edited_text[0].contents.contains("function Foo(Foo: int) { Foo + 1; }"));
        assert!(edited_text[0].contents.contains("function F() { self.Foo(42); }"));
        assert!(edited_text[0].contents.contains("Foo /* <- TEST_ME_7 "));

        let edit =
            rename_identifier_from_declaration(&document_cache, &identifiers[1], "XxxYyyZzz")
                .unwrap();

        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("component Foo { /* Foo 1 "));
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz }"));
        assert!(edited_text[0].contents.contains("enum Xyz { Foo,"));
        assert!(edited_text[0].contents.contains("struct Abc { Foo:"));
        assert!(edited_text[0].contents.contains("Foo /* <- TEST_ME_1 "));
        assert!(edited_text[0].contents.contains("component XxxYyyZzz /* Foo 2 "));
        assert!(edited_text[0].contents.contains("Foo /* <- TEST_ME_2 "));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* <- TEST_ME_3 "));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* <- TEST_ME_4 "));
        assert!(edited_text[0].contents.contains("Foo := Baz {"));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* <- TEST_ME_5 "));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* <- TEST_ME_6 "));
        assert!(edited_text[0].contents.contains("function Foo(Foo: int) { Foo + 1; }"));
        assert!(edited_text[0].contents.contains("function F() { self.Foo(42); }"));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* <- TEST_ME_7 "));
    }

    #[test]
    fn test_rename_component_from_definition_live_preview_rename() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                format!("component Foo {{ }}\nexport component _SLINT_LivePreview inherits Foo {{ /* @lsp:ignore-node */ }}\n")
            )]),
            true,
        );

        let doc = document_cache.get_document_by_path(&test::main_test_file_name()).unwrap();

        let identifiers = preview::find_component_identifiers(doc.node.as_ref().unwrap(), "Foo");
        let edit = rename_identifier_from_declaration(
            &document_cache,
            identifiers.first().unwrap(),
            "FooXXX",
        )
        .unwrap();

        assert_eq!(text_edit::EditIterator::new(&edit).count(), 1);
        // This does not compile as the type was not changed in the _SLINT_LivePreview part!
    }

    #[test]
    fn test_rename_component_from_definition_with_renaming_export_ok() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([
                (
                    Url::from_file_path(test::main_test_file_name()).unwrap(),
                    r#"
import { FExport} from "source.slint";

export component Foo {
    FExport { }
}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("source.slint")).unwrap(),
                    r#"
component Foo { }

export { Foo as FExport }
                "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let doc =
            document_cache.get_document_by_path(&test::test_file_name("source.slint")).unwrap();

        let identifiers = preview::find_component_identifiers(doc.node.as_ref().unwrap(), "Foo");
        let edit = rename_identifier_from_declaration(
            &document_cache,
            &identifiers.first().unwrap(),
            "XxxYyyZzz",
        )
        .unwrap();

        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert_eq!(
            edited_text[0].url.to_file_path().unwrap(),
            test::test_file_name("source.slint")
        );
        assert!(edited_text[0].contents.contains("XxxYyyZzz"));
        assert!(!edited_text[0].contents.contains("Foo"));
    }

    #[test]
    fn test_rename_component_from_definition_with_export_ok() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([
                (
                    Url::from_file_path(test::main_test_file_name()).unwrap(),
                    r#"
import { Foo } from "source.slint";
import { UserComponent } from "user.slint";
import { User2Component } from "user2.slint";
import { Foo as User3Fxx } from "user3.slint";
import { User4Fxx } from "user4.slint";

export component Main {
    Foo { }
    UserComponent { }
    User2Component { }
}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("source.slint")).unwrap(),
                    r#"
export component Foo { }
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user.slint")).unwrap(),
                    r#"
import { Foo as Bar } from "source.slint";

export component UserComponent {
    Bar { }
}

export { Bar }
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user2.slint")).unwrap(),
                    r#"
import { Foo as XxxYyyZzz } from "source.slint";

export component User2Component {
    XxxYyyZzz { }
}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user3.slint")).unwrap(),
                    r#"
import { Foo } from "source.slint";

export { Foo }
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user4.slint")).unwrap(),
                    r#"
import { Foo } from "source.slint";

export { Foo as User4Fxx }
                "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let doc =
            document_cache.get_document_by_path(&test::test_file_name("source.slint")).unwrap();

        let identifier = preview::find_component_identifiers(doc.node.as_ref().unwrap(), "Foo")
            .first()
            .cloned()
            .unwrap();
        let edit =
            rename_identifier_from_declaration(&document_cache, &identifier, "XxxYyyZzz").unwrap();

        let edited_text = compile_test_changes(&document_cache, &edit, false);

        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed.contents.contains("XxxYyyZzz"));
                assert!(!ed.contents.contains("Foo"));
                assert!(ed.contents.contains("UserComponent"));
                assert!(ed.contents.contains("import { XxxYyyZzz as User3Fxx }"));
                assert!(ed.contents.contains("import { User4Fxx }"));
            } else if ed_path == test::test_file_name("source.slint") {
                assert!(ed.contents.contains("export component XxxYyyZzz {"));
                assert!(!ed.contents.contains("Foo"));
            } else if ed_path == test::test_file_name("user.slint") {
                assert!(ed.contents.contains("{ XxxYyyZzz as Bar }"));
                assert!(ed.contents.contains("Bar { }"));
                assert!(!ed.contents.contains("Foo"));
            } else if ed_path == test::test_file_name("user2.slint") {
                assert!(ed.contents.contains("import { XxxYyyZzz }"));
                assert!(ed.contents.contains("XxxYyyZzz { }"));
            } else if ed_path == test::test_file_name("user3.slint") {
                assert!(ed.contents.contains("import { XxxYyyZzz }"));
                assert!(ed.contents.contains("export { XxxYyyZzz }"));
            } else if ed_path == test::test_file_name("user4.slint") {
                assert!(ed.contents.contains("import { XxxYyyZzz }"));
                assert!(ed.contents.contains("export { XxxYyyZzz as User4Fxx }"));
            } else {
                unreachable!();
            }
        }
    }

    #[test]
    fn test_rename_component_from_definition_with_export_and_relative_paths_ok() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([
                (
                    Url::from_file_path(test::main_test_file_name()).unwrap(),
                    r#"
import { Foo } from "s/source.slint";
import { UserComponent } from "u/user.slint";
import { User2Component } from "u/user2.slint";
import { Foo as User3Fxx } from "u/user3.slint";
import { User4Fxx } from "u/user4.slint";

export component Main {
    Foo { }
    UserComponent { }
    User2Component { }
}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("s/source.slint")).unwrap(),
                    r#"
export component Foo { }
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("u/user.slint")).unwrap(),
                    r#"
import { Foo as Bar } from "../s/source.slint";

export component UserComponent {
    Bar { }
}

export { Bar }
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("u/user2.slint")).unwrap(),
                    r#"
import { Foo as XxxYyyZzz } from "../s/source.slint";

export component User2Component {
    XxxYyyZzz { }
}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("u/user3.slint")).unwrap(),
                    r#"
import { Foo } from "../s/source.slint";

export { Foo }
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("u/user4.slint")).unwrap(),
                    r#"
import { Foo } from "../s/source.slint";

export { Foo as User4Fxx }
                "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let doc =
            document_cache.get_document_by_path(&test::test_file_name("s/source.slint")).unwrap();

        let identifier = preview::find_component_identifiers(doc.node.as_ref().unwrap(), "Foo")
            .first()
            .cloned()
            .unwrap();
        let edit =
            rename_identifier_from_declaration(&document_cache, &identifier, "XxxYyyZzz").unwrap();

        let edited_text = compile_test_changes(&document_cache, &edit, false);

        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed.contents.contains("XxxYyyZzz"));
                assert!(!ed.contents.contains("Foo"));
                assert!(ed.contents.contains("UserComponent"));
                assert!(ed.contents.contains("import { XxxYyyZzz as User3Fxx }"));
                assert!(ed.contents.contains("import { User4Fxx }"));
            } else if ed_path == test::test_file_name("s/source.slint") {
                assert!(ed.contents.contains("export component XxxYyyZzz {"));
                assert!(!ed.contents.contains("Foo"));
            } else if ed_path == test::test_file_name("u/user.slint") {
                assert!(ed.contents.contains("{ XxxYyyZzz as Bar }"));
                assert!(ed.contents.contains("Bar { }"));
                assert!(!ed.contents.contains("Foo"));
            } else if ed_path == test::test_file_name("u/user2.slint") {
                assert!(ed.contents.contains("import { XxxYyyZzz }"));
                assert!(ed.contents.contains("XxxYyyZzz { }"));
            } else if ed_path == test::test_file_name("u/user3.slint") {
                assert!(ed.contents.contains("import { XxxYyyZzz }"));
                assert!(ed.contents.contains("export { XxxYyyZzz }"));
            } else if ed_path == test::test_file_name("u/user4.slint") {
                assert!(ed.contents.contains("import { XxxYyyZzz }"));
                assert!(ed.contents.contains("export { XxxYyyZzz as User4Fxx }"));
            } else {
                unreachable!();
            }
        }
    }

    #[test]
    fn test_rename_component_from_definition_import_confusion_ok() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([
                (
                    Url::from_file_path(test::main_test_file_name()).unwrap(),
                    r#"
import { Foo as User1Fxx } from "user1.slint";
import { Foo as User2Fxx } from "user2.slint";

export component Main {
    User1Fxx { }
    User2Fxx { }
}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user1.slint")).unwrap(),
                    r#"
export component Foo { }
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user2.slint")).unwrap(),
                    r#"
export component Foo { }
                "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let doc =
            document_cache.get_document_by_path(&test::test_file_name("user1.slint")).unwrap();

        let identifier = preview::find_component_identifiers(doc.node.as_ref().unwrap(), "Foo")
            .first()
            .cloned()
            .unwrap();
        let edit =
            rename_identifier_from_declaration(&document_cache, &identifier, "XxxYyyZzz").unwrap();

        let edited_text = compile_test_changes(&document_cache, &edit, false);

        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed.contents.contains("import { XxxYyyZzz as User1Fxx }"));
                assert!(ed.contents.contains("import { Foo as User2Fxx }"));
            } else if ed_path == test::test_file_name("user1.slint") {
                assert!(ed.contents.contains("export component XxxYyyZzz {"));
                assert!(!ed.contents.contains("Foo"));
            } else {
                unreachable!();
            }
        }

        let doc =
            document_cache.get_document_by_path(&test::test_file_name("user2.slint")).unwrap();

        let identifiers = preview::find_component_identifiers(doc.node.as_ref().unwrap(), "Foo");
        let edit = rename_identifier_from_declaration(
            &document_cache,
            &identifiers.first().unwrap(),
            "XxxYyyZzz",
        )
        .unwrap();

        let edited_text = compile_test_changes(&document_cache, &edit, false);

        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed.contents.contains("import { XxxYyyZzz as User2Fxx }"));
                assert!(ed.contents.contains("import { Foo as User1Fxx }"));
            } else if ed_path == test::test_file_name("user2.slint") {
                assert!(ed.contents.contains("export component XxxYyyZzz {"));
                assert!(!ed.contents.contains("Foo"));
            } else {
                unreachable!();
            }
        }
    }

    #[test]
    fn test_rename_component_from_definition_redefinition_error() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
struct UsedStruct { value: int, }
enum UsedEnum { x, y }

component Foo { }

component Baz {
    Foo { }
}

export component Bar {
    Foo { }
    Rectangle {
        Foo { }
        Baz { }
    }
}
                    "#
                .to_string(),
            )]),
            false,
        );

        let doc = document_cache.get_document_by_path(&test::main_test_file_name()).unwrap();

        let identifiers = preview::find_component_identifiers(doc.node.as_ref().unwrap(), "Foo");

        let id = identifiers.first().unwrap();

        assert!(rename_identifier_from_declaration(&document_cache, id, "Foo").is_err());
        assert!(rename_identifier_from_declaration(&document_cache, id, "UsedStruct").is_ok());
        assert!(rename_identifier_from_declaration(&document_cache, id, "UsedEnum").is_ok());
        assert!(rename_identifier_from_declaration(&document_cache, id, "Baz").is_err());
        assert!(
            rename_identifier_from_declaration(&document_cache, id, "HorizontalLayout").is_err()
        );
    }

    #[test]
    fn test_exported_type_names() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
export component Foo {}
export component Baz {}

component Bar {}
component Bat {}
component Cat {}

export { Bat, Cat as Cat, Bar as RenamedBar, Baz as RenamedBaz, StructBar as RenamedStructBar }

export struct StructBar { foo: int }

export enum EnumBar { bar }
                    "#
                .to_string(),
            )]),
            false,
        );

        let doc = document_cache.get_document_by_path(&test::main_test_file_name()).unwrap();
        let doc = doc.node.as_ref().unwrap();

        assert!(!is_symbol_name_exported(doc, &SmolStr::from("Foobar"))); // does not exist
        assert!(is_symbol_name_exported(doc, &SmolStr::from("Foo")));
        assert!(is_symbol_name_exported(doc, &SmolStr::from("Baz")));
        assert!(!is_symbol_name_exported(doc, &SmolStr::from("Bar"))); // not exported
        assert!(is_symbol_name_exported(doc, &SmolStr::from("Bat")));
        assert!(is_symbol_name_exported(doc, &SmolStr::from("Cat")));
        assert!(is_symbol_name_exported(doc, &SmolStr::from("RenamedBar")));
        assert!(is_symbol_name_exported(doc, &SmolStr::from("RenamedBaz")));
        assert!(is_symbol_name_exported(doc, &SmolStr::from("RenamedStructBar")));
        assert!(is_symbol_name_exported(doc, &SmolStr::from("StructBar")));
        assert!(is_symbol_name_exported(doc, &SmolStr::from("EnumBar")));
    }

    #[test]
    fn test_rename_struct_from_definition_ok() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
export struct Foo {
    test-me: bool,
}

component Baz {
    in-out property <Foo> baz-prop;
}

export component Bar {
    in-out property <Foo> bar-prop <=> baz.baz-prop;

    baz := Baz {}
}
                    "#
                .to_string(),
            )]),
            false,
        );

        let doc = document_cache.get_document_by_path(&test::main_test_file_name()).unwrap();

        let identifiers = find_struct_identifiers(doc.node.as_ref().unwrap(), "Foo");
        assert_eq!(identifiers.len(), 1);
        let edit =
            rename_identifier_from_declaration(&document_cache, &identifiers[0], "XxxYyyZzz")
                .unwrap();

        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("XxxYyyZzz"));
        assert!(!edited_text[0].contents.contains("Foo"));
    }

    #[test]
    fn test_rename_struct_from_definition_with_dash_ok() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
export struct F-oo {
    test-me: bool,
}

component Baz {
    in-out property <F_oo> baz-prop;
}

export component Bar {
    in-out property <F-oo> bar-prop <=> baz.baz-prop;

    baz := Baz {}
}
                    "#
                .to_string(),
            )]),
            false,
        );

        let doc = document_cache.get_document_by_path(&test::main_test_file_name()).unwrap();

        let identifiers = find_struct_identifiers(doc.node.as_ref().unwrap(), "F_oo");
        assert_eq!(identifiers.len(), 1);
        let edit =
            rename_identifier_from_declaration(&document_cache, &identifiers[0], "Xxx_Yyy-Zzz")
                .unwrap();

        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("export struct Xxx_Yyy-Zzz {"));
        assert!(edited_text[0].contents.contains("<Xxx_Yyy-Zzz> baz-prop"));
        assert!(edited_text[0].contents.contains("<Xxx_Yyy-Zzz> bar-prop"));
    }

    #[test]
    fn test_rename_struct_from_definition_with_underscore_ok() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
export struct F_oo {
    test-me: bool,
}

component Baz {
    in-out property <F_oo> baz-prop;
}

export component Bar {
    in-out property <F-oo> bar-prop <=> baz.baz-prop;

    baz := Baz {}
}
                    "#
                .to_string(),
            )]),
            false,
        );

        let doc = document_cache.get_document_by_path(&test::main_test_file_name()).unwrap();

        let identifiers = find_struct_identifiers(doc.node.as_ref().unwrap(), "F-oo");
        assert_eq!(identifiers.len(), 1);
        let edit =
            rename_identifier_from_declaration(&document_cache, &identifiers[0], "Xxx_Yyy-Zzz")
                .unwrap();

        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("export struct Xxx_Yyy-Zzz {"));
        assert!(edited_text[0].contents.contains("<Xxx_Yyy-Zzz> baz-prop"));
        assert!(edited_text[0].contents.contains("<Xxx_Yyy-Zzz> bar-prop"));
    }

    #[test]
    fn test_rename_struct_from_definition_with_renaming_export_ok() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([
                (
                    Url::from_file_path(test::main_test_file_name()).unwrap(),
                    r#"
import { FExport} from "source.slint";

export component Foo {
    property <FExport> foo-prop;
}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("source.slint")).unwrap(),
                    r#"
struct Foo {
    test-me: bool,
}

export { Foo as FExport }
                "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let doc =
            document_cache.get_document_by_path(&test::test_file_name("source.slint")).unwrap();

        let identifiers = find_struct_identifiers(doc.node.as_ref().unwrap(), "Foo");
        assert_eq!(identifiers.len(), 1);

        let edit =
            rename_identifier_from_declaration(&document_cache, &identifiers[0], "XxxYyyZzz")
                .unwrap();

        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert_eq!(
            edited_text[0].url.to_file_path().unwrap(),
            test::test_file_name("source.slint")
        );
        assert!(edited_text[0].contents.contains("XxxYyyZzz"));
        assert!(!edited_text[0].contents.contains("Foo"));
    }

    #[test]
    fn test_rename_struct_from_definition_with_export_ok() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([
                (
                    Url::from_file_path(test::main_test_file_name()).unwrap(),
                    r#"
import { Foo } from "source.slint";
import { UserComponent } from "user.slint";
import { User2Struct } from "user2.slint";
import { Foo as User3Fxx } from "user3.slint";
import { User4Fxx } from "user4.slint";

export component Main {
    property <Foo> main-prop;
    property <User3Fxx> main-prop2;
    property <User2Struct> main-prop3;
    property <User3Fxx> main-prop4 <=> uc.user-component-prop;

    uc := UserComponent { }
}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("source.slint")).unwrap(),
                    r#"
export struct Foo { test-me: bool, }
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user.slint")).unwrap(),
                    r#"
import { Foo as Bar } from "source.slint";

export component UserComponent {
    in-out property <Bar> user-component-prop;
}

export { Bar }
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user2.slint")).unwrap(),
                    r#"
import { Foo as XxxYyyZzz } from "source.slint";

export struct User2Struct {
    member: XxxYyyZzz,
}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user3.slint")).unwrap(),
                    r#"
import { Foo } from "source.slint";

export { Foo }
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user4.slint")).unwrap(),
                    r#"
import { Foo } from "source.slint";

export { Foo as User4Fxx }
                "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let doc =
            document_cache.get_document_by_path(&test::test_file_name("source.slint")).unwrap();

        let identifiers = find_struct_identifiers(doc.node.as_ref().unwrap(), "Foo");
        assert_eq!(identifiers.len(), 1);
        let edit =
            rename_identifier_from_declaration(&document_cache, &identifiers[0], "XxxYyyZzz")
                .unwrap();

        let edited_text = compile_test_changes(&document_cache, &edit, false);

        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed.contents.contains("XxxYyyZzz"));
                assert!(!ed.contents.contains("Foo"));
                assert!(ed.contents.contains("UserComponent"));
                assert!(ed.contents.contains("import { XxxYyyZzz as User3Fxx }"));
                assert!(ed.contents.contains("import { User4Fxx }"));
            } else if ed_path == test::test_file_name("source.slint") {
                assert!(ed.contents.contains("export struct XxxYyyZzz {"));
                assert!(!ed.contents.contains("Foo"));
            } else if ed_path == test::test_file_name("user.slint") {
                assert!(ed.contents.contains("{ XxxYyyZzz as Bar }"));
                assert!(ed.contents.contains("property <Bar> user-component-prop"));
                assert!(!ed.contents.contains("Foo"));
            } else if ed_path == test::test_file_name("user2.slint") {
                assert!(ed.contents.contains("import { XxxYyyZzz }"));
                assert!(ed.contents.contains("member: XxxYyyZzz,"));
            } else if ed_path == test::test_file_name("user3.slint") {
                assert!(ed.contents.contains("import { XxxYyyZzz }"));
                assert!(ed.contents.contains("export { XxxYyyZzz }"));
            } else if ed_path == test::test_file_name("user4.slint") {
                assert!(ed.contents.contains("import { XxxYyyZzz }"));
                assert!(ed.contents.contains("export { XxxYyyZzz as User4Fxx }"));
            } else {
                unreachable!();
            }
        }
    }

    #[test]
    fn test_rename_enum_from_definition_ok() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
    export { Foo }

    enum Foo /* Foo 1 */ {
        M1, M2,
    }

    enum Foo /* Foo 3 */ {
        test,
    }

    component Baz {
        in-out property <Foo> baz-prop: Foo.test;
    }

    export component Bar {
        in-out property <Foo> bar-prop <=> baz.baz-prop;

        baz := Baz {}
    }
                        "#
                .to_string(),
            )]),
            true, // redefinition of type warning
        );

        let doc = document_cache.get_document_by_path(&test::main_test_file_name()).unwrap();

        let identifiers = find_enum_identifiers(doc.node.as_ref().unwrap(), "Foo");
        assert_eq!(identifiers.len(), 2);

        let edit =
            rename_identifier_from_declaration(&document_cache, &identifiers[0], "XxxYyyZzz")
                .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("export { Foo }"));
        assert!(edited_text[0].contents.contains("enum XxxYyyZzz /* Foo 1 "));
        assert!(edited_text[0].contents.contains("enum Foo /* Foo 3 "));
        assert!(edited_text[0].contents.contains("property <Foo> baz-prop"));
        assert!(edited_text[0].contents.contains("baz-prop: Foo.test;"));
        assert!(edited_text[0].contents.contains("property <Foo> bar-prop"));

        let edit =
            rename_identifier_from_declaration(&document_cache, &identifiers[1], "XxxYyyZzz")
                .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz }"));
        assert!(edited_text[0].contents.contains("enum Foo /* Foo 1 "));
        assert!(edited_text[0].contents.contains("enum XxxYyyZzz /* Foo 3 "));
        assert!(edited_text[0].contents.contains("property <XxxYyyZzz> baz-prop"));
        assert!(edited_text[0].contents.contains("baz-prop: XxxYyyZzz.test;"));
        assert!(edited_text[0].contents.contains("property <XxxYyyZzz> bar-prop"));
    }

    #[test]
    fn test_rename_enum_from_definition_with_struct_ok() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
    enum Foo /* Foo 1 */ {
        M1, M2,
    }

    export { Foo }

    struct Foo /* Foo 2 */ {
        test: Foo,
    }

    component Baz {
        in-out property <Foo> baz-prop;
    }

    export component Bar {
        in-out property <Foo> bar-prop <=> baz.baz-prop;

        baz := Baz {}
    }
                        "#
                .to_string(),
            )]),
            true, // redefinition of type warning
        );

        let doc = document_cache.get_document_by_path(&test::main_test_file_name()).unwrap();

        let identifiers = find_enum_identifiers(doc.node.as_ref().unwrap(), "Foo");
        assert_eq!(identifiers.len(), 1);

        let edit =
            rename_identifier_from_declaration(&document_cache, &identifiers[0], "XxxYyyZzz")
                .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("enum XxxYyyZzz /* Foo 1 */"));
        assert!(edited_text[0].contents.contains("struct Foo /* Foo 2 */"));
        assert!(edited_text[0].contents.contains("export { Foo }"));
        assert!(edited_text[0].contents.contains("test: XxxYyyZzz,"));
        assert!(edited_text[0].contents.contains("property <Foo> baz-prop"));
        assert!(edited_text[0].contents.contains("property <Foo> bar-prop"));

        let identifiers = find_struct_identifiers(doc.node.as_ref().unwrap(), "Foo");
        assert_eq!(identifiers.len(), 1);

        let edit =
            rename_identifier_from_declaration(&document_cache, &identifiers[0], "XxxYyyZzz")
                .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("enum Foo /* Foo 1 */"));
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz }"));
        assert!(edited_text[0].contents.contains("struct XxxYyyZzz /* Foo 2 */"));
        assert!(edited_text[0].contents.contains("test: Foo,"));
        assert!(edited_text[0].contents.contains("property <XxxYyyZzz> baz-prop"));
        assert!(edited_text[0].contents.contains("property <XxxYyyZzz> bar-prop"));
    }

    #[test]
    fn test_rename_enum_from_definition_with_renaming_export_ok() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([
                (
                    Url::from_file_path(test::main_test_file_name()).unwrap(),
                    r#"
    import { FExport} from "source.slint";

    export enum Foo {
        M1, M2
    }
                    "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("source.slint")).unwrap(),
                    r#"
    export enum Foo {
        OM1, OM2,
    }

    export { Foo as FExport }
                    "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let doc =
            document_cache.get_document_by_path(&test::test_file_name("source.slint")).unwrap();

        let identifiers = find_enum_identifiers(doc.node.as_ref().unwrap(), "Foo");
        assert_eq!(identifiers.len(), 1);
        let edit =
            rename_identifier_from_declaration(&document_cache, &identifiers[0], "XxxYyyZzz")
                .unwrap();

        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert_eq!(
            edited_text[0].url.to_file_path().unwrap(),
            test::test_file_name("source.slint")
        );
        assert!(edited_text[0].contents.contains("XxxYyyZzz"));
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz as FExport"));
        assert!(!edited_text[0].contents.contains("Foo"));
    }

    #[test]
    fn test_rename_enum_from_definition_with_export_ok() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([
                (
                    Url::from_file_path(test::main_test_file_name()).unwrap(),
                    r#"
    import { F_o_o } from "source.slint";
    import { UserComponent } from "user.slint";
    import { User2Struct } from "user2.slint";
    import { F-o-o as User3Fxx } from "user3.slint";
    import { User4Fxx } from "user4.slint";

    export component Main {
        property <F-o_o> main-prop: F_o_o.M1;
        property <User3Fxx> main-prop2: User3Fxx.M1;
        property <User2Struct> main-prop3;
        property <User3Fxx> main-prop4 <=> uc.user-component-prop;

        uc := UserComponent { }
    }
                    "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("source.slint")).unwrap(),
                    r#"
    export enum F_o-o { M1, M2, }
                    "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user.slint")).unwrap(),
                    r#"
    import { F-o_o as B_a-r } from "source.slint";

    export component UserComponent {
        in-out property <B_a-r> user-component-prop: B-a-r.M1;
    }

    export { B-a-r }
                    "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user2.slint")).unwrap(),
                    r#"
    import { F_o_o as XxxYyyZzz } from "source.slint";

    export struct User2Struct {
        member: XxxYyyZzz,
    }
                    "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user3.slint")).unwrap(),
                    r#"
    import { F-o-o } from "source.slint";

    export { F_o_o }
                    "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user4.slint")).unwrap(),
                    r#"
    import { F-o-o } from "source.slint";

    export { F_o_o as User4Fxx }
                    "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let doc =
            document_cache.get_document_by_path(&test::test_file_name("source.slint")).unwrap();

        let identifiers = find_enum_identifiers(doc.node.as_ref().unwrap(), "F-o-o");
        assert_eq!(identifiers.len(), 1);
        let edit =
            rename_identifier_from_declaration(&document_cache, &identifiers[0], "XxxYyyZzz")
                .unwrap();

        let edited_text = compile_test_changes(&document_cache, &edit, false);

        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed.contents.contains("import { XxxYyyZzz } from \"source.slint\""));
                assert!(ed.contents.contains("import { UserComponent } from \"user.slint\""));
                assert!(ed.contents.contains("import { User2Struct } from \"user2.slint\""));
                assert!(ed
                    .contents
                    .contains("import { XxxYyyZzz as User3Fxx } from \"user3.slint\""));
                assert!(ed.contents.contains("import { User4Fxx } from \"user4.slint\""));
                assert!(ed.contents.contains("property <XxxYyyZzz> main-prop: XxxYyyZzz.M1"));
                assert!(ed.contents.contains("property <User3Fxx> main-prop2: User3Fxx.M1"));
                assert!(ed.contents.contains("property <User2Struct> main-prop3;"));
                assert!(ed
                    .contents
                    .contains("property <User3Fxx> main-prop4 <=> uc.user-component-prop;"));
                assert!(ed.contents.contains("uc := UserComponent"));
            } else if ed_path == test::test_file_name("source.slint") {
                assert!(ed.contents.contains("export enum XxxYyyZzz {"));
            } else if ed_path == test::test_file_name("user.slint") {
                assert!(ed.contents.contains("import { XxxYyyZzz as B_a-r }"));
                assert!(ed.contents.contains("property <B_a-r> user-component-prop"));
                assert!(ed.contents.contains("> user-component-prop: B-a-r.M1;"));
                assert!(ed.contents.contains("export { B-a-r }"));
            } else if ed_path == test::test_file_name("user2.slint") {
                assert!(ed.contents.contains("import { XxxYyyZzz } from \"source.slint\""));
                assert!(ed.contents.contains("export struct User2Struct {"));
                assert!(ed.contents.contains("member: XxxYyyZzz,"));
            } else if ed_path == test::test_file_name("user3.slint") {
                assert!(ed.contents.contains("import { XxxYyyZzz }"));
                assert!(ed.contents.contains("export { XxxYyyZzz }"));
            } else if ed_path == test::test_file_name("user4.slint") {
                assert!(ed.contents.contains("import { XxxYyyZzz }"));
                assert!(ed.contents.contains("export { XxxYyyZzz as User4Fxx }"));
            } else {
                unreachable!();
            }
        }
    }
}
