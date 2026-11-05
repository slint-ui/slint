// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::ui;
use core::hash::{Hash, Hasher as _};
use i_slint_compiler::source_path::SourcePath;
use i_slint_editor_preview::editing::text_edit;
use i_slint_live_preview::protocol::PreviewToLspMessage;
use slint::ComponentHandle;

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

fn content_hash(content: impl Hash) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

pub(super) struct PreviewRecovery {
    component: super::PreviewComponent,
    config: super::PreviewConfig,
    sources: FileHashes,
    resources: FileHashes,
    reverse_edit: lsp_types::WorkspaceEdit,
    failed: bool,
}

impl PreviewRecovery {
    fn matches(&self, state: &super::PreviewState) -> bool {
        state.current_previewed_component.as_ref() == Some(&self.component)
            && state.config == self.config
            && state.undo_redo_stack.undo_stack.last().is_some_and(|item| {
                item.edit == self.reverse_edit
                    && item
                        .file_hashes
                        .iter()
                        .all(|(url, hash)| self.sources.get(url) == Some(hash))
            })
            && state.dependencies.iter().all(|url| self.sources.contains_key(url))
            && self.sources.iter().all(|(url, hash)| {
                state.source_code.get(url).is_some_and(|source| content_hash(&source.code) == *hash)
                    && SourcePath::from_url(url)
                        .read_to_string()
                        .is_ok_and(|source| content_hash(&source) == *hash)
            })
            && self.resources.iter().all(|(url, hash)| resource_hash(url) == Some(*hash))
    }
}

fn resource_hash(url: &lsp_types::Url) -> Option<u64> {
    std::fs::read(url.to_file_path().ok()?).ok().map(content_hash)
}

pub(super) fn recovery_candidate(
    state: &super::PreviewState,
    reverse_edit: Option<&lsp_types::WorkspaceEdit>,
    file_hashes: &FileHashes,
) -> Option<PreviewRecovery> {
    let editor = state.editor_ui.as_ref()?;
    if editor.global::<ui::Diagnostics>().get_preview_availability()
        != ui::PreviewAvailability::Current
        || state.loading_state != super::PreviewFutureState::Pending
        || edit_pending(state)
    {
        return None;
    }
    let mut sources = state
        .dependencies
        .iter()
        .chain(file_hashes.keys())
        .map(|url| {
            let source = state.source_code.get(url)?;
            let disk = SourcePath::from_url(url).read_to_string().ok()?;
            (source.code == disk).then(|| (url.clone(), content_hash(&source.code)))
        })
        .collect::<Option<FileHashes>>()?;
    if file_hashes.iter().all(|(url, hash)| sources.get(url) == Some(hash)) {
        return None;
    }
    sources.extend(file_hashes.clone());
    Some(PreviewRecovery {
        component: state.current_component()?,
        config: state.config.clone(),
        sources,
        resources: state
            .resources
            .iter()
            .filter(|url| url.scheme() == "file")
            .map(|url| Some((url.clone(), resource_hash(url)?)))
            .collect::<Option<FileHashes>>()?,
        reverse_edit: reverse_edit?.clone(),
        failed: false,
    })
}

pub(super) fn compilation_finished(state: &mut super::PreviewState, failed: bool) {
    if state.loading_state == super::PreviewFutureState::NeedsReload {
        return;
    }
    let recovery =
        state.preview_recovery.take().filter(|recovery| failed && recovery.matches(state));
    state.preview_recovery = recovery.map(|mut recovery| {
        recovery.failed = true;
        recovery
    });
    set_undo_redo_enabled(state);
}

fn can_undo_failed_change(state: &super::PreviewState) -> bool {
    !edit_pending(state)
        && state.loading_state == super::PreviewFutureState::Pending
        && state
            .preview_recovery
            .as_ref()
            .is_some_and(|recovery| recovery.failed && recovery.matches(state))
}

pub(super) fn undo_failed_change() {
    let api = super::PREVIEW_STATE.with_borrow_mut(|state| {
        if can_undo_failed_change(state) {
            state.api.upgrade()
        } else {
            state.preview_recovery = None;
            set_undo_redo_enabled(state);
            None
        }
    });
    if let Some(api) = api {
        api.invoke_undo();
    }
}

fn prepare_history_edit(
    document_cache: &i_slint_editor_preview::DocumentCache,
    source_code: &super::SourceCodeCache,
    item: &EditItem,
) -> Option<(lsp_types::WorkspaceEdit, FileHashes)> {
    let source_file = |url: &lsp_types::Url| {
        source_code
            .get(url)
            .map(|source| {
                std::sync::Arc::new(i_slint_compiler::diagnostics::SourceFileInner::new(
                    SourcePath::from_url(url),
                    source.code.clone(),
                ))
            })
            .or_else(|| super::workspace_edit_source_file(document_cache, source_code, url))
    };
    for (url, expected) in &item.file_hashes {
        let source_file = source_file(url)?;
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
        text_edit::apply_workspace_edit_with_source(&item.edit, document_cache.format, source_file)
            .ok()?;
    let reverse =
        text_edit::reversed_edit_with_source(&item.edit, document_cache.format, source_file)?;
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
            state.preview_recovery = None;
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
            let recovery = recovery_candidate(state, Some(&reverse), &file_hashes);
            state.undo_redo_stack.undo_stack.push(EditItem {
                title: edit.title.clone(),
                edit: reverse,
                file_hashes,
            });
            state.preview_recovery = recovery;
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
    state.workspace_edit_sent
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
    if let Some(editor) = state.editor_ui.as_ref() {
        editor.global::<ui::Diagnostics>().set_can_undo_change(can_undo_failed_change(state));
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
