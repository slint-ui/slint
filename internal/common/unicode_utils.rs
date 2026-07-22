// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! UTF-16 ↔ UTF-8 offset conversion utilities.
//!
//! Slint uses UTF-8 byte offsets internally. Platform protocols and language
//! servers often use UTF-16 code unit offsets. This module converts between
//! the two without allocating.

/// Converts a UTF-8 byte offset to a UTF-16 code unit offset.
///
/// `byte_offset` must lie on a valid UTF-8 character boundary within `text`.
/// In debug builds an assertion fires for invalid offsets.
pub fn byte_offset_to_utf16_offset(text: &str, byte_offset: usize) -> usize {
    debug_assert!(
        text.is_char_boundary(byte_offset),
        "byte_offset {byte_offset} is not on a UTF-8 character boundary"
    );
    text[..byte_offset.min(text.len())].chars().map(|c| c.len_utf16()).sum()
}

/// Converts a UTF-16 code unit offset to a UTF-8 byte offset.
///
/// If the offset falls in the middle of a surrogate pair or beyond the end of
/// the string, it is clamped to the next character boundary or `text.len()`.
pub fn utf16_offset_to_byte_offset_clamped(text: &str, utf16_offset: usize) -> usize {
    let mut counter = 0;
    for (idx, c) in text.char_indices() {
        if counter >= utf16_offset {
            return idx;
        }
        counter += c.len_utf16();
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byte_to_utf16() {
        let cases: &[(&str, usize, usize)] = &[
            ("hello", 0, 0),
            ("hello", 3, 3), // ASCII: byte == UTF-16 code unit
            ("hello", 5, 5),
            ("", 0, 0),
            ("日本語", 0, 0),
            ("日本語", 3, 1), // BMP: 3 bytes → 1 code unit
            ("日本語", 6, 2),
            ("日本語", 9, 3),
            ("a😀b", 0, 0),
            ("a😀b", 1, 1),
            ("a😀b", 5, 3), // emoji: 4 bytes → 2 code units
            ("a😀b", 6, 4),
        ];
        for &(text, byte_col, expected) in cases {
            assert_eq!(
                byte_offset_to_utf16_offset(text, byte_col),
                expected,
                "byte_offset_to_utf16_offset({text:?}, {byte_col})"
            );
        }
    }

    #[test]
    fn test_utf16_to_byte_clamped() {
        let cases: &[(&str, usize, usize)] = &[
            ("hello", 0, 0),
            ("hello", 3, 3), // ASCII
            ("hello", 5, 5),
            ("hello", 100, 5), // beyond end → clamped to text.len()
            ("", 0, 0),
            ("", 5, 0),
            ("日本語", 0, 0),
            ("日本語", 1, 3), // BMP
            ("日本語", 2, 6),
            ("日本語", 3, 9),
            ("a😀b", 0, 0),
            ("a😀b", 1, 1),
            ("a😀b", 2, 5), // mid-surrogate → clamp past emoji
            ("a😀b", 3, 5),
            ("a😀b", 4, 6),
        ];
        for &(text, utf16_col, expected) in cases {
            assert_eq!(
                utf16_offset_to_byte_offset_clamped(text, utf16_col),
                expected,
                "utf16_offset_to_byte_offset_clamped({text:?}, {utf16_col})"
            );
        }
    }

    #[test]
    fn test_roundtrip() {
        let text = "héllo 日本語 😀 world"; // cspell:disable-line
        for (byte_idx, _) in text.char_indices() {
            let utf16 = byte_offset_to_utf16_offset(text, byte_idx);
            assert_eq!(utf16_offset_to_byte_offset_clamped(text, utf16), byte_idx);
        }
        let utf16 = byte_offset_to_utf16_offset(text, text.len());
        assert_eq!(utf16_offset_to_byte_offset_clamped(text, utf16), text.len());
    }
}
