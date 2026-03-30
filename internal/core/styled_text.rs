// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// Styled text that has been parsed and seperated into paragraphs
#[repr(transparent)]
#[derive(Debug, PartialEq, Clone, Default)]
pub struct StyledText {
    /// Paragraphs of styled text
    pub(crate) paragraphs: crate::SharedVector<i_slint_common::styled_text::StyledTextParagraph>,
}

#[cfg(feature = "std")]
impl StyledText {
    pub fn parse_interpolated<S: AsRef<[i_slint_common::styled_text::StyledTextParagraph]>>(
        format_string: &str,
        args: &[S],
    ) -> Result<Self, i_slint_common::styled_text::StyledTextError<'static>> {
        Ok(i_slint_common::styled_text::StyledText::parse_interpolated(format_string, args)?.into())
    }
}

impl AsRef<[i_slint_common::styled_text::StyledTextParagraph]> for StyledText {
    fn as_ref(&self) -> &[i_slint_common::styled_text::StyledTextParagraph] {
        &self.paragraphs
    }
}

impl From<i_slint_common::styled_text::StyledText> for StyledText {
    fn from(styled_text: i_slint_common::styled_text::StyledText) -> Self {
        Self { paragraphs: (&styled_text.paragraphs[..]).into() }
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
        StyledText::parse_interpolated(_format_string, _args).unwrap()
    }
    #[cfg(not(feature = "std"))]
    Default::default()
}

pub fn string_to_styled_text(_string: alloc::string::String) -> StyledText {
    #[cfg(feature = "std")]
    {
        i_slint_common::styled_text::StyledText::from_plain_text(_string).into()
    }
    #[cfg(not(feature = "std"))]
    Default::default()
}
