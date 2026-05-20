// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// Styled text that has been parsed and separated into paragraphs
#[repr(transparent)]
#[derive(Debug, PartialEq, Clone, Default)]
pub struct StyledText {
    /// Paragraphs of styled text
    pub(crate) paragraphs: crate::SharedVector<i_slint_common::styled_text::StyledTextParagraph>,
}

/// Error returned when [`StyledText::from_markdown`] cannot parse the provided markdown input.
#[cfg(feature = "std")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledTextFromMarkdownError {
    message: alloc::string::String,
}

#[cfg(feature = "std")]
impl StyledTextFromMarkdownError {
    fn new(errors: alloc::vec::Vec<i_slint_common::styled_text::StyledTextParseError>) -> Self {
        Self {
            message: errors
                .iter()
                .map(alloc::string::ToString::to_string)
                .collect::<alloc::vec::Vec<_>>()
                .join("\n"),
        }
    }
}

#[cfg(feature = "std")]
impl core::fmt::Display for StyledTextFromMarkdownError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.message)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for StyledTextFromMarkdownError {}

#[cfg(feature = "std")]
impl StyledText {
    /// Creates styled text from plain text without applying markdown parsing.
    pub fn from_plain_text(text: &str) -> Self {
        Self {
            paragraphs: [i_slint_common::styled_text::paragraph_from_plain_text(text.into())]
                .into(),
        }
    }

    /// Parses markdown into styled text.
    pub fn from_markdown(markdown: &str) -> Result<Self, StyledTextFromMarkdownError> {
        let (paragraphs, errors) = i_slint_common::styled_text::parse_interpolated::<
            &[i_slint_common::styled_text::StyledTextParagraph],
        >(markdown, &[]);

        if errors.is_empty() {
            Ok(Self { paragraphs: paragraphs.as_slice().into() })
        } else {
            Err(StyledTextFromMarkdownError::new(errors))
        }
    }
}

pub fn get_raw_text(styled_text: &StyledText) -> alloc::borrow::Cow<'_, str> {
    match styled_text.paragraphs.as_slice() {
        [] => "".into(),
        [paragraph] => paragraph.text.as_str().into(),
        _ => {
            let mut result = alloc::string::String::new();
            for paragraph in styled_text.paragraphs.iter() {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(paragraph.text.as_str());
            }
            result.into()
        }
    }
}

/// Bindings for cbindgen
#[cfg(feature = "ffi")]
pub mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    #[unsafe(no_mangle)]
    /// Create a new default styled text
    pub unsafe extern "C" fn slint_styled_text_new(out: *mut StyledText) {
        unsafe {
            core::ptr::write(out, Default::default());
        }
    }

    #[unsafe(no_mangle)]
    /// Destroy the shared string
    pub unsafe extern "C" fn slint_styled_text_drop(text: *const StyledText) {
        unsafe {
            core::ptr::read(text);
        }
    }

    #[unsafe(no_mangle)]
    /// Returns true if \a a is equal to \a b; otherwise returns false.
    pub extern "C" fn slint_styled_text_eq(a: &StyledText, b: &StyledText) -> bool {
        a == b
    }

    #[unsafe(no_mangle)]
    /// Clone the styled text
    pub unsafe extern "C" fn slint_styled_text_clone(out: *mut StyledText, ss: &StyledText) {
        unsafe { core::ptr::write(out, ss.clone()) }
    }

    #[cfg(feature = "std")]
    #[unsafe(no_mangle)]
    /// Create a styled text from plain text
    pub extern "C" fn slint_styled_text_from_plain_text(
        text: crate::slice::Slice<u8>,
        out: &mut StyledText,
    ) {
        let text = unsafe { core::str::from_utf8_unchecked(text.as_slice()) };
        *out = StyledText::from_plain_text(text);
    }

    #[cfg(feature = "std")]
    #[unsafe(no_mangle)]
    /// Parse markdown into styled text. Returns true on success.
    /// On failure, resets `out` to default.
    pub extern "C" fn slint_styled_text_from_markdown(
        markdown: crate::slice::Slice<u8>,
        out: &mut StyledText,
    ) -> bool {
        let markdown = unsafe { core::str::from_utf8_unchecked(markdown.as_slice()) };
        match StyledText::from_markdown(markdown) {
            Ok(styled) => {
                *out = styled;
                true
            }
            Err(_) => false,
        }
    }
}

pub fn parse_markdown(_format_string: &str, _args: &[StyledText]) -> StyledText {
    #[cfg(feature = "std")]
    {
        let paragraph_slices = _args
            .iter()
            .map(|styled_text| styled_text.paragraphs.as_slice())
            .collect::<alloc::vec::Vec<_>>();

        let (paragraphs, errors) =
            i_slint_common::styled_text::parse_interpolated(_format_string, &paragraph_slices);

        for e in &errors {
            crate::debug_log!("@markdown: {e}");
        }

        StyledText { paragraphs: paragraphs.as_slice().into() }
    }
    #[cfg(not(feature = "std"))]
    Default::default()
}

pub fn color_to_styled_text(_color: crate::Color) -> StyledText {
    #[cfg(feature = "std")]
    {
        let hex = alloc::format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            _color.red(),
            _color.green(),
            _color.blue(),
            _color.alpha()
        );
        StyledText::from_plain_text(&hex)
    }
    #[cfg(not(feature = "std"))]
    Default::default()
}

pub fn string_to_styled_text(_string: alloc::string::String) -> StyledText {
    #[cfg(feature = "std")]
    {
        if _string.is_empty() {
            return Default::default();
        }
        StyledText::from_plain_text(&_string)
    }
    #[cfg(not(feature = "std"))]
    Default::default()
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn string_to_styled_text_returns_default_for_empty_string() {
        assert_eq!(super::string_to_styled_text(Default::default()), StyledText::default());
    }
}
