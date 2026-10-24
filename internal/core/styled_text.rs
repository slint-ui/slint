// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// Styled text that has been parsed and separated into paragraphs
///
/// Clones of a StyledText share the same underlying data, so cloning is cheap.
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

/// The text without its markup: the paragraphs joined by `'\n'`, an empty one included.
/// [`raw_text_ranges`] gives each paragraph's byte range in it.
pub fn get_raw_text(styled_text: &StyledText) -> alloc::borrow::Cow<'_, str> {
    match styled_text.paragraphs.as_slice() {
        [] => "".into(),
        [paragraph] => paragraph.text.as_str().into(),
        _ => {
            let mut result = alloc::string::String::new();
            for (index, paragraph) in styled_text.paragraphs.iter().enumerate() {
                if index > 0 {
                    result.push('\n');
                }
                result.push_str(paragraph.text.as_str());
            }
            result.into()
        }
    }
}

/// Each paragraph's byte range in [`get_raw_text`], in order.
pub fn raw_text_ranges(
    styled_text: &StyledText,
) -> impl Iterator<Item = core::ops::Range<usize>> + '_ {
    let mut start = 0;
    styled_text.paragraphs.iter().map(move |paragraph| {
        let range = start..start + paragraph.text.len();
        start = range.end + 1;
        range
    })
}

/// The length of [`get_raw_text`], without joining it.
pub fn raw_text_len(styled_text: &StyledText) -> usize {
    raw_text_ranges(styled_text).last().map_or(0, |range| range.end)
}

/// The paragraph that `offset` into [`get_raw_text`] falls in, with its range there.
/// The `'\n'` after a paragraph counts as its own; `None` past the end of the text.
pub(crate) fn paragraph_at(
    styled_text: &StyledText,
    offset: usize,
) -> Option<(&i_slint_common::styled_text::StyledTextParagraph, core::ops::Range<usize>)> {
    styled_text
        .paragraphs
        .iter()
        .zip(raw_text_ranges(styled_text))
        .find(|(_, range)| range.end >= offset)
}

/// `offset` clamped into [`get_raw_text`] and up to a character boundary, as
/// `str::ceil_char_boundary` does, without joining the text.
pub fn clamp_byte_offset(styled_text: &StyledText, offset: i32) -> usize {
    let Ok(offset) = usize::try_from(offset) else {
        return 0;
    };
    match paragraph_at(styled_text, offset) {
        Some((paragraph, range)) => {
            range.start + paragraph.text.ceil_char_boundary(offset - range.start)
        }
        None => raw_text_len(styled_text),
    }
}

#[cfg(test)]
mod raw_text_tests {
    use super::*;

    fn styled(paragraphs: &[&str]) -> StyledText {
        StyledText {
            paragraphs: paragraphs
                .iter()
                .map(|text| i_slint_common::styled_text::paragraph_from_plain_text((*text).into()))
                .collect(),
        }
    }

    #[test]
    fn raw_text_ranges_count_the_line_breaks() {
        let text = styled(&["café", "", "last"]);
        assert_eq!(get_raw_text(&text), "café\n\nlast");
        assert_eq!(raw_text_ranges(&text).collect::<alloc::vec::Vec<_>>(), [0..5, 6..6, 7..11]);
        assert_eq!(raw_text_len(&text), 11);
        assert_eq!(raw_text_len(&styled(&[])), 0);
    }

    #[test]
    fn clamp_byte_offset_rounds_up_within_the_paragraph() {
        let text = styled(&["café", "", "last"]);
        assert_eq!(clamp_byte_offset(&text, -3), 0);
        assert_eq!(clamp_byte_offset(&text, 4), 5); // inside the é
        assert_eq!(clamp_byte_offset(&text, 5), 5); // the line break
        assert_eq!(clamp_byte_offset(&text, 6), 6); // the empty paragraph
        assert_eq!(clamp_byte_offset(&text, 99), 11);
        assert_eq!(clamp_byte_offset(&styled(&[]), 3), 0);
    }
}

/// Bindings for cbindgen
#[cfg(feature = "ffi")]
pub mod ffi {
    #![allow(unsafe_code)]

    use super::*;
    #[cfg(feature = "std")]
    use alloc::string::String;

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
        *out = StyledText::from_plain_text(&String::from_utf8_lossy(text.as_slice()));
    }

    #[cfg(feature = "std")]
    #[unsafe(no_mangle)]
    /// Parse markdown into styled text. Returns true on success.
    /// On failure, resets `out` to default.
    pub extern "C" fn slint_styled_text_from_markdown(
        markdown: crate::slice::Slice<u8>,
        out: &mut StyledText,
    ) -> bool {
        match StyledText::from_markdown(&String::from_utf8_lossy(markdown.as_slice())) {
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
