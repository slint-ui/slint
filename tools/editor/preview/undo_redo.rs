// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::ui;
use core::hash::{Hash as _, Hasher as _};
use i_slint_editor_preview::editing::text_edit;
use std::collections::HashMap;

type FileHashes = HashMap<lsp_types::Url, u64>;

#[derive(Clone)]
pub(super) struct EditItem {
    pub(super) title: String,
    pub(super) edit: lsp_types::WorkspaceEdit,
    pub(super) file_hashes: FileHashes,
}

pub fn compute_file_hashes(
    edits: &[i_slint_editor_preview::editing::text_edit::EditedText],
) -> FileHashes {
    edits.iter().map(|e| (e.url.clone(), content_hash(&e.contents))).collect()
}

fn content_hash(content: &str) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

fn prepare_history_edit(
    document_cache: &i_slint_editor_preview::DocumentCache,
    item: &EditItem,
) -> Option<(lsp_types::WorkspaceEdit, FileHashes, Vec<text_edit::EditedText>)> {
    for (url, expected) in &item.file_hashes {
        let document = document_cache.get_document(url)?;
        let cached = document.node.as_ref()?.source_file.source()?;
        let disk = std::fs::read_to_string(url.to_file_path().ok()?).ok()?;
        if content_hash(cached) != *expected || content_hash(&disk) != *expected {
            return None;
        }
    }
    let result = text_edit::apply_workspace_edit(document_cache, &item.edit).ok()?;
    let reverse = text_edit::reversed_edit(document_cache, &item.edit)?;
    Some((reverse, compute_file_hashes(&result), result))
}

#[derive(Default)]
pub struct UndoRedoStack {
    undo_stack: Vec<EditItem>,
    redo_stack: Vec<EditItem>,
}

impl UndoRedoStack {
    /// Clear the undo/redo stack
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn check_set_contents_valid(&mut self, url: &lsp_types::Url, content: &str) -> bool {
        let expected = self
            .undo_stack
            .iter()
            .rev()
            .chain(self.redo_stack.iter().rev())
            .find_map(|item| item.file_hashes.get(url));
        let ok = expected.is_none_or(|hash| *hash == content_hash(content));
        if !ok {
            self.clear();
        }
        ok
    }

    fn commit(&mut self, pending: PendingEdit) {
        match pending {
            PendingEdit::New(item) => {
                if let Some(item) = item {
                    self.undo_stack.push(item);
                    self.redo_stack.clear();
                } else {
                    self.clear();
                }
            }
            PendingEdit::Undo { replacement } => {
                if self.undo_stack.pop().is_some() {
                    self.redo_stack.push(replacement);
                } else {
                    self.clear();
                }
            }
            PendingEdit::Redo { replacement } => {
                if self.redo_stack.pop().is_some() {
                    self.undo_stack.push(replacement);
                } else {
                    self.clear();
                }
            }
        }
    }
}

/// The history mutation associated with the one workspace edit currently in
/// flight. The stacks stay unchanged until the editor confirms the filesystem
/// write, so a rejected edit cannot strand an undo or redo entry.
pub(super) enum PendingEdit {
    New(Option<EditItem>),
    Undo { replacement: EditItem },
    Redo { replacement: EditItem },
}

pub(super) struct PendingWorkspaceEdit {
    pub(super) id: u64,
    pub(super) history: PendingEdit,
}

pub(super) struct PendingHistory {
    redo: bool,
    #[cfg(feature = "system-testing")]
    work: super::test_sync::Work,
}
impl PendingHistory {
    fn new(redo: bool) -> Self {
        Self {
            redo,
            #[cfg(feature = "system-testing")]
            work: super::test_sync::Work::capture("queued history"),
        }
    }
}

pub fn setup(api: &ui::Api<'_>) {
    api.on_undo(|| {
        let Some(document_cache) = super::document_cache() else { return };
        super::PREVIEW_STATE.with_borrow_mut(|state| {
            if state.workspace_edit_sent {
                state.pending_history.push_back(PendingHistory::new(false));
                return;
            }
            let Some(edit) = state.undo_redo_stack.undo_stack.last().cloned() else {
                return;
            };
            let Some((reverse, file_hashes, expected)) =
                prepare_history_edit(&document_cache, &edit)
            else {
                #[cfg(feature = "system-testing")]
                super::test_sync::effect("rejected");
                state.undo_redo_stack.clear();
                set_undo_redo_enabled(state);
                return;
            };
            let replacement = EditItem { title: edit.title.clone(), edit: reverse, file_hashes };
            super::dispatch_workspace_edit(
                state,
                format!("Undo \"{}\"", edit.title),
                edit.edit.clone(),
                PendingEdit::Undo { replacement },
                expected.into_iter().map(|e| (e.url, e.contents)).collect(),
            );
        })
    });
    api.on_redo(|| {
        let Some(document_cache) = super::document_cache() else { return };
        super::PREVIEW_STATE.with_borrow_mut(|state| {
            if state.workspace_edit_sent {
                state.pending_history.push_back(PendingHistory::new(true));
                return;
            }
            let Some(edit) = state.undo_redo_stack.redo_stack.last().cloned() else {
                return;
            };
            let Some((reverse, file_hashes, expected)) =
                prepare_history_edit(&document_cache, &edit)
            else {
                #[cfg(feature = "system-testing")]
                super::test_sync::effect("rejected");
                state.undo_redo_stack.clear();
                set_undo_redo_enabled(state);
                return;
            };
            let replacement = EditItem { title: edit.title.clone(), edit: reverse, file_hashes };
            super::dispatch_workspace_edit(
                state,
                format!("Redo \"{}\"", edit.title),
                edit.edit.clone(),
                PendingEdit::Redo { replacement },
                expected.into_iter().map(|e| (e.url, e.contents)).collect(),
            );
        })
    });
}

pub(super) fn commit_pending(state: &mut super::PreviewState) {
    if let Some(pending) = state.pending_workspace_edit.take() {
        state.undo_redo_stack.commit(pending.history);
    }
    set_undo_redo_enabled(state);
}

pub(super) fn discard_pending(state: &mut super::PreviewState, partial_write: bool) {
    state.pending_workspace_edit.take();
    if partial_write {
        // Some files reached disk and others did not. The source and the
        // cached document can no longer identify a reversible history state.
        state.undo_redo_stack.clear();
    }
    set_undo_redo_enabled(state);
}

pub(super) fn cancel_pending(state: &mut super::PreviewState) {
    for _history in state.pending_history.drain(..) {
        #[cfg(feature = "system-testing")]
        _history.work.run(|| super::test_sync::effect("canceled"));
    }
}

pub(super) fn apply_pending() {
    loop {
        let next = super::PREVIEW_STATE.with_borrow_mut(|state| {
            if state.workspace_edit_sent {
                return None;
            }
            Some((state.api.upgrade()?, state.pending_history.pop_front()?))
        });
        let Some((api, redo)) = next else { return };
        let invoke = || {
            if redo.redo {
                api.invoke_redo();
            } else {
                api.invoke_undo();
            }
        };
        #[cfg(feature = "system-testing")]
        redo.work.run(invoke);
        #[cfg(not(feature = "system-testing"))]
        invoke();
    }
}

pub fn set_undo_redo_enabled(state: &super::PreviewState) {
    if let Some(api) = state.api.upgrade() {
        api.set_undo_enabled(!state.undo_redo_stack.undo_stack.is_empty());
        api.set_redo_enabled(!state.undo_redo_stack.redo_stack.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(url: &lsp_types::Url, content: &str) -> EditItem {
        EditItem {
            title: "Edit".into(),
            edit: Default::default(),
            file_hashes: HashMap::from([(url.clone(), content_hash(content))]),
        }
    }

    #[test]
    fn external_change_invalidates_redo_without_undo() {
        let url = lsp_types::Url::parse("file:///rectangle.slint").unwrap();
        let mut stack = UndoRedoStack::default();
        stack.redo_stack.push(item(&url, "x: 80px;"));
        assert!(stack.check_set_contents_valid(&url, "x: 80px;"));
        assert!(!stack.check_set_contents_valid(&url, "x: 900px;"));
        assert!(stack.undo_stack.is_empty());
        assert!(stack.redo_stack.is_empty());
    }

    #[test]
    fn validates_each_file_in_history() {
        let first = lsp_types::Url::parse("file:///first.slint").unwrap();
        let second = lsp_types::Url::parse("file:///second.slint").unwrap();
        let mut stack = UndoRedoStack::default();
        stack.undo_stack.push(item(&first, "first edit"));
        stack.undo_stack.push(item(&second, "second edit"));
        assert!(stack.check_set_contents_valid(&first, "first edit"));
        assert!(!stack.check_set_contents_valid(&first, "external edit"));
        assert!(stack.undo_stack.is_empty());
    }

    #[test]
    fn validates_nearest_redo_state_and_ignores_unrelated_files() {
        let url = lsp_types::Url::parse("file:///rectangle.slint").unwrap();
        let unrelated = lsp_types::Url::parse("file:///unrelated.slint").unwrap();
        let mut stack = UndoRedoStack::default();
        stack.redo_stack.push(item(&url, "x: 104px;"));
        stack.redo_stack.push(item(&url, "x: 80px;"));
        assert!(stack.check_set_contents_valid(&url, "x: 80px;"));
        assert!(stack.check_set_contents_valid(&unrelated, "external edit"));
        assert_eq!(stack.redo_stack.len(), 2);
    }
}
