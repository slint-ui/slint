// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The debugger console: a preview's `debug()` output and compiler diagnostics, in arrival order.

use std::collections::VecDeque;
use std::path::PathBuf;

use lsp_types::{DiagnosticSeverity, Url};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleLevel {
    Debug,
    Note,
    Info,
    Warning,
    Error,
}

/// A position in a file; `line` and `column` start at 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleLocation {
    pub url: Url,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleEntry {
    pub level: ConsoleLevel,
    pub location: Option<ConsoleLocation>,
    pub message: String,
}

/// Collects the console entries of the previews a debugger is attached to.
///
/// It keeps at most [`Self::MAX_ENTRIES`] entries and drops the oldest ones first.
#[derive(Debug)]
pub struct Console {
    entries: VecDeque<ConsoleEntry>,
    auto_clear: bool,
}

impl Default for Console {
    fn default() -> Self {
        Self { entries: VecDeque::new(), auto_clear: true }
    }
}

impl Console {
    pub const MAX_ENTRIES: usize = 10_000;

    /// Whether [`Self::reloaded()`] clears the console.
    pub fn auto_clear(&self) -> bool {
        self.auto_clear
    }

    /// Takes effect at the next reload; turning auto clear on doesn't clear.
    pub fn set_auto_clear(&mut self, auto_clear: bool) {
        self.auto_clear = auto_clear;
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// The entries, oldest first.
    pub fn entries(&self) -> &VecDeque<ConsoleEntry> {
        &self.entries
    }

    /// Call for each `Reloaded` message; clears the console when auto clear is on.
    pub fn reloaded(&mut self) {
        if self.auto_clear {
            self.clear();
        }
    }

    /// Appends an entry for each of a preview's diagnostics in `uri`.
    /// Columns stay in the units the preview sent, UTF-8 or UTF-16.
    pub fn push_diagnostics(&mut self, uri: Url, diagnostics: Vec<lsp_types::Diagnostic>) {
        for diagnostic in diagnostics {
            let level = match diagnostic.severity {
                Some(DiagnosticSeverity::WARNING) => ConsoleLevel::Warning,
                Some(DiagnosticSeverity::INFORMATION) => ConsoleLevel::Info,
                Some(DiagnosticSeverity::HINT) => ConsoleLevel::Note,
                _ => ConsoleLevel::Error,
            };
            let start = diagnostic.range.start;
            self.push(ConsoleEntry {
                level,
                location: Some(ConsoleLocation {
                    url: uri.clone(),
                    line: start.line as usize + 1,
                    column: start.character as usize + 1,
                }),
                message: diagnostic.message,
            });
        }
    }

    /// Appends a preview's `debug()` output.
    /// `location` is the file path, line and column; a relative path gives no location.
    pub fn push_debug_message(
        &mut self,
        location: Option<(PathBuf, usize, usize)>,
        message: String,
    ) {
        let location = location.and_then(|(path, line, column)| {
            Some(ConsoleLocation { url: Url::from_file_path(path).ok()?, line, column })
        });
        self.push(ConsoleEntry { level: ConsoleLevel::Debug, location, message });
    }

    fn push(&mut self, entry: ConsoleEntry) {
        self.entries.push_back(entry);
        while self.entries.len() > Self::MAX_ENTRIES {
            self.entries.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(name: &str) -> Url {
        Url::from_file_path(std::env::temp_dir().join(name)).unwrap()
    }

    fn diagnostic(
        severity: Option<DiagnosticSeverity>,
        line: u32,
        character: u32,
        message: &str,
    ) -> lsp_types::Diagnostic {
        lsp_types::Diagnostic {
            range: lsp_types::Range::new(
                lsp_types::Position::new(line, character),
                lsp_types::Position::new(line, character + 1),
            ),
            severity,
            message: message.into(),
            ..Default::default()
        }
    }

    fn messages(console: &Console) -> Vec<&str> {
        console.entries().iter().map(|entry| entry.message.as_str()).collect()
    }

    #[test]
    fn a_successful_reload_starts_over_and_keeps_the_arrival_order() {
        let mut console = Console::default();
        console.push_debug_message(None, "before".into());
        console.reloaded();
        console.push_diagnostics(
            url("a.slint"),
            vec![diagnostic(Some(DiagnosticSeverity::WARNING), 0, 0, "warning")],
        );
        console.push_debug_message(None, "first".into());
        console.push_diagnostics(
            url("b.slint"),
            vec![diagnostic(Some(DiagnosticSeverity::INFORMATION), 0, 0, "info")],
        );
        console.push_debug_message(None, "second".into());
        assert_eq!(messages(&console), ["warning", "first", "info", "second"]);
    }

    #[test]
    fn a_failed_reload_leaves_only_its_diagnostics() {
        let mut console = Console::default();
        console.reloaded();
        console.push_diagnostics(
            url("a.slint"),
            vec![diagnostic(Some(DiagnosticSeverity::WARNING), 0, 0, "old")],
        );
        console.push_debug_message(None, "running".into());
        console.reloaded();
        console.push_diagnostics(
            url("a.slint"),
            vec![
                diagnostic(Some(DiagnosticSeverity::ERROR), 0, 0, "first error"),
                diagnostic(Some(DiagnosticSeverity::ERROR), 1, 0, "second error"),
            ],
        );
        console.push_diagnostics(url("b.slint"), Vec::new());
        assert_eq!(messages(&console), ["first error", "second error"]);
    }

    #[test]
    fn without_auto_clear_only_clear_empties() {
        let mut console = Console::default();
        assert!(console.auto_clear());
        console.set_auto_clear(false);
        assert!(!console.auto_clear());
        console.push_debug_message(None, "first".into());
        console.reloaded();
        console.push_debug_message(None, "second".into());
        console.reloaded();
        assert_eq!(messages(&console), ["first", "second"]);

        console.set_auto_clear(true);
        assert_eq!(messages(&console), ["first", "second"]);
        console.clear();
        assert!(console.entries().is_empty());
    }

    #[test]
    fn the_oldest_entries_are_dropped_beyond_the_cap() {
        let mut console = Console::default();
        for index in 0..=Console::MAX_ENTRIES {
            console.push_debug_message(None, index.to_string());
        }
        assert_eq!(console.entries().len(), Console::MAX_ENTRIES);
        assert_eq!(console.entries().front().unwrap().message, "1");
        assert_eq!(console.entries().back().unwrap().message, Console::MAX_ENTRIES.to_string());
    }

    #[test]
    fn severities_and_locations_are_converted() {
        let mut console = Console::default();
        let file = url("a.slint");
        console.push_diagnostics(
            file.clone(),
            [
                Some(DiagnosticSeverity::ERROR),
                Some(DiagnosticSeverity::WARNING),
                Some(DiagnosticSeverity::INFORMATION),
                Some(DiagnosticSeverity::HINT),
                None,
            ]
            .into_iter()
            .map(|severity| diagnostic(severity, 2, 4, "diagnostic"))
            .collect(),
        );
        let levels: Vec<_> = console.entries().iter().map(|entry| entry.level).collect();
        assert_eq!(
            levels,
            [
                ConsoleLevel::Error,
                ConsoleLevel::Warning,
                ConsoleLevel::Info,
                ConsoleLevel::Note,
                ConsoleLevel::Error,
            ]
        );
        assert_eq!(
            console.entries()[0].location,
            Some(ConsoleLocation { url: file.clone(), line: 3, column: 5 })
        );

        console.clear();
        let path = std::env::temp_dir().join("a.slint");
        console.push_debug_message(Some((path, 7, 9)), "absolute".into());
        console.push_debug_message(Some(("a.slint".into(), 7, 9)), "relative".into());
        let [absolute, relative] = console.entries().iter().collect::<Vec<_>>()[..] else {
            panic!("{:?}", console.entries())
        };
        assert_eq!(absolute.level, ConsoleLevel::Debug);
        assert_eq!(absolute.location, Some(ConsoleLocation { url: file, line: 7, column: 9 }));
        assert_eq!(relative.location, None);
    }
}
