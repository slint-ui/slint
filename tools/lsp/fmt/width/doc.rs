// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The choice document the layout search runs on: text runs, newlines, and
//! open either/or choices, in an arena so sub-documents can be shared and
//! the resolver can memoize on their ids.

// Everything in this file is exercised by the width tests until the search is
// wired into the pipeline.
#![allow(dead_code)]

use super::{GroupId, Variant};

/// Index of a [`Doc`] in its [`DocumentArena`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocId(u32);

/// One node of the choice document.
///
/// Docs carry only what the search needs — widths and indents, no text or
/// token references: the winning decisions are replayed through the engine's
/// ordinary gap resolution, so the search never renders anything.
#[derive(Debug, PartialEq, Eq)]
pub enum Doc {
    /// A single-line run of characters — never contains a newline. Multi-line
    /// verbatim content must be split with [`DocumentArena::verbatim`] so
    /// that widths are counted per line.
    Text {
        width: u32,
    },
    /// A line break followed by its indentation. The indent is baked in at
    /// build time (indent atoms are never conditional), so the search needs
    /// no indent bookkeeping of its own.
    Newline {
        indent_width: u32,
    },
    Concat(Vec<DocId>),
    /// One group's open choice. Built only when both layouts are possible —
    /// a group whose body contains a fixed newline never gets a Choice, its
    /// multiline body is emitted directly.
    Choice {
        group: GroupId,
        single_line: DocId,
        multiline: DocId,
        /// Which variant deviates from the author's input layout (and pays
        /// the deviation penalty).
        penalized: Variant,
    },
}

/// Owns every [`Doc`] of one format run. Sharing sub-documents by id is what
/// keeps the document (and the resolver's memo table) linear in the input:
/// each group's two bodies are built once, however many enclosing flat
/// bodies reference them.
#[derive(Default)]
pub struct DocumentArena {
    docs: Vec<Doc>,
}

impl DocumentArena {
    pub fn alloc(&mut self, doc: Doc) -> DocId {
        self.docs.push(doc);
        DocId(u32::try_from(self.docs.len() - 1).expect("more than u32::MAX docs"))
    }

    pub fn get(&self, id: DocId) -> &Doc {
        &self.docs[id.0 as usize]
    }

    /// The docs for a piece of verbatim text (a token, a comment): its lines
    /// as [`Doc::Text`] with fixed newlines between them. Splitting is what
    /// makes a multi-line block comment count width per line — one `Text`
    /// with the total length would compute garbage columns. The verbatim
    /// newline carries no indentation of its own: the next line's leading
    /// whitespace is part of that line's text.
    pub fn verbatim(&mut self, text: &str) -> Vec<DocId> {
        let mut docs = Vec::new();
        for (index, line) in text.split('\n').enumerate() {
            if index > 0 {
                docs.push(self.alloc(Doc::Newline { indent_width: 0 }));
            }
            docs.push(self.alloc(Doc::Text { width: line_width(line) }));
        }
        docs
    }
}

/// The width of one line in characters. A trailing `\r` (from a `\r\n` line
/// ending) is invisible and must not count.
pub fn line_width(line: &str) -> u32 {
    line.strip_suffix('\r').unwrap_or(line).chars().count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_splits_lines_and_ignores_carriage_returns() {
        let mut arena = DocumentArena::default();
        let docs = arena.verbatim("/* a\r\n   bé */");
        let nodes: Vec<&Doc> = docs.iter().map(|id| arena.get(*id)).collect();
        assert_eq!(
            nodes,
            [
                &Doc::Text { width: 4 },
                &Doc::Newline { indent_width: 0 },
                // 8 characters, not 9 bytes: `é` is two bytes but one column.
                &Doc::Text { width: 8 },
            ]
        );
    }

    #[test]
    fn verbatim_of_single_line_text_is_one_text_doc() {
        let mut arena = DocumentArena::default();
        let docs = arena.verbatim("width");
        assert_eq!(docs.len(), 1);
        assert_eq!(arena.get(docs[0]), &Doc::Text { width: 5 });
    }
}
