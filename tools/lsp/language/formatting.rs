// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::common::DocumentCache;
use crate::util::text_range_to_lsp_range;
use dissimilar::Chunk;
use i_slint_compiler::parser::{TextRange, TextSize};
use lsp_types::{DocumentFormattingParams, TextEdit};

#[cfg(not(target_arch = "wasm32"))]
use i_slint_formatter::Formatter;

#[cfg(not(target_arch = "wasm32"))]
pub fn format_document(
    params: DocumentFormattingParams,
    document_cache: &DocumentCache,
) -> Option<Vec<TextEdit>> {
    let doc = document_cache.get_document(&params.text_document.uri)?;
    let doc = doc.node.as_ref()?;

    let original: String = doc.text().into();
    let formatter = Formatter::new().ok()?;
    let formatted = formatter.format_str(&original).ok()?;
    let diff = dissimilar::diff(&original, &formatted.text);

    let mut pos = TextSize::default();
    let mut last_was_deleted = false;
    let mut edits: Vec<TextEdit> = Vec::new();

    for d in diff {
        match d {
            Chunk::Equal(text) => {
                last_was_deleted = false;
                pos += TextSize::of(text)
            }
            Chunk::Delete(text) => {
                let len = TextSize::of(text);
                let deleted_range = text_range_to_lsp_range(
                    &doc.source_file,
                    TextRange::at(pos, len),
                    document_cache.format,
                );
                edits.push(TextEdit { range: deleted_range, new_text: String::new() });
                last_was_deleted = true;
                pos += len;
            }
            Chunk::Insert(text) => {
                if last_was_deleted {
                    // if last was deleted, then this is a replace
                    edits.last_mut().unwrap().new_text = text.into();
                    last_was_deleted = false;
                    continue;
                }

                let range = TextRange::empty(pos);
                let range = text_range_to_lsp_range(&doc.source_file, range, document_cache.format);
                edits.push(TextEdit { range, new_text: text.into() });
            }
        }
    }
    Some(edits)
}

#[cfg(target_arch = "wasm32")]
pub fn format_document(
    _params: DocumentFormattingParams,
    _document_cache: &DocumentCache,
) -> Option<Vec<TextEdit>> {
    None
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::common::text_edit::TextEditor;
    use i_slint_formatter::Formatter;

    /// Given an unformatted source text, return text edits that will turn the source into formatted text
    fn get_formatting_edits(source: &str) -> Option<Vec<TextEdit>> {
        let (dc, uri, _) = crate::language::test::loaded_document_cache(source.into());
        // we only care about "uri" in params
        let params = lsp_types::DocumentFormattingParams {
            text_document: lsp_types::TextDocumentIdentifier { uri },
            options: lsp_types::FormattingOptions::default(),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        };
        format_document(params, &dc)
    }

    #[test]
    fn test_formatting() {
        let source = "component Bar inherits Text { nope := Rectangle {} property <string> red; }";
        let edits = get_formatting_edits(source).unwrap();

        assert!(!edits.is_empty());

        let (dc, uri, _) = crate::language::test::loaded_document_cache(source.into());
        let source_file = dc
            .get_document(&uri)
            .and_then(|document| document.node.as_ref())
            .unwrap()
            .source_file
            .clone();
        let mut editor = TextEditor::new(source_file).unwrap();
        for edit in &edits {
            editor.apply(edit, dc.format).unwrap();
        }

        let formatter = Formatter::new().unwrap();
        let expected = formatter.format_str(source).unwrap().text;

        assert_eq!(editor.finalize().unwrap().0, expected);
    }
}
