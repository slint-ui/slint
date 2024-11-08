// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::Path;

use crate::{common, util};

use i_slint_compiler::diagnostics::Spanned;
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind};
use lsp_types::Url;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

fn symbol_export_names(document_node: &syntax_nodes::Document, type_name: &str) -> Vec<String> {
    let mut result = vec![];

    for export in document_node.ExportsList() {
        for specifier in export.ExportSpecifier() {
            if specifier.ExportIdentifier().text() == type_name {
                result.push(
                    specifier
                        .ExportName()
                        .map(|n| n.text().to_string())
                        .unwrap_or_else(|| type_name.to_string()),
                );
            }
        }
        if let Some(component) = export.Component() {
            if component.DeclaredIdentifier().text() == type_name {
                result.push(type_name.to_string());
            }
        }
        for structs in export.StructDeclaration() {
            if structs.DeclaredIdentifier().text() == type_name {
                result.push(type_name.to_string());
            }
        }
        for enums in export.EnumDeclaration() {
            if enums.DeclaredIdentifier().text() == type_name {
                result.push(type_name.to_string());
            }
        }
    }

    result.sort();
    result
}

fn replace_element_types(
    document_cache: &common::DocumentCache,
    element: &syntax_nodes::Element,
    old_type: &str,
    new_type: &str,
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
    if let Some(name) = element.QualifiedName() {
        if name.text().to_string().trim() == old_type {
            edits.push(
                common::SingleTextEdit::from_path(
                    document_cache,
                    element.source_file.path(),
                    lsp_types::TextEdit {
                        range: util::node_to_lsp_range(&name),
                        new_text: new_type.to_string(),
                    },
                )
                .expect("URL conversion can not fail here"),
            )
        }
    }

    for c in element.children() {
        match c.kind() {
            SyntaxKind::SubElement => {
                let e: syntax_nodes::SubElement = c.into();
                replace_element_types(document_cache, &e.Element(), old_type, new_type, edits);
            }
            SyntaxKind::RepeatedElement => {
                let e: syntax_nodes::RepeatedElement = c.into();
                replace_element_types(
                    document_cache,
                    &e.SubElement().Element(),
                    old_type,
                    new_type,
                    edits,
                );
            }
            SyntaxKind::ConditionalElement => {
                let e: syntax_nodes::ConditionalElement = c.into();
                replace_element_types(
                    document_cache,
                    &e.SubElement().Element(),
                    old_type,
                    new_type,
                    edits,
                );
            }
            _ => { /* do nothing */ }
        }
    }
}

fn fix_imports(
    document_cache: &common::DocumentCache,
    exporter_path: &Path,
    old_type: &str,
    new_type: &str,
    edits: &mut Vec<common::SingleTextEdit>,
) {
    let Ok(exporter_url) = Url::from_file_path(exporter_path) else {
        return;
    };
    for (url, doc) in document_cache.all_url_documents() {
        if url.scheme() == "builtin" || url.path() == exporter_url.path() {
            continue;
        }

        fix_import_in_document(document_cache, doc, exporter_path, old_type, new_type, edits);
    }
}

fn fix_import_in_document(
    document_cache: &common::DocumentCache,
    document_node: &syntax_nodes::Document,
    exporter_path: &Path,
    old_type: &str,
    new_type: &str,
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
            let external = identifier.ExternalName();

            if external.text().to_string().trim() != old_type {
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
                        range: util::node_to_lsp_range(&external),
                        new_text: new_type.to_string(),
                    },
                )
                .expect("URL conversion can not fail here"),
            );

            if let Some(internal) = identifier.InternalName() {
                let internal_name = internal.text().to_string().trim().to_string();
                if internal_name == new_type {
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
            fix_exports(document_cache, document_node, old_type, new_type, edits);

            // Change all local usages:
            change_local_element_type(document_cache, document_node, old_type, new_type, edits);
        }
    }
}

fn change_local_element_type(
    document_cache: &common::DocumentCache,
    document_node: &syntax_nodes::Document,
    old_type: &str,
    new_type: &str,
    edits: &mut Vec<common::SingleTextEdit>,
) {
    for component in document_node.Component() {
        replace_element_types(document_cache, &component.Element(), old_type, new_type, edits);
    }
    for exported in document_node.ExportsList() {
        if let Some(component) = exported.Component() {
            replace_element_types(document_cache, &component.Element(), old_type, new_type, edits);
        }
    }
}

fn fix_exports(
    document_cache: &common::DocumentCache,
    document_node: &syntax_nodes::Document,
    old_type: &str,
    new_type: &str,
    edits: &mut Vec<common::SingleTextEdit>,
) {
    for export in document_node.ExportsList() {
        for specifier in export.ExportSpecifier() {
            let identifier = specifier.ExportIdentifier();
            if identifier.text().to_string().trim() == old_type {
                let Some(source_file) = identifier.source_file() else {
                    continue;
                };
                edits.push(
                    common::SingleTextEdit::from_path(
                        document_cache,
                        source_file.path(),
                        lsp_types::TextEdit {
                            range: util::node_to_lsp_range(&identifier),
                            new_text: new_type.to_string(),
                        },
                    )
                    .expect("URL conversion can not fail here"),
                );

                let update_imports = if let Some(export_name) = specifier.ExportName() {
                    // Remove "as Foo"
                    if export_name.text().to_string().trim() == new_type {
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
                    fix_imports(document_cache, my_path, old_type, new_type, edits);
                }
            }
        }
    }
}

/// Rename a component by providing the `DeclaredIdentifier` in the component definition.
pub fn rename_component_from_definition(
    document_cache: &common::DocumentCache,
    identifier: &syntax_nodes::DeclaredIdentifier,
    new_name: &str,
) -> crate::Result<lsp_types::WorkspaceEdit> {
    let source_file = identifier.source_file().expect("Identifier had no source file");
    let document = document_cache
        .get_document_for_source_file(source_file)
        .expect("Identifier is in unknown document");

    if document.local_registry.lookup(new_name) != i_slint_compiler::langtype::Type::Invalid {
        return Err(format!("{new_name} is already a registered type").into());
    }
    if document.local_registry.lookup_element(new_name).is_ok() {
        return Err(format!("{new_name} is already a registered element").into());
    }

    let component_type = identifier.text().to_string().trim().to_string();
    if component_type == new_name {
        return Ok(lsp_types::WorkspaceEdit::default());
    }

    let component = identifier.parent().expect("Identifier had no parent");
    debug_assert_eq!(component.kind(), SyntaxKind::Component);

    let Some(document_node) = &document.node else {
        return Err("No document found".into());
    };

    let mut edits = vec![];

    // Replace the identifier itself
    edits.push(
        common::SingleTextEdit::from_path(
            document_cache,
            source_file.path(),
            lsp_types::TextEdit {
                range: util::node_to_lsp_range(identifier),
                new_text: new_name.to_string(),
            },
        )
        .expect("URL conversion can not fail here"),
    );

    // Change all local usages:
    change_local_element_type(document_cache, document_node, &component_type, new_name, &mut edits);

    // Change exports
    fix_exports(document_cache, document_node, &component_type, new_name, &mut edits);

    let export_names = symbol_export_names(document_node, &component_type);
    if export_names.contains(&component_type) {
        let my_path = source_file.path();

        fix_imports(document_cache, my_path, &component_type, new_name, &mut edits);
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

    if true: Rectangle {
        Foo { }
    }

    for i in [1, 2, 3]: Foo { }
}
                    "#
                .to_string(),
            )]),
            false,
        );

        let doc = document_cache.get_document_by_path(&test::main_test_file_name()).unwrap();

        let foo_identifier =
            preview::find_component_identifier(doc.node.as_ref().unwrap(), "Foo").unwrap();
        let edit = rename_component_from_definition(&document_cache, &foo_identifier, "XxxYyyZzz")
            .unwrap();

        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("XxxYyyZzz"));
        assert!(!edited_text[0].contents.contains("Foo"));
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

        let foo_identifier =
            preview::find_component_identifier(doc.node.as_ref().unwrap(), "Foo").unwrap();
        let edit =
            rename_component_from_definition(&document_cache, &foo_identifier, "FooXXX").unwrap();

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

        let foo_identifier =
            preview::find_component_identifier(doc.node.as_ref().unwrap(), "Foo").unwrap();
        let edit = rename_component_from_definition(&document_cache, &foo_identifier, "XxxYyyZzz")
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

        let foo_identifier =
            preview::find_component_identifier(doc.node.as_ref().unwrap(), "Foo").unwrap();
        let edit = rename_component_from_definition(&document_cache, &foo_identifier, "XxxYyyZzz")
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

        let foo_identifier =
            preview::find_component_identifier(doc.node.as_ref().unwrap(), "Foo").unwrap();
        let edit = rename_component_from_definition(&document_cache, &foo_identifier, "XxxYyyZzz")
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

        let foo_identifier =
            preview::find_component_identifier(doc.node.as_ref().unwrap(), "Foo").unwrap();
        let edit = rename_component_from_definition(&document_cache, &foo_identifier, "XxxYyyZzz")
            .unwrap();

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

        let foo_identifier =
            preview::find_component_identifier(doc.node.as_ref().unwrap(), "Foo").unwrap();
        let edit = rename_component_from_definition(&document_cache, &foo_identifier, "XxxYyyZzz")
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

        let foo_identifier =
            preview::find_component_identifier(doc.node.as_ref().unwrap(), "Foo").unwrap();

        assert!(rename_component_from_definition(&document_cache, &foo_identifier, "Foo").is_err());
        assert!(rename_component_from_definition(&document_cache, &foo_identifier, "UsedStruct")
            .is_err());
        assert!(
            rename_component_from_definition(&document_cache, &foo_identifier, "UsedEnum").is_err()
        );
        assert!(rename_component_from_definition(&document_cache, &foo_identifier, "Baz").is_err());
        assert!(rename_component_from_definition(
            &document_cache,
            &foo_identifier,
            "HorizontalLayout"
        )
        .is_err());
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

export { Bat, Bar as RenamedBar, Baz as RenamedBaz, StructBar as RenamedStructBar }

export struct StructBar { foo: int }

export enum EnumBar { bar }
                    "#
                .to_string(),
            )]),
            false,
        );

        let doc = document_cache.get_document_by_path(&test::main_test_file_name()).unwrap();
        let doc = doc.node.as_ref().unwrap();

        assert!(symbol_export_names(doc, "Foobar").is_empty());
        assert_eq!(symbol_export_names(doc, "Foo"), vec!["Foo".to_string()]);
        assert_eq!(
            symbol_export_names(doc, "Baz"),
            vec!["Baz".to_string(), "RenamedBaz".to_string()]
        );
        assert_eq!(symbol_export_names(doc, "Bar"), vec!["RenamedBar".to_string()]);
        assert_eq!(symbol_export_names(doc, "Bat"), vec!["Bat".to_string()]);
        assert_eq!(
            symbol_export_names(doc, "StructBar"),
            vec!["RenamedStructBar".to_string(), "StructBar".to_string()]
        );
        assert_eq!(symbol_export_names(doc, "EnumBar"), vec!["EnumBar".to_string()]);
    }
}
