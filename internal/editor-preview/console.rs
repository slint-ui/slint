// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The debugger console: a preview's compiler diagnostics and its `debug()` output.

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;

use lsp_types::Url;

/// A position in a file; `line` and `column` start at 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugLocation {
    pub url: Url,
    pub line: usize,
    pub column: usize,
}

/// The output of one `debug()` call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugMessage {
    pub location: Option<DebugLocation>,
    pub message: String,
}

/// Collects the diagnostics and `debug()` output of the previews a debugger is attached to.
///
/// The diagnostics are those of the latest compilation.
/// The `debug()` output is a log of at most [`Self::MAX_DEBUG_MESSAGES`] messages,
/// which drops the oldest ones first.
#[derive(Debug)]
pub struct Console {
    diagnostics: BTreeMap<Url, Vec<lsp_types::Diagnostic>>,
    debug_messages: VecDeque<DebugMessage>,
    auto_clear: bool,
}

impl Default for Console {
    fn default() -> Self {
        Self { diagnostics: BTreeMap::new(), debug_messages: VecDeque::new(), auto_clear: true }
    }
}

impl Console {
    pub const MAX_DEBUG_MESSAGES: usize = 10_000;

    /// Whether [`Self::reloaded()`] clears the `debug()` output.
    pub fn auto_clear(&self) -> bool {
        self.auto_clear
    }

    /// Takes effect at the next reload; turning auto clear on doesn't clear.
    pub fn set_auto_clear(&mut self, auto_clear: bool) {
        self.auto_clear = auto_clear;
    }

    /// The latest compilation's diagnostics, by file.
    /// Files without diagnostics aren't in it.
    /// Columns are in the units the preview sent, UTF-8 or UTF-16.
    pub fn diagnostics(&self) -> &BTreeMap<Url, Vec<lsp_types::Diagnostic>> {
        &self.diagnostics
    }

    /// The `debug()` output, oldest first.
    pub fn debug_messages(&self) -> &VecDeque<DebugMessage> {
        &self.debug_messages
    }

    pub fn clear_debug_messages(&mut self) {
        self.debug_messages.clear();
    }

    /// Call for each `Reloaded` message.
    /// Drops the diagnostics, since the new compilation's follow,
    /// and clears the `debug()` output when auto clear is on.
    pub fn reloaded(&mut self) {
        self.diagnostics.clear();
        if self.auto_clear {
            self.clear_debug_messages();
        }
    }

    /// Replaces the diagnostics of `uri`.
    pub fn set_diagnostics(&mut self, uri: Url, diagnostics: Vec<lsp_types::Diagnostic>) {
        if diagnostics.is_empty() {
            self.diagnostics.remove(&uri);
        } else {
            self.diagnostics.insert(uri, diagnostics);
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
            Some(DebugLocation { url: Url::from_file_path(path).ok()?, line, column })
        });
        self.debug_messages.push_back(DebugMessage { location, message });
        while self.debug_messages.len() > Self::MAX_DEBUG_MESSAGES {
            self.debug_messages.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(name: &str) -> Url {
        Url::from_file_path(std::env::temp_dir().join(name)).unwrap()
    }

    fn diagnostic(message: &str) -> lsp_types::Diagnostic {
        lsp_types::Diagnostic { message: message.into(), ..Default::default() }
    }

    fn diagnostic_messages(console: &Console) -> Vec<(Url, Vec<&str>)> {
        console
            .diagnostics()
            .iter()
            .map(|(url, diagnostics)| {
                (url.clone(), diagnostics.iter().map(|d| d.message.as_str()).collect())
            })
            .collect()
    }

    fn debug_messages(console: &Console) -> Vec<&str> {
        console.debug_messages().iter().map(|entry| entry.message.as_str()).collect()
    }

    #[test]
    fn new_diagnostics_replace_a_files_old_ones() {
        let mut console = Console::default();
        console.set_diagnostics(url("a.slint"), vec![diagnostic("old")]);
        console.set_diagnostics(url("b.slint"), vec![diagnostic("fixed")]);
        console.set_diagnostics(url("a.slint"), vec![diagnostic("first"), diagnostic("second")]);
        console.set_diagnostics(url("b.slint"), Vec::new());
        assert_eq!(diagnostic_messages(&console), [(url("a.slint"), vec!["first", "second"])]);
    }

    #[test]
    fn a_reload_drops_the_diagnostics_even_without_auto_clear() {
        let mut console = Console::default();
        console.set_auto_clear(false);
        console.set_diagnostics(url("removed.slint"), vec![diagnostic("old")]);
        console.reloaded();
        console.set_diagnostics(url("a.slint"), vec![diagnostic("new")]);
        assert_eq!(diagnostic_messages(&console), [(url("a.slint"), vec!["new"])]);
    }

    #[test]
    fn debug_output_leaves_the_diagnostics_alone() {
        let mut console = Console::default();
        console.set_diagnostics(url("a.slint"), vec![diagnostic("warning")]);
        for index in 0..=Console::MAX_DEBUG_MESSAGES {
            console.push_debug_message(None, index.to_string());
        }
        console.clear_debug_messages();
        assert_eq!(diagnostic_messages(&console), [(url("a.slint"), vec!["warning"])]);
    }

    #[test]
    fn auto_clear_clears_the_debug_output_on_reload() {
        let mut console = Console::default();
        assert!(console.auto_clear());
        console.push_debug_message(None, "before".into());
        console.reloaded();
        console.push_debug_message(None, "after".into());
        assert_eq!(debug_messages(&console), ["after"]);

        console.set_auto_clear(false);
        assert!(!console.auto_clear());
        console.reloaded();
        console.push_debug_message(None, "second".into());
        assert_eq!(debug_messages(&console), ["after", "second"]);

        console.set_auto_clear(true);
        assert_eq!(debug_messages(&console), ["after", "second"]);
        console.clear_debug_messages();
        assert!(console.debug_messages().is_empty());
    }

    #[test]
    fn the_oldest_debug_messages_are_dropped_beyond_the_cap() {
        let mut console = Console::default();
        for index in 0..=Console::MAX_DEBUG_MESSAGES {
            console.push_debug_message(None, index.to_string());
        }
        let messages = console.debug_messages();
        assert_eq!(messages.len(), Console::MAX_DEBUG_MESSAGES);
        assert_eq!(messages.front().unwrap().message, "1");
        assert_eq!(messages.back().unwrap().message, Console::MAX_DEBUG_MESSAGES.to_string());
    }

    #[test]
    fn debug_locations_are_converted_to_urls() {
        let mut console = Console::default();
        let path = std::env::temp_dir().join("a.slint");
        console.push_debug_message(Some((path, 7, 9)), "absolute".into());
        console.push_debug_message(Some(("a.slint".into(), 7, 9)), "relative".into());
        let [absolute, relative] = console.debug_messages().iter().collect::<Vec<_>>()[..] else {
            panic!("{:?}", console.debug_messages())
        };
        assert_eq!(
            absolute.location,
            Some(DebugLocation { url: url("a.slint"), line: 7, column: 9 })
        );
        assert_eq!(relative.location, None);
    }
}
