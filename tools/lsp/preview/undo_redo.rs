// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::ui;
use crate::editor_preview::editing::text_edit;
use core::hash::{Hash as _, Hasher as _};
use i_slint_live_preview::protocol::PreviewToLspMessage;

use std::collections::{HashMap, HashSet};

type FileHashes = HashMap<lsp_types::Url, u64>;

#[derive(Clone)]
struct EditItem {
    title: String,
    edit: lsp_types::WorkspaceEdit,
    file_hashes: FileHashes,
}

pub fn compute_file_hashes(
    edits: &[crate::editor_preview::editing::text_edit::EditedText],
) -> FileHashes {
    edits
        .iter()
        .map(|e| {
            let mut hasher = std::hash::DefaultHasher::new();
            e.contents.hash(&mut hasher);
            (e.url.clone(), hasher.finish())
        })
        .collect()
}

enum HistoryChange {
    Push(Option<EditItem>),
    Undo(Option<EditItem>),
    Redo(Option<EditItem>),
}

struct PendingEdit {
    history_change: Option<HistoryChange>,
    file_hashes: FileHashes,
    applied: bool,
    completion_expected: bool,
    preview_updated: bool,
    waiting_for_contents: HashSet<lsp_types::Url>,
}

#[derive(Default)]
pub struct UndoRedoStack {
    undo_stack: Vec<EditItem>,
    redo_stack: Vec<EditItem>,
    pending: Option<PendingEdit>,
    completion_supported: bool,
}

impl UndoRedoStack {
    fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        if let Some(pending) = &mut self.pending {
            pending.history_change = None;
            pending.waiting_for_contents.clear();
            pending.preview_updated = false;
        }
    }

    fn begin(&mut self, history_change: HistoryChange, file_hashes: FileHashes) -> bool {
        if self.pending.is_some() {
            return false;
        }
        self.pending = Some(PendingEdit {
            history_change: Some(history_change),
            waiting_for_contents: file_hashes.keys().cloned().collect(),
            file_hashes,
            applied: false,
            completion_expected: self.completion_supported,
            preview_updated: false,
        });
        true
    }

    pub fn begin_edit(
        &mut self,
        title: String,
        reverse_edit: Option<lsp_types::WorkspaceEdit>,
        file_hashes: FileHashes,
    ) -> bool {
        let reverse =
            reverse_edit.map(|edit| EditItem { title, edit, file_hashes: file_hashes.clone() });
        self.begin(HistoryChange::Push(reverse), file_hashes)
    }

    pub fn set_completion_supported(&mut self, supported: bool) {
        self.completion_supported = supported;
    }

    pub fn finish(&mut self, applied: bool) {
        let Some(mut pending) = self.pending.take() else { return };
        if !applied {
            if pending.waiting_for_contents.len() < pending.file_hashes.len() {
                self.clear();
            }
            return;
        }
        match pending.history_change.take() {
            Some(HistoryChange::Push(reverse)) => {
                if let Some(reverse) = reverse {
                    self.undo_stack.push(reverse);
                    self.redo_stack.clear();
                } else {
                    self.clear();
                }
            }
            Some(HistoryChange::Undo(reverse)) => {
                self.undo_stack.pop();
                self.redo_stack.extend(reverse);
            }
            Some(HistoryChange::Redo(reverse)) => {
                self.redo_stack.pop();
                self.undo_stack.extend(reverse);
            }
            None => {}
        }
        pending.applied = true;
        if !pending.preview_updated {
            self.pending = Some(pending);
        }
    }

    pub fn preview_updated(&mut self) {
        if let Some(pending) = &mut self.pending
            && pending.waiting_for_contents.is_empty()
        {
            pending.preview_updated = true;
            if pending.applied || !pending.completion_expected {
                self.finish(true);
            }
        }
    }

    /// Reset history when editor contents differ from the expected edit result.
    pub fn check_set_contents_valid(&mut self, url: &lsp_types::Url, content: &str) -> bool {
        let expected = self
            .pending
            .as_ref()
            .and_then(|pending| pending.file_hashes.get(url))
            .or_else(|| self.undo_stack.last().and_then(|top| top.file_hashes.get(url)));
        let ok = match expected {
            Some(hash) => {
                let mut hasher = std::hash::DefaultHasher::new();
                content.hash(&mut hasher);
                *hash == hasher.finish()
            }
            None => !self.redo_stack.iter().any(|item| item.file_hashes.contains_key(url)),
        };
        if !ok {
            self.clear();
        } else if let Some(pending) = &mut self.pending {
            pending.waiting_for_contents.remove(url);
        }
        ok
    }

    fn begin_undo_redo(
        &mut self,
        document_cache: &crate::editor_preview::DocumentCache,
        redo: bool,
    ) -> Option<(String, lsp_types::WorkspaceEdit)> {
        if self.pending.is_some() {
            return None;
        }
        let stack = if redo { &self.redo_stack } else { &self.undo_stack };
        let item = stack.last()?.clone();
        let result = text_edit::apply_workspace_edit(document_cache, &item.edit).ok()?;
        let reverse = text_edit::reversed_edit(document_cache, &item.edit).map(|edit| EditItem {
            title: item.title.clone(),
            edit,
            file_hashes: item.file_hashes,
        });
        self.begin(
            if redo { HistoryChange::Redo(reverse) } else { HistoryChange::Undo(reverse) },
            compute_file_hashes(&result),
        );
        Some((format!("{} \"{}\"", if redo { "Redo" } else { "Undo" }, item.title), item.edit))
    }
}

pub fn send_pending_edit(
    state: &mut super::PreviewState,
    label: String,
    edit: lsp_types::WorkspaceEdit,
) -> bool {
    let result =
        state.to_lsp.borrow().as_ref().unwrap().send(&PreviewToLspMessage::SendWorkspaceEdit {
            label: Some(label),
            edit,
            notify_on_completion: state.config.workspace_edit_completion,
        });
    if let Err(error) = &result {
        tracing::warn!("Failed to send preview edit: {error}");
        state.undo_redo_stack.finish(false);
    }
    set_undo_redo_enabled(state);
    result.is_ok()
}

pub fn setup(api: &ui::Api<'_>) {
    fn undo_redo(redo: bool) {
        let Some(document_cache) = super::document_cache() else { return };
        super::PREVIEW_STATE.with_borrow_mut(|state| {
            if let Some((label, edit)) =
                state.undo_redo_stack.begin_undo_redo(&document_cache, redo)
            {
                send_pending_edit(state, label, edit);
            }
        });
    }
    api.on_undo(|| undo_redo(false));
    api.on_redo(|| undo_redo(true));
}

pub fn set_undo_redo_enabled(state: &super::PreviewState) {
    if let Some(api) = state.api.upgrade() {
        let ready = state.undo_redo_stack.pending.is_none();
        api.set_undo_enabled(ready && !state.undo_redo_stack.undo_stack.is_empty());
        api.set_redo_enabled(ready && !state.undo_redo_stack.redo_stack.is_empty());
    }
}
