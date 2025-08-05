// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::ui;
use crate::common::{text_edit, PreviewToLspMessage};
use core::hash::{Hash as _, Hasher as _};
use slint::ComponentHandle as _;
use std::collections::HashMap;

type FileHashes = HashMap<lsp_types::Url, u64>;

#[derive(Clone)]
struct EditItem {
    title: String,
    edit: lsp_types::WorkspaceEdit,
    file_hashes: FileHashes,
}

pub fn compute_file_hashes(edits: &[crate::common::text_edit::EditedText]) -> FileHashes {
    edits
        .iter()
        .map(|e| {
            let mut hasher = std::hash::DefaultHasher::new();
            e.contents.hash(&mut hasher);
            let hash = hasher.finish();
            (e.url.clone(), hash)
        })
        .collect()
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

    /*/// Redo the last edit
    pub fn redo(&mut self) -> Option<lsp_types::WorkspaceEdit> {
        let item = self.redo_stack.pop()?;
        let edit = item.edit.clone();
        self.undo_stack.push(item);
        Some(edit)
    }*/

    pub fn push(
        &mut self,
        title: String,
        reverse_edit: Option<lsp_types::WorkspaceEdit>,
        file_hashes: FileHashes,
    ) {
        match reverse_edit {
            Some(edit) => {
                self.undo_stack.push(EditItem { title, edit, file_hashes });
                self.redo_stack.clear();
            }
            None => {
                self.clear();
            }
        }
    }

    /// When we get contents from the editor, we check that it matches the expected state of the top of the undo stack,
    /// otherwise, we reset the stack.
    pub fn check_set_contents_valid(&mut self, url: &lsp_types::Url, content: &str) -> bool {
        let Some(top) = self.undo_stack.last() else {
            return true;
        };
        let ok = top.file_hashes.get(url).map_or(true, |hash| {
            let mut hasher = std::hash::DefaultHasher::new();
            content.hash(&mut hasher);
            *hash == hasher.finish()
        });
        if !ok {
            self.clear();
        }
        ok
    }
}

pub fn setup(ui: &ui::PreviewUi) {
    let api = ui.global::<ui::Api>();
    api.on_undo(|| {
        let Some(document_cache) = super::document_cache() else { return };
        super::PREVIEW_STATE.with_borrow_mut(|state| {
            if state.workspace_edit_sent {
                return;
            }
            let Some(edit) = state.undo_redo_stack.undo_stack.pop() else {
                return;
            };
            if let Some(reverse) = text_edit::reversed_edit(&document_cache, &edit.edit) {
                state.undo_redo_stack.redo_stack.push(EditItem {
                    title: edit.title.clone(),
                    edit: reverse,
                    file_hashes: edit.file_hashes.clone(),
                });
            }
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
            if state.workspace_edit_sent {
                return;
            }
            let Some(edit) = state.undo_redo_stack.redo_stack.pop() else {
                return;
            };
            if let Some(reverse) = text_edit::reversed_edit(&document_cache, &edit.edit) {
                state.undo_redo_stack.undo_stack.push(EditItem {
                    title: edit.title.clone(),
                    edit: reverse,
                    file_hashes: edit.file_hashes.clone(),
                });
            }
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

pub fn set_undo_redo_enabled(state: &super::PreviewState) {
    if let Some(ui) = state.ui.as_ref() {
        let api = ui.global::<ui::Api>();
        api.set_undo_enabled(state.undo_redo_stack.undo_stack.len() > 0);
        api.set_redo_enabled(state.undo_redo_stack.redo_stack.len() > 0);
    }
}
