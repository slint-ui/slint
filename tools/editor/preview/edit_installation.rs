// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;

/// The inputs actually supplied to one compilation, retained until installation.
pub(super) struct CompilationSnapshot {
    pub id: u64,
    pub inputs: HashMap<lsp_types::Url, String>,
}

pub(super) struct PendingInstallation {
    after: u64,
    expected: HashMap<lsp_types::Url, String>,
    installed: Option<CompilationSnapshot>,
    acknowledged: bool,
    abandoned: bool,
}

impl PendingInstallation {
    pub fn new(after: u64, expected: HashMap<lsp_types::Url, String>) -> Self {
        Self { after, expected, installed: None, acknowledged: false, abandoned: false }
    }

    pub fn expects(&self, url: &lsp_types::Url, content: &str) -> bool {
        self.expected.get(url).is_some_and(|expected| expected == content)
    }

    pub fn acknowledge(&mut self) {
        self.acknowledged = true;
    }

    pub fn observe(&mut self, compilation: &CompilationSnapshot) -> bool {
        self.installed = Some(CompilationSnapshot {
            id: compilation.id,
            inputs: self
                .expected
                .keys()
                .filter_map(|url| {
                    compilation.inputs.get(url).map(|content| (url.clone(), content.clone()))
                })
                .collect(),
        });
        self.is_installed()
    }

    pub fn is_superseded(&self) -> bool {
        let Some(installed) = &self.installed else { return false };
        // Coalescing can skip the edited revision entirely. Only retire that edit
        // after its write succeeded and the mounted inputs match current disk.
        // An older compilation or a stale cache alone is not evidence of replacement.
        self.acknowledged
            && !self.abandoned
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
        self.abandoned = true;
        self.installed = None;
    }

    pub fn is_finished(&self) -> bool {
        self.abandoned || self.is_installed()
    }

    pub fn is_installed(&self) -> bool {
        !self.abandoned
            && !self.expected.is_empty()
            && self.installed.as_ref().is_some_and(|installed| {
                installed.id > self.after
                    && self
                        .expected
                        .iter()
                        .all(|(url, content)| installed.inputs.get(url) == Some(content))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut edit = PendingInstallation::new(1, HashMap::from([(url.clone(), "edit".into())]));
        let snapshot = |id| CompilationSnapshot {
            id,
            inputs: HashMap::from([(url.clone(), "external".into())]),
        };
        std::fs::write(&path, "external").unwrap();
        edit.observe(&snapshot(2));
        assert!(!edit.is_superseded());
        edit.acknowledge();
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
        let mut edit = PendingInstallation::new(1, inputs(&[("Main.slint", "new")]));
        edit.abandon();
        assert!(edit.is_finished());
        assert!(!edit.is_installed());
        assert!(
            !edit.observe(&CompilationSnapshot { id: 2, inputs: inputs(&[("Main.slint", "new")]) })
        );
    }

    #[test]
    fn compilation_started_before_edit_cannot_complete_it_even_with_identical_content() {
        let expected = inputs(&[("Main.slint", "new")]);
        let mut edit = PendingInstallation::new(7, expected.clone());
        assert!(!edit.observe(&CompilationSnapshot { id: 7, inputs: expected.clone() }));
        assert!(!edit.is_installed());
        assert!(edit.observe(&CompilationSnapshot { id: 8, inputs: expected }));
        assert!(edit.is_installed());
    }

    #[test]
    fn every_edited_dependency_must_match_actual_compiler_inputs() {
        let mut edit =
            PendingInstallation::new(1, inputs(&[("Main.slint", "root"), ("Child.slint", "new")]));
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
        let mut edit = PendingInstallation::new(1, inputs(&[("Main.slint", "new")]));
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
