// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;

use i_slint_compiler::parser::{TextRange, TextSize};

use crate::common;

#[derive(Clone, Debug)]
pub struct TextOffsetAdjustment {
    pub start_offset: TextSize,
    pub end_offset: TextSize,
    pub new_text_length: u32,
}

impl TextOffsetAdjustment {
    pub fn new(
        edit: &lsp_types::TextEdit,
        source_file: &i_slint_compiler::diagnostics::SourceFile,
    ) -> Self {
        let new_text_length = edit.new_text.len() as u32;
        let (start_offset, end_offset) = {
            let so = source_file.offset(
                edit.range.start.line as usize + 1,
                edit.range.start.character as usize + 1,
            );
            let eo = source_file
                .offset(edit.range.end.line as usize + 1, edit.range.end.character as usize + 1);
            (
                TextSize::new(std::cmp::min(so, eo) as u32),
                TextSize::new(std::cmp::max(so, eo) as u32),
            )
        };

        Self { start_offset, end_offset, new_text_length }
    }

    pub fn adjust(&self, offset: TextSize) -> TextSize {
        // This is a bit simplistic... Worst case: Some unexpected element gets selected. We can live with that.

        debug_assert!(self.end_offset >= self.start_offset);
        let old_length = self.end_offset - self.start_offset;

        if offset >= self.end_offset {
            offset + TextSize::new(self.new_text_length) - old_length
        } else if offset >= self.start_offset {
            ((u32::from(offset) as i64 + self.new_text_length as i64 - u32::from(old_length) as i64)
                .clamp(
                    u32::from(self.start_offset) as i64,
                    u32::from(
                        self.end_offset
                            .min(self.start_offset + TextSize::new(self.new_text_length)),
                    ) as i64,
                ) as u32)
                .into()
        } else {
            offset
        }
    }
}

#[derive(Clone, Default)]
pub struct TextOffsetAdjustments(Vec<TextOffsetAdjustment>);

impl TextOffsetAdjustments {
    pub fn add_adjustment(&mut self, adjustment: TextOffsetAdjustment) {
        self.0.push(adjustment);
    }

    pub fn adjust(&self, input: TextSize) -> TextSize {
        let input_ = i64::from(u32::from(input));
        let total_adjustment = self
            .0
            .iter()
            .fold(0_i64, |acc, a| acc + i64::from(u32::from(a.adjust(input))) - input_);
        ((input_ + total_adjustment) as u32).into()
    }

    pub fn adjust_range(&self, range: TextRange) -> TextRange {
        TextRange::new(self.adjust(range.start()), self.adjust(range.end()))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Clone)]
enum EditIteratorState<'a> {
    Changes { urls: Vec<&'a lsp_types::Url>, main_index: usize, index: usize },
    DocumentChanges { main_index: usize, index: usize },
    Done,
}

#[derive(Clone)]
pub struct EditIterator<'a> {
    workspace_edit: &'a lsp_types::WorkspaceEdit,
    state: EditIteratorState<'a>,
}

impl<'a> EditIterator<'a> {
    pub fn new(workspace_edit: &'a lsp_types::WorkspaceEdit) -> Self {
        Self {
            workspace_edit,
            state: EditIteratorState::Changes {
                urls: workspace_edit
                    .changes
                    .as_ref()
                    .map(|hm| hm.keys().collect::<Vec<_>>())
                    .unwrap_or_default(),
                main_index: 0,
                index: 0,
            },
        }
    }
}

impl<'a> Iterator for EditIterator<'a> {
    type Item = (lsp_types::OptionalVersionedTextDocumentIdentifier, &'a lsp_types::TextEdit);

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.state {
            EditIteratorState::Changes { urls, main_index, index } => {
                if let Some(changes) = &self.workspace_edit.changes {
                    if let Some(uri) = urls.get(*main_index) {
                        if let Some(edits) = changes.get(uri) {
                            if let Some(edit) = edits.get(*index) {
                                *index += 1;
                                return Some((
                                    lsp_types::OptionalVersionedTextDocumentIdentifier {
                                        uri: (*uri).clone(),
                                        version: None,
                                    },
                                    edit,
                                ));
                            } else {
                                *index = 0;
                                *main_index += 1;
                                return self.next();
                            }
                        }
                    }
                }

                self.state = EditIteratorState::DocumentChanges { main_index: 0, index: 0 };
                self.next()
            }
            EditIteratorState::DocumentChanges { main_index, index } => {
                if let Some(lsp_types::DocumentChanges::Edits(edits)) =
                    &self.workspace_edit.document_changes
                {
                    if let Some(doc_edit) = edits.get(*main_index) {
                        if let Some(edit) = doc_edit.edits.get(*index) {
                            *index += 1;
                            let te = match edit {
                                lsp_types::OneOf::Left(te) => te,
                                lsp_types::OneOf::Right(ate) => &ate.text_edit,
                            };
                            return Some((doc_edit.text_document.clone(), te));
                        } else {
                            *index = 0;
                            *main_index += 1;
                            return self.next();
                        }
                    }
                }

                self.state = EditIteratorState::Done;
                None
            }
            EditIteratorState::Done => None,
        }
    }
}

#[derive(Clone)]
pub struct TextEditor {
    source_file: i_slint_compiler::diagnostics::SourceFile,
    contents: String,
    original_offset_range: (usize, usize),
    adjustments: TextOffsetAdjustments,
}

impl TextEditor {
    pub fn new(source_file: i_slint_compiler::diagnostics::SourceFile) -> crate::Result<Self> {
        let Some(contents) = source_file.source().map(|s| s.to_string()) else {
            return Err(format!("Source file {:?} had no contents set", source_file.path()).into());
        };
        Ok(Self {
            source_file,
            contents,
            original_offset_range: (usize::MAX, 0),
            adjustments: TextOffsetAdjustments::default(),
        })
    }

    pub fn apply(&mut self, text_edit: &lsp_types::TextEdit) -> crate::Result<()> {
        let current_range =
            crate::util::lsp_range_to_text_range(&self.source_file, text_edit.range);
        let adjusted_range = self.adjustments.adjust_range(current_range);

        if self.contents.len() < adjusted_range.end().into() {
            return Err("Text edit range is out of bounds".into());
        }

        // Book keeping:
        self.original_offset_range.0 =
            self.original_offset_range.0.min(current_range.start().into());
        self.original_offset_range.1 = self.original_offset_range.1.max(current_range.end().into());

        let r: std::ops::Range<usize> = adjusted_range.start().into()..adjusted_range.end().into();
        self.contents.replace_range(r, &text_edit.new_text);

        self.adjustments.add_adjustment(TextOffsetAdjustment::new(text_edit, &self.source_file));

        Ok(())
    }

    pub fn finalize(self) -> Option<(String, TextOffsetAdjustments, (usize, usize))> {
        if self.source_file.source() == Some(&self.contents) {
            None
        } else {
            (!self.adjustments.is_empty()).then_some((
                self.contents,
                self.adjustments,
                self.original_offset_range,
            ))
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct EditedText {
    pub url: lsp_types::Url,
    pub contents: String,
}

pub fn apply_workspace_edit(
    document_cache: &common::DocumentCache,
    workspace_edit: &lsp_types::WorkspaceEdit,
) -> common::Result<Vec<EditedText>> {
    let mut processing = HashMap::new();

    for (doc, edit) in EditIterator::new(workspace_edit) {
        // This is ugly but necessary since the constructor might error out:-/
        if !processing.contains_key(&doc.uri) {
            let Some(document) = document_cache.get_document(&doc.uri) else {
                continue;
            };
            let Some(document_node) = &document.node else {
                continue;
            };
            let editor = TextEditor::new(document_node.source_file.clone())?;
            processing.insert(doc.uri.clone(), editor);
        }

        processing.get_mut(&doc.uri).expect("just added if missing").apply(edit)?;
    }

    Ok(processing
        .drain()
        .filter_map(|(url, v)| {
            let edit_result = v.finalize()?;
            Some(EditedText { url, contents: edit_result.0 })
        })
        .collect())
}

/// Given a WorkspaceEdit, return a reversed version of it (that undoes it)
/// Returns none if it cannot be reverted (eg, if it deletes files)
pub fn reversed_edit(
    document_cache: &common::DocumentCache,
    edit: &lsp_types::WorkspaceEdit,
) -> Option<lsp_types::WorkspaceEdit> {
    struct UndoHelper {
        source_file: i_slint_compiler::diagnostics::SourceFile,
        /// (Range in the original, original string, new length)
        edits: Vec<lsp_types::TextEdit>,
    }

    let mut processing = HashMap::new();

    for (doc, edit) in EditIterator::new(edit) {
        if !processing.contains_key(&doc.uri) {
            let Some(document) = document_cache.get_document(&doc.uri) else {
                return None;
            };
            let Some(document_node) = &document.node else {
                return None;
            };
            let helper =
                UndoHelper { source_file: document_node.source_file.clone(), edits: Vec::new() };
            processing.insert(doc.uri.clone(), helper);
        }

        let helper = processing.get_mut(&doc.uri).unwrap();
        helper.edits.push(edit.clone());
    }

    let mut tde = Vec::new();
    for (uri, mut helper) in processing {
        let source = helper.source_file.source()?;
        helper.edits.sort_by_key(|e| e.range.start);
        let mut adjust_line = 0i32;
        let mut current_line = 0;
        let mut adjust_column = 0i32;
        let edits = helper
            .edits
            .into_iter()
            .map(|e| {
                let orig_range = crate::util::lsp_range_to_text_range(&helper.source_file, e.range);
                let orig_string = source[orig_range].to_string();

                // Count the number of \n in the original string and the replaced string
                let orig_lines = orig_string.chars().filter(|c| *c == '\n').count() as i32;
                let new_lines = e.new_text.chars().filter(|c| *c == '\n').count() as i32;

                let adjust_pos = |pos: lsp_types::Position| {
                    let line = pos.line.saturating_add_signed(adjust_line);
                    if pos.line == current_line {
                        lsp_types::Position::new(
                            line,
                            pos.character.saturating_add_signed(adjust_column),
                        )
                    } else {
                        lsp_types::Position::new(line, pos.character)
                    }
                };

                let adjusted_start = adjust_pos(e.range.start);

                let new_range = lsp_types::Range::new(
                    adjusted_start,
                    lsp_types::Position::new(
                        adjusted_start.line.saturating_add_signed(new_lines),
                        if new_lines == 0 {
                            adjusted_start.character + e.new_text.len() as u32
                        } else {
                            e.new_text.bytes().rev().take_while(|b| *b != b'\n').count() as u32
                        },
                    ),
                );

                adjust_line += new_lines - orig_lines;
                current_line = e.range.end.line;
                adjust_column = new_range.end.character as i32 - e.range.end.character as i32;

                lsp_types::OneOf::Left(lsp_types::TextEdit {
                    range: new_range,
                    new_text: orig_string,
                })
            })
            .collect();

        tde.push(lsp_types::TextDocumentEdit {
            text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                uri,
                version: None,
            },
            edits,
        });
    }

    Some(lsp_types::WorkspaceEdit {
        document_changes: Some(lsp_types::DocumentChanges::Edits(tde)),
        ..Default::default()
    })
}

#[test]
fn test_text_offset_adjustments() {
    let mut a = TextOffsetAdjustments::default();
    // same length change
    a.add_adjustment(TextOffsetAdjustment {
        start_offset: 10.into(),
        end_offset: 20.into(),
        new_text_length: 10,
    });
    // insert
    a.add_adjustment(TextOffsetAdjustment {
        start_offset: 25.into(),
        end_offset: 25.into(),
        new_text_length: 1,
    });
    // smaller replacement
    a.add_adjustment(TextOffsetAdjustment {
        start_offset: 30.into(),
        end_offset: 40.into(),
        new_text_length: 5,
    });
    // longer replacement
    a.add_adjustment(TextOffsetAdjustment {
        start_offset: 50.into(),
        end_offset: 60.into(),
        new_text_length: 20,
    });
    // deletion
    a.add_adjustment(TextOffsetAdjustment {
        start_offset: 70.into(),
        end_offset: 80.into(),
        new_text_length: 0,
    });

    assert_eq!(a.adjust(0.into()), 0.into());
    assert_eq!(a.adjust(20.into()), 20.into());
    assert_eq!(a.adjust(25.into()), 26.into());
    assert_eq!(a.adjust(30.into()), 31.into());
    assert_eq!(a.adjust(40.into()), 36.into());
    assert_eq!(a.adjust(60.into()), 66.into());
    assert_eq!(a.adjust(70.into()), 76.into());
    assert_eq!(a.adjust(80.into()), 76.into());
}

#[test]
fn test_text_offset_adjustments_reverse() {
    let mut a = TextOffsetAdjustments::default();
    // deletion
    a.add_adjustment(TextOffsetAdjustment {
        start_offset: 70.into(),
        end_offset: 80.into(),
        new_text_length: 0,
    });
    // longer replacement
    a.add_adjustment(TextOffsetAdjustment {
        start_offset: 50.into(),
        end_offset: 60.into(),
        new_text_length: 20,
    });
    // smaller replacement
    a.add_adjustment(TextOffsetAdjustment {
        start_offset: 30.into(),
        end_offset: 40.into(),
        new_text_length: 5,
    });
    // insert
    a.add_adjustment(TextOffsetAdjustment {
        start_offset: 25.into(),
        end_offset: 25.into(),
        new_text_length: 1,
    });
    // same length change
    a.add_adjustment(TextOffsetAdjustment {
        start_offset: 10.into(),
        end_offset: 20.into(),
        new_text_length: 10,
    });

    assert_eq!(a.adjust(0.into()), 0.into());
    assert_eq!(a.adjust(20.into()), 20.into());
    assert_eq!(a.adjust(25.into()), 26.into());
    assert_eq!(a.adjust(30.into()), 31.into());
    assert_eq!(a.adjust(40.into()), 36.into());
    assert_eq!(a.adjust(60.into()), 66.into());
    assert_eq!(a.adjust(70.into()), 76.into());
    assert_eq!(a.adjust(80.into()), 76.into());
}

#[test]
fn test_edit_iterator_empty() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: None,
        document_changes: None,
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_changes_one_empty() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: Some(std::collections::HashMap::from([(
            lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
            vec![],
        )])),
        document_changes: None,
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_changes_one_one() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: Some(std::collections::HashMap::from([(
            lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
            vec![lsp_types::TextEdit {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(22, 41),
                    lsp_types::Position::new(41, 22),
                ),
                new_text: "Replacement".to_string(),
            }],
        )])),
        document_changes: None,
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
    assert_eq!(r.0.version, None);
    assert_eq!(&r.1.new_text, "Replacement");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(22, 41));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(41, 22));
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_changes_one_two() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: Some(std::collections::HashMap::from([(
            lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
            vec![
                lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(22, 41),
                        lsp_types::Position::new(41, 22),
                    ),
                    new_text: "Replacement".to_string(),
                },
                lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(43, 11),
                        lsp_types::Position::new(43, 12),
                    ),
                    new_text: "Foo".to_string(),
                },
            ],
        )])),
        document_changes: None,
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);

    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
    assert_eq!(r.0.version, None);
    assert_eq!(&r.1.new_text, "Replacement");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(22, 41));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(41, 22));

    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
    assert_eq!(r.0.version, None);
    assert_eq!(&r.1.new_text, "Foo");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(43, 11));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(43, 12));

    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_changes_two() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: Some(std::collections::HashMap::from([
            (
                lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(22, 41),
                        lsp_types::Position::new(41, 22),
                    ),
                    new_text: "Replacement".to_string(),
                }],
            ),
            (
                lsp_types::Url::parse("file://foo/baz.slint").unwrap(),
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(43, 11),
                        lsp_types::Position::new(43, 12),
                    ),
                    new_text: "Foo".to_string(),
                }],
            ),
        ])),
        document_changes: None,
        change_annotations: None,
    };

    let mut seen1 = false;
    let mut seen2 = false;

    for r in EditIterator::new(&workspace_edit) {
        // random order!
        if r.0.uri.to_string() == "file://foo/bar.slint" {
            assert!(!seen1);
            assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
            assert_eq!(r.0.version, None);
            assert_eq!(&r.1.new_text, "Replacement");
            assert_eq!(&r.1.range.start, &lsp_types::Position::new(22, 41));
            assert_eq!(&r.1.range.end, &lsp_types::Position::new(41, 22));
            seen1 = true;
        } else {
            assert!(!seen2);
            assert_eq!(&r.0.uri.to_string(), "file://foo/baz.slint");
            assert_eq!(r.0.version, None);
            assert_eq!(&r.1.new_text, "Foo");
            assert_eq!(&r.1.range.start, &lsp_types::Position::new(43, 11));
            assert_eq!(&r.1.range.end, &lsp_types::Position::new(43, 12));
            seen2 = true;
        }
    }
    assert!(seen1 && seen2);
}

#[test]
fn test_edit_iterator_document_changes_empty() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: None,
        document_changes: Some(lsp_types::DocumentChanges::Edits(vec![])),
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_document_changes_operations() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: None,
        document_changes: Some(lsp_types::DocumentChanges::Operations(vec![])),
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_document_changes_one_empty() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: None,
        document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
            lsp_types::TextDocumentEdit {
                edits: vec![],
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
                    version: Some(99),
                },
            },
        ])),
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_document_changes_one_one() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: None,
        document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
            lsp_types::TextDocumentEdit {
                edits: vec![lsp_types::OneOf::Left(lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(22, 41),
                        lsp_types::Position::new(41, 22),
                    ),
                    new_text: "Replacement".to_string(),
                })],
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
                    version: Some(99),
                },
            },
        ])),
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
    assert_eq!(r.0.version, Some(99));
    assert_eq!(&r.1.new_text, "Replacement");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(22, 41));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(41, 22));
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_document_changes_one_two() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: None,
        document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
            lsp_types::TextDocumentEdit {
                edits: vec![
                    lsp_types::OneOf::Left(lsp_types::TextEdit {
                        range: lsp_types::Range::new(
                            lsp_types::Position::new(22, 41),
                            lsp_types::Position::new(41, 22),
                        ),
                        new_text: "Replacement".to_string(),
                    }),
                    lsp_types::OneOf::Right(lsp_types::AnnotatedTextEdit {
                        text_edit: lsp_types::TextEdit {
                            range: lsp_types::Range::new(
                                lsp_types::Position::new(43, 11),
                                lsp_types::Position::new(43, 12),
                            ),
                            new_text: "Foo".to_string(),
                        },
                        annotation_id: "CID".to_string(),
                    }),
                ],
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
                    version: Some(99),
                },
            },
        ])),
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
    assert_eq!(r.0.version, Some(99));
    assert_eq!(&r.1.new_text, "Replacement");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(22, 41));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(41, 22));

    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
    assert_eq!(r.0.version, Some(99));
    assert_eq!(&r.1.new_text, "Foo");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(43, 11));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(43, 12));
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_document_changes_two() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: None,
        document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
            lsp_types::TextDocumentEdit {
                edits: vec![lsp_types::OneOf::Left(lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(22, 41),
                        lsp_types::Position::new(41, 22),
                    ),
                    new_text: "Replacement".to_string(),
                })],
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
                    version: Some(99),
                },
            },
            lsp_types::TextDocumentEdit {
                edits: vec![lsp_types::OneOf::Right(lsp_types::AnnotatedTextEdit {
                    text_edit: lsp_types::TextEdit {
                        range: lsp_types::Range::new(
                            lsp_types::Position::new(43, 11),
                            lsp_types::Position::new(43, 12),
                        ),
                        new_text: "Foo".to_string(),
                    },
                    annotation_id: "CID".to_string(),
                })],
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file://foo/baz.slint").unwrap(),
                    version: Some(98),
                },
            },
        ])),
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
    assert_eq!(r.0.version, Some(99));
    assert_eq!(&r.1.new_text, "Replacement");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(22, 41));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(41, 22));

    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/baz.slint");
    assert_eq!(r.0.version, Some(98));
    assert_eq!(&r.1.new_text, "Foo");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(43, 11));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(43, 12));
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_document_mixed() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: Some(std::collections::HashMap::from([
            (
                lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(22, 41),
                        lsp_types::Position::new(41, 22),
                    ),
                    new_text: "Replacement".to_string(),
                }],
            ),
            (
                lsp_types::Url::parse("file://foo/baz.slint").unwrap(),
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(43, 11),
                        lsp_types::Position::new(43, 12),
                    ),
                    new_text: "Foo".to_string(),
                }],
            ),
        ])),
        document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
            lsp_types::TextDocumentEdit {
                edits: vec![lsp_types::OneOf::Left(lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(22, 41),
                        lsp_types::Position::new(41, 22),
                    ),
                    new_text: "Doc Replacement".to_string(),
                })],
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file://doc/bar.slint").unwrap(),
                    version: Some(99),
                },
            },
            lsp_types::TextDocumentEdit {
                edits: vec![lsp_types::OneOf::Right(lsp_types::AnnotatedTextEdit {
                    text_edit: lsp_types::TextEdit {
                        range: lsp_types::Range::new(
                            lsp_types::Position::new(43, 11),
                            lsp_types::Position::new(43, 12),
                        ),
                        new_text: "Doc Foo".to_string(),
                    },
                    annotation_id: "CID".to_string(),
                })],
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file://doc/baz.slint").unwrap(),
                    version: Some(98),
                },
            },
        ])),
        change_annotations: None,
    };

    let mut seen = [false; 4];

    for r in EditIterator::new(&workspace_edit) {
        // random order!
        if r.0.uri.to_string() == "file://foo/bar.slint" {
            assert!(!seen[0]);
            assert!(!seen[2]);
            assert!(!seen[3]);
            assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
            assert_eq!(r.0.version, None);
            assert_eq!(&r.1.new_text, "Replacement");
            assert_eq!(&r.1.range.start, &lsp_types::Position::new(22, 41));
            assert_eq!(&r.1.range.end, &lsp_types::Position::new(41, 22));
            seen[0] = true;
        } else if r.0.uri.to_string() == "file://foo/baz.slint" {
            assert!(!seen[1]);
            assert!(!seen[2]);
            assert!(!seen[3]);
            assert_eq!(&r.0.uri.to_string(), "file://foo/baz.slint");
            assert_eq!(r.0.version, None);
            assert_eq!(&r.1.new_text, "Foo");
            assert_eq!(&r.1.range.start, &lsp_types::Position::new(43, 11));
            assert_eq!(&r.1.range.end, &lsp_types::Position::new(43, 12));
            seen[1] = true;
        } else if r.0.uri.to_string() == "file://doc/bar.slint" {
            assert!(seen[0]);
            assert!(seen[1]);
            assert!(!seen[2]);
            assert!(!seen[3]);
            assert_eq!(&r.0.uri.to_string(), "file://doc/bar.slint");
            assert_eq!(r.0.version, Some(99));
            assert_eq!(&r.1.new_text, "Doc Replacement");
            assert_eq!(&r.1.range.start, &lsp_types::Position::new(22, 41));
            assert_eq!(&r.1.range.end, &lsp_types::Position::new(41, 22));
            seen[2] = true;
        } else {
            assert!(seen[0]);
            assert!(seen[1]);
            assert!(seen[2]);
            assert!(!seen[3]);
            assert_eq!(&r.0.uri.to_string(), "file://doc/baz.slint");
            assert_eq!(r.0.version, Some(98));
            assert_eq!(&r.1.new_text, "Doc Foo");
            assert_eq!(&r.1.range.start, &lsp_types::Position::new(43, 11));
            assert_eq!(&r.1.range.end, &lsp_types::Position::new(43, 12));
            seen[3] = true;
        }
    }
}

#[test]
fn test_texteditor_no_content_in_source_file() {
    use i_slint_compiler::diagnostics::SourceFileInner;

    let source_file = SourceFileInner::from_path_only(std::path::PathBuf::from("/tmp/foo.slint"));

    assert!(TextEditor::new(source_file).is_err());
}

#[test]
fn test_texteditor_edit_out_of_range() {
    use i_slint_compiler::diagnostics::SourceFileInner;

    let source_file = std::rc::Rc::new(SourceFileInner::new(
        std::path::PathBuf::from("/tmp/foo.slint"),
        r#""#.to_string(),
    ));

    let mut editor = TextEditor::new(source_file.clone()).unwrap();

    let edit = lsp_types::TextEdit {
        range: lsp_types::Range::new(
            lsp_types::Position::new(1, 2),
            lsp_types::Position::new(1, 3),
        ),
        new_text: "Foobar".to_string(),
    };
    assert!(editor.apply(&edit).is_err());
}

#[test]
fn test_texteditor_delete_everything() {
    use i_slint_compiler::diagnostics::SourceFileInner;

    let source_file = std::rc::Rc::new(SourceFileInner::new(
        std::path::PathBuf::from("/tmp/foo.slint"),
        r#"abc
def
geh"#
            .to_string(),
    ));

    let mut editor = TextEditor::new(source_file.clone()).unwrap();

    let edit = lsp_types::TextEdit {
        range: lsp_types::Range::new(
            lsp_types::Position::new(0, 0),
            lsp_types::Position::new(2, 3),
        ),
        new_text: "".to_string(),
    };
    assert!(editor.apply(&edit).is_ok());

    let result = editor.finalize().unwrap();
    assert!(result.0.is_empty());
    assert_eq!(result.1.adjust(42.into()), 31.into());
    assert_eq!(result.2 .0, 0);
    assert_eq!(result.2 .1, 3 * 3 + 2);
}

#[test]
fn test_texteditor_replace() {
    use i_slint_compiler::diagnostics::SourceFileInner;

    let source_file = std::rc::Rc::new(SourceFileInner::new(
        std::path::PathBuf::from("/tmp/foo.slint"),
        r#"abc
def
geh"#
            .to_string(),
    ));

    let mut editor = TextEditor::new(source_file.clone()).unwrap();

    let edit = lsp_types::TextEdit {
        range: lsp_types::Range::new(
            lsp_types::Position::new(1, 0),
            lsp_types::Position::new(1, 3),
        ),
        new_text: "REPLACEMENT".to_string(),
    };
    assert!(editor.apply(&edit).is_ok());

    let result = editor.finalize().unwrap();
    assert_eq!(
        &result.0,
        r#"abc
REPLACEMENT
geh"#
    );
    assert_eq!(result.1.adjust(42.into()), 50.into());
    assert_eq!(result.2 .0, 3 + 1);
    assert_eq!(result.2 .1, 3 + 1 + 3);
}

#[test]
fn test_texteditor_2step_replace_all() {
    use i_slint_compiler::diagnostics::SourceFileInner;

    let source_file = std::rc::Rc::new(SourceFileInner::new(
        std::path::PathBuf::from("/tmp/foo.slint"),
        r#"abc
def
geh"#
            .to_string(),
    ));

    let mut editor = TextEditor::new(source_file.clone()).unwrap();

    let edit = lsp_types::TextEdit {
        range: lsp_types::Range::new(
            lsp_types::Position::new(0, 0),
            lsp_types::Position::new(2, 3),
        ),
        new_text: "".to_string(),
    };
    assert!(editor.apply(&edit).is_ok());
    let edit = lsp_types::TextEdit {
        range: lsp_types::Range::new(
            lsp_types::Position::new(0, 0),
            lsp_types::Position::new(0, 0),
        ),
        new_text: "REPLACEMENT".to_string(),
    };
    assert!(editor.apply(&edit).is_ok());

    let result = editor.finalize().unwrap();
    assert_eq!(&result.0, "REPLACEMENT");
    assert_eq!(result.1.adjust(42.into()), 42.into());
    assert_eq!(result.2 .0, 0);
    assert_eq!(result.2 .1, 3 * 3 + 2);
}

#[cfg(test)]
mod test_apply_reversed_edit {
    use super::*;

    fn check_apply_and_reversed_edit(
        document_cache: &mut common::DocumentCache,
        edit: &lsp_types::WorkspaceEdit,
    ) {
        let edits = apply_workspace_edit(document_cache, edit).unwrap();
        let reversed = reversed_edit(document_cache, edit).unwrap();

        let snapshot = document_cache.snapshot().unwrap();

        for e in &edits {
            common::poll_once(document_cache.load_url(
                &e.url,
                None,
                e.contents.clone(),
                &mut Default::default(),
            ));
        }

        let reversed_edits = apply_workspace_edit(document_cache, &reversed).unwrap();
        assert_eq!(edits.len(), reversed_edits.len());
        for e in reversed_edits {
            let doc = snapshot.get_document(&e.url).unwrap();
            assert_eq!(doc.node.as_ref().unwrap().source_file.source().unwrap(), e.contents);
        }
        let reversed_twice = reversed_edit(document_cache, &reversed).unwrap();
        assert_eq!(edits, apply_workspace_edit(&snapshot, &reversed_twice).unwrap());
    }

    #[test]
    fn test_multi_line_edit() {
        let url = lsp_types::Url::from_file_path(common::test::main_test_file_name()).unwrap();
        let code = HashMap::from([(url.clone(), "component Foo { /*..*/ }\n".to_string())]);
        let mut document_cache =
            crate::common::test::compile_test_with_sources("fluent", code, true);

        check_apply_and_reversed_edit(
            &mut document_cache,
            &lsp_types::WorkspaceEdit::new(std::collections::HashMap::from([(
                url.clone(),
                vec![
                    lsp_types::TextEdit {
                        range: lsp_types::Range::new(
                            lsp_types::Position::new(0, 0),
                            lsp_types::Position::new(0, 0),
                        ),
                        new_text: "import { AboutSlint } from \"std-widgets.slint\";\n".to_string(),
                    },
                    lsp_types::TextEdit {
                        range: lsp_types::Range::new(
                            lsp_types::Position::new(0, 15),
                            lsp_types::Position::new(0, 23),
                        ),
                        new_text: "\n    AboutSlint {}\n".to_string(),
                    },
                    lsp_types::TextEdit {
                        range: lsp_types::Range::new(
                            lsp_types::Position::new(0, 24),
                            lsp_types::Position::new(0, 24),
                        ),
                        new_text: "\n//comment\n".to_string(),
                    },
                ],
            )])),
        );

        assert_eq!(
            document_cache.get_document(&url).unwrap().node.as_ref().unwrap().source_file.source().unwrap(),
            "import { AboutSlint } from \"std-widgets.slint\";\ncomponent Foo {\n    AboutSlint {}\n}\n//comment\n\n"
        );
    }
}
