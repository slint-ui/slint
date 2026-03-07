// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! UTF-16 ↔ UTF-8 byte offset conversion utilities for text handling.
//!
//! Slint uses UTF-8 byte offsets internally for text positions. Platform IME
//! protocols (Android InputConnection, iOS UITextInput) use UTF-16 code unit
//! offsets. This module provides conversions between the two encodings.

/// Finds the nearest valid byte offset at or after the given offset.
///
/// Equivalent to [`str::ceil_char_boundary`]. Remove when MSRV >= 1.91.
///
/// If the offset is already valid, returns it unchanged.
/// If the offset is beyond the string length, returns the string length.
/// If the offset is in the middle of a UTF-8 character, returns the start of the next character.
pub fn ceil_byte_offset(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }
    let mut pos = offset;
    while pos < text.len() && !text.is_char_boundary(pos) {
        pos += 1;
    }
    pos
}

// Android (Java) and iOS (NSString) use UTF-16 code unit offsets, while Rust
// strings are UTF-8. These functions convert between the two encodings.
//
// Key differences:
// - ASCII (U+0000-U+007F): 1 UTF-8 byte, 1 UTF-16 code unit
// - BMP (U+0080-U+FFFF): 2-3 UTF-8 bytes, 1 UTF-16 code unit (includes most CJK)
// - Supplementary (U+10000+): 4 UTF-8 bytes, 2 UTF-16 code units (surrogate pair)

/// Converts a UTF-16 code unit offset to a UTF-8 byte offset.
///
/// Returns `None` if the offset is beyond the string or falls inside a
/// surrogate pair. See [`utf16_offset_to_byte_offset_clamped`] for a
/// variant that clamps instead of returning `None`.
///
/// # Examples
/// ```
/// use i_slint_core::unicode_utils::utf16_offset_to_byte_offset;
///
/// // CJK: "日" is 3 UTF-8 bytes, 1 UTF-16 code unit
/// assert_eq!(utf16_offset_to_byte_offset("日本", 1), Some(3));
///
/// // Emoji: "😀" is 4 UTF-8 bytes, 2 UTF-16 code units (surrogate pair)
/// assert_eq!(utf16_offset_to_byte_offset("a😀b", 1), Some(1));
/// assert_eq!(utf16_offset_to_byte_offset("a😀b", 2), None); // inside surrogate pair
/// assert_eq!(utf16_offset_to_byte_offset("a😀b", 3), Some(5));
/// ```
pub fn utf16_offset_to_byte_offset(text: &str, utf16_offset: usize) -> Option<usize> {
    if utf16_offset == 0 {
        return Some(0);
    }

    let mut utf16_count = 0usize;
    for (byte_idx, ch) in text.char_indices() {
        if utf16_count == utf16_offset {
            return Some(byte_idx);
        }
        let ch_utf16_len = ch.len_utf16();
        utf16_count += ch_utf16_len;

        // Check if the target offset falls inside a surrogate pair
        if ch_utf16_len == 2 && utf16_count > utf16_offset {
            return None;
        }
    }

    if utf16_count == utf16_offset {
        return Some(text.len());
    }

    None
}

/// Converts a UTF-8 byte offset to a UTF-16 code unit offset.
///
/// This function panics on invalid input because callers are expected to
/// hold valid byte offsets (e.g. from `TextInput::cursor_position`).
///
/// # Panics
/// Panics if `byte_offset` is not on a valid UTF-8 character boundary or is
/// beyond the string length.
///
/// # Examples
/// ```
/// use i_slint_core::unicode_utils::byte_offset_to_utf16_offset;
///
/// // CJK: "日" is 3 UTF-8 bytes, 1 UTF-16 code unit
/// assert_eq!(byte_offset_to_utf16_offset("日本", 3), 1);
///
/// // Emoji: "😀" is 4 UTF-8 bytes, 2 UTF-16 code units
/// assert_eq!(byte_offset_to_utf16_offset("a😀b", 5), 3);
/// ```
pub fn byte_offset_to_utf16_offset(text: &str, byte_offset: usize) -> usize {
    assert!(
        byte_offset <= text.len() && text.is_char_boundary(byte_offset),
        "byte_offset {} is not a valid UTF-8 boundary in string of length {}",
        byte_offset,
        text.len()
    );

    text[..byte_offset].chars().map(|ch| ch.len_utf16()).sum()
}

/// Converts a UTF-16 code unit offset to a UTF-8 byte offset, clamping to
/// valid boundaries.
///
/// Unlike [`utf16_offset_to_byte_offset`], this function never returns `None`.
/// If the offset falls inside a surrogate pair, it clamps forward to the end
/// of that character (i.e. the next character boundary). If the offset is
/// beyond the string, it clamps to the end.
///
/// # Examples
/// ```
/// use i_slint_core::unicode_utils::utf16_offset_to_byte_offset_clamped;
///
/// // Inside surrogate pair - clamps forward past the character
/// assert_eq!(utf16_offset_to_byte_offset_clamped("a😀b", 2), 5);
///
/// // Beyond string - clamps to end
/// assert_eq!(utf16_offset_to_byte_offset_clamped("hello", 100), 5);
/// ```
pub fn utf16_offset_to_byte_offset_clamped(text: &str, utf16_offset: usize) -> usize {
    let mut utf16_count = 0usize;

    for (byte_idx, ch) in text.char_indices() {
        if utf16_count >= utf16_offset {
            return byte_idx;
        }
        utf16_count += ch.len_utf16();
    }

    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ceil_byte_offset() {
        let text = "héllo";
        assert_eq!(ceil_byte_offset(text, 0), 0);
        assert_eq!(ceil_byte_offset(text, 1), 1);
        assert_eq!(ceil_byte_offset(text, 2), 3); // middle of é → after é
        assert_eq!(ceil_byte_offset(text, 3), 3);
        assert_eq!(ceil_byte_offset(text, 10), 6); // beyond → end
    }

    #[test]
    fn test_ceil_byte_offset_multibyte() {
        let text = "日本語";
        assert_eq!(ceil_byte_offset(text, 1), 3);
        assert_eq!(ceil_byte_offset(text, 2), 3);
        assert_eq!(ceil_byte_offset(text, 3), 3);
        assert_eq!(ceil_byte_offset(text, 4), 6);
    }

    #[test]
    fn test_ceil_byte_offset_empty() {
        assert_eq!(ceil_byte_offset("", 0), 0);
        assert_eq!(ceil_byte_offset("", 5), 0);
    }

    // ===== UTF-16 Conversion Tests =====

    #[test]
    fn test_utf16_to_byte_ascii() {
        let text = "hello";
        assert_eq!(utf16_offset_to_byte_offset(text, 0), Some(0));
        assert_eq!(utf16_offset_to_byte_offset(text, 3), Some(3));
        assert_eq!(utf16_offset_to_byte_offset(text, 5), Some(5));
        assert_eq!(utf16_offset_to_byte_offset(text, 6), None);
    }

    #[test]
    fn test_utf16_to_byte_empty() {
        assert_eq!(utf16_offset_to_byte_offset("", 0), Some(0));
        assert_eq!(utf16_offset_to_byte_offset("", 1), None);
    }

    #[test]
    fn test_utf16_to_byte_bmp() {
        let text = "日本語"; // 3 UTF-8 bytes each, 1 UTF-16 unit each
        assert_eq!(utf16_offset_to_byte_offset(text, 0), Some(0));
        assert_eq!(utf16_offset_to_byte_offset(text, 1), Some(3));
        assert_eq!(utf16_offset_to_byte_offset(text, 2), Some(6));
        assert_eq!(utf16_offset_to_byte_offset(text, 3), Some(9));
        assert_eq!(utf16_offset_to_byte_offset(text, 4), None);
    }

    #[test]
    fn test_utf16_to_byte_accented() {
        let text = "héllo"; // 'é' is 2 UTF-8 bytes, 1 UTF-16 unit
        assert_eq!(utf16_offset_to_byte_offset(text, 1), Some(1));
        assert_eq!(utf16_offset_to_byte_offset(text, 2), Some(3));
        assert_eq!(utf16_offset_to_byte_offset(text, 5), Some(6));
    }

    #[test]
    fn test_utf16_to_byte_emoji() {
        let text = "a😀b"; // emoji is 4 UTF-8 bytes, 2 UTF-16 units
        assert_eq!(utf16_offset_to_byte_offset(text, 0), Some(0));
        assert_eq!(utf16_offset_to_byte_offset(text, 1), Some(1));
        assert_eq!(utf16_offset_to_byte_offset(text, 2), None); // inside surrogate pair
        assert_eq!(utf16_offset_to_byte_offset(text, 3), Some(5));
        assert_eq!(utf16_offset_to_byte_offset(text, 4), Some(6));
    }

    #[test]
    fn test_utf16_to_byte_multiple_emoji() {
        let text = "😀😀";
        assert_eq!(utf16_offset_to_byte_offset(text, 0), Some(0));
        assert_eq!(utf16_offset_to_byte_offset(text, 1), None);
        assert_eq!(utf16_offset_to_byte_offset(text, 2), Some(4));
        assert_eq!(utf16_offset_to_byte_offset(text, 3), None);
        assert_eq!(utf16_offset_to_byte_offset(text, 4), Some(8));
    }

    #[test]
    fn test_utf16_to_byte_mixed() {
        // "a日😀z": UTF-8 = 1+3+4+1=9, UTF-16 = 1+1+2+1=5
        let text = "a日😀z";
        assert_eq!(utf16_offset_to_byte_offset(text, 0), Some(0));
        assert_eq!(utf16_offset_to_byte_offset(text, 1), Some(1));
        assert_eq!(utf16_offset_to_byte_offset(text, 2), Some(4));
        assert_eq!(utf16_offset_to_byte_offset(text, 3), None); // inside emoji
        assert_eq!(utf16_offset_to_byte_offset(text, 4), Some(8));
        assert_eq!(utf16_offset_to_byte_offset(text, 5), Some(9));
    }

    #[test]
    fn test_byte_to_utf16_ascii() {
        let text = "hello";
        assert_eq!(byte_offset_to_utf16_offset(text, 0), 0);
        assert_eq!(byte_offset_to_utf16_offset(text, 3), 3);
        assert_eq!(byte_offset_to_utf16_offset(text, 5), 5);
    }

    #[test]
    fn test_byte_to_utf16_empty() {
        assert_eq!(byte_offset_to_utf16_offset("", 0), 0);
    }

    #[test]
    fn test_byte_to_utf16_bmp() {
        let text = "日本語";
        assert_eq!(byte_offset_to_utf16_offset(text, 0), 0);
        assert_eq!(byte_offset_to_utf16_offset(text, 3), 1);
        assert_eq!(byte_offset_to_utf16_offset(text, 6), 2);
        assert_eq!(byte_offset_to_utf16_offset(text, 9), 3);
    }

    #[test]
    fn test_byte_to_utf16_emoji() {
        let text = "a😀b";
        assert_eq!(byte_offset_to_utf16_offset(text, 0), 0);
        assert_eq!(byte_offset_to_utf16_offset(text, 1), 1);
        assert_eq!(byte_offset_to_utf16_offset(text, 5), 3);
        assert_eq!(byte_offset_to_utf16_offset(text, 6), 4);
    }

    #[test]
    #[should_panic(expected = "is not a valid UTF-8 boundary")]
    fn test_byte_to_utf16_invalid_boundary() {
        byte_offset_to_utf16_offset("日本", 1);
    }

    #[test]
    #[should_panic(expected = "is not a valid UTF-8 boundary")]
    fn test_byte_to_utf16_beyond_string() {
        byte_offset_to_utf16_offset("hello", 10);
    }

    #[test]
    fn test_utf16_clamped_valid() {
        let text = "a😀b";
        assert_eq!(utf16_offset_to_byte_offset_clamped(text, 0), 0);
        assert_eq!(utf16_offset_to_byte_offset_clamped(text, 1), 1);
        assert_eq!(utf16_offset_to_byte_offset_clamped(text, 3), 5);
        assert_eq!(utf16_offset_to_byte_offset_clamped(text, 4), 6);
    }

    #[test]
    fn test_utf16_clamped_surrogate() {
        // Mid-surrogate clamps forward (past the character), matching
        // the original convert_utf16_index_to_utf8 from the Android backend.
        assert_eq!(utf16_offset_to_byte_offset_clamped("a😀b", 2), 5);
    }

    #[test]
    fn test_utf16_clamped_beyond() {
        assert_eq!(utf16_offset_to_byte_offset_clamped("hello", 100), 5);
        assert_eq!(utf16_offset_to_byte_offset_clamped("a😀", 10), 5);
    }

    #[test]
    fn test_utf16_clamped_consecutive_surrogate_pairs() {
        // "😀😀": each emoji is 4 UTF-8 bytes, 2 UTF-16 code units
        let text = "😀😀";
        assert_eq!(utf16_offset_to_byte_offset_clamped(text, 0), 0);
        assert_eq!(utf16_offset_to_byte_offset_clamped(text, 1), 4); // mid-first → after first
        assert_eq!(utf16_offset_to_byte_offset_clamped(text, 2), 4);
        assert_eq!(utf16_offset_to_byte_offset_clamped(text, 3), 8); // mid-second → after second
        assert_eq!(utf16_offset_to_byte_offset_clamped(text, 4), 8);
    }

    #[test]
    fn test_utf16_clamped_empty() {
        assert_eq!(utf16_offset_to_byte_offset_clamped("", 0), 0);
        assert_eq!(utf16_offset_to_byte_offset_clamped("", 5), 0);
    }

    #[test]
    fn test_roundtrip_utf16_byte() {
        let text = "héllo 日本語 😀 world";
        for (idx, _) in text.char_indices() {
            let utf16 = byte_offset_to_utf16_offset(text, idx);
            let back = utf16_offset_to_byte_offset(text, utf16);
            assert_eq!(back, Some(idx), "Roundtrip failed for byte offset {idx} (utf16 {utf16})");
        }
        let utf16 = byte_offset_to_utf16_offset(text, text.len());
        assert_eq!(utf16_offset_to_byte_offset(text, utf16), Some(text.len()));
    }

    #[test]
    fn test_utf16_combining_characters() {
        let text = "e\u{0301}"; // e + combining acute accent
        assert_eq!(text.chars().count(), 2);
        assert_eq!(text.len(), 3); // 1 + 2 UTF-8 bytes

        // UTF-16: 'e' = 1 unit, combining accent = 1 unit
        assert_eq!(utf16_offset_to_byte_offset(text, 0), Some(0));
        assert_eq!(utf16_offset_to_byte_offset(text, 1), Some(1));
        assert_eq!(utf16_offset_to_byte_offset(text, 2), Some(3));

        assert_eq!(byte_offset_to_utf16_offset(text, 0), 0);
        assert_eq!(byte_offset_to_utf16_offset(text, 1), 1);
        assert_eq!(byte_offset_to_utf16_offset(text, 3), 2);
    }
}
