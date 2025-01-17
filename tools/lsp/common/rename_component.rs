// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::{common, util};

use i_slint_compiler::diagnostics::Spanned;
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxNode, SyntaxToken, TextRange};
use lsp_types::Url;
use smol_str::SmolStr;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

pub fn main_identifier(input: &SyntaxNode) -> Option<SyntaxToken> {
    input.child_token(SyntaxKind::Identifier)
}

fn is_symbol_name_exported(
    document_cache: &common::DocumentCache,
    document_node: &syntax_nodes::Document,
    query: &DeclarationNodeQuery,
) -> bool {
    for export in document_node.ExportsList() {
        for specifier in export.ExportSpecifier() {
            if let Some(export_name) = specifier.ExportName() {
                if i_slint_compiler::parser::identifier_text(&export_name).as_ref()
                    == Some(&query.name)
                    && query.is_same_symbol(document_cache, main_identifier(&export_name).unwrap())
                {
                    return true;
                }
            } else {
                let export_id = specifier.ExportIdentifier();
                if i_slint_compiler::parser::identifier_text(&export_id).as_ref()
                    == Some(&query.name)
                    && query.is_same_symbol(document_cache, main_identifier(&export_id).unwrap())
                {
                    return true;
                }
            }
        }
        if let Some(component) = export.Component() {
            let identifier = component.DeclaredIdentifier();
            if i_slint_compiler::parser::identifier_text(&identifier).as_ref() == Some(&query.name)
                && query.is_same_symbol(document_cache, main_identifier(&identifier).unwrap())
            {
                return true;
            }
        }
        for structs in export.StructDeclaration() {
            let identifier = structs.DeclaredIdentifier();
            if i_slint_compiler::parser::identifier_text(&identifier).as_ref() == Some(&query.name)
                && query.is_same_symbol(document_cache, main_identifier(&identifier).unwrap())
            {
                return true;
            }
        }
        for enums in export.EnumDeclaration() {
            if i_slint_compiler::parser::identifier_text(&enums.DeclaredIdentifier()).as_ref()
                == Some(&query.name)
            {
                return true;
            }
        }
    }

    false
}

fn fix_imports(
    document_cache: &common::DocumentCache,
    query: &DeclarationNodeQuery,
    exporter_path: &Path,
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

        fix_import_in_document(document_cache, query, doc, exporter_path, new_type, edits);
    }
}

fn import_path(
    document_directory: &Path,
    import_specifier: &syntax_nodes::ImportSpecifier,
) -> Option<PathBuf> {
    let import = import_specifier
        .child_token(SyntaxKind::StringLiteral)
        .map(|t| t.text().trim_matches('"').to_string())?;

    if import == "std-widgets.slint" || import.starts_with("@") {
        return None; // No need to ever look at this!
    }

    // Do not bother with the TypeLoader: It will check the FS, which we do not use:-/
    Some(i_slint_compiler::pathutils::clean_path(&document_directory.join(import)))
}

fn fix_import_in_document(
    document_cache: &common::DocumentCache,
    query: &DeclarationNodeQuery,
    document_node: &syntax_nodes::Document,
    exporter_path: &Path,
    new_type: &str,
    edits: &mut Vec<common::SingleTextEdit>,
) {
    let Some(document_directory) =
        document_node.source_file().and_then(|sf| sf.path().parent()).map(|p| p.to_owned())
    else {
        return;
    };

    for import_specifier in document_node.ImportSpecifier() {
        let Some(import_path) = import_path(&document_directory, &import_specifier) else {
            continue;
        };

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
            if i_slint_compiler::parser::normalize_identifier(external.text()) != query.name {
                continue;
            }
            let Some(source_file) = external.source_file() else {
                continue;
            };

            if let Some(internal) = identifier.InternalName().and_then(|i| main_identifier(&i)) {
                if i_slint_compiler::parser::normalize_identifier(&internal.text())
                    == i_slint_compiler::parser::normalize_identifier(new_type)
                {
                    // `Old as New` => `New`
                    let start_position =
                        util::text_size_to_lsp_position(source_file, external.text_range().start());
                    let end_position =
                        util::text_size_to_lsp_position(source_file, identifier.text_range().end());
                    edits.push(
                        common::SingleTextEdit::from_path(
                            document_cache,
                            source_file.path(),
                            lsp_types::TextEdit {
                                range: lsp_types::Range::new(start_position, end_position),
                                new_text: new_type.to_string(),
                            },
                        )
                        .expect("URL conversion can not fail here"),
                    );
                } else {
                    // `Old as New` => `New`
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
                }
                // Nothing else to change: We still use the old internal name.
                continue;
            } else {
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
            }

            let Some(sub_query) = query.sub_query(external) else {
                continue;
            };

            // Change exports
            fix_exports(document_cache, document_node, &sub_query, new_type, edits);

            // Change all local usages:
            rename_local_symbols(document_cache, document_node, &sub_query, new_type, edits);
        }
    }
}

fn fix_exports(
    document_cache: &common::DocumentCache,
    document_node: &syntax_nodes::Document,
    query: &DeclarationNodeQuery,
    new_type: &str,
    edits: &mut Vec<common::SingleTextEdit>,
) {
    let normalized_new_type = i_slint_compiler::parser::normalize_identifier(new_type);

    for export in document_node.ExportsList() {
        for specifier in export.ExportSpecifier() {
            let Some(identifier) = main_identifier(&specifier.ExportIdentifier()) else {
                continue;
            };

            if i_slint_compiler::parser::normalize_identifier(identifier.text()) == query.name
                && query.is_same_symbol(document_cache, identifier.clone())
            {
                let Some(source_file) = identifier.source_file().cloned() else {
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

                let sub_query = if let Some(export_name) = specifier.ExportName() {
                    // Remove "as Foo"
                    if i_slint_compiler::parser::identifier_text(&export_name).as_ref()
                        == Some(&normalized_new_type)
                    {
                        let start_position = util::text_size_to_lsp_position(
                            &source_file,
                            identifier
                                .text_range()
                                .end()
                                .checked_add(1.into())
                                .expect("There are more tokens"),
                        );
                        let end_position = util::text_size_to_lsp_position(
                            &source_file,
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
                        query.sub_query(identifier)
                    } else {
                        None
                    }
                } else {
                    query.sub_query(identifier)
                };

                if let Some(sub_query) = sub_query {
                    let my_path = document_node.source_file.path();
                    fix_imports(document_cache, &sub_query, my_path, new_type, edits);
                }
            }
        }
    }
}

/// Rename all local non import/export related identifiers
fn rename_local_symbols(
    document_cache: &common::DocumentCache,
    document_node: &syntax_nodes::Document,
    query: &DeclarationNodeQuery,
    new_type: &str,
    edits: &mut Vec<common::SingleTextEdit>,
) {
    let mut current_token = document_node.first_token();
    while let Some(current) = current_token {
        if current.kind() == SyntaxKind::Identifier
            && i_slint_compiler::parser::normalize_identifier(current.text()) == query.name
        {
            if ![
                SyntaxKind::ExternalName,
                SyntaxKind::InternalName,
                SyntaxKind::ExportIdentifier,
                SyntaxKind::ExportName,
                SyntaxKind::PropertyDeclaration,
            ]
            .contains(&current.parent().kind())
                && query.is_same_symbol(document_cache, current.clone())
            {
                edits.push(
                    common::SingleTextEdit::from_path(
                        document_cache,
                        current.source_file.path(),
                        lsp_types::TextEdit {
                            range: util::token_to_lsp_range(&current),
                            new_text: new_type.to_string(),
                        },
                    )
                    .expect("URL conversion can not fail here"),
                )
            }
        }

        current_token = current.next_token();
    }
}

/// Rename an InternalName in an impoort statement
///
/// The ExternalName is different form our name, which is why we ended up here.
///
/// Change the InternalName, fix up local usage and then fix up exports. If exports
/// change something, also fix all the necessary imports.
fn rename_internal_name(
    document_cache: &common::DocumentCache,
    query: &DeclarationNodeQuery,
    internal_name: &syntax_nodes::InternalName,
    new_type: &str,
) -> lsp_types::WorkspaceEdit {
    let Some(old_type) = i_slint_compiler::parser::identifier_text(&internal_name) else {
        return Default::default();
    };
    let Some(document) = document_cache.get_document_for_source_file(&internal_name.source_file)
    else {
        return Default::default();
    };
    let Some(document_node) = &document.node else {
        return Default::default();
    };

    let mut edits = vec![];

    let parent: syntax_nodes::ImportIdentifier = internal_name.parent().unwrap().into();
    let external_name = parent.ExternalName();
    let external_name_token = main_identifier(&external_name).unwrap();
    let external_str = i_slint_compiler::parser::normalize_identifier(external_name_token.text());

    let normalized_new_type = i_slint_compiler::parser::normalize_identifier(new_type);

    if external_str == normalized_new_type {
        // `New as Old` -> `New`
        edits.push(
            common::SingleTextEdit::from_path(
                document_cache,
                query.token.source_file.path(),
                lsp_types::TextEdit {
                    range: util::text_range_to_lsp_range(
                        &external_name_token.source_file,
                        TextRange::new(
                            external_name_token.next_token().unwrap().text_range().start(),
                            query.token.text_range().end(),
                        ),
                    ),
                    new_text: String::new(),
                },
            )
            .expect("URL conversion can not fail here"),
        )
    } else if old_type != normalized_new_type {
        // `Some as Old` -> `Some as New`
        edits.push(
            common::SingleTextEdit::from_path(
                document_cache,
                query.token.source_file.path(),
                lsp_types::TextEdit {
                    range: util::token_to_lsp_range(&main_identifier(&internal_name).unwrap()),
                    new_text: new_type.to_string(),
                },
            )
            .expect("URL conversion can not fail here"),
        );
    }

    // Change exports
    fix_exports(document_cache, document_node, &query, new_type, &mut edits);

    // Change all local usages:
    rename_local_symbols(document_cache, document_node, &query, new_type, &mut edits);

    common::create_workspace_edit_from_single_text_edits(edits)
}

/// We ended up in an ExportName that we need to rename.
///
/// The internal name is different, otherwise we would not have ended up here:-)
/// So we need to rename the export itself and then fix up imports.
fn rename_export_name(
    document_cache: &common::DocumentCache,
    query: &DeclarationNodeQuery,
    export_name: &syntax_nodes::ExportName,
    new_type: &str,
) -> lsp_types::WorkspaceEdit {
    let mut edits = vec![];

    let specifier: syntax_nodes::ExportSpecifier = export_name.parent().unwrap().into();
    let internal_name = specifier.ExportIdentifier();
    if i_slint_compiler::parser::identifier_text(&internal_name).as_ref()
        == Some(&i_slint_compiler::parser::normalize_identifier(new_type))
    {
        edits.push(
            common::SingleTextEdit::from_path(
                document_cache,
                export_name.source_file.path(),
                lsp_types::TextEdit {
                    range: util::node_to_lsp_range(&specifier),
                    new_text: new_type.to_string(),
                },
            )
            .expect("URL conversion can not fail here"),
        );
    } else {
        edits.push(
            common::SingleTextEdit::from_path(
                document_cache,
                export_name.source_file.path(),
                lsp_types::TextEdit {
                    range: util::token_to_lsp_range(&main_identifier(export_name).unwrap()),
                    new_text: new_type.to_string(),
                },
            )
            .expect("URL conversion can not fail here"),
        );
    }

    // Change exports
    fix_imports(document_cache, &query, export_name.source_file.path(), new_type, &mut edits);

    common::create_workspace_edit_from_single_text_edits(edits)
}

#[derive(Clone, Debug)]
pub enum DeclarationNodeKind {
    DeclaredIdentifier(syntax_nodes::DeclaredIdentifier),
    InternalName(syntax_nodes::InternalName),
    ExportName(syntax_nodes::ExportName),
}

#[derive(Clone, Debug)]
pub struct DeclarationNode {
    kind: DeclarationNodeKind,
    query: DeclarationNodeQuery,
}

pub fn find_declaration_node(
    document_cache: &common::DocumentCache,
    token: &SyntaxToken,
) -> Option<DeclarationNode> {
    if token.kind() != SyntaxKind::Identifier {
        return None;
    }

    DeclarationNodeQuery::new(document_cache, token.clone())?.find_declaration_node(document_cache)
}

impl DeclarationNode {
    pub fn rename(
        &self,
        document_cache: &common::DocumentCache,
        new_type: &str,
    ) -> crate::Result<lsp_types::WorkspaceEdit> {
        match &self.kind {
            DeclarationNodeKind::DeclaredIdentifier(id) => {
                rename_declared_identifier(document_cache, &self.query, &id, &new_type)
            }
            DeclarationNodeKind::InternalName(internal) => {
                Ok(rename_internal_name(document_cache, &self.query, &internal, &new_type))
            }
            DeclarationNodeKind::ExportName(export) => {
                Ok(rename_export_name(document_cache, &self.query, &export, &new_type))
            }
        }
    }
}

fn find_last_declared_identifier_at_or_before(
    token: SyntaxToken,
    type_name: &SmolStr,
) -> Option<SyntaxNode> {
    let mut token = Some(token);

    while let Some(t) = token {
        if t.kind() == SyntaxKind::Identifier {
            let node = t.parent();
            if node.kind() == SyntaxKind::DeclaredIdentifier
                && i_slint_compiler::parser::identifier_text(&node).as_ref() == Some(type_name)
            {
                return Some(node.into());
            }
        }
        token = t.prev_token();
    }

    None
}

#[derive(Clone, Debug)]
struct DeclarationNodeQuery {
    info: common::token_info::TokenInfo,
    name: SmolStr,
    token: SyntaxToken,
}

impl DeclarationNodeQuery {
    fn new(document_cache: &common::DocumentCache, token: SyntaxToken) -> Option<Self> {
        let info = common::token_info::token_info(document_cache, token.clone())?;
        let name = i_slint_compiler::parser::normalize_identifier(token.text());

        Some(DeclarationNodeQuery { info, name, token })
    }

    fn sub_query(&self, token: SyntaxToken) -> Option<Self> {
        let name = i_slint_compiler::parser::normalize_identifier(token.text());

        Some(DeclarationNodeQuery { info: self.info.clone(), name, token })
    }

    fn is_export_identifier_or_external_name(&self) -> bool {
        self.token.kind() == SyntaxKind::Identifier
            && [SyntaxKind::ExportIdentifier, SyntaxKind::ExternalName]
                .contains(&self.token.parent().kind())
    }

    fn start_token(&self) -> Option<SyntaxToken> {
        if self.is_export_identifier_or_external_name() {
            None
        } else {
            Some(self.token.clone())
        }
    }

    fn is_same_symbol(&self, document_cache: &common::DocumentCache, token: SyntaxToken) -> bool {
        let Some(info) = common::token_info::token_info(document_cache, token) else {
            return false;
        };

        match (&self.info, &info) {
            (common::token_info::TokenInfo::Type(s), common::token_info::TokenInfo::Type(o)) => {
                s == o
            }
            (
                common::token_info::TokenInfo::ElementType(s),
                common::token_info::TokenInfo::ElementType(o),
            ) => s == o,
            (
                common::token_info::TokenInfo::ElementRc(s),
                common::token_info::TokenInfo::ElementRc(o),
            ) => Rc::ptr_eq(s, o),
            (
                common::token_info::TokenInfo::LocalProperty(s),
                common::token_info::TokenInfo::LocalProperty(o),
            ) => Rc::ptr_eq(&s.source_file, &o.source_file) && s.text_range() == o.text_range(),
            (
                common::token_info::TokenInfo::NamedReference(nl),
                common::token_info::TokenInfo::NamedReference(nr),
            ) => Rc::ptr_eq(&nl.element(), &nr.element()) && nl.name() == nr.name(),
            (
                common::token_info::TokenInfo::NamedReference(nr),
                common::token_info::TokenInfo::LocalProperty(s),
            )
            | (
                common::token_info::TokenInfo::LocalProperty(s),
                common::token_info::TokenInfo::NamedReference(nr),
            ) => nr.element().borrow().debug.iter().any(|di| {
                di.node.source_file.path() == s.source_file.path()
                    && di.node.PropertyDeclaration().any(|pd| pd.text_range() == s.text_range())
            }),
            (_, _) => false,
        }
    }

    /// Find the declaration node we should rename
    fn find_declaration_node(
        self: Self,
        document_cache: &common::DocumentCache,
    ) -> Option<DeclarationNode> {
        let node = self.token.parent();

        match node.kind() {
            SyntaxKind::DeclaredIdentifier => Some(DeclarationNode {
                kind: DeclarationNodeKind::DeclaredIdentifier(node.into()),
                query: self,
            }),
            SyntaxKind::InternalName => Some(DeclarationNode {
                kind: DeclarationNodeKind::InternalName(node.into()),
                query: self,
            }),
            SyntaxKind::ExportName => Some(DeclarationNode {
                kind: DeclarationNodeKind::ExportName(node.into()),
                query: self,
            }),
            _ => {
                let document =
                    document_cache.get_document_by_path(self.token.source_file.path())?;
                let document_node = document.node.clone()?;
                let start_token = self.start_token();

                find_declaration_node_impl(document_cache, &document_node, start_token, self)
            }
        }
    }
}

fn find_declaration_node_impl(
    document_cache: &common::DocumentCache,
    document_node: &syntax_nodes::Document,
    start_token: Option<SyntaxToken>,
    query: DeclarationNodeQuery,
) -> Option<DeclarationNode> {
    // Exported under a custom name?
    if start_token.is_none() {
        for export_item in document_node.ExportsList() {
            for specifier in export_item.ExportSpecifier() {
                if let Some(export_name) = specifier.ExportName() {
                    if i_slint_compiler::parser::identifier_text(&export_name).as_ref()
                        == Some(&query.name)
                    {
                        return Some(DeclarationNode {
                            kind: DeclarationNodeKind::ExportName(export_name),
                            query,
                        });
                    }
                }
            }
        }
    }

    let mut token = document_node.last_token();

    while let Some(t) = token {
        if let Some(declared_identifier) =
            find_last_declared_identifier_at_or_before(t.clone(), &query.name)
        {
            if query.is_same_symbol(document_cache, main_identifier(&declared_identifier).unwrap())
            {
                return Some(DeclarationNode {
                    kind: DeclarationNodeKind::DeclaredIdentifier(declared_identifier.into()),
                    query,
                });
            }

            token = declared_identifier.first_token().and_then(|t| t.prev_token());
        } else {
            token = None;
        }
    }

    // Imported?
    for import_spec in document_node.ImportSpecifier() {
        if let Some(import_id) = import_spec.ImportIdentifierList() {
            for id in import_id.ImportIdentifier() {
                let external = i_slint_compiler::parser::identifier_text(&id.ExternalName());
                let internal =
                    id.InternalName().and_then(|i| i_slint_compiler::parser::identifier_text(&i));

                if internal.as_ref() == Some(&query.name) {
                    return Some(DeclarationNode {
                        kind: DeclarationNodeKind::InternalName(id.InternalName().unwrap()),
                        query,
                    });
                }

                if external.as_ref() == Some(&query.name) {
                    let document_path = document_node.source_file.path();
                    let document_dir = document_path.parent()?;
                    let path = import_path(document_dir, &import_spec)?;
                    let import_doc = document_cache.get_document_by_path(&path)?;
                    let import_doc_node = import_doc.node.as_ref()?;

                    return find_declaration_node_impl(
                        document_cache,
                        import_doc_node,
                        None,
                        query,
                    );
                }
            }
        }
    }

    None
}

/// Rename a `DeclaredIdentifier`.
///
/// This is a locally defined thing.
///
/// Fix up local usages, fix exports and any imports elsewhere if the exports changed
fn rename_declared_identifier(
    document_cache: &common::DocumentCache,
    query: &DeclarationNodeQuery,
    declared_identifier: &syntax_nodes::DeclaredIdentifier,
    new_type: &str,
) -> crate::Result<lsp_types::WorkspaceEdit> {
    let source_file = &declared_identifier.source_file;
    let document = document_cache
        .get_document_for_source_file(source_file)
        .expect("Identifier is in unknown document");

    let Some(document_node) = &document.node else {
        return Err("No document found".into());
    };

    let parent = declared_identifier.parent().unwrap();

    let normalized_new_type = i_slint_compiler::parser::normalize_identifier(new_type);

    if parent.kind() != SyntaxKind::Component
        && document.local_registry.lookup(normalized_new_type.as_str())
            != i_slint_compiler::langtype::Type::Invalid
    {
        return Err(format!("{new_type} is already a registered type").into());
    }
    if parent.kind() == SyntaxKind::Component
        && document.local_registry.lookup_element(&normalized_new_type.as_str()).is_ok()
    {
        return Err(format!("{new_type} is already a registered element").into());
    }

    let old_type = &query.name;

    if *old_type == normalized_new_type {
        return Ok(lsp_types::WorkspaceEdit::default());
    }

    let mut edits = vec![];

    // Change all local usages:
    rename_local_symbols(document_cache, document_node, query, new_type, &mut edits);

    // Change exports (if the type lives till the end of the document!)
    fix_exports(document_cache, document_node, query, new_type, &mut edits);

    if is_symbol_name_exported(document_cache, document_node, query) {
        fix_imports(document_cache, query, source_file.path(), new_type, &mut edits);
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

    #[track_caller]
    fn find_token_by_comment(
        document_cache: &common::DocumentCache,
        document_path: &Path,
        suffix: &str,
    ) -> SyntaxToken {
        let document = document_cache.get_document_by_path(document_path).unwrap();
        let document = document.node.as_ref().unwrap();

        let offset =
            document.text().to_string().find(&format!("<- TEST_ME{suffix} ")).unwrap() as u32;
        let comment = document.token_at_offset(offset.into()).next().unwrap();
        assert_eq!(comment.kind(), SyntaxKind::Comment);
        let mut token = comment.prev_token();

        while let Some(t) = &token {
            if ![SyntaxKind::Comment, SyntaxKind::Eof, SyntaxKind::Whitespace].contains(&t.kind()) {
                break;
            }
            token = t.prev_token();
        }
        token.unwrap()
    }

    #[track_caller]
    fn find_node_by_comment(
        document_cache: &common::DocumentCache,
        document_path: &Path,
        suffix: &str,
    ) -> SyntaxNode {
        find_token_by_comment(document_cache, document_path, suffix).parent()
    }

    #[track_caller]
    fn apply_text_changes(
        document_cache: &common::DocumentCache,
        edit: &lsp_types::WorkspaceEdit,
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

        changed_text
    }

    #[track_caller]
    fn compile_test_changes(
        document_cache: &common::DocumentCache,
        edit: &lsp_types::WorkspaceEdit,
        allow_warnings: bool,
    ) -> Vec<text_edit::EditedText> {
        let changed_text = apply_text_changes(document_cache, edit);

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

    #[track_caller]
    pub fn rename_tester_with_new_name(
        document_cache: &common::DocumentCache,
        document_path: &Path,
        suffix: &str,
        new_name: &str,
    ) -> Vec<text_edit::EditedText> {
        let edit = find_declaration_node(
            &document_cache,
            &find_token_by_comment(&document_cache, document_path, suffix),
        )
        .unwrap()
        .rename(&document_cache, new_name)
        .unwrap();
        compile_test_changes(&document_cache, &edit, false)
    }

    #[track_caller]
    pub fn rename_tester(
        document_cache: &common::DocumentCache,
        document_path: &Path,
        suffix: &str,
    ) -> Vec<text_edit::EditedText> {
        rename_tester_with_new_name(document_cache, document_path, suffix, "XxxYyyZzz")
    }

    #[test]
    fn test_rename_redefined_component() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
component Foo /* <- TEST_ME_1 */ { @children }

export { Foo /* 2 */ }

component Bar {
    Foo /* 1.1 */ { }
}

component Foo /* <- TEST_ME_2 */ inherits Foo /* 1.2 */ {
    Foo /* 1.3 */ { }
}
                "#
                .to_string(),
            )]),
            true, // Component `Foo` is replacing a component with the same name
        );

        // Can not rename the first one...
        assert!(find_declaration_node(
            &document_cache,
            &find_token_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .is_none(),);

        let edit = find_declaration_node(
            &document_cache,
            &find_token_by_comment(&document_cache, &test::main_test_file_name(), "_2"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
        .unwrap();

        let edited_text = apply_text_changes(&document_cache, &edit); // DO NOT COMPILE, THAT WILL FAIL!

        assert_eq!(edited_text.len(), 1);

        assert!(edited_text[0].contents.contains("component Foo /* <- TEST_ME_1 "));
        // The *last* Foo gets exported
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz /* 2 */ }"));

        // All the following are wrong:
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* 1.1 "));
        assert!(edited_text[0].contents.contains("inherits XxxYyyZzz /* 1.2 "));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* 1.3 "));
    }

    #[test]
    fn test_rename_redefined_enum() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
enum Foo /* <- TEST_ME_1 */ { test1 }

export { Foo /* 2 */ }

struct Bar {
    bar_test: Foo
}

enum Foo /* <- TEST_ME_2 */ {
    test2
}

export struct Baz {
    baz_test: Foo
}
                "#
                .to_string(),
            )]),
            true, // Component `Foo` is replacing a component with the same name
        );

        // Can not rename the first one...
        assert!(find_declaration_node(
            &document_cache,
            &find_token_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .is_none(),);

        let edit = find_declaration_node(
            &document_cache,
            &find_token_by_comment(&document_cache, &test::main_test_file_name(), "_2"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
        .unwrap();

        let edited_text = apply_text_changes(&document_cache, &edit); // DO NOT COMPILE, THAT WILL FAIL!

        assert_eq!(edited_text.len(), 1);

        assert!(edited_text[0].contents.contains("enum Foo /* <- TEST_ME_1 "));
        // The *last* Foo gets exported!
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz /* 2 */ }"));
        assert!(edited_text[0].contents.contains("baz_test: XxxYyyZzz"));

        // All the following are wrong:
        assert!(edited_text[0].contents.contains("bar_test: XxxYyyZzz"));
    }

    #[test]
    fn test_rename_redefined_struct() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
struct Foo /* <- TEST_ME_1 */ { test: bool }

export { Foo /* 2 */ }

struct Bar {
    bar_test: Foo
}

struct Foo /* <- TEST_ME_2 */ {
    foo_test: Foo
}
                "#
                .to_string(),
            )]),
            true, // Component `Foo` is replacing a component with the same name
        );

        // Can not rename the first one...
        assert!(find_declaration_node(
            &document_cache,
            &find_token_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .is_none(),);

        let edit = find_declaration_node(
            &document_cache,
            &find_token_by_comment(&document_cache, &test::main_test_file_name(), "_2"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
        .unwrap();

        let edited_text = apply_text_changes(&document_cache, &edit); // DO NOT COMPILE, THAT WILL FAIL!

        assert_eq!(edited_text.len(), 1);

        assert!(edited_text[0].contents.contains("struct Foo /* <- TEST_ME_1 "));
        // The *last* Foo gets exported!
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz /* 2 */ }"));

        // All the following are wrong:
        assert!(edited_text[0].contents.contains("bar_test: XxxYyyZzz"));
        assert!(edited_text[0].contents.contains("foo_test: XxxYyyZzz"));
    }

    #[test]
    fn test_rename_component_from_definition() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
export { Foo }

enum Xyz { Foo, Bar }

struct Abc { Foo: Xyz }

component Foo /* <- TEST_ME_1 */ inherits Rectangle {
    @children
}

component Baz {
    Foo /* <- TEST_ME_2 */ { }
}

struct Foo {
    bar: bool,
}

export component Bar inherits Foo /* <- TEST_ME_3 */ {
    Foo /* <- TEST_ME_4 */ { }
    Rectangle {
        Foo /* <- TEST_ME_5 */ { }
        Foo := Baz { }
    }

    if true: Rectangle {
        Foo /* <- TEST_ME_6 */ { }
    }

    if false: Rectangle {
        Foo /* <- TEST_ME_7 */ { }
    }

    function Foo(Foo: int) { Foo + 1; }
    function F() { self.Foo(42); }

    for i in [1, 2, 3]: Foo /* <- TEST_ME_8 */ { }
}
                "#
                .to_string(),
            )]),
            true, // Component `Foo` is replacing a component with the same name
        );

        let edited_text = rename_tester(&document_cache, &test::main_test_file_name(), "_1");
        assert_eq!(edited_text.len(), 1);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz }"));
        assert!(edited_text[0].contents.contains("enum Xyz { Foo,"));
        assert!(edited_text[0].contents.contains("struct Abc { Foo:"));
        assert!(edited_text[0].contents.contains("component XxxYyyZzz /* <- TEST_ME_1 "));
        assert!(edited_text[0].contents.contains("TEST_ME_1 */ inherits Rectangle "));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* <- TEST_ME_2 "));
        assert!(edited_text[0].contents.contains("struct Foo {"));
        assert!(edited_text[0]
            .contents
            .contains("component Bar inherits XxxYyyZzz /* <- TEST_ME_3 "));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* <- TEST_ME_4 "));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* <- TEST_ME_5 "));
        assert!(edited_text[0].contents.contains("Foo := Baz "));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* <- TEST_ME_6 "));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* <- TEST_ME_7 "));
        assert!(edited_text[0].contents.contains("function Foo(Foo:"));
        assert!(edited_text[0].contents.contains("F() { self.Foo("));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* <- TEST_ME_8 "));
    }

    #[test]
    fn test_rename_component_from_definition_live_preview_rename() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                format!("component Foo/* <- TEST_ME_1 */{{ }}\nexport component _SLINT_LivePreview inherits Foo {{ /* @lsp:ignore-node */ }}\n")
            )]),
            true,
        );

        let edit = find_declaration_node(
            &document_cache,
            &find_token_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
        .unwrap();

        assert_eq!(text_edit::EditIterator::new(&edit).count(), 1);

        // This does not compile as the type was not changed in the _SLINT_LivePreview part.
        // This is inteneded as that code does not really exist in the first place!
    }

    #[test]
    fn test_rename_component_from_definition_with_renaming_export() {
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

enum Foo {
    foo, bar
}

component Foo /* <- TEST_ME_1 */ {
    property <Foo> test-property: Foo.bar;
}

export { Foo as FExport }
                "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let edited_text =
            rename_tester(&document_cache, &test::test_file_name("source.slint"), "_1");

        assert_eq!(edited_text.len(), 1);
        assert_eq!(
            edited_text[0].url.to_file_path().unwrap(),
            test::test_file_name("source.slint")
        );
        assert!(edited_text[0].contents.contains("enum Foo {"));
        assert!(edited_text[0].contents.contains("component XxxYyyZzz /* <- TEST_ME_1 "));
        assert!(edited_text[0].contents.contains("property <Foo> test-property"));
        assert!(edited_text[0].contents.contains("test-property: Foo.bar;"));
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz as FExport }"));
    }

    #[test]
    fn test_rename_component_from_definition_with_export() {
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
export component Foo /* <- TEST_ME_1 */ { }
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

        let edited_text =
            rename_tester(&document_cache, &test::test_file_name("source.slint"), "_1");

        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed.contents.contains("XxxYyyZzz"));
                assert!(!ed.contents.contains("Foo"));
                assert!(ed.contents.contains("UserComponent"));
                assert!(ed.contents.contains("import { XxxYyyZzz as User3Fxx }"));
                assert!(ed.contents.contains("import { User4Fxx }"));
            } else if ed_path == test::test_file_name("source.slint") {
                assert!(ed.contents.contains("export component XxxYyyZzz /* <- TEST_ME_1 "));
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
    fn test_rename_component_from_definition_with_export_and_relative_paths() {
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
export component Foo /* <- TEST_ME_1 */ { }
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

        let edited_text =
            rename_tester(&document_cache, &test::test_file_name("s/source.slint"), "_1");

        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed.contents.contains("XxxYyyZzz"));
                assert!(!ed.contents.contains("Foo"));
                assert!(ed.contents.contains("UserComponent"));
                assert!(ed.contents.contains("import { XxxYyyZzz as User3Fxx }"));
                assert!(ed.contents.contains("import { User4Fxx }"));
            } else if ed_path == test::test_file_name("s/source.slint") {
                assert!(ed.contents.contains("export component XxxYyyZzz /* <- TEST_ME_1 "));
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
    fn test_rename_component_from_definition_import_confusion() {
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
export component Foo /* <- TEST_ME_1 */ { }
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user2.slint")).unwrap(),
                    r#"
export component Foo /* <- TEST_ME_2 */ { }
                "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let edited_text =
            rename_tester(&document_cache, &test::test_file_name("user1.slint"), "_1");

        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed.contents.contains("import { XxxYyyZzz as User1Fxx }"));
                assert!(ed.contents.contains("import { Foo as User2Fxx }"));
            } else if ed_path == test::test_file_name("user1.slint") {
                assert!(ed.contents.contains("export component XxxYyyZzz /* <- TEST_ME_1 "));
                assert!(!ed.contents.contains("Foo"));
            } else {
                unreachable!();
            }
        }

        let edited_text =
            rename_tester(&document_cache, &test::test_file_name("user2.slint"), "_2");

        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed.contents.contains("import { XxxYyyZzz as User2Fxx }"));
                assert!(ed.contents.contains("import { Foo as User1Fxx }"));
            } else if ed_path == test::test_file_name("user2.slint") {
                assert!(ed.contents.contains("export component XxxYyyZzz /* <- TEST_ME_2 "));
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

component Foo /* <- TEST_ME_1 */ { }

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

        let dn = find_declaration_node(
            &document_cache,
            &find_token_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap();

        assert!(dn.rename(&document_cache, "Foo").is_err());
        assert!(dn.rename(&document_cache, "UsedStruct").is_ok());
        assert!(dn.rename(&document_cache, "UsedEnum").is_ok());
        assert!(dn.rename(&document_cache, "Baz").is_err());
        assert!(dn.rename(&document_cache, "HorizontalLayout").is_err());
    }

    #[test]
    fn test_rename_struct_from_definition() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
export struct Foo /* <- TEST_ME_1 */ {
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

        let edited_text = rename_tester(&document_cache, &test::main_test_file_name(), "_1");

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("struct XxxYyyZzz /* <- TEST_ME_1 "));
        assert!(edited_text[0].contents.contains("property <XxxYyyZzz> baz-prop"));
        assert!(edited_text[0].contents.contains("property <XxxYyyZzz> bar-prop"));
    }

    #[test]
    fn test_rename_struct_from_definition_with_dash() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
export struct F-oo /* <- TEST_ME_1 */ {
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

        let edited_text = rename_tester_with_new_name(
            &document_cache,
            &test::main_test_file_name(),
            "_1",
            "Xxx_Yyy-Zzz",
        );

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("export struct Xxx_Yyy-Zzz /* <- TEST_ME_1 "));
        assert!(edited_text[0].contents.contains("<Xxx_Yyy-Zzz> baz-prop"));
        assert!(edited_text[0].contents.contains("<Xxx_Yyy-Zzz> bar-prop"));
    }

    #[test]
    fn test_rename_struct_from_definition_with_underscore() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
export struct F_oo /* <- TEST_ME_1 */ {
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

        let edited_text = rename_tester_with_new_name(
            &document_cache,
            &test::main_test_file_name(),
            "_1",
            "Xxx_Yyy-Zzz",
        );

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("export struct Xxx_Yyy-Zzz /* <- TEST_ME_1 "));
        assert!(edited_text[0].contents.contains("<Xxx_Yyy-Zzz> baz-prop"));
        assert!(edited_text[0].contents.contains("<Xxx_Yyy-Zzz> bar-prop"));
    }

    #[test]
    fn test_rename_struct_from_definition_with_renaming_export() {
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
struct Foo /* <- TEST_ME_1 */ {
    test-me: bool,
}

export { Foo as FExport }
                "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let edited_text =
            rename_tester(&document_cache, &test::test_file_name("source.slint"), "_1");

        assert_eq!(edited_text.len(), 1);
        assert_eq!(
            edited_text[0].url.to_file_path().unwrap(),
            test::test_file_name("source.slint")
        );
        assert!(edited_text[0].contents.contains("struct XxxYyyZzz /* <- TEST_ME_1 "));
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz as FExport }"));
    }

    #[test]
    fn test_rename_struct_from_definition_with_export() {
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
export struct Foo /* <- TEST_ME_1 */ { test-me: bool, }
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

        let edited_text =
            rename_tester(&document_cache, &test::test_file_name("source.slint"), "_1");

        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed.contents.contains("XxxYyyZzz"));
                assert!(!ed.contents.contains("Foo"));
                assert!(ed.contents.contains("UserComponent"));
                assert!(ed.contents.contains("import { XxxYyyZzz as User3Fxx }"));
                assert!(ed.contents.contains("import { User4Fxx }"));
            } else if ed_path == test::test_file_name("source.slint") {
                assert!(ed.contents.contains("export struct XxxYyyZzz /* <- TEST_ME_1 "));
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
    fn test_rename_enum_from_definition() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
    export { Foo }

    enum Foo /* <- TEST_ME_1 */ {
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

        let edited_text = rename_tester(&document_cache, &test::main_test_file_name(), "_1");

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz }"));
        assert!(edited_text[0].contents.contains("enum XxxYyyZzz /* <- TEST_ME_1 "));
        assert!(edited_text[0].contents.contains("test,"));
        assert!(edited_text[0].contents.contains("property <XxxYyyZzz> baz-prop"));
        assert!(edited_text[0].contents.contains("baz-prop: XxxYyyZzz.test;"));
        assert!(edited_text[0].contents.contains("property <XxxYyyZzz> bar-prop"));
    }

    #[test]
    fn test_rename_enum_from_definition_with_struct() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
    enum Foo /* <- TEST_ME_1 */ {
        M1, M2,
    }

    export { Foo }

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

        let edited_text = rename_tester(&document_cache, &test::main_test_file_name(), "_1");

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("enum XxxYyyZzz /* <- TEST_ME_1 */ "));
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz }"));
        assert!(edited_text[0].contents.contains("property <XxxYyyZzz> baz-prop"));
        assert!(edited_text[0].contents.contains("property <XxxYyyZzz> bar-prop"));
    }

    #[test]
    fn test_rename_enum_from_definition_with_renaming_export() {
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
    export enum Foo /* <- TEST_ME_1 */ {
        OM1, OM2,
    }

    export { Foo as FExport }
                    "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let edited_text =
            rename_tester(&document_cache, &test::test_file_name("source.slint"), "_1");

        assert_eq!(edited_text.len(), 1);
        assert_eq!(
            edited_text[0].url.to_file_path().unwrap(),
            test::test_file_name("source.slint")
        );
        assert!(edited_text[0].contents.contains("export enum XxxYyyZzz /* <- TEST_ME_1 "));
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz as FExport"));
        assert!(!edited_text[0].contents.contains("Foo"));
    }

    #[test]
    fn test_rename_enum_from_definition_with_export() {
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
    export enum F_o-o /* <- TEST_ME_1 */ { M1, M2, }
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

        let edited_text =
            rename_tester(&document_cache, &test::test_file_name("source.slint"), "_1");

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
                assert!(ed.contents.contains("export enum XxxYyyZzz /* <- TEST_ME_1 "));
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

    #[track_caller]
    fn find_declaration_node_by_comment(
        document_cache: &common::DocumentCache,
        document_path: &Path,
        suffix: &str,
    ) -> DeclarationNode {
        let name = find_node_by_comment(document_cache, document_path, suffix);
        find_declaration_node(document_cache, &main_identifier(&name).unwrap()).unwrap()
    }

    #[test]
    fn test_rename_component_from_use() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
export { Foo /* <- TEST_ME_1 */ }

enum Xyz { Foo, Bar }

struct Abc { Foo: Xyz }

component Foo /* <- TEST_ME_TARGET */ inherits Rectangle {
    @children
}

component Baz {
    Foo /* <- TEST_ME_2 */ { }
}

struct Foo {
    bar: bool,
}

export component Bar inherits Foo /* <- TEST_ME_3 */ {
    Foo /* <- TEST_ME_4 */ { }
    Rectangle {
        Foo /* <- TEST_ME_5 */ { }
        Foo := Baz { }
    }

    if true: Rectangle {
        Foo /* <- TEST_ME_6 */ { }
    }

    if false: Rectangle {
        Foo /* <- TEST_ME_7 */ { }
    }

    function Foo(Foo: int) { Foo + 1; }
    function F() { self.Foo(42); }

    for i in [1, 2, 3]: Foo /* <- TEST_ME_8 */ { }
}
                "#
                .to_string(),
            )]),
            true, // Component `Foo` is replacing a component with the same name
        );

        let target =
            find_token_by_comment(&document_cache, &test::main_test_file_name(), "_TARGET");

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_1");
        id.query.is_same_symbol(&document_cache, target.clone());

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_2");
        id.query.is_same_symbol(&document_cache, target.clone());

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_3");
        id.query.is_same_symbol(&document_cache, target.clone());

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_4");
        id.query.is_same_symbol(&document_cache, target.clone());

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_5");
        id.query.is_same_symbol(&document_cache, target.clone());

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_6");
        id.query.is_same_symbol(&document_cache, target.clone());

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_7");
        id.query.is_same_symbol(&document_cache, target.clone());

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_8");
        id.query.is_same_symbol(&document_cache, target);
    }

    #[test]
    fn test_rename_struct_from_use() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
struct Foo /* <- TEST_ME_DECL */ {
    test: bool,
}

struct Bar {
    bar-test: Foo /* <- TEST_ME_1 */,
}

export component Bar {
    property <Foo /* <- TEST_ME_2 */> bar-prop: { test: false };
}
                    "#
                .to_string(),
            )]),
            false,
        );

        let declaration = find_declaration_node(
            &document_cache,
            &find_token_by_comment(&document_cache, &test::main_test_file_name(), "_DECL"),
        )
        .unwrap()
        .query
        .token;

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_1");
        id.query.is_same_symbol(&document_cache, declaration.clone());

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_2");
        id.query.is_same_symbol(&document_cache, declaration);
    }

    #[test]
    fn test_rename_component_from_use_with_export() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([
                (
                    Url::from_file_path(test::main_test_file_name()).unwrap(),
                    r#"
    import { F-o_o /* <- TEST_ME_IMPORT1 */ } from "source.slint";
    import { UserComponent } from "user.slint";
    import { User2Component } from "user2.slint";
    import { F-o-o /* <- TEST_ME_IMPORT2 */ as User3Fxx /* <- TEST_ME_IN1 */ } from "user3.slint";
    import { User4Fxx } from "user4.slint";

    export component Main {
        F_o_o /* <- TEST_ME_1 */ { }
        UserComponent { }
        User2Component { }
        User3Fxx /* <- TEST_ME_2 */ { }
        User4Fxx /* <- TEST_ME_3 */ { }
    }
                    "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("source.slint")).unwrap(),
                    r#"
    export component F_o-o /* <- TEST_ME_DEF1 */ { @children }
                    "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user.slint")).unwrap(),
                    r#"
    import { F-o-o /* <- TEST_ME_IMPORT3 */ as Bar } from "source.slint";

    export component UserComponent {
        Bar /* <- TEST_ME_4 */ { }
    }

    export { Bar }
                    "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user2.slint")).unwrap(),
                    r#"
    import { F_o_o /* <- TEST_ME_IMPORT4 */ as XxxYyyZzz } from "source.slint";

    export component User2Component {
        XxxYyyZzz /* <- TEST_ME_5 */ { }
    }
                    "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user3.slint")).unwrap(),
                    r#"
    import { F-o_o /* <- TEST_ME_IMPORT5 */ } from "source.slint";

    export { F_o-o /* <- TEST_ME_EXPORT1 */ }
                    "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user4.slint")).unwrap(),
                    r#"
    import { F-o_o /* <- TEST_ME_IMPORT6 */ } from "source.slint";

    export { F_o-o /* <- TEST_ME_EXPORT2 */ as User4Fxx /* <- TEST_ME_EXT1 */}
                    "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let declaration =
            find_token_by_comment(&document_cache, &test::test_file_name("source.slint"), "_DEF1");

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_1");
        id.query.is_same_symbol(&document_cache, declaration.clone());

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_2");
        let internal_name =
            find_token_by_comment(&document_cache, &test::main_test_file_name(), "_IN1");
        id.query.is_same_symbol(&document_cache, internal_name);

        let export_name =
            find_token_by_comment(&document_cache, &test::test_file_name("user4.slint"), "_EXT1");

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_3");
        id.query.is_same_symbol(&document_cache, export_name);
    }

    #[test]
    fn test_rename_struct_from_use_with_export() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([
                (
                    Url::from_file_path(test::main_test_file_name()).unwrap(),
                    r#"
import { Foo /* <- TEST_ME_IMPORT1 */ } from "source.slint";
import { UserComponent } from "user.slint";
import { User2Struct } from "user2.slint";
import { Foo /* <- TEST_ME_IMPORT2 */ as User3Fxx /* <- TEST_ME_IN1 */} from "user3.slint";
import { User4Fxx } from "user4.slint";

export component Main {
    property <Foo /* <- TEST_ME_1 */> main-prop;
    property <User3Fxx /* <- TEST_ME_2 */> main-prop2;
    property <User2Struct> main-prop3;
    property <User4Fxx /* <- TEST_ME_3 */> main-prop4 <=> uc.user-component-prop;

    property <bool> test: main-prop3.member.test_me;

    uc := UserComponent { }
}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("source.slint")).unwrap(),
                    r#"
export struct Foo /* <- TEST_ME_DEF1 */ { test-me: bool, }
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user.slint")).unwrap(),
                    r#"
import { Foo /* <- TEST_ME_IMPORT1 */ as Bar } from "source.slint";


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
import { Foo /* <- TEST_ME_IMPORT2 */ as XxxYyyZzz } from "source.slint";

export struct User2Struct {
    member: XxxYyyZzz,
}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user3.slint")).unwrap(),
                    r#"
import { Foo /* <- TEST_ME_IMPORT3 */} from "source.slint";

export { Foo /* <- TEST_ME_EXPORT1 */}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user4.slint")).unwrap(),
                    r#"
import { Foo /* <- TEST_ME_IMPORT4 */ } from "source.slint";

export { Foo /* <- TEST_ME_EXPORT2 */ as User4Fxx /* <- TEST_ME_EN1 */}
                "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let declaration =
            find_token_by_comment(&document_cache, &test::test_file_name("source.slint"), "_DEF1");

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_1");
        id.query.is_same_symbol(&document_cache, declaration);

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_2");
        let internal_name =
            find_token_by_comment(&document_cache, &test::main_test_file_name(), "_IN1");
        id.query.is_same_symbol(&document_cache, internal_name);

        let export_name =
            find_token_by_comment(&document_cache, &test::test_file_name("user4.slint"), "_EN1");

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_3");
        id.query.is_same_symbol(&document_cache, export_name);
    }

    #[test]
    fn test_rename_enum_from_use_with_export() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([
                (
                    Url::from_file_path(test::main_test_file_name()).unwrap(),
                    r#"
import { Foo } from "source.slint";
import { UserComponent } from "user.slint";
import { User2Struct } from "user2.slint";
import { Foo as User3Fxx /* <- TEST_ME_IN1 */} from "user3.slint";
import { User4Fxx } from "user4.slint";

export component Main {
    property <Foo /* <- TEST_ME_1 */> main-prop;
    property <User3Fxx /* <- TEST_ME_2 */> main-prop2;
    property <User2Struct> main-prop3;
    property <User4Fxx /* <- TEST_ME_3 */> main-prop4 <=> uc.user-component-prop;

    property <bool> test: main-prop3.member == Foo/* <- TEST_ME_4 */.test;

    uc := UserComponent { }
}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("source.slint")).unwrap(),
                    r#"
export enum Foo /* <- TEST_ME_DEF1 */ { test, }
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user.slint")).unwrap(),
                    r#"
import { Foo /* <- TEST_ME_IMPORT1 */ as Bar } from "source.slint";


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
import { Foo /* <- TEST_ME_IMPORT2 */ as XxxYyyZzz } from "source.slint";

export struct User2Struct {
    member: XxxYyyZzz,
}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user3.slint")).unwrap(),
                    r#"
import { Foo /* <- TEST_ME_IMPORT3 */} from "source.slint";

export { Foo /* <- TEST_ME_EXPORT1 */}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("user4.slint")).unwrap(),
                    r#"
import { Foo /* <- TEST_ME_IMPORT4 */ } from "source.slint";

export { Foo /* <- TEST_ME_EXPORT2 */ as User4Fxx /* <- TEST_ME_EN1 */}
                "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let declaration =
            find_token_by_comment(&document_cache, &test::test_file_name("source.slint"), "_DEF1");

        let internal_name =
            find_token_by_comment(&document_cache, &test::main_test_file_name(), "_IN1");

        let export_name =
            find_token_by_comment(&document_cache, &test::test_file_name("user4.slint"), "_EN1");

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_1");
        id.query.is_same_symbol(&document_cache, declaration);

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_2");
        id.query.is_same_symbol(&document_cache, internal_name);

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_3");
        id.query.is_same_symbol(&document_cache, export_name.clone());

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_4");
        id.query.is_same_symbol(&document_cache, export_name);
    }

    #[test]
    fn test_rename_import_from_internal_name() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([
                (
                    Url::from_file_path(test::main_test_file_name()).unwrap(),
                    r#"
import { Foo as Bar /* <- TEST_ME_1 */ } from "source.slint";

export component Main {
    Bar { }
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
            ]),
            false,
        );

        let edited_text =
            rename_tester_with_new_name(&document_cache, &test::main_test_file_name(), "_1", "Baz");

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0]
            .contents
            .contains("import { Foo as Baz /* <- TEST_ME_1 */ } from \"source.slint\";"));
        assert!(edited_text[0].contents.contains("component Main {"));
        assert!(edited_text[0].contents.contains(" Baz { "));

        let edited_text =
            rename_tester_with_new_name(&document_cache, &test::main_test_file_name(), "_1", "Foo");

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0]
            .contents
            .contains("import { Foo /* <- TEST_ME_1 */ } from \"source.slint\";"));
        assert!(edited_text[0].contents.contains("component Main {"));
        assert!(edited_text[0].contents.contains(" Foo { "));
    }

    #[test]
    fn test_rename_import_from_external_name() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([
                (
                    Url::from_file_path(test::main_test_file_name()).unwrap(),
                    r#"
import { Foo /* <- TEST_ME_1 */ as Bar } from "source.slint";

export component Main {
    Bar { }
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
            ]),
            false,
        );

        let edited_text =
            rename_tester_with_new_name(&document_cache, &test::main_test_file_name(), "_1", "Baz");

        assert_eq!(edited_text.len(), 2);

        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed
                    .contents
                    .contains("import { Baz /* <- TEST_ME_1 */ as Bar } from \"source.slint\";"));
                assert!(ed.contents.contains("component Main {"));
                assert!(ed.contents.contains(" Bar { "));
            } else if ed_path == test::test_file_name("source.slint") {
                assert!(ed.contents.contains("export component Baz { }"));
            } else {
                panic!("Unexpected file!");
            }
        }

        let edited_text =
            rename_tester_with_new_name(&document_cache, &test::main_test_file_name(), "_1", "Bar");

        assert_eq!(edited_text.len(), 2);
        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed.contents.contains("import { Bar } from \"source.slint\";"));
                assert!(ed.contents.contains("component Main {"));
                assert!(ed.contents.contains(" Bar { "));
            } else if ed_path == test::test_file_name("source.slint") {
                assert!(ed.contents.contains("export component Bar { }"));
            } else {
                panic!("Unexpected file!");
            }
        }
    }

    #[test]
    fn test_rename_import_from_external_name_with_export_renaming() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([
                (
                    Url::from_file_path(test::main_test_file_name()).unwrap(),
                    r#"
import { Foo /* <- TEST_ME_1 */ as Bar } from "source.slint";

export component Main {
    Bar { }
}
                "#
                    .to_string(),
                ),
                (
                    Url::from_file_path(test::test_file_name("source.slint")).unwrap(),
                    r#"
component XxxYyyZzz { }

export { XxxYyyZzz as Foo }
                "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let edited_text = rename_tester_with_new_name(
            &document_cache,
            &test::main_test_file_name(),
            "_1",
            "XFooX",
        );

        assert_eq!(edited_text.len(), 2);

        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed
                    .contents
                    .contains("import { XFooX /* <- TEST_ME_1 */ as Bar } from \"source.slint\";"));
                assert!(ed.contents.contains("component Main {"));
                assert!(ed.contents.contains(" Bar { "));
            } else if ed_path == test::test_file_name("source.slint") {
                assert!(ed.contents.contains("component XxxYyyZzz { }"));
                assert!(ed.contents.contains("export { XxxYyyZzz as XFooX }"));
            } else {
                panic!("Unexpected file!");
            }
        }

        let edited_text =
            rename_tester_with_new_name(&document_cache, &test::main_test_file_name(), "_1", "Bar");

        assert_eq!(edited_text.len(), 2);

        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed.contents.contains("import { Bar } from \"source.slint\";"));
                assert!(ed.contents.contains("component Main {"));
                assert!(ed.contents.contains(" Bar { "));
            } else if ed_path == test::test_file_name("source.slint") {
                assert!(ed.contents.contains("component XxxYyyZzz { }"));
                assert!(ed.contents.contains("export { XxxYyyZzz as Bar }"));
            } else {
                panic!("Unexpected file!");
            }
        }

        let edited_text = rename_tester(&document_cache, &test::main_test_file_name(), "_1");

        assert_eq!(edited_text.len(), 2);
        for ed in &edited_text {
            let ed_path = ed.url.to_file_path().unwrap();
            if ed_path == test::main_test_file_name() {
                assert!(ed.contents.contains(
                    "import { XxxYyyZzz /* <- TEST_ME_1 */ as Bar } from \"source.slint\";"
                ));
                assert!(ed.contents.contains("component Main {"));
                assert!(ed.contents.contains(" Bar { "));
            } else if ed_path == test::test_file_name("source.slint") {
                assert!(ed.contents.contains("component XxxYyyZzz { }"));
                assert!(ed.contents.contains("export { XxxYyyZzz }"));
            } else {
                panic!("Unexpected file!");
            }
        }
    }

    #[test]
    fn test_rename_property_from_definition() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"

component re_name-me {
    property <bool> re_name-me /* <- TEST_ME_1 */: true;

    function re_name-me_(re-name_me: int) { /* 1 */ self.re-name_me = re-name_me >= 42; }
}

export component Bar {
    property <bool> re_name-me /* <- TEST_ME_2 */: true;

    function re_name-me_(re-name_me: int) { /* 2 */ self.re-name_me = re-name_me >= 42; }

    re_name-me { }
}


                "#
                .to_string(),
            )]),
            true, // Component `Foo` is replacing a component with the same name
        );

        let edited_text = rename_tester(&document_cache, &test::main_test_file_name(), "_1");
        assert_eq!(edited_text.len(), 1);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("component re_name-me {"));
        assert!(edited_text[0].contents.contains("property <bool> XxxYyyZzz /* <- TEST_ME_1 "));
        assert!(edited_text[0].contents.contains(" <- TEST_ME_1 */: true;"));
        assert!(edited_text[0]
            .contents
            .contains("function re_name-me_(re-name_me: int) { /* 1 */"));
        assert!(edited_text[0].contents.contains("/* 1 */ self.XxxYyyZzz = re-name_me >= 42;"));

        assert!(edited_text[0].contents.contains("export component Bar {"));
        assert!(edited_text[0].contents.contains("property <bool> re_name-me /* <- TEST_ME_2 "));
        assert!(edited_text[0].contents.contains(" <- TEST_ME_2 */: true;"));
        assert!(edited_text[0]
            .contents
            .contains("function re_name-me_(re-name_me: int) { /* 2 */"));
        assert!(edited_text[0].contents.contains("/* 2 */ self.re-name_me = re-name_me >= 42;"));
        assert!(edited_text[0].contents.contains("re_name-me { }"));

        let edited_text = rename_tester(&document_cache, &test::main_test_file_name(), "_2");
        assert_eq!(edited_text.len(), 1);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("component re_name-me {"));
        assert!(edited_text[0].contents.contains("property <bool> re_name-me /* <- TEST_ME_1 "));
        assert!(edited_text[0].contents.contains(" <- TEST_ME_1 */: true;"));
        assert!(edited_text[0]
            .contents
            .contains("function re_name-me_(re-name_me: int) { /* 1 */"));
        assert!(edited_text[0].contents.contains("/* 1 */ self.re-name_me = re-name_me >= 42;"));

        assert!(edited_text[0].contents.contains("export component Bar {"));
        assert!(edited_text[0].contents.contains("property <bool> XxxYyyZzz /* <- TEST_ME_2 "));
        assert!(edited_text[0].contents.contains(" <- TEST_ME_2 */: true;"));
        assert!(edited_text[0]
            .contents
            .contains("function re_name-me_(re-name_me: int) { /* 2 */"));
        assert!(edited_text[0].contents.contains("/* 2 */ self.XxxYyyZzz = re-name_me >= 42;"));
        assert!(edited_text[0].contents.contains("re_name-me { }"));
    }

    #[test]
    fn test_rename_property_from_use() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"

component re_name-me {
    property <bool> re_name-me /* 1 */: true;

    function re_name-me_(re-name_me: int) { /* 1 */ self.re-name_me /* <- TEST_ME_1 */ = re-name_me >= 42; }
}

export component Bar {
    property <bool> re_name-me /* 2 */: true;

    function re_name-me_(re-name_me: int) { /* 2 */ self.re-name_me /* <- TEST_ME_2 */ = re-name_me >= 42; }

    re_name-me { }
}


                "#
                .to_string(),
            )]),
            true, // Component `Foo` is replacing a component with the same name
        );

        let edited_text = rename_tester(&document_cache, &test::main_test_file_name(), "_1");
        assert_eq!(edited_text.len(), 1);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("component re_name-me {"));
        assert!(edited_text[0].contents.contains("property <bool> XxxYyyZzz /* 1 */"));
        assert!(edited_text[0].contents.contains(" 1 */: true;"));
        assert!(edited_text[0]
            .contents
            .contains("function re_name-me_(re-name_me: int) { /* 1 */"));
        assert!(edited_text[0]
            .contents
            .contains("/* 1 */ self.XxxYyyZzz /* <- TEST_ME_1 */ = re-name_me >= 42;"));

        assert!(edited_text[0].contents.contains("export component Bar {"));
        assert!(edited_text[0].contents.contains("property <bool> re_name-me /* 2 "));
        assert!(edited_text[0].contents.contains(" 2 */: true;"));
        assert!(edited_text[0]
            .contents
            .contains("function re_name-me_(re-name_me: int) { /* 2 */"));
        assert!(edited_text[0]
            .contents
            .contains("/* 2 */ self.re-name_me /* <- TEST_ME_2 */ = re-name_me >= 42;"));
        assert!(edited_text[0].contents.contains("re_name-me { }"));

        let edited_text = rename_tester(&document_cache, &test::main_test_file_name(), "_2");
        assert_eq!(edited_text.len(), 1);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("component re_name-me {"));
        assert!(edited_text[0].contents.contains("property <bool> re_name-me /* 1 "));
        assert!(edited_text[0].contents.contains(" 1 */: true;"));
        assert!(edited_text[0]
            .contents
            .contains("function re_name-me_(re-name_me: int) { /* 1 */"));
        assert!(edited_text[0]
            .contents
            .contains("/* 1 */ self.re-name_me /* <- TEST_ME_1 */ = re-name_me >= 42;"));

        assert!(edited_text[0].contents.contains("export component Bar {"));
        assert!(edited_text[0].contents.contains("property <bool> XxxYyyZzz /* 2 "));
        assert!(edited_text[0].contents.contains(" 2 */: true;"));
        assert!(edited_text[0]
            .contents
            .contains("function re_name-me_(re-name_me: int) { /* 2 */"));
        assert!(edited_text[0]
            .contents
            .contains("/* 2 */ self.XxxYyyZzz /* <- TEST_ME_2 */ = re-name_me >= 42;"));
        assert!(edited_text[0].contents.contains("re_name-me { }"));
    }
}
