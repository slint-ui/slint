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
    installed: Option<u64>,
    abandoned: bool,
}

impl PendingInstallation {
    pub fn new(after: u64, expected: HashMap<lsp_types::Url, String>) -> Self {
        Self { after, expected, installed: None, abandoned: false }
    }

    pub fn expects(&self, url: &lsp_types::Url, content: &str) -> bool {
        self.expected.get(url).is_some_and(|expected| expected == content)
    }

    pub fn observe(&mut self, compilation: &CompilationSnapshot) -> bool {
        let matches = !self.abandoned
            && compilation.id > self.after
            && !self.expected.is_empty()
            && self
                .expected
                .iter()
                .all(|(url, contents)| compilation.inputs.get(url) == Some(contents));
        // A later unrelated installation must not leave an earlier match valid.
        self.installed = matches.then_some(compilation.id);
        matches
    }

    pub fn abandon(&mut self) {
        self.abandoned = true;
        self.installed = None;
    }

    pub fn is_finished(&self) -> bool {
        self.abandoned || self.is_installed()
    }

    pub fn is_installed(&self) -> bool {
        self.installed.is_some()
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
