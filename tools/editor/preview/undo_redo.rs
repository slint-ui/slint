// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::ui;
use core::hash::{Hash as _, Hasher as _};
use i_slint_compiler::source_path::SourcePath;
use i_slint_editor_preview::editing::text_edit;

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
    item: &EditItem,
) -> Option<(lsp_types::WorkspaceEdit, FileHashes)> {
    for (url, expected) in &item.file_hashes {
        let document = document_cache.get_document(url)?;
        let cached = document.node.as_ref()?.source_file.source()?;
        let disk = SourcePath::from_url(url).read_to_string().ok()?;
        if content_hash(cached) != *expected || content_hash(&disk) != *expected {
            return None;
        }
    }
    let result = text_edit::apply_workspace_edit(document_cache, &item.edit).ok()?;
    let reverse = text_edit::reversed_edit(document_cache, &item.edit)?;
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
    api.on_undo(|| apply_history(false));
    api.on_redo(|| apply_history(true));
}

fn apply_history(redo: bool) {
    let Some(document_cache) = super::document_cache() else { return };
    super::PREVIEW_STATE.with_borrow_mut(|state| {
        if !state.preview_editable() {
            return;
        }
        if edit_pending(state) {
            state.pending_history.push_back(redo);
            return;
        }
        let stack = if redo {
            &state.undo_redo_stack.redo_stack
        } else {
            &state.undo_redo_stack.undo_stack
        };
        let Some(edit) = stack.last() else { return };
        let Some((reverse, file_hashes)) = prepare_history_edit(&document_cache, edit) else {
            state.undo_redo_stack.clear();
            set_undo_redo_enabled(state);
            return;
        };
        let action = if redo { "Redo" } else { "Undo" };
        let title = edit.title.clone();
        let edit = edit.edit.clone();
        if !state.dispatch_workspace_edit(format!("{action} \"{title}\""), edit) {
            return;
        }
        let reverse = EditItem { title, edit: reverse, file_hashes };
        if redo {
            state.undo_redo_stack.redo_stack.pop();
            state.undo_redo_stack.undo_stack.push(reverse);
        } else {
            state.undo_redo_stack.undo_stack.pop();
            state.undo_redo_stack.redo_stack.push(reverse);
        }
        set_undo_redo_enabled(state);
    });
}

pub(super) fn edit_pending(state: &super::PreviewState) -> bool {
    state.workspace_edit_sent || state.fill_refresh.is_some()
}

pub(super) fn apply_pending() {
    loop {
        let next = super::PREVIEW_STATE.with_borrow_mut(|state| {
            if !state.preview_editable() || edit_pending(state) {
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
        api.set_undo_enabled(
            state.preview_editable() && !state.undo_redo_stack.undo_stack.is_empty(),
        );
        api.set_redo_enabled(
            state.preview_editable() && !state.undo_redo_stack.redo_stack.is_empty(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_refreshes_both_history_controls() {
        use slint::ComponentHandle;
        i_slint_backend_testing::init_no_event_loop();
        let window = ui::create_ui().unwrap();
        let api = window.global::<ui::Api>();
        let global = window.global::<ui::Diagnostics>();
        let mut state = crate::preview::PreviewState {
            api: <ui::Api as slint::Global<'_, ui::EditorUi>>::as_weak(&api),
            editor_ui: Some(window.clone_strong()),
            ..Default::default()
        };
        state.undo_redo_stack.undo_stack.push(EditItem {
            title: "Undo".into(),
            edit: Default::default(),
            file_hashes: Default::default(),
        });
        state.undo_redo_stack.redo_stack.push(EditItem {
            title: "Redo".into(),
            edit: Default::default(),
            file_hashes: Default::default(),
        });
        for availability in [
            ui::PreviewAvailability::Current,
            ui::PreviewAvailability::Stale,
            ui::PreviewAvailability::Current,
            ui::PreviewAvailability::Unavailable,
        ] {
            state.set_preview_availability(availability);
            let editable = availability == ui::PreviewAvailability::Current;
            assert_eq!(state.preview_editable(), editable);
            assert_eq!(global.get_preview_editable(), editable);
            assert_eq!(api.get_undo_enabled(), editable);
            assert_eq!(api.get_redo_enabled(), editable);
        }
    }

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
