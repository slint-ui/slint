// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::editor_preview::DocumentCache;
use crate::fmt::{fmt, writer};
use crate::util::text_range_to_lsp_range;
use dissimilar::Chunk;
use i_slint_compiler::parser::{SyntaxToken, TextRange, TextSize};
use lsp_types::{DocumentFormattingParams, TextEdit};

struct StringWriter {
    text: String,
}

impl writer::TokenWriter for StringWriter {
    fn no_change(&mut self, token: SyntaxToken) -> std::io::Result<()> {
        self.text += token.text();
        Ok(())
    }

    fn with_new_content(&mut self, _token: SyntaxToken, contents: &str) -> std::io::Result<()> {
        self.text += contents;
        Ok(())
    }

    fn insert_before(&mut self, token: SyntaxToken, contents: &str) -> std::io::Result<()> {
        self.text += contents;
        self.text += token.text();
        Ok(())
    }

    fn insert_content(&mut self, contents: &str) -> std::io::Result<()> {
        self.text += contents;
        Ok(())
    }
}

pub fn format_document(
    params: DocumentFormattingParams,
    document_cache: &DocumentCache,
) -> Option<Vec<TextEdit>> {
    let doc = document_cache.get_document(&params.text_document.uri)?;
    let doc = doc.node.as_ref()?;

    let indentation = if params.options.insert_spaces { fmt::DEFAULT_INDENTATION } else { "\t" };
    let mut writer = StringWriter { text: String::new() };
    fmt::format_document(doc.clone(), indentation, &mut writer).ok()?;

    let original: String = doc.text().into();
    let diff = dissimilar::diff(&original, &writer.text);

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

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{Position, Range};

    /// Given an unformatted source text, return text edits that will turn the source into formatted text
    fn get_formatting_edits(source: &str) -> Option<Vec<TextEdit>> {
        get_formatting_edits_with_options(
            source,
            lsp_types::FormattingOptions { tab_size: 4, insert_spaces: true, ..Default::default() },
        )
    }

    fn get_formatting_edits_with_options(
        source: &str,
        options: lsp_types::FormattingOptions,
    ) -> Option<Vec<TextEdit>> {
        let (dc, uri, _) = crate::language::test::loaded_document_cache(source.into());
        let params = lsp_types::DocumentFormattingParams {
            text_document: lsp_types::TextDocumentIdentifier { uri },
            options,
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        };
        format_document(params, &dc)
    }

    #[test]
    fn test_formatting() {
        let edits = get_formatting_edits(
            "component Bar inherits Text { nope := Rectangle {} property <string> red; }",
        )
        .unwrap();

        macro_rules! text_edit {
            ($start_line:literal, $start_col:literal, $end_line:literal, $end_col:literal, $text:literal) => {
                TextEdit {
                    range: Range {
                        start: Position { line: $start_line, character: $start_col },
                        end: Position { line: $end_line, character: $end_col },
                    },
                    new_text: $text.into(),
                }
            };
        }

        let expected = [
            text_edit!(0, 29, 0, 29, "\n   "),
            text_edit!(0, 49, 0, 50, " }\n\n   "),
            text_edit!(0, 73, 0, 75, "\n}\n"),
        ];

        assert_eq!(edits.len(), expected.len());
        for (actual, expected) in edits.iter().zip(expected.iter()) {
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn test_formatting_with_tabs() {
        let source = "component Bar { Rectangle { width: 1px; } }";
        let edits = get_formatting_edits_with_options(
            source,
            lsp_types::FormattingOptions {
                tab_size: 4,
                insert_spaces: false,
                ..Default::default()
            },
        )
        .unwrap();

        let mut formatted = source.to_string();
        for edit in edits.iter().rev() {
            assert_eq!(edit.range.start.line, 0);
            assert_eq!(edit.range.end.line, 0);
            let range = edit.range.start.character as usize..edit.range.end.character as usize;
            formatted.replace_range(range, &edit.new_text);
        }
        assert_eq!(formatted, "component Bar {\n\tRectangle {\n\t\twidth: 1px;\n\t}\n}\n");
    }
}
