// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::ui;
use core::hash::{Hash as _, Hasher as _};
use i_slint_compiler::source_path::SourcePath;
use i_slint_editor_preview::editing::text_edit;
use i_slint_live_preview::protocol::PreviewToLspMessage;

use std::collections::HashMap;

type FileHashes = HashMap<lsp_types::Url, u64>;

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
    source_code: &super::SourceCodeCache,
    item: &EditItem,
) -> Option<(lsp_types::WorkspaceEdit, FileHashes)> {
    for (url, expected) in &item.file_hashes {
        let source_file = super::workspace_edit_source_file(document_cache, source_code, url)?;
        let cached = source_file.source()?;
        let disk = SourcePath::from_url(url).read_to_string().ok()?;
        let cached_hash = content_hash(cached);
        let disk_hash = content_hash(&disk);
        if cached_hash != *expected || disk_hash != *expected {
            tracing::debug!(%url, expected, cached_hash, disk_hash, "Preview: history source mismatch");
            return None;
        }
    }
    let result =
        text_edit::apply_workspace_edit_with_source(&item.edit, document_cache.format, |url| {
            super::workspace_edit_source_file(document_cache, source_code, url)
        })
        .ok()?;
    let reverse = text_edit::reversed_edit_with_source(&item.edit, document_cache.format, |url| {
        super::workspace_edit_source_file(document_cache, source_code, url)
    })?;
    Some((reverse, compute_file_hashes(&result)))
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

    pub fn push(
        &mut self,
        title: String,
        reverse_edit: Option<lsp_types::WorkspaceEdit>,
        file_hashes: FileHashes,
    ) {
        match reverse_edit {
            Some(edit) => {
                self.push_item(EditItem { title, edit, file_hashes });
            }
            None => {
                self.clear();
            }
        }
    }

    pub(super) fn push_item(&mut self, item: EditItem) {
        self.undo_stack.push(item);
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
}

pub fn setup(api: &ui::Api<'_>) {
    api.on_undo(|| {
        let Some(document_cache) = super::document_cache() else { return };
        super::PREVIEW_STATE.with_borrow_mut(|state| {
            if edit_pending(state) {
                state.pending_history.push_back(false);
                return;
            }
            let Some(edit) = state.undo_redo_stack.undo_stack.pop() else {
                tracing::debug!("Preview: Undo requested without history");
                return;
            };
            let Some((reverse, file_hashes)) =
                prepare_history_edit(&document_cache, &state.source_code, &edit)
            else {
                state.undo_redo_stack.clear();
                set_undo_redo_enabled(state);
                return;
            };
            state.undo_redo_stack.redo_stack.push(EditItem {
                title: edit.title.clone(),
                edit: reverse,
                file_hashes,
            });
            state
                .to_lsp
                .borrow()
                .as_ref()
                .unwrap()
                .send(&PreviewToLspMessage::SendWorkspaceEdit {
                    label: Some(format!("Undo \"{}\"", edit.title)),
                    edit: edit.edit,
                })
                .unwrap();
            set_undo_redo_enabled(state);
            state.workspace_edit_sent = true;
        })
    });
    api.on_redo(|| {
        let Some(document_cache) = super::document_cache() else { return };
        super::PREVIEW_STATE.with_borrow_mut(|state| {
            if edit_pending(state) {
                state.pending_history.push_back(true);
                return;
            }
            let Some(edit) = state.undo_redo_stack.redo_stack.pop() else {
                tracing::debug!("Preview: Redo requested without history");
                return;
            };
            let Some((reverse, file_hashes)) =
                prepare_history_edit(&document_cache, &state.source_code, &edit)
            else {
                state.undo_redo_stack.clear();
                set_undo_redo_enabled(state);
                return;
            };
            state.undo_redo_stack.undo_stack.push(EditItem {
                title: edit.title.clone(),
                edit: reverse,
                file_hashes,
            });
            state
                .to_lsp
                .borrow()
                .as_ref()
                .unwrap()
                .send(&PreviewToLspMessage::SendWorkspaceEdit {
                    label: Some(format!("Redo \"{}\"", edit.title)),
                    edit: edit.edit,
                })
                .unwrap();
            set_undo_redo_enabled(state);
            state.workspace_edit_sent = true;
        })
    });
}

pub(super) fn edit_pending(state: &super::PreviewState) -> bool {
    state.workspace_edit_sent || state.fill_refresh.is_some()
}

pub(super) fn apply_pending() {
    loop {
        let next = super::PREVIEW_STATE.with_borrow_mut(|state| {
            if edit_pending(state) {
                return None;
            }
            Some((state.api.upgrade()?, state.pending_history.pop_front()?))
        });
        let Some((api, redo)) = next else { return };
        if redo {
            api.invoke_redo();
        } else {
            api.invoke_undo();
        }
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

    #[test]
    fn unchanged_source_notifications_preserve_pending_history() {
        use crate::preview::{PREVIEW_STATE, PreviewState, SourceCodeCacheEntry};
        use i_slint_live_preview::protocol::VersionedUrl;

        let url = lsp_types::Url::parse("file:///rectangle.slint").unwrap();
        for undo in [true, false] {
            PREVIEW_STATE.with_borrow_mut(|state| {
                *state = PreviewState::default();
                state.source_code.insert(
                    url.clone(),
                    SourceCodeCacheEntry { version: None, code: "original".into() },
                );
                state.workspace_edit_sent = true;
                let stack = &mut state.undo_redo_stack;
                if undo {
                    stack.undo_stack.push(item(&url, "edited"));
                } else {
                    stack.redo_stack.push(item(&url, "edited"));
                }
            });
            crate::preview::set_contents(&VersionedUrl::new(url.clone(), None), "original".into());
            PREVIEW_STATE.with_borrow(|state| {
                let stack = &state.undo_redo_stack;
                assert_eq!(stack.undo_stack.len() + stack.redo_stack.len(), 1);
            });
            crate::preview::set_contents(&VersionedUrl::new(url.clone(), None), "external".into());
            PREVIEW_STATE.with_borrow(|state| {
                let stack = &state.undo_redo_stack;
                assert!(stack.undo_stack.is_empty());
                assert!(stack.redo_stack.is_empty());
            });
        }
    }
}
