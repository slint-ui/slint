// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// Styled text that has been parsed and seperated into paragraphs
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
    fn new(error: i_slint_common::styled_text::StyledTextError<'static>) -> Self {
        Self { message: alloc::format!("{error}") }
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
        parse_interpolated_paragraphs::<[i_slint_common::styled_text::StyledTextParagraph; 0]>(
            markdown,
            &[],
        )
        .map(|paragraphs| Self { paragraphs })
        .map_err(StyledTextFromMarkdownError::new)
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
}

pub fn parse_markdown<S: AsRef<[i_slint_common::styled_text::StyledTextParagraph]>>(
    _format_string: &str,
    _args: &[S],
) -> StyledText {
    #[cfg(feature = "std")]
    {
        parse_interpolated_styled_text(_format_string, _args).unwrap_or_default()
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

#[cfg(feature = "std")]
fn parse_interpolated_styled_text<S: AsRef<[i_slint_common::styled_text::StyledTextParagraph]>>(
    format_string: &str,
    args: &[S],
) -> Result<StyledText, StyledTextFromMarkdownError> {
    parse_interpolated_paragraphs(format_string, args)
        .map(|paragraphs| StyledText { paragraphs })
        .map_err(StyledTextFromMarkdownError::new)
}

#[cfg(feature = "std")]
fn parse_interpolated_paragraphs<S: AsRef<[i_slint_common::styled_text::StyledTextParagraph]>>(
    format_string: &str,
    args: &[S],
) -> Result<
    crate::SharedVector<i_slint_common::styled_text::StyledTextParagraph>,
    i_slint_common::styled_text::StyledTextError<'static>,
> {
    i_slint_common::styled_text::parse_interpolated(format_string, args)
        .collect::<Result<crate::SharedVector<_>, _>>()
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn string_to_styled_text_returns_default_for_empty_string() {
        assert_eq!(super::string_to_styled_text(Default::default()), StyledText::default());
    }

    #[test]
    fn parse_markdown_returns_default_on_runtime_parse_error() {
        let multi_paragraph_argument = [
            i_slint_common::styled_text::paragraph_from_plain_text("first".into()),
            i_slint_common::styled_text::paragraph_from_plain_text("second".into()),
        ];

        assert_eq!(
            parse_markdown(
                &format!(
                    "Text: {}",
                    i_slint_common::styled_text::MARKDOWN_INTERPOLATION_PLACEHOLDER
                ),
                &[multi_paragraph_argument],
            ),
            StyledText::default()
        );
    }
}
