// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::styled_text::StyledText;

/// Styled text parsed from markdown or plain text.
///
/// Use `StyledText.fromMarkdown()` or `StyledText.fromPlainText()` to create instances.
/// Assign the result to a `styled-text` property in a Slint component to display it.
#[napi(js_name = "StyledText")]
pub struct SlintStyledText {
    pub(crate) inner: StyledText,
}

impl From<StyledText> for SlintStyledText {
    fn from(styled_text: StyledText) -> Self {
        Self { inner: styled_text }
    }
}

#[napi]
impl SlintStyledText {
    /// Creates styled text from plain text without applying markdown parsing.
    #[napi(factory)]
    pub fn from_plain_text(text: String) -> Self {
        Self { inner: StyledText::from_plain_text(&text) }
    }

    /// Parses markdown into styled text.
    ///
    /// @throws {Error} if the markdown contains unsupported syntax.
    #[napi(factory)]
    pub fn from_markdown(markdown: String) -> napi::Result<Self> {
        StyledText::from_markdown(&markdown)
            .map(|st| Self { inner: st })
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    /// Returns `true` if this styled text is equal to `other`.
    #[napi]
    pub fn equals(&self, other: &SlintStyledText) -> bool {
        self.inner == other.inner
    }
}
