// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;

/// The inputs actually supplied to one compilation, retained until installation.
pub(super) struct CompilationSnapshot {
    pub id: u64,
    pub inputs: HashMap<lsp_types::Url, String>,
}

enum Installation {
    Waiting,
    Installed(CompilationSnapshot),
    Abandoned,
}

enum Acknowledgment {
    Pending(super::undo_redo::PendingEdit),
    Applied,
}

pub(super) struct PendingWorkspaceEdit {
    pub id: u64,
    after: u64,
    expected: HashMap<lsp_types::Url, String>,
    installation: Installation,
    acknowledgment: Acknowledgment,
}

impl PendingWorkspaceEdit {
    pub fn new(
        id: u64,
        after: u64,
        expected: HashMap<lsp_types::Url, String>,
        history: super::undo_redo::PendingEdit,
    ) -> Self {
        Self {
            id,
            after,
            expected,
            installation: Installation::Waiting,
            acknowledgment: Acknowledgment::Pending(history),
        }
    }

    pub fn expects(&self, url: &lsp_types::Url, content: &str) -> bool {
        self.awaiting_acknowledgment()
            && self.expected.get(url).is_some_and(|expected| expected == content)
    }

    pub fn awaiting_acknowledgment(&self) -> bool {
        matches!(self.acknowledgment, Acknowledgment::Pending(_))
    }

    pub fn acknowledge(&mut self) -> Option<super::undo_redo::PendingEdit> {
        match std::mem::replace(&mut self.acknowledgment, Acknowledgment::Applied) {
            Acknowledgment::Pending(history) => Some(history),
            Acknowledgment::Applied => None,
        }
    }

    pub fn observe(&mut self, compilation: &CompilationSnapshot) -> bool {
        if !matches!(self.installation, Installation::Abandoned) {
            self.installation = Installation::Installed(CompilationSnapshot {
                id: compilation.id,
                inputs: self
                    .expected
                    .keys()
                    .filter_map(|url| {
                        compilation.inputs.get(url).map(|content| (url.clone(), content.clone()))
                    })
                    .collect(),
            });
        }
        self.is_installed()
    }

    pub fn is_superseded(&self) -> bool {
        let Installation::Installed(installed) = &self.installation else { return false };
        // Coalescing can skip the edited revision entirely. Only retire that edit
        // after its write succeeded and the mounted inputs match current disk.
        // An older compilation or a stale cache alone is not evidence of replacement.
        !self.awaiting_acknowledgment()
            && installed.id > self.after
            && !self.is_installed()
            && !self.expected.is_empty()
            && self.expected.keys().all(|url| {
                installed.inputs.get(url).is_some_and(|content| {
                    url.to_file_path()
                        .ok()
                        .and_then(|path| std::fs::read_to_string(path).ok())
                        .as_ref()
                        == Some(content)
                })
            })
    }

    pub fn abandon(&mut self) {
        self.installation = Installation::Abandoned;
    }

    pub fn is_finished(&self) -> bool {
        !self.awaiting_acknowledgment()
            && (matches!(self.installation, Installation::Abandoned) || self.is_installed())
    }

    pub fn is_installed(&self) -> bool {
        let Installation::Installed(installed) = &self.installation else { return false };
        !self.expected.is_empty()
            && installed.id > self.after
            && self.expected.iter().all(|(url, content)| installed.inputs.get(url) == Some(content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(after: u64, expected: HashMap<lsp_types::Url, String>) -> PendingWorkspaceEdit {
        PendingWorkspaceEdit::new(
            1,
            after,
            expected,
            super::super::undo_redo::PendingEdit::New(None),
        )
    }

    fn inputs(entries: &[(&str, &str)]) -> HashMap<lsp_types::Url, String> {
        entries
            .iter()
            .map(|(path, content)| (format!("file:///{path}").parse().unwrap(), (*content).into()))
            .collect()
    }

    #[test]
    fn supersession_requires_successful_write_and_current_installed_source() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Main.slint");
        let url = lsp_types::Url::from_file_path(&path).unwrap();
        let mut edit = pending(1, HashMap::from([(url.clone(), "edit".into())]));
        let snapshot = |id| CompilationSnapshot {
            id,
            inputs: HashMap::from([(url.clone(), "external".into())]),
        };
        std::fs::write(&path, "external").unwrap();
        edit.observe(&snapshot(2));
        assert!(!edit.is_superseded());
        assert!(edit.acknowledge().is_some());
        assert!(edit.is_superseded());
        edit.observe(&snapshot(1));
        assert!(!edit.is_superseded());
        edit.observe(&snapshot(2));
        std::fs::write(&path, "edit").unwrap();
        assert!(!edit.is_superseded());
        std::fs::remove_file(&path).unwrap();
        assert!(!edit.is_superseded());
    }

    #[test]
    fn abandoned_installation_is_terminal_without_claiming_application() {
        let mut edit = pending(1, inputs(&[("Main.slint", "new")]));
        edit.abandon();
        assert!(!edit.is_finished());
        assert!(edit.acknowledge().is_some());
        assert!(edit.is_finished());
        assert!(edit.acknowledge().is_none());
        assert!(!edit.is_installed());
        assert!(
            !edit.observe(&CompilationSnapshot { id: 2, inputs: inputs(&[("Main.slint", "new")]) })
        );
    }

    #[test]
    fn compilation_started_before_edit_cannot_complete_it_even_with_identical_content() {
        let expected = inputs(&[("Main.slint", "new")]);
        let mut edit = pending(7, expected.clone());
        assert!(!edit.observe(&CompilationSnapshot { id: 7, inputs: expected.clone() }));
        assert!(!edit.is_installed());
        assert!(edit.observe(&CompilationSnapshot { id: 8, inputs: expected }));
        assert!(edit.is_installed());
    }

    #[test]
    fn every_edited_dependency_must_match_actual_compiler_inputs() {
        let mut edit = pending(1, inputs(&[("Main.slint", "root"), ("Child.slint", "new")]));
        for entries in
            [vec![("Main.slint", "root")], vec![("Main.slint", "root"), ("Child.slint", "old")]]
        {
            assert!(!edit.observe(&CompilationSnapshot { id: 2, inputs: inputs(&entries) }));
        }
        assert!(edit.observe(&CompilationSnapshot {
            id: 3,
            inputs: inputs(&[("Main.slint", "root"), ("Child.slint", "new")])
        }));
    }

    #[test]
    fn unrelated_installation_before_acknowledgment_invalidates_previous_match() {
        let mut edit = pending(1, inputs(&[("Main.slint", "new")]));
        assert!(
            edit.observe(&CompilationSnapshot { id: 2, inputs: inputs(&[("Main.slint", "new")]) })
        );
        assert!(
            !edit
                .observe(&CompilationSnapshot { id: 3, inputs: inputs(&[("Other.slint", "new")]) })
        );
        assert!(!edit.is_installed());
    }
}
