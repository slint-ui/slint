// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub file: std::path::PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Hash)]
pub struct MessageKey(String, String);

impl MessageKey {
    pub fn new(msgid: String, msgctxt: String) -> Self {
        MessageKey(msgid, msgctxt)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Message {
    pub msgctxt: Option<String>,
    pub msgid: String,
    pub plural: Option<String>,
    pub locations: Vec<Location>,
    pub comments: Option<String>,
    /// that's just keeping the count, so they can be sorted
    pub index: usize,
}

pub type Messages = std::collections::HashMap<MessageKey, Message>;
