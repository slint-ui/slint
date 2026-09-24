// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Submits reversible document edits and commits their history after acknowledgement.
//!
//! Every submitted edit has an inverse before it reaches the editor.
//! The pending command keeps history unchanged until the editor reports success.

use super::*;

#[derive(Clone, Copy)]
pub(super) enum ValidationPolicy {
    Compile,
    StructuralOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SubmitEditOutcome {
    Submitted,
    NoChange,
    Rejected,
}

impl SubmitEditOutcome {
    pub(super) fn accepted(self) -> bool {
        matches!(self, Self::Submitted | Self::NoChange)
    }
}

pub(super) enum HistoryDirection {
    Undo,
    Redo,
}

enum Completion {
    Commit { undo: undo_redo::EditItem, fill: Option<ui::FillData>, inspector: bool },
    Undo { redo: undo_redo::EditItem },
    Redo { undo: undo_redo::EditItem },
}

pub(super) struct PendingDocumentEdit {
    submitted_edit: lsp_types::WorkspaceEdit,
    expected: Vec<text_edit::EditedText>,
    history_generation: u64,
    record_history: bool,
    completion: Option<Completion>,
}

fn has_unsupported_operations(edit: &lsp_types::WorkspaceEdit) -> bool {
    matches!(
        edit.document_changes.as_ref(),
        Some(lsp_types::DocumentChanges::Operations(operations)) if !operations.is_empty()
    )
}

pub(super) fn submit(
    label: String,
    edit: lsp_types::WorkspaceEdit,
    validation: ValidationPolicy,
) -> SubmitEditOutcome {
    submit_commit(label, edit, validation, None, false)
}

pub(super) fn submit_inspector(
    label: String,
    edit: lsp_types::WorkspaceEdit,
    validation: ValidationPolicy,
) -> SubmitEditOutcome {
    submit_commit(label, edit, validation, None, true)
}

pub(super) fn submit_fill(
    label: String,
    edit: lsp_types::WorkspaceEdit,
    validation: ValidationPolicy,
    fill: ui::FillData,
) -> SubmitEditOutcome {
    submit_commit(label, edit, validation, Some(fill), true)
}

fn submit_commit(
    label: String,
    edit: lsp_types::WorkspaceEdit,
    validation: ValidationPolicy,
    fill: Option<ui::FillData>,
    inspector: bool,
) -> SubmitEditOutcome {
    if has_unsupported_operations(&edit) {
        return SubmitEditOutcome::Rejected;
    }
    let Some(document_cache) = document_cache() else {
        return SubmitEditOutcome::Rejected;
    };
    let Ok(result) = text_edit::apply_workspace_edit(&document_cache, &edit) else {
        return SubmitEditOutcome::Rejected;
    };
    if result.is_empty() {
        if inspector {
            inspector::cancel();
        }
        return SubmitEditOutcome::NoChange;
    }
    if fill.is_some() && result.len() != 1 {
        return SubmitEditOutcome::Rejected;
    }
    if matches!(validation, ValidationPolicy::Compile)
        && !matches!(
            drop_location::edited_text_compiles(&document_cache, clone_edited_text(&result)),
            CompilationResult::ChangeCompiles
        )
    {
        return SubmitEditOutcome::Rejected;
    }
    let Some(reverse) = text_edit::reversed_edit(&document_cache, &edit) else {
        return SubmitEditOutcome::Rejected;
    };
    let undo = undo_redo::EditItem {
        title: label.clone(),
        edit: reverse,
        file_hashes: undo_redo::compute_file_hashes(&result),
    };
    let pending = PendingDocumentEdit {
        submitted_edit: edit,
        expected: result,
        history_generation: 0,
        record_history: true,
        completion: Some(Completion::Commit { undo, fill, inspector }),
    };
    send(label, pending)
}

fn clone_edited_text(edits: &[text_edit::EditedText]) -> Vec<text_edit::EditedText> {
    edits
        .iter()
        .map(|edit| text_edit::EditedText {
            url: edit.url.clone(),
            contents: edit.contents.clone(),
        })
        .collect()
}

fn send(label: String, mut pending: PendingDocumentEdit) -> SubmitEditOutcome {
    let message_edit = pending.submitted_edit.clone();
    let sender = PREVIEW_STATE.with_borrow_mut(|state| {
        if edit_pending(state) {
            return None;
        }
        pending.history_generation = state.undo_redo_stack.generation();
        let fill_pending =
            matches!(&pending.completion, Some(Completion::Commit { fill: Some(_), .. }));
        state.pending_document_edit = Some(pending);
        undo_redo::set_undo_redo_enabled(state);
        Some((state.to_lsp.borrow().clone()?, state.api.upgrade(), fill_pending))
    });
    let Some((sender, api, fill_pending)) = sender else {
        return SubmitEditOutcome::Rejected;
    };
    if fill_pending && let Some(api) = api.as_ref() {
        api.set_inspector_fill_refresh_pending(true);
    }
    if sender
        .send(&PreviewToLspMessage::SendWorkspaceEdit { label: Some(label), edit: message_edit })
        .is_err()
    {
        cancel_pending();
        return SubmitEditOutcome::Rejected;
    }
    SubmitEditOutcome::Submitted
}

pub(super) fn submit_history(direction: HistoryDirection) {
    let Some(document_cache) = document_cache() else { return };
    let prepared = PREVIEW_STATE.with_borrow_mut(|state| {
        if edit_pending(state) {
            state.pending_history.push_back(matches!(direction, HistoryDirection::Redo));
            return None;
        }
        let prepared = match direction {
            HistoryDirection::Undo => state.undo_redo_stack.prepare_undo(&document_cache),
            HistoryDirection::Redo => state.undo_redo_stack.prepare_redo(&document_cache),
        };
        if prepared.is_none() {
            undo_redo::set_undo_redo_enabled(state);
        }
        prepared
    });
    let Some((item, reverse, expected)) = prepared else {
        return;
    };
    let label = match direction {
        HistoryDirection::Undo => format!("Undo \"{}\"", item.title),
        HistoryDirection::Redo => format!("Redo \"{}\"", item.title),
    };
    let pending = PendingDocumentEdit {
        submitted_edit: item.edit.clone(),
        expected,
        history_generation: 0,
        record_history: true,
        completion: Some(match direction {
            HistoryDirection::Undo => Completion::Undo { redo: reverse },
            HistoryDirection::Redo => Completion::Redo { undo: reverse },
        }),
    };
    let _ = send(label, pending);
}

pub(super) fn contents_changed(url: &Url, content: &str) -> bool {
    PREVIEW_STATE.with_borrow_mut(|state| {
        let changed = state.source_code.get(url).is_none_or(|source| source.code != content);
        let (own_edit, complete) = {
            let Some(pending) = state.pending_document_edit.as_mut() else { return false };
            let Some(index) = pending.expected.iter().position(|expected| expected.url == *url)
            else {
                return false;
            };
            let own_edit = pending.expected[index].contents == content;
            if own_edit {
                pending.expected.remove(index);
            } else if changed {
                pending.record_history = false;
                pending.expected.remove(index);
            }
            let complete = pending.expected.is_empty() && pending.completion.is_none();
            (own_edit, complete)
        };
        if complete {
            state.pending_document_edit = None;
        }
        own_edit
    })
}

pub(super) fn finished(edit: lsp_types::WorkspaceEdit, applied: bool, changed_on_failure: bool) {
    let result = PREVIEW_STATE.with_borrow_mut(|state| {
        let mut pending = state.pending_document_edit.take()?;
        if pending.submitted_edit != edit {
            state.pending_document_edit = Some(pending);
            return None;
        }
        let mut fill = None;
        let mut cancel_inspector = false;
        let Some(completion) = pending.completion.take() else {
            state.pending_document_edit = Some(pending);
            return None;
        };
        if applied {
            let history_is_current =
                pending.history_generation == state.undo_redo_stack.generation();
            match completion {
                Completion::Commit { undo, fill: committed_fill, inspector } => {
                    if pending.record_history && history_is_current {
                        state.undo_redo_stack.push(undo);
                    }
                    if inspector {
                        state.inspector_edit.take();
                    }
                    fill = committed_fill;
                }
                Completion::Undo { redo } if history_is_current => {
                    state.undo_redo_stack.complete_undo(redo)
                }
                Completion::Redo { undo } if history_is_current => {
                    state.undo_redo_stack.complete_redo(undo)
                }
                Completion::Undo { .. } | Completion::Redo { .. } => {}
            }
            if !pending.expected.is_empty() {
                state.pending_document_edit = Some(pending);
            }
        } else {
            if changed_on_failure {
                state.undo_redo_stack.clear();
                state.pending_history.clear();
            }
            if let Completion::Commit { inspector, .. } = completion {
                cancel_inspector = inspector;
            }
        }
        undo_redo::set_undo_redo_enabled(state);
        Some((state.api.upgrade(), fill, cancel_inspector))
    });
    let Some((api, fill, cancel_inspector)) = result else { return };
    if let Some(api) = api.as_ref() {
        api.set_inspector_fill_refresh_pending(false);
        if applied && let Some(fill) = fill {
            api.invoke_add_recent_fill(fill);
        }
    }
    if cancel_inspector {
        inspector::cancel();
        inspector::invalidate_fill();
    }
    if !applied {
        undo_redo::apply_pending();
    }
}

pub(super) fn cancel_pending() {
    let api = PREVIEW_STATE.with_borrow_mut(|state| {
        state.pending_document_edit = None;
        state.api.upgrade()
    });
    if let Some(api) = api {
        api.set_inspector_fill_refresh_pending(false);
    }
}

pub(super) fn edit_pending(state: &PreviewState) -> bool {
    state.pending_document_edit.is_some()
}

#[cfg(test)]
pub(super) fn mark_pending_for_test(state: &mut PreviewState) {
    state.pending_document_edit = Some(PendingDocumentEdit {
        submitted_edit: Default::default(),
        expected: Vec::new(),
        history_generation: 0,
        record_history: false,
        completion: Some(Completion::Commit {
            undo: undo_redo::EditItem {
                title: String::new(),
                edit: Default::default(),
                file_hashes: Default::default(),
            },
            fill: None,
            inspector: false,
        }),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_editor_preview::PreviewToLsp;
    use std::{cell::RefCell, rc::Rc};

    const SOURCE: &str = "export component Main inherits Rectangle {\n    width: 30px;\n}\n";

    struct CapturePreviewToLsp {
        messages: Rc<RefCell<Vec<PreviewToLspMessage>>>,
    }

    impl PreviewToLsp for CapturePreviewToLsp {
        fn send(&self, message: &PreviewToLspMessage) -> i_slint_editor_preview::Result<()> {
            self.messages.as_ref().borrow_mut().push(message.clone());
            Ok(())
        }
    }

    fn reset(sources: &[(Url, &str)]) -> Rc<RefCell<Vec<PreviewToLspMessage>>> {
        let messages = Rc::new(RefCell::new(Vec::new()));
        let mut cache = i_slint_editor_preview::test::empty_document_cache();
        for (url, source) in sources {
            let mut diagnostics = i_slint_compiler::diagnostics::BuildDiagnostics::default();
            spin_on::spin_on(cache.load_url(url, Some(1), (*source).to_owned(), &mut diagnostics))
                .unwrap();
            assert!(!diagnostics.has_errors());
        }
        PREVIEW_STATE.with_borrow_mut(|state| {
            *state = PreviewState::default();
            state.document_cache.replace(Some(Rc::new(cache)));
            state.to_lsp =
                RefCell::new(Some(Rc::new(CapturePreviewToLsp { messages: messages.clone() })));
        });
        messages
    }

    fn width_edit(url: Url, value: &str) -> lsp_types::WorkspaceEdit {
        i_slint_editor_preview::editing::create_workspace_edit(
            url,
            Some(1),
            vec![lsp_types::TextEdit {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(1, 11),
                    lsp_types::Position::new(1, 13),
                ),
                new_text: value.into(),
            }],
        )
    }

    fn stack_lengths() -> (usize, usize) {
        PREVIEW_STATE.with_borrow(|state| state.undo_redo_stack.lengths())
    }

    #[test]
    fn records_history_only_after_successful_application() {
        let url = Url::parse("file:///document-edit.slint").unwrap();
        let messages = reset(&[(url.clone(), SOURCE)]);
        let edit = width_edit(url, "40");

        assert_eq!(
            submit("Change width".into(), edit.clone(), ValidationPolicy::StructuralOnly),
            SubmitEditOutcome::Submitted
        );
        assert_eq!(stack_lengths(), (0, 0));
        assert!(PREVIEW_STATE.with_borrow(edit_pending));
        finished(edit.clone(), false, false);
        assert_eq!(stack_lengths(), (0, 0));
        assert!(!PREVIEW_STATE.with_borrow(edit_pending));

        assert_eq!(
            submit("Change width".into(), edit.clone(), ValidationPolicy::StructuralOnly),
            SubmitEditOutcome::Submitted
        );
        finished(edit, true, false);
        assert_eq!(stack_lengths(), (1, 0));
        assert!(PREVIEW_STATE.with_borrow(edit_pending));
        assert!(contents_changed(
            &Url::parse("file:///document-edit.slint").unwrap(),
            &SOURCE.replace("30px", "40px")
        ));
        assert!(!PREVIEW_STATE.with_borrow(edit_pending));
        assert_eq!(messages.borrow().len(), 2);
    }

    #[test]
    fn distinguishes_no_change_from_rejected_edits() {
        let url = Url::parse("file:///document-edit.slint").unwrap();
        let messages = reset(&[(url.clone(), SOURCE)]);

        assert_eq!(
            submit(
                "Keep width".into(),
                width_edit(url.clone(), "30"),
                ValidationPolicy::StructuralOnly,
            ),
            SubmitEditOutcome::NoChange
        );
        let resource_edit = lsp_types::WorkspaceEdit {
            document_changes: Some(lsp_types::DocumentChanges::Operations(vec![
                lsp_types::DocumentChangeOperation::Op(lsp_types::ResourceOp::Create(
                    lsp_types::CreateFile { uri: url, options: None, annotation_id: None },
                )),
            ])),
            ..Default::default()
        };
        assert_eq!(
            submit("Create file".into(), resource_edit, ValidationPolicy::StructuralOnly),
            SubmitEditOutcome::Rejected
        );
        assert!(messages.borrow().is_empty());
        assert_eq!(stack_lengths(), (0, 0));
    }

    #[test]
    fn groups_multi_file_changes_into_one_history_entry() {
        let first = Url::parse("file:///first-document-edit.slint").unwrap();
        let second = Url::parse("file:///second-document-edit.slint").unwrap();
        reset(&[(first.clone(), SOURCE), (second.clone(), SOURCE)]);
        let edit = lsp_types::WorkspaceEdit {
            document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
                match width_edit(first, "40").document_changes.unwrap() {
                    lsp_types::DocumentChanges::Edits(mut edits) => edits.remove(0),
                    lsp_types::DocumentChanges::Operations(_) => unreachable!(),
                },
                match width_edit(second, "50").document_changes.unwrap() {
                    lsp_types::DocumentChanges::Edits(mut edits) => edits.remove(0),
                    lsp_types::DocumentChanges::Operations(_) => unreachable!(),
                },
            ])),
            ..Default::default()
        };

        assert_eq!(
            submit("Change widths".into(), edit.clone(), ValidationPolicy::StructuralOnly),
            SubmitEditOutcome::Submitted
        );
        finished(edit, true, false);
        assert_eq!(stack_lengths(), (1, 0));
        assert!(contents_changed(
            &Url::parse("file:///first-document-edit.slint").unwrap(),
            &SOURCE.replace("30px", "40px")
        ));
        assert!(contents_changed(
            &Url::parse("file:///second-document-edit.slint").unwrap(),
            &SOURCE.replace("30px", "50px")
        ));
        assert_eq!(
            PREVIEW_STATE.with_borrow(|state| state.undo_redo_stack.latest_undo_file_count()),
            Some(2)
        );
    }

    #[test]
    fn external_source_change_invalidates_pending_and_existing_history() {
        let url = Url::parse("file:///external-document-edit.slint").unwrap();
        reset(&[(url.clone(), SOURCE)]);
        let first = width_edit(url.clone(), "40");
        assert_eq!(
            submit("First width".into(), first.clone(), ValidationPolicy::StructuralOnly),
            SubmitEditOutcome::Submitted
        );
        finished(first, true, false);
        let changed = SOURCE.replace("30px", "40px");
        assert!(contents_changed(&url, &changed));
        assert_eq!(stack_lengths(), (1, 0));

        reset_cache(&[(url.clone(), changed.as_str())]);
        let second = width_edit(url.clone(), "50");
        assert_eq!(
            submit("Second width".into(), second.clone(), ValidationPolicy::StructuralOnly),
            SubmitEditOutcome::Submitted
        );
        let external = SOURCE.replace("30px", "900px");
        assert!(!contents_changed(&url, &external));
        PREVIEW_STATE.with_borrow_mut(|state| {
            assert!(!state.undo_redo_stack.check_set_contents_valid(&url, &external));
        });
        finished(second, true, false);
        assert_eq!(stack_lengths(), (0, 0));
        assert!(!PREVIEW_STATE.with_borrow(edit_pending));
    }

    #[test]
    fn external_source_change_does_not_restore_pending_undo() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pending-undo.slint");
        std::fs::write(&path, SOURCE).unwrap();
        let url = Url::from_file_path(&path).unwrap();
        let messages = reset(&[(url.clone(), SOURCE)]);
        let edit = width_edit(url.clone(), "40");
        assert_eq!(
            submit("Change width".into(), edit.clone(), ValidationPolicy::StructuralOnly),
            SubmitEditOutcome::Submitted
        );

        let changed = SOURCE.replace("30px", "40px");
        std::fs::write(&path, &changed).unwrap();
        reset_cache(&[(url.clone(), changed.as_str())]);
        assert!(contents_changed(&url, &changed));
        finished(edit, true, false);
        assert_eq!(stack_lengths(), (1, 0));

        submit_history(HistoryDirection::Undo);
        let undo = sent_edit(&messages, 1);
        let external = SOURCE.replace("30px", "900px");
        assert!(!contents_changed(&url, &external));
        PREVIEW_STATE.with_borrow_mut(|state| {
            assert!(!state.undo_redo_stack.check_set_contents_valid(&url, &external));
        });
        finished(undo, true, false);

        assert_eq!(stack_lengths(), (0, 0));
        assert!(!PREVIEW_STATE.with_borrow(edit_pending));
    }

    #[test]
    fn external_source_change_does_not_restore_pending_redo() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pending-redo.slint");
        std::fs::write(&path, SOURCE).unwrap();
        let url = Url::from_file_path(&path).unwrap();
        let messages = reset(&[(url.clone(), SOURCE)]);
        let edit = width_edit(url.clone(), "40");
        assert_eq!(
            submit("Change width".into(), edit.clone(), ValidationPolicy::StructuralOnly),
            SubmitEditOutcome::Submitted
        );

        let changed = SOURCE.replace("30px", "40px");
        std::fs::write(&path, &changed).unwrap();
        reset_cache(&[(url.clone(), changed.as_str())]);
        assert!(contents_changed(&url, &changed));
        finished(edit, true, false);

        submit_history(HistoryDirection::Undo);
        let undo = sent_edit(&messages, 1);
        std::fs::write(&path, SOURCE).unwrap();
        reset_cache(&[(url.clone(), SOURCE)]);
        assert!(contents_changed(&url, SOURCE));
        finished(undo, true, false);
        assert_eq!(stack_lengths(), (0, 1));

        submit_history(HistoryDirection::Redo);
        let redo = sent_edit(&messages, 2);
        let external = SOURCE.replace("30px", "900px");
        assert!(!contents_changed(&url, &external));
        PREVIEW_STATE.with_borrow_mut(|state| {
            assert!(!state.undo_redo_stack.check_set_contents_valid(&url, &external));
        });
        finished(redo, true, false);

        assert_eq!(stack_lengths(), (0, 0));
        assert!(!PREVIEW_STATE.with_borrow(edit_pending));
    }

    #[test]
    fn failed_history_action_preserves_stacks_and_queued_actions_continue() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("document-edit.slint");
        std::fs::write(&path, SOURCE).unwrap();
        let url = Url::from_file_path(&path).unwrap();
        let messages = reset(&[(url.clone(), SOURCE)]);
        let edit = width_edit(url.clone(), "40");
        assert_eq!(
            submit("Change width".into(), edit.clone(), ValidationPolicy::StructuralOnly),
            SubmitEditOutcome::Submitted
        );
        finished(edit, true, false);

        let changed = SOURCE.replace("30px", "40px");
        std::fs::write(&path, &changed).unwrap();
        reset_cache(&[(url, changed.as_str())]);
        assert!(contents_changed(&Url::from_file_path(&path).unwrap(), changed.as_str()));
        submit_history(HistoryDirection::Undo);
        let undo = sent_edit(&messages, 1);
        assert_eq!(stack_lengths(), (1, 0));
        finished(undo.clone(), false, false);
        assert_eq!(stack_lengths(), (1, 0));

        submit_history(HistoryDirection::Undo);
        let undo = sent_edit(&messages, 2);
        submit_history(HistoryDirection::Redo);
        assert_eq!(PREVIEW_STATE.with_borrow(|state| state.pending_history.len()), 1);
        finished(undo, true, false);
        assert_eq!(stack_lengths(), (0, 1));
        assert!(PREVIEW_STATE.with_borrow(edit_pending));
        std::fs::write(&path, SOURCE).unwrap();
        reset_cache(&[(Url::from_file_path(&path).unwrap(), SOURCE)]);
        assert!(contents_changed(&Url::from_file_path(&path).unwrap(), SOURCE));
        undo_redo::apply_pending();
        assert!(PREVIEW_STATE.with_borrow(edit_pending));
        let redo = sent_edit(&messages, 3);
        finished(redo, true, false);
        assert_eq!(stack_lengths(), (1, 0));
        std::fs::write(&path, &changed).unwrap();
        reset_cache(&[(Url::from_file_path(&path).unwrap(), changed.as_str())]);
        assert!(contents_changed(&Url::from_file_path(&path).unwrap(), changed.as_str()));
        assert!(!PREVIEW_STATE.with_borrow(edit_pending));
    }

    fn reset_cache(sources: &[(Url, &str)]) {
        let mut cache = i_slint_editor_preview::test::empty_document_cache();
        for (url, source) in sources {
            let mut diagnostics = i_slint_compiler::diagnostics::BuildDiagnostics::default();
            spin_on::spin_on(cache.load_url(url, Some(2), (*source).to_owned(), &mut diagnostics))
                .unwrap();
            assert!(!diagnostics.has_errors());
        }
        PREVIEW_STATE.with_borrow_mut(|state| state.document_cache.replace(Some(Rc::new(cache))));
    }

    fn sent_edit(
        messages: &Rc<RefCell<Vec<PreviewToLspMessage>>>,
        index: usize,
    ) -> lsp_types::WorkspaceEdit {
        let PreviewToLspMessage::SendWorkspaceEdit { edit, .. } = &messages.borrow()[index] else {
            panic!("expected a workspace edit")
        };
        edit.clone()
    }
}
