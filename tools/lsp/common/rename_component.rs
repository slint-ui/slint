// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::{Path, PathBuf};

use crate::{common, util};

use i_slint_compiler::diagnostics::Spanned;
use i_slint_compiler::parser::{
    syntax_nodes, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize,
};
use lsp_types::Url;
use smol_str::SmolStr;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

pub fn main_identifier(input: &SyntaxNode) -> Option<SyntaxToken> {
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
    new_type: &SmolStr,
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
    new_type: &SmolStr,
    local_rename_function: &dyn Fn(
        &common::DocumentCache,
        &syntax_nodes::Document,
        &TextRange,
        &SmolStr,
        &SmolStr,
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
            local_rename_function,
            edits,
        );
    }
}

fn import_path(
    document_directory: &Path,
    import_specifier: &syntax_nodes::ImportSpecifier,
) -> Option<PathBuf> {
    let import = import_specifier
        .child_token(SyntaxKind::StringLiteral)
        .map(|t| t.text().trim_matches('"').to_string())?;

    if import == "std-widgets.slint" || import == "std-widgets.s60" {
        return None; // No need to ever look at this!
    }

    // Do not bother with the TypeLoader: It will check the FS, which we do not use:-/
    Some(i_slint_compiler::pathutils::clean_path(&document_directory.join(import)))
}

fn fix_import_in_document(
    document_cache: &common::DocumentCache,
    document_node: &syntax_nodes::Document,
    exporter_path: &Path,
    old_type: &SmolStr,
    new_type: &SmolStr,
    local_rename_function: &dyn Fn(
        &common::DocumentCache,
        &syntax_nodes::Document,
        &TextRange,
        &SmolStr,
        &SmolStr,
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
            fix_exports(
                document_cache,
                document_node,
                old_type,
                new_type,
                local_rename_function,
                edits,
            );

            // Change all local usages:
            local_rename_function(
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
    new_type: &SmolStr,
    local_rename_function: &dyn Fn(
        &common::DocumentCache,
        &syntax_nodes::Document,
        &TextRange,
        &SmolStr,
        &SmolStr,
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

                let update_imports = if let Some(export_name) = specifier.ExportName() {
                    // Remove "as Foo"
                    if i_slint_compiler::parser::identifier_text(&export_name).as_ref()
                        == Some(new_type)
                    {
                        let start_position = util::text_size_to_lsp_position(
                            source_file,
                            identifier
                                .text_range()
                                .end()
                                .checked_add(1.into())
                                .expect("There are more tokens"),
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
                        local_rename_function,
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
) -> TextRange {
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
                            return TextRange::new(
                                start,
                                new_grand_parent.last_token().unwrap().text_range().end(),
                            );
                        }
                    }
                    SyntaxKind::EnumDeclaration | SyntaxKind::StructDeclaration => {
                        if [SyntaxKind::EnumDeclaration, SyntaxKind::StructDeclaration]
                            .contains(&new_grand_parent.kind())
                        {
                            return TextRange::new(
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

    TextRange::new(start, document_node.text_range().end())
}

fn local_rename_function(
    document_cache: &common::DocumentCache,
    identifier: &SyntaxNode,
) -> &'static dyn Fn(
    &common::DocumentCache,
    &syntax_nodes::Document,
    &TextRange,
    &SmolStr,
    &SmolStr,
    &mut Vec<common::SingleTextEdit>,
) {
    fn change_local_element_type(
        document_cache: &common::DocumentCache,
        document_node: &syntax_nodes::Document,
        validity_range: &TextRange,
        old_type: &SmolStr,
        new_type: &SmolStr,
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
        validity_range: &TextRange,
        old_type: &SmolStr,
        new_type: &SmolStr,
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

    let type_name = i_slint_compiler::parser::identifier_text(&identifier).unwrap_or_default();
    let document = document_cache.get_document_for_source_file(&identifier.source_file).unwrap();

    if document.local_registry.lookup_element(type_name.as_str()).is_ok() {
        &change_local_element_type
    } else {
        &change_local_data_type
    }
}

fn rename_local_import(
    document_cache: &common::DocumentCache,
    internal_name: &syntax_nodes::InternalName,
    new_type: &SmolStr,
    local_rename_function: &dyn Fn(
        &common::DocumentCache,
        &syntax_nodes::Document,
        &TextRange,
        &SmolStr,
        &SmolStr,
        &mut Vec<common::SingleTextEdit>,
    ),
) -> lsp_types::WorkspaceEdit {
    let Some(old_type) = i_slint_compiler::parser::identifier_text(internal_name) else {
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

    let internal_name_identifier = main_identifier(&internal_name).unwrap();

    let parent: syntax_nodes::ImportIdentifier = internal_name.parent().unwrap().into();
    let external_name = parent.ExternalName();
    let external_name_identifier = main_identifier(&external_name).unwrap();
    let external_str =
        i_slint_compiler::parser::normalize_identifier(external_name_identifier.text());

    if external_str == *new_type {
        // `New as Old` -> `New`
        edits.push(
            common::SingleTextEdit::from_path(
                document_cache,
                internal_name.source_file.path(),
                lsp_types::TextEdit {
                    range: util::text_range_to_lsp_range(
                        &external_name_identifier.source_file,
                        TextRange::new(
                            external_name_identifier.next_token().unwrap().text_range().start(),
                            internal_name_identifier.text_range().end(),
                        ),
                    ),
                    new_text: String::new(),
                },
            )
            .expect("URL conversion can not fail here"),
        )
    } else if old_type != *new_type {
        // `Some as Old` -> `Some as New`
        edits.push(
            common::SingleTextEdit::from_path(
                document_cache,
                internal_name.source_file.path(),
                lsp_types::TextEdit {
                    range: util::token_to_lsp_range(
                        main_identifier(internal_name).as_ref().unwrap(),
                    ),
                    new_text: new_type.to_string(),
                },
            )
            .expect("URL conversion can not fail here"),
        );
    }

    // Change exports
    fix_exports(
        document_cache,
        document_node,
        &old_type,
        new_type,
        local_rename_function,
        &mut edits,
    );

    // Change all local usages:
    local_rename_function(
        document_cache,
        document_node,
        &document_node.text_range(),
        &old_type,
        new_type,
        &mut edits,
    );

    common::create_workspace_edit_from_single_text_edits(edits)
}

fn rename_local_export_name(
    document_cache: &common::DocumentCache,
    export_name: &syntax_nodes::ExportName,
    new_type: &SmolStr,
    local_rename_function: &dyn Fn(
        &common::DocumentCache,
        &syntax_nodes::Document,
        &TextRange,
        &SmolStr,
        &SmolStr,
        &mut Vec<common::SingleTextEdit>,
    ),
) -> lsp_types::WorkspaceEdit {
    let Some(old_type) = &i_slint_compiler::parser::identifier_text(export_name) else {
        return Default::default();
    };
    let Some(document) = document_cache.get_document_for_source_file(&export_name.source_file)
    else {
        return Default::default();
    };
    let Some(document_node) = &document.node else {
        return Default::default();
    };

    let mut edits = vec![];

    let specifier: syntax_nodes::ExportSpecifier = export_name.parent().unwrap().into();
    let internal_name = specifier.ExportIdentifier();
    if i_slint_compiler::parser::identifier_text(&internal_name).as_ref() == Some(new_type) {
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
                    range: util::token_to_lsp_range(&main_identifier(&export_name).unwrap()),
                    new_text: new_type.to_string(),
                },
            )
            .expect("URL conversion can not fail here"),
        );
    }

    // Change exports
    let my_path = document_node.source_file.path();
    fix_imports(document_cache, my_path, old_type, new_type, local_rename_function, &mut edits);

    common::create_workspace_edit_from_single_text_edits(edits)
}

#[derive(Debug)]
pub enum DeclarationNode {
    DeclaredIdentifier(syntax_nodes::DeclaredIdentifier),
    InternalName(syntax_nodes::InternalName),
    ExportName(syntax_nodes::ExportName),
}

pub fn find_declaration_node(
    document_cache: &common::DocumentCache,
    node: SyntaxNode,
) -> Option<DeclarationNode> {
    match node.kind() {
        SyntaxKind::DeclaredIdentifier => Some(DeclarationNode::DeclaredIdentifier(node.into())),
        SyntaxKind::InternalName => Some(DeclarationNode::InternalName(node.into())),
        SyntaxKind::ExportName => Some(DeclarationNode::ExportName(node.into())),
        _ => find_declaration_node_for_syntax_node(document_cache, node),
    }
}

impl DeclarationNode {
    pub fn rename(
        &self,
        document_cache: &common::DocumentCache,
        new_type: &str,
    ) -> crate::Result<lsp_types::WorkspaceEdit> {
        let new_type = SmolStr::from(new_type);

        match self {
            DeclarationNode::DeclaredIdentifier(identifier) => rename_declared_identifier(
                document_cache,
                identifier,
                &new_type,
                local_rename_function(document_cache, identifier),
            ),
            DeclarationNode::InternalName(name) => Ok(rename_local_import(
                document_cache,
                name,
                &new_type,
                local_rename_function(document_cache, name),
            )),
            DeclarationNode::ExportName(name) => Ok(rename_local_export_name(
                document_cache,
                name,
                &new_type,
                local_rename_function(document_cache, name),
            )),
        }
    }

    #[cfg(test)]
    fn as_declared_identifier(&self) -> syntax_nodes::DeclaredIdentifier {
        match &self {
            DeclarationNode::DeclaredIdentifier(identifier) => identifier.clone(),
            _ => panic!("Wrong declaration node"),
        }
    }

    #[cfg(test)]
    fn as_internal_name(&self) -> syntax_nodes::InternalName {
        match &self {
            DeclarationNode::InternalName(name) => name.clone(),
            _ => panic!("Wrong declaration node"),
        }
    }

    #[cfg(test)]
    fn as_export_name(&self) -> syntax_nodes::ExportName {
        match &self {
            DeclarationNode::ExportName(name) => name.clone(),
            _ => panic!("Wrong declaration node"),
        }
    }
}

impl From<syntax_nodes::DeclaredIdentifier> for DeclarationNode {
    fn from(value: syntax_nodes::DeclaredIdentifier) -> Self {
        Self::DeclaredIdentifier(value)
    }
}

impl From<syntax_nodes::InternalName> for DeclarationNode {
    fn from(value: syntax_nodes::InternalName) -> Self {
        Self::InternalName(value)
    }
}

impl From<syntax_nodes::ExportName> for DeclarationNode {
    fn from(value: syntax_nodes::ExportName) -> Self {
        Self::ExportName(value)
    }
}

impl TryFrom<SyntaxNode> for DeclarationNode {
    type Error = String;

    fn try_from(value: SyntaxNode) -> Result<Self, Self::Error> {
        match value.kind() {
            SyntaxKind::DeclaredIdentifier => Ok(Self::DeclaredIdentifier(value.into())),
            SyntaxKind::InternalName => Ok(Self::InternalName(value.into())),
            SyntaxKind::ExportName => Ok(Self::ExportName(value.into())),
            _ => Err("Can not convert into an DeclarationNode".into()),
        }
    }
}

fn find_last_declared_identifier_at_or_before(
    token: SyntaxToken,
    type_name: &SmolStr,
) -> Option<syntax_nodes::DeclaredIdentifier> {
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

#[derive(Clone, Debug, PartialEq)]
enum DeclarationNodeQueryKind {
    Component,
    Type,
    Unknown,
}

#[derive(Clone, Debug)]
struct DeclarationNodeQuery {
    kind: DeclarationNodeQueryKind,
    name: SmolStr,
    node: SyntaxNode,
}

impl DeclarationNodeQuery {
    fn new(node: SyntaxNode) -> Option<Self> {
        let name = i_slint_compiler::parser::identifier_text(&node)?;
        let parent = node.parent()?;
        let grand_parent = parent.parent()?;

        let kind = match (node.kind(), parent.kind(), grand_parent.kind()) {
            (SyntaxKind::QualifiedName, SyntaxKind::Element, _) => {
                Some(DeclarationNodeQueryKind::Component)
            }
            (SyntaxKind::DeclaredIdentifier, SyntaxKind::Component, _) => {
                Some(DeclarationNodeQueryKind::Component)
            }
            (SyntaxKind::QualifiedName, SyntaxKind::Type, _) => {
                Some(DeclarationNodeQueryKind::Type)
            }
            (SyntaxKind::QualifiedName, SyntaxKind::Expression, _) => {
                Some(DeclarationNodeQueryKind::Type)
            }
            (
                SyntaxKind::DeclaredIdentifier,
                SyntaxKind::EnumDeclaration | SyntaxKind::StructDeclaration,
                _,
            ) => Some(DeclarationNodeQueryKind::Type),
            (SyntaxKind::ExportIdentifier | SyntaxKind::ExternalName, _, _) => {
                Some(DeclarationNodeQueryKind::Unknown)
            }
            (_, _, _) => None,
        };

        kind.map(|k| DeclarationNodeQuery { kind: k, name, node })
    }

    fn name(&self) -> &SmolStr {
        &self.name
    }

    fn start_token(&self) -> Option<SyntaxToken> {
        if self.kind == DeclarationNodeQueryKind::Unknown {
            None
        } else {
            self.node.first_token().and_then(|t| t.prev_token())
        }
    }

    fn is_declaration_for(&self, declared_identifier: &syntax_nodes::DeclaredIdentifier) -> bool {
        if let Some(parent) = declared_identifier.parent() {
            match self.kind {
                DeclarationNodeQueryKind::Component => parent.kind() == SyntaxKind::Component,
                DeclarationNodeQueryKind::Type => {
                    parent.kind() == SyntaxKind::StructDeclaration
                        || parent.kind() == SyntaxKind::EnumDeclaration
                }
                DeclarationNodeQueryKind::Unknown => [
                    SyntaxKind::Component,
                    SyntaxKind::StructDeclaration,
                    SyntaxKind::EnumDeclaration,
                ]
                .contains(&parent.kind()),
            }
        } else {
            false
        }
    }
}

fn find_declaration_node_before_token(
    document_cache: &common::DocumentCache,
    document_node: &syntax_nodes::Document,
    start_token: Option<SyntaxToken>,
    query: &DeclarationNodeQuery,
) -> Option<DeclarationNode> {
    // Exported under a custom name?
    if start_token.is_none() {
        for export_item in document_node.ExportsList() {
            for specifier in export_item.ExportSpecifier() {
                if let Some(export_name) = specifier.ExportName() {
                    if i_slint_compiler::parser::identifier_text(&export_name).as_ref()
                        == Some(&query.name)
                    {
                        return Some(DeclarationNode::from(export_name));
                    }
                }
            }
        }
    }

    // Locally defined?
    let last_document_token = document_node.last_token();
    let mut token = start_token.clone().or_else(|| last_document_token);

    while let Some(t) = token {
        if let Some(declared_identifier) =
            find_last_declared_identifier_at_or_before(t.clone(), query.name())
        {
            if query.is_declaration_for(&declared_identifier) {
                if let Some(start_token) = &start_token {
                    if declared_identifier
                        .parent()
                        .map(|p| p.text_range().contains_range(start_token.text_range()))
                        .unwrap_or_default()
                    {
                        if let Some(prev_token) =
                            declared_identifier.first_token().and_then(|t| t.prev_token())
                        {
                            // We are "inside" a new declaration of `type_name`, ignore this one as it is not valid yet!
                            return find_declaration_node_before_token(
                                document_cache,
                                document_node,
                                Some(prev_token),
                                query,
                            );
                        }
                    }
                }
                return Some(DeclarationNode::from(declared_identifier));
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

                if internal.as_ref() == Some(query.name()) {
                    return Some(id.InternalName().unwrap().into());
                }

                if external.as_ref() == Some(query.name()) {
                    let document_path = document_node.source_file.path();
                    let document_dir = document_path.parent()?;
                    let path = import_path(document_dir, &import_spec)?;
                    let import_doc = document_cache.get_document_by_path(&path)?;
                    let import_doc_node = import_doc.node.as_ref()?;

                    return find_declaration_node_before_token(
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

/// Find the closest "rename-able" SyntaxNode
fn find_declaration_node_for_syntax_node(
    document_cache: &common::DocumentCache,
    node: SyntaxNode,
) -> Option<DeclarationNode> {
    let source_file = node.source_file().cloned()?;
    let expected_node = DeclarationNodeQuery::new(node)?;
    let document = document_cache.get_document_for_source_file(&source_file)?;

    find_declaration_node_before_token(
        document_cache,
        document.node.as_ref()?,
        expected_node.start_token(),
        &expected_node,
    )
}

/// Helper function to rename a `DeclaredIdentifier`.
fn rename_declared_identifier(
    document_cache: &common::DocumentCache,
    identifier: &syntax_nodes::DeclaredIdentifier,
    new_type: &SmolStr,
    local_rename_function: &dyn Fn(
        &common::DocumentCache,
        &syntax_nodes::Document,
        &TextRange,
        &SmolStr,
        &SmolStr,
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
    local_rename_function(
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
            local_rename_function,
            &mut edits,
        );

        if is_symbol_name_exported(document_node, &old_type) {
            let my_path = source_file.path();

            fix_imports(
                document_cache,
                my_path,
                &old_type,
                new_type,
                local_rename_function,
                &mut edits,
            );
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

    #[track_caller]
    fn find_node_by_comment(
        document_cache: &common::DocumentCache,
        document_path: &Path,
        suffix: &str,
    ) -> SyntaxNode {
        let document = document_cache.get_document_by_path(document_path).unwrap();
        let document = document.node.as_ref().unwrap();

        let comment = document
            .descendants_with_tokens()
            .find(|node_or_token| {
                node_or_token.kind() == SyntaxKind::Comment
                    && node_or_token
                        .as_token()
                        .unwrap()
                        .text()
                        .contains(&format!("<- TEST_ME{suffix} "))
            })
            .unwrap()
            .as_token()
            .unwrap()
            .clone();

        let source_file = document.source_file.clone();

        let mut token = comment.prev_token();

        while let Some(t) = &token {
            if ![SyntaxKind::Comment, SyntaxKind::Whitespace].contains(&t.kind()) {
                break;
            }
            token = t.prev_token();
        }

        token
            .map(|t| SyntaxNode { node: t.parent().unwrap(), source_file: source_file.clone() })
            .unwrap()
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
    fn test_rename_component_from_definition() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
component Foo /* <- TEST_ME_1 */ { @children }

export { Foo }

enum Xyz { Foo, Bar }

struct Abc { Foo: Xyz }

component Baz {
    Foo /* Baz */ { }
}

component Foo /* <- TEST_ME_2 */ inherits Foo {
    Foo /* 1 */ { }
    @children
}

export component Bar inherits Foo {
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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);

        assert!(edited_text[0].contents.contains("component XxxYyyZzz /* <- TEST_ME_1 "));
        // The *last* Foo gets exported!
        assert!(edited_text[0].contents.contains("export { Foo }"));
        assert!(edited_text[0].contents.contains("enum Xyz { Foo,"));
        assert!(edited_text[0].contents.contains("struct Abc { Foo:"));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* Baz "));
        assert!(edited_text[0]
            .contents
            .contains("component Foo /* <- TEST_ME_2 */ inherits XxxYyyZzz "));
        assert!(edited_text[0].contents.contains("XxxYyyZzz /* 1 */"));

        assert!(edited_text[0].contents.contains("export component Bar inherits Foo {"));
        assert!(edited_text[0].contents.contains("Foo /* <- TEST_ME_3 "));
        assert!(edited_text[0].contents.contains("Foo /* <- TEST_ME_4 "));
        assert!(edited_text[0].contents.contains("Foo := Baz {"));
        assert!(edited_text[0].contents.contains("Foo /* <- TEST_ME_5 "));
        assert!(edited_text[0].contents.contains("Foo /* <- TEST_ME_6 "));
        assert!(edited_text[0].contents.contains("function Foo(Foo: int) { Foo + 1; }"));
        assert!(edited_text[0].contents.contains("function F() { self.Foo(42); }"));
        assert!(edited_text[0].contents.contains("Foo /* <- TEST_ME_7 "));

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_2"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("component Foo /* <- TEST_ME_1 "));
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz }"));
        assert!(edited_text[0].contents.contains("enum Xyz { Foo,"));
        assert!(edited_text[0].contents.contains("struct Abc { Foo:"));
        assert!(edited_text[0].contents.contains("Foo /* Baz "));
        assert!(edited_text[0]
            .contents
            .contains("component XxxYyyZzz /* <- TEST_ME_2 */ inherits Foo "));
        assert!(edited_text[0].contents.contains("Foo /* 1 */"));
        assert!(edited_text[0].contents.contains("export component Bar inherits XxxYyyZzz {"));
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
                format!("component Foo/* <- TEST_ME_1 */{{ }}\nexport component _SLINT_LivePreview inherits Foo {{ /* @lsp:ignore-node */ }}\n")
            )]),
            true,
        );

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
        .unwrap();

        assert_eq!(text_edit::EditIterator::new(&edit).count(), 1);
        // This does not compile as the type was not changed in the _SLINT_LivePreview part!
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
component Foo /* <- TEST_ME_1 */{ }

export { Foo as FExport }
                "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::test_file_name("source.slint"), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::test_file_name("source.slint"), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::test_file_name("s/source.slint"), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
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
export component Foo /* <- TEST_ME_1 */{ }
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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::test_file_name("user1.slint"), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::test_file_name("user2.slint"), "_2"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

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
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap();

        assert!(dn.rename(&document_cache, "Foo").is_err());
        assert!(dn.rename(&document_cache, "UsedStruct").is_ok());
        assert!(dn.rename(&document_cache, "UsedEnum").is_ok());
        assert!(dn.rename(&document_cache, "Baz").is_err());
        assert!(dn.rename(&document_cache, "HorizontalLayout").is_err());
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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("XxxYyyZzz"));
        assert!(!edited_text[0].contents.contains("Foo"));
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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "Xxx_Yyy-Zzz")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "Xxx_Yyy-Zzz")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

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
struct Foo /* <- TEST_ME_1 */{
    test-me: bool,
}

export { Foo as FExport }
                "#
                    .to_string(),
                ),
            ]),
            false,
        );

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::test_file_name("source.slint"), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::test_file_name("source.slint"), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
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
        M1, M2,
    }

    enum Foo /* <- TEST_ME_2 */ {
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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("export { Foo }"));
        assert!(edited_text[0].contents.contains("enum XxxYyyZzz /* <- TEST_ME_1 "));
        assert!(edited_text[0].contents.contains("M1, M2,"));
        assert!(edited_text[0].contents.contains("enum Foo /* <- TEST_ME_2 "));
        assert!(edited_text[0].contents.contains("test,"));
        assert!(edited_text[0].contents.contains("property <Foo> baz-prop"));
        assert!(edited_text[0].contents.contains("baz-prop: Foo.test;"));
        assert!(edited_text[0].contents.contains("property <Foo> bar-prop"));

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_2"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz }"));
        assert!(edited_text[0].contents.contains("enum Foo /* <- TEST_ME_1 "));
        assert!(edited_text[0].contents.contains("M1, M2,"));
        assert!(edited_text[0].contents.contains("enum XxxYyyZzz /* <- TEST_ME_2 "));
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

    struct Foo /* <- TEST_ME_2 */ {
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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("enum XxxYyyZzz /* <- TEST_ME_1 */"));
        assert!(edited_text[0].contents.contains("struct Foo /* <- TEST_ME_2 */"));
        assert!(edited_text[0].contents.contains("export { Foo }"));
        assert!(edited_text[0].contents.contains("test: XxxYyyZzz,"));
        assert!(edited_text[0].contents.contains("property <Foo> baz-prop"));
        assert!(edited_text[0].contents.contains("property <Foo> bar-prop"));

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_2"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0].contents.contains("enum Foo /* <- TEST_ME_1 */"));
        assert!(edited_text[0].contents.contains("export { XxxYyyZzz }"));
        assert!(edited_text[0].contents.contains("struct XxxYyyZzz /* <- TEST_ME_2 */"));
        assert!(edited_text[0].contents.contains("test: Foo,"));
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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::test_file_name("source.slint"), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::test_file_name("source.slint"), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
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
    fn node_eq(id1: &SyntaxNode, id2: &SyntaxNode) {
        assert_eq!(id1.kind(), id2.kind());
        assert_eq!(id1.source_file.path(), id2.source_file.path());
        assert_eq!(id1.text_range(), id2.text_range());
    }

    #[track_caller]
    fn find_declaration_node_by_comment(
        document_cache: &common::DocumentCache,
        document_path: &Path,
        suffix: &str,
    ) -> DeclarationNode {
        let name = find_node_by_comment(document_cache, document_path, suffix);
        find_declaration_node(document_cache, name).unwrap()
    }

    #[track_caller]
    fn find_identifier_by_comment(
        document_cache: &common::DocumentCache,
        document_path: &Path,
        suffix: &str,
    ) -> syntax_nodes::DeclaredIdentifier {
        find_declaration_node_by_comment(document_cache, document_path, suffix)
            .as_declared_identifier()
    }

    #[test]
    fn test_rename_component_from_use() {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                r#"
component Foo /* <- TEST_ME_TARGET1 */ { @children }

export { Foo /* <- TEST_ME_1 */}

enum Xyz { Foo, Bar }

struct Abc { Foo: Xyz }

component Baz {
    Foo /* <- TEST_ME_2 */ { }
}

component Foo /* <- TEST_ME_TARGET2 */ inherits Foo /* <- TEST_ME_3 */ {
    Foo /* <- TEST_ME_4 */ { }
}

export component Bar inherits Foo /* <- TEST_ME_5 */ {
    Foo /* <- TEST_ME_6 */ { }
    Rectangle {
        Foo /* <- TEST_ME_7 */ { }
        Foo := Baz { }
    }

    if true: Rectangle {
        Foo /* <- TEST_ME_8 */ { }
    }

    if false: Rectangle {
        Foo /* <- TEST_ME_9 */ { }
    }

    function Foo(Foo: int) { Foo + 1; }
    function F() { self.Foo(42); }

    for i in [1, 2, 3]: Foo /* <- TEST_ME_10 */ { }
}
                    "#
                .to_string(),
            )]),
            true, // Foo 1 is not used or exported... ?!
        );

        let target1 =
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_TARGET1").into();
        let target2 =
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_TARGET2").into();

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_1");
        node_eq(&target2, &id);

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_2");
        node_eq(&target1, &id);

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_3");
        node_eq(&target1, &id);

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_4");
        node_eq(&target1, &id);

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_5");
        node_eq(&target2, &id);

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_6");
        node_eq(&target2, &id);

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_7");
        node_eq(&target2, &id);

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_8");
        node_eq(&target2, &id);

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_9");
        node_eq(&target2, &id);

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_10");
        node_eq(&target2, &id);
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
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_DECL"),
        )
        .unwrap()
        .as_declared_identifier();

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_1");
        node_eq(&declaration, &id);

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_2");
        node_eq(&declaration, &id);
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
            find_node_by_comment(&document_cache, &test::test_file_name("source.slint"), "_DEF1")
                .into();

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_1");
        node_eq(&declaration, &id);

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_2")
                .as_internal_name();
        let internal_name: syntax_nodes::InternalName =
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_IN1").into();
        node_eq(&internal_name, &id);

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_3")
                .as_export_name();
        let export_name: syntax_nodes::ExportName =
            find_node_by_comment(&document_cache, &test::test_file_name("user4.slint"), "_EXT1")
                .into();
        node_eq(&export_name, &id);
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
            find_node_by_comment(&document_cache, &test::test_file_name("source.slint"), "_DEF1")
                .into();

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_1");
        node_eq(&declaration, &id);

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_2")
                .as_internal_name();
        let internal_name: syntax_nodes::InternalName =
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_IN1").into();
        node_eq(&internal_name, &id);

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_3")
                .as_export_name();
        let export_name: syntax_nodes::ExportName =
            find_node_by_comment(&document_cache, &test::test_file_name("user4.slint"), "_EN1")
                .into();
        node_eq(&export_name, &id);
    }

    #[test]
    fn test_rename_enum_from_use_with_export() {
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
            find_node_by_comment(&document_cache, &test::test_file_name("source.slint"), "_DEF1")
                .into();

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_1");
        node_eq(&declaration, &id);

        let id =
            find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_IMPORT1");
        node_eq(&declaration, &id);

        let id =
            find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_IMPORT2");
        node_eq(&declaration, &id);

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_2")
                .as_internal_name();
        let internal_name: syntax_nodes::InternalName =
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_IN1").into();
        node_eq(&internal_name, &id);

        let id =
            find_declaration_node_by_comment(&document_cache, &test::main_test_file_name(), "_3")
                .as_export_name();
        let export_name: syntax_nodes::ExportName =
            find_node_by_comment(&document_cache, &test::test_file_name("user4.slint"), "_EN1")
                .into();
        node_eq(&export_name, &id);

        let id = find_identifier_by_comment(&document_cache, &test::main_test_file_name(), "_4");
        node_eq(&declaration, &id);
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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "Baz")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

        assert_eq!(edited_text.len(), 1);
        assert!(edited_text[0]
            .contents
            .contains("import { Foo as Baz /* <- TEST_ME_1 */ } from \"source.slint\";"));
        assert!(edited_text[0].contents.contains("component Main {"));
        assert!(edited_text[0].contents.contains(" Baz { "));

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "Foo")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "Baz")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "Bar")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XFooX")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

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

        let edit = find_declaration_node(
            &document_cache,
            find_node_by_comment(&document_cache, &test::main_test_file_name(), "_1"),
        )
        .unwrap()
        .rename(&document_cache, "XxxYyyZzz")
        .unwrap();
        let edited_text = compile_test_changes(&document_cache, &edit, false);

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
}
