// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Creation of `import { XXX } from "foo.slint";` text edits

use crate::util::text_size_to_lsp_position;

use i_slint_compiler::diagnostics::Spanned;
use i_slint_compiler::parser::{SyntaxKind, syntax_nodes};
use lsp_types::{Position, Range, TextEdit};

use std::collections::HashMap;

/// Find the insert location for new imports in the `document`
///
/// The result is a tuple with the first element pointing to the place new import statements should
/// get added. The second element in the tuple is a HashMap mapping import file names to the
/// correct location to enter more components into the existing import statement.
pub fn find_import_locations(
    document: &syntax_nodes::Document,
    format: crate::ByteFormat,
) -> (Position, HashMap<String, Position>) {
    let mut import_locations = HashMap::new();
    let mut last = 0u32;
    for import in document.ImportSpecifier() {
        if let Some((loc, file)) = import.ImportIdentifierList().and_then(|list| {
            let node = list.ImportIdentifier().last()?;
            let id = crate::util::last_non_ws_token(&node).or_else(|| node.first_token())?;
            Some((
                text_size_to_lsp_position(id.source_file()?, id.text_range().end(), format),
                import.child_token(SyntaxKind::StringLiteral)?,
            ))
        }) {
            import_locations.insert(file.text().to_string().trim_matches('\"').to_string(), loc);
        }
        last = import.text_range().end().into();
    }

    let new_import_position = if last == 0 {
        // There are currently no input statement, place it at the location of the first non-empty token.
        // This should also work in the slint! macro.
        // consider this file:  We want to insert before the doc1 position
        // ```
        // //not doc (eg, license header)
        //
        // //doc1
        // //doc2
        // component Foo {
        // ```
        let mut offset = None;
        for it in document.children_with_tokens() {
            match it.kind() {
                SyntaxKind::Comment => {
                    if offset.is_none() {
                        offset = Some(it.text_range().start());
                    }
                }
                SyntaxKind::Whitespace => {
                    // Single newline is just considered part of the comment
                    // but more new lines means it splits that comment
                    if it.as_token().unwrap().text() != "\n" {
                        offset = None;
                    }
                }
                _ => {
                    if offset.is_none() {
                        offset = Some(it.text_range().start());
                    }
                    break;
                }
            }
        }
        text_size_to_lsp_position(&document.source_file, offset.unwrap_or_default(), format)
    } else {
        Position::new(
            text_size_to_lsp_position(&document.source_file, last.into(), format).line + 1,
            0,
        )
    };

    (new_import_position, import_locations)
}

pub fn create_import_edit_impl(
    component: &str,
    import_path: &str,
    missing_import_location: &Position,
    known_import_locations: &HashMap<String, Position>,
) -> TextEdit {
    known_import_locations.get(import_path).map_or_else(
        || {
            TextEdit::new(
                Range::new(*missing_import_location, *missing_import_location),
                format!("import {{ {component} }} from \"{import_path}\";\n"),
            )
        },
        |pos| TextEdit::new(Range::new(*pos, *pos), format!(", {component}")),
    )
}

/// Creates a text edit
#[cfg(feature = "preview-engine")]
pub fn create_import_edit(
    document: &i_slint_compiler::object_tree::Document,
    component: &str,
    import_path: &Option<String>,
    format: crate::ByteFormat,
) -> Option<TextEdit> {
    let import_path = import_path.as_ref()?;
    let doc_node = document.node.as_ref().unwrap();

    if document.local_registry.lookup_element(component).is_ok() {
        None // already known, no import needed
    } else {
        let (missing_import_location, known_import_locations) =
            find_import_locations(doc_node, format);

        Some(create_import_edit_impl(
            component,
            import_path,
            &missing_import_location,
            &known_import_locations,
        ))
    }
}
