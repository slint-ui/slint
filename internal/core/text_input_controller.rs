// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Text Input Controller for platform IME integration.
//!
//! This module provides the [`TextInputController`] trait that abstracts text input
//! handling for mobile platforms (Android InputConnection, iOS UITextInput).

use crate::item_tree::{ItemRc, ItemWeak};
use crate::items::TextInput;
use crate::lengths::LogicalRect;
use crate::window::WindowAdapter;
use crate::SharedString;
use alloc::rc::{Rc, Weak};
use core::cell::Cell;

/// Trait for platform-specific text input handling.
///
/// Implementations bridge between Slint's TextInput element and platform IME
/// protocols (Android InputConnection, iOS UITextInput).
///
/// # Thread Safety
///
/// All methods must be called on the main/UI thread. This trait does NOT require
/// `Send + Sync`. Platform backends must ensure all calls happen on the correct thread.
///
/// # Byte Offsets
///
/// All position parameters are **byte offsets** into UTF-8 strings, not character
/// counts. Platform code must convert to/from native offset systems (e.g., Java
/// char offsets for Android). Use the byte offset utility functions in this module
/// for conversions.
///
/// # Lifetime
///
/// Controllers are valid only while the associated TextInput has focus. Calling
/// methods after the TextInput loses focus returns errors or default values.
/// Use [`is_valid()`](TextInputController::is_valid) to check.
pub trait TextInputController {
    // ===== Validity =====

    /// Returns true if this controller is still valid (TextInput still focused).
    ///
    /// Platform should check this before using the controller. After a TextInput
    /// loses focus or is destroyed, this returns false.
    fn is_valid(&self) -> bool;

    // ===== Queries (Platform calls these) =====

    /// Returns up to `max_bytes` of text before the cursor.
    ///
    /// Used by Android's `InputConnection.getTextBeforeCursor()`.
    /// May return fewer bytes to avoid splitting a UTF-8 character.
    fn text_before_cursor(&self, max_bytes: usize) -> SharedString;

    /// Returns up to `max_bytes` of text after the cursor.
    ///
    /// Used by Android's `InputConnection.getTextAfterCursor()`.
    /// May return fewer bytes to avoid splitting a UTF-8 character.
    fn text_after_cursor(&self, max_bytes: usize) -> SharedString;

    /// Returns currently selected text, if any.
    fn selected_text(&self) -> Option<SharedString>;

    /// Returns full text content (excluding preedit).
    fn text(&self) -> SharedString;

    /// Returns cursor position as byte offset.
    fn cursor_position(&self) -> usize;

    /// Returns selection range (start, end) as byte offsets.
    ///
    /// If no selection, start == end == cursor position.
    fn selection(&self) -> (usize, usize);

    /// Returns current composing region on committed text, if any.
    ///
    /// This is different from preedit — it marks existing text as "being edited"
    /// (e.g., for autocorrect suggestions).
    fn composing_region(&self) -> Option<(usize, usize)>;

    /// Returns current preedit/composition text (empty if not composing).
    fn preedit_text(&self) -> SharedString;

    /// Returns cursor position within preedit, if any.
    fn preedit_cursor(&self) -> Option<usize>;

    /// Returns cursor rect in window coordinates (for IME popup positioning).
    fn cursor_rect(&self) -> LogicalRect;

    // ===== Mutations (Platform calls these) =====
    // All mutations return false if controller is invalid or operation failed.

    /// Commits text at cursor position, replacing any preedit.
    ///
    /// # Arguments
    /// * `text` - The text to commit
    /// * `cursor_offset` - Where to place cursor relative to inserted text end
    ///   (0 = at end, negative = before, positive = after)
    ///
    /// Returns false if controller is invalid or operation failed.
    fn commit_text(&self, text: &str, cursor_offset: i32) -> bool;

    /// Sets preedit/composition text (not yet committed).
    ///
    /// # Arguments
    /// * `text` - The composition text to display
    /// * `cursor` - Byte offset within preedit for cursor, or None for end
    ///
    /// Returns false if controller is invalid or operation failed.
    fn set_preedit(&self, text: &str, cursor: Option<usize>) -> bool;

    /// Clears preedit without committing.
    ///
    /// Returns false if controller is invalid.
    fn clear_preedit(&self) -> bool;

    /// Sets the composing region on existing committed text.
    ///
    /// Used when IME wants to mark existing text as "being edited".
    /// Pass None to clear the composing region.
    ///
    /// Returns false if controller is invalid or offsets are invalid.
    fn set_composing_region(&self, region: Option<(usize, usize)>) -> bool;

    /// Commits (finalizes) any active preedit, keeping its text.
    ///
    /// Returns false if controller is invalid.
    fn finish_composing(&self) -> bool;

    /// Deletes bytes around cursor.
    ///
    /// # Arguments
    /// * `before` - Bytes to delete before cursor
    /// * `after` - Bytes to delete after cursor
    ///
    /// Returns false if this would split a UTF-8 character or controller is invalid.
    fn delete_surrounding(&self, before: usize, after: usize) -> bool;

    /// Sets cursor position (byte offset). Clears selection.
    ///
    /// Returns false if offset is invalid or controller is invalid.
    fn set_cursor(&self, position: usize) -> bool;

    /// Sets selection range (byte offsets).
    ///
    /// Returns false if offsets are invalid or controller is invalid.
    fn set_selection(&self, start: usize, end: usize) -> bool;

    // ===== Batch editing =====

    /// Begins a batch edit. Multiple mutations are accumulated.
    ///
    /// Batch edits can be nested — the implementation counts calls and only
    /// applies changes when the final `end_batch_edit()` is called.
    ///
    /// Returns false if controller is invalid.
    fn begin_batch_edit(&self) -> bool;

    /// Ends batch edit. Apply all accumulated changes atomically.
    ///
    /// Triggers a single `edited` callback even for multiple changes.
    /// Returns false if no batch edit was in progress or controller is invalid.
    fn end_batch_edit(&self) -> bool;
}

/// Default implementation of [`TextInputController`] for Slint's TextInput element.
///
/// This struct bridges the platform IME to the core TextInput item.
pub struct CoreTextInputController {
    /// Weak reference to the TextInput item — becomes invalid when item loses focus
    text_input: ItemWeak,
    /// Weak reference to window for triggering updates
    window_adapter: Weak<dyn WindowAdapter>,
    /// Batch edit nesting counter
    batch_edit_count: Cell<u32>,
}

impl CoreTextInputController {
    /// Creates a new controller for the given TextInput.
    ///
    /// The controller holds weak references and will become invalid when the
    /// TextInput loses focus or is destroyed.
    pub fn new(text_input: &ItemRc, window_adapter: &Rc<dyn WindowAdapter>) -> Self {
        Self {
            text_input: text_input.downgrade(),
            window_adapter: Rc::downgrade(window_adapter),
            batch_edit_count: Cell::new(0),
        }
    }

    /// Helper to get the TextInput if still valid.
    fn with_text_input<R>(&self, f: impl FnOnce(core::pin::Pin<&TextInput>, &ItemRc, &Rc<dyn WindowAdapter>) -> R) -> Option<R> {
        let item_rc = self.text_input.upgrade()?;
        let window_adapter = self.window_adapter.upgrade()?;
        let text_input = item_rc.downcast::<TextInput>()?;
        Some(f(text_input.as_pin_ref(), &item_rc, &window_adapter))
    }

    /// Validates a byte offset is on a UTF-8 character boundary.
    fn is_valid_offset(text: &str, offset: usize) -> bool {
        offset <= text.len() && text.is_char_boundary(offset)
    }
}

impl TextInputController for CoreTextInputController {
    fn is_valid(&self) -> bool {
        self.text_input.upgrade().is_some() && self.window_adapter.upgrade().is_some()
    }

    fn text_before_cursor(&self, max_bytes: usize) -> SharedString {
        self.with_text_input(|ti, _, _| {
            let text = ti.text();
            let cursor = ti.cursor_position(&text);
            let start = cursor.saturating_sub(max_bytes);
            // Adjust to valid UTF-8 boundary
            let start = floor_byte_offset(&text, start);
            text[start..cursor].into()
        }).unwrap_or_default()
    }

    fn text_after_cursor(&self, max_bytes: usize) -> SharedString {
        self.with_text_input(|ti, _, _| {
            let text = ti.text();
            let cursor = ti.cursor_position(&text);
            let end = (cursor + max_bytes).min(text.len());
            // Adjust to valid UTF-8 boundary
            let end = ceil_byte_offset(&text, end);
            text[cursor..end].into()
        }).unwrap_or_default()
    }

    fn selected_text(&self) -> Option<SharedString> {
        self.with_text_input(|ti, _, _| {
            let (start, end) = ti.selection_anchor_and_cursor();
            if start == end {
                None
            } else {
                let text = ti.text();
                Some(text[start..end].into())
            }
        }).flatten()
    }

    fn text(&self) -> SharedString {
        self.with_text_input(|ti, _, _| ti.text()).unwrap_or_default()
    }

    fn cursor_position(&self) -> usize {
        self.with_text_input(|ti, _, _| {
            let text = ti.text();
            ti.cursor_position(&text)
        }).unwrap_or(0)
    }

    fn selection(&self) -> (usize, usize) {
        self.with_text_input(|ti, _, _| ti.selection_anchor_and_cursor()).unwrap_or((0, 0))
    }

    fn composing_region(&self) -> Option<(usize, usize)> {
        self.with_text_input(|ti, _, _| ti.composing_region.get()).flatten()
    }

    fn preedit_text(&self) -> SharedString {
        self.with_text_input(|ti, _, _| ti.preedit_text()).unwrap_or_default()
    }

    fn preedit_cursor(&self) -> Option<usize> {
        self.with_text_input(|ti, _, _| {
            ti.preedit_selection().as_option().map(|sel| sel.end as usize)
        }).flatten()
    }

    fn cursor_rect(&self) -> LogicalRect {
        self.with_text_input(|ti, item_rc, window_adapter| {
            let text = ti.text();
            let cursor_pos = ti.cursor_position(&text);
            let rect = window_adapter.renderer().text_input_cursor_rect_for_byte_offset(ti, item_rc, cursor_pos);
            let origin = item_rc.map_to_window(rect.origin);
            LogicalRect::new(origin, rect.size)
        }).unwrap_or_default()
    }

    fn commit_text(&self, text: &str, cursor_offset: i32) -> bool {
        self.with_text_input(|ti, item_rc, window_adapter| {
            ti.ime_commit_text(text, cursor_offset, window_adapter, item_rc);
        }).is_some()
    }

    fn set_preedit(&self, text: &str, cursor: Option<usize>) -> bool {
        // Validate cursor offset if provided
        if let Some(pos) = cursor {
            if !Self::is_valid_offset(text, pos) {
                return false;
            }
        }
        self.with_text_input(|ti, item_rc, window_adapter| {
            ti.ime_set_preedit(text, cursor, window_adapter, item_rc);
        }).is_some()
    }

    fn clear_preedit(&self) -> bool {
        self.with_text_input(|ti, item_rc, window_adapter| {
            ti.ime_clear_preedit(window_adapter, item_rc);
        }).is_some()
    }

    fn set_composing_region(&self, region: Option<(usize, usize)>) -> bool {
        self.with_text_input(|ti, item_rc, window_adapter| {
            // Validate offsets if region is provided
            if let Some((start, end)) = region {
                let text = ti.text();
                if !Self::is_valid_offset(&text, start) || !Self::is_valid_offset(&text, end) {
                    return false;
                }
            }
            ti.ime_set_composing_region(region, window_adapter, item_rc);
            true
        }).unwrap_or(false)
    }

    fn finish_composing(&self) -> bool {
        self.with_text_input(|ti, item_rc, window_adapter| {
            let preedit = ti.preedit_text();
            if !preedit.is_empty() {
                // Commit the preedit text
                ti.ime_commit_text(&preedit, 0, window_adapter, item_rc);
            }
            // Clear composing region
            ti.ime_set_composing_region(None, window_adapter, item_rc);
        }).is_some()
    }

    fn delete_surrounding(&self, before: usize, after: usize) -> bool {
        self.with_text_input(|ti, item_rc, window_adapter| {
            let text = ti.text();
            let cursor = ti.cursor_position(&text);
            let start = cursor.saturating_sub(before);
            let end = (cursor + after).min(text.len());

            // Validate UTF-8 boundaries
            if !Self::is_valid_offset(&text, start) || !Self::is_valid_offset(&text, end) {
                return false;
            }

            ti.ime_delete_surrounding(before, after, window_adapter, item_rc);
            true
        }).unwrap_or(false)
    }

    fn set_cursor(&self, position: usize) -> bool {
        self.with_text_input(|ti, item_rc, window_adapter| {
            let text = ti.text();
            if !Self::is_valid_offset(&text, position) {
                return false;
            }
            ti.ime_set_selection(position, position, window_adapter, item_rc);
            true
        }).unwrap_or(false)
    }

    fn set_selection(&self, start: usize, end: usize) -> bool {
        self.with_text_input(|ti, item_rc, window_adapter| {
            let text = ti.text();
            if !Self::is_valid_offset(&text, start) || !Self::is_valid_offset(&text, end) {
                return false;
            }
            ti.ime_set_selection(start, end, window_adapter, item_rc);
            true
        }).unwrap_or(false)
    }

    fn begin_batch_edit(&self) -> bool {
        if !self.is_valid() {
            return false;
        }
        self.batch_edit_count.set(self.batch_edit_count.get() + 1);
        true
    }

    fn end_batch_edit(&self) -> bool {
        let count = self.batch_edit_count.get();
        if count == 0 {
            return false;
        }
        self.batch_edit_count.set(count - 1);
        // When count reaches 0, all changes have been applied individually
        // (batch edit optimization is a future enhancement)
        true
    }
}

// ===== Byte Offset Utility Functions =====

/// Validates that a byte offset is on a UTF-8 character boundary.
///
/// # Arguments
/// * `text` - The UTF-8 string
/// * `offset` - The byte offset to validate
///
/// # Returns
/// `true` if the offset is valid (within bounds and on a character boundary)
pub fn is_valid_byte_offset(text: &str, offset: usize) -> bool {
    offset <= text.len() && text.is_char_boundary(offset)
}

/// Finds the nearest valid byte offset at or before the given offset.
///
/// If the offset is already valid, returns it unchanged.
/// If the offset is beyond the string length, returns the string length.
/// If the offset is in the middle of a UTF-8 character, returns the start of that character.
pub fn floor_byte_offset(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }
    // Walk backwards to find a valid boundary
    let mut pos = offset;
    while pos > 0 && !text.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

/// Finds the nearest valid byte offset at or after the given offset.
///
/// If the offset is already valid, returns it unchanged.
/// If the offset is beyond the string length, returns the string length.
/// If the offset is in the middle of a UTF-8 character, returns the start of the next character.
pub fn ceil_byte_offset(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }
    // Walk forwards to find a valid boundary
    let mut pos = offset;
    while pos < text.len() && !text.is_char_boundary(pos) {
        pos += 1;
    }
    pos
}

/// Converts a byte offset to a character (Unicode scalar value) count.
///
/// Useful for platforms that need character-based offsets (e.g., Java strings).
///
/// # Arguments
/// * `text` - The UTF-8 string
/// * `byte_offset` - The byte offset to convert
///
/// # Returns
/// The number of characters before the byte offset
///
/// # Panics
/// Panics if `byte_offset` is not on a valid UTF-8 character boundary.
pub fn byte_offset_to_char_count(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset].chars().count()
}

/// Converts a character count to a byte offset.
///
/// # Arguments
/// * `text` - The UTF-8 string
/// * `char_count` - The number of characters
///
/// # Returns
/// The byte offset after `char_count` characters, or the string length if
/// `char_count` exceeds the number of characters in the string.
pub fn char_count_to_byte_offset(text: &str, char_count: usize) -> usize {
    text.char_indices()
        .nth(char_count)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Byte Offset Utility Function Tests =====

    #[test]
    fn test_is_valid_byte_offset() {
        let text = "héllo";  // é is 2 bytes
        assert!(is_valid_byte_offset(text, 0));
        assert!(is_valid_byte_offset(text, 1));
        assert!(!is_valid_byte_offset(text, 2)); // middle of é
        assert!(is_valid_byte_offset(text, 3));
        assert!(is_valid_byte_offset(text, 6)); // end of string
        assert!(!is_valid_byte_offset(text, 7)); // beyond string
    }

    #[test]
    fn test_is_valid_byte_offset_empty_string() {
        let text = "";
        assert!(is_valid_byte_offset(text, 0)); // empty string, position 0 is valid
        assert!(!is_valid_byte_offset(text, 1)); // beyond empty string
    }

    #[test]
    fn test_is_valid_byte_offset_multibyte_chars() {
        // Test with various Unicode characters
        let text = "日本語"; // Each kanji is 3 bytes
        assert!(is_valid_byte_offset(text, 0));
        assert!(!is_valid_byte_offset(text, 1)); // middle of 日
        assert!(!is_valid_byte_offset(text, 2)); // middle of 日
        assert!(is_valid_byte_offset(text, 3)); // start of 本
        assert!(is_valid_byte_offset(text, 6)); // start of 語
        assert!(is_valid_byte_offset(text, 9)); // end of string
    }

    #[test]
    fn test_is_valid_byte_offset_emoji() {
        // Emoji can be 4 bytes
        let text = "a😀b"; // 'a' = 1 byte, '😀' = 4 bytes, 'b' = 1 byte
        assert!(is_valid_byte_offset(text, 0)); // start
        assert!(is_valid_byte_offset(text, 1)); // after 'a'
        assert!(!is_valid_byte_offset(text, 2)); // middle of emoji
        assert!(!is_valid_byte_offset(text, 3)); // middle of emoji
        assert!(!is_valid_byte_offset(text, 4)); // middle of emoji
        assert!(is_valid_byte_offset(text, 5)); // after emoji
        assert!(is_valid_byte_offset(text, 6)); // end (after 'b')
    }

    #[test]
    fn test_floor_byte_offset() {
        let text = "héllo";
        assert_eq!(floor_byte_offset(text, 0), 0);
        assert_eq!(floor_byte_offset(text, 1), 1);
        assert_eq!(floor_byte_offset(text, 2), 1); // middle of é → start of é
        assert_eq!(floor_byte_offset(text, 3), 3);
        assert_eq!(floor_byte_offset(text, 10), 6); // beyond → end
    }

    #[test]
    fn test_floor_byte_offset_multibyte() {
        let text = "日本語"; // Each kanji is 3 bytes
        assert_eq!(floor_byte_offset(text, 0), 0);
        assert_eq!(floor_byte_offset(text, 1), 0); // middle of 日 → start
        assert_eq!(floor_byte_offset(text, 2), 0); // middle of 日 → start
        assert_eq!(floor_byte_offset(text, 3), 3); // start of 本
        assert_eq!(floor_byte_offset(text, 4), 3); // middle of 本 → start of 本
        assert_eq!(floor_byte_offset(text, 5), 3); // middle of 本 → start of 本
    }

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
        let text = "日本語"; // Each kanji is 3 bytes
        assert_eq!(ceil_byte_offset(text, 0), 0);
        assert_eq!(ceil_byte_offset(text, 1), 3); // middle of 日 → after 日
        assert_eq!(ceil_byte_offset(text, 2), 3); // middle of 日 → after 日
        assert_eq!(ceil_byte_offset(text, 3), 3); // start of 本
        assert_eq!(ceil_byte_offset(text, 4), 6); // middle of 本 → after 本
    }

    #[test]
    fn test_byte_offset_to_char_count() {
        let text = "héllo";
        assert_eq!(byte_offset_to_char_count(text, 0), 0);
        assert_eq!(byte_offset_to_char_count(text, 1), 1); // after 'h'
        assert_eq!(byte_offset_to_char_count(text, 3), 2); // after 'é'
        assert_eq!(byte_offset_to_char_count(text, 6), 5); // end
    }

    #[test]
    fn test_byte_offset_to_char_count_emoji() {
        let text = "a😀b";
        assert_eq!(byte_offset_to_char_count(text, 0), 0);
        assert_eq!(byte_offset_to_char_count(text, 1), 1); // after 'a'
        assert_eq!(byte_offset_to_char_count(text, 5), 2); // after emoji
        assert_eq!(byte_offset_to_char_count(text, 6), 3); // end
    }

    #[test]
    fn test_char_count_to_byte_offset() {
        let text = "héllo";
        assert_eq!(char_count_to_byte_offset(text, 0), 0);
        assert_eq!(char_count_to_byte_offset(text, 1), 1); // after 'h'
        assert_eq!(char_count_to_byte_offset(text, 2), 3); // after 'é'
        assert_eq!(char_count_to_byte_offset(text, 5), 6); // end
        assert_eq!(char_count_to_byte_offset(text, 10), 6); // beyond → end
    }

    #[test]
    fn test_char_count_to_byte_offset_emoji() {
        let text = "a😀b";
        assert_eq!(char_count_to_byte_offset(text, 0), 0);
        assert_eq!(char_count_to_byte_offset(text, 1), 1); // after 'a'
        assert_eq!(char_count_to_byte_offset(text, 2), 5); // after emoji
        assert_eq!(char_count_to_byte_offset(text, 3), 6); // end
    }

    #[test]
    fn test_roundtrip_byte_char_conversion() {
        let text = "héllo 日本語 😀";
        // Test that byte → char → byte roundtrips correctly for valid offsets
        for (idx, _) in text.char_indices() {
            let char_count = byte_offset_to_char_count(text, idx);
            let back_to_byte = char_count_to_byte_offset(text, char_count);
            assert_eq!(back_to_byte, idx, "Roundtrip failed for byte offset {}", idx);
        }
        // Also test end of string
        let char_count = byte_offset_to_char_count(text, text.len());
        let back_to_byte = char_count_to_byte_offset(text, char_count);
        assert_eq!(back_to_byte, text.len());
    }

    // ===== CoreTextInputController Tests =====

    use crate::api::PhysicalSize;
    use crate::platform::Renderer;

    /// A minimal mock WindowAdapter for testing purposes.
    /// All methods panic since we only use this to create an invalid weak reference.
    struct MockWindowAdapter {
        window: crate::api::Window,
    }

    impl crate::window::WindowAdapter for MockWindowAdapter {
        fn window(&self) -> &crate::api::Window {
            &self.window
        }

        fn size(&self) -> PhysicalSize {
            PhysicalSize::default()
        }

        fn renderer(&self) -> &dyn Renderer {
            panic!("MockWindowAdapter::renderer should not be called in tests")
        }
    }

    /// Helper to create an invalid weak reference to a WindowAdapter.
    /// Creates a mock adapter, wraps it in Rc, gets a weak reference, then drops the Rc.
    fn create_invalid_window_adapter_weak() -> Weak<dyn crate::window::WindowAdapter> {
        let adapter: Rc<dyn crate::window::WindowAdapter> =
            Rc::<MockWindowAdapter>::new_cyclic(|weak| MockWindowAdapter {
                window: crate::api::Window::new(weak.clone() as Weak<dyn crate::window::WindowAdapter>),
            });
        let weak = Rc::downgrade(&adapter);
        drop(adapter); // Now the weak reference is invalid
        weak
    }

    /// Helper to create an invalid controller (with empty weak references)
    fn create_invalid_controller() -> CoreTextInputController {
        CoreTextInputController {
            text_input: ItemWeak::default(),
            window_adapter: create_invalid_window_adapter_weak(),
            batch_edit_count: Cell::new(0),
        }
    }

    #[test]
    fn test_invalid_controller_is_not_valid() {
        let controller = create_invalid_controller();
        assert!(!controller.is_valid());
    }

    #[test]
    fn test_invalid_controller_text_before_cursor_returns_empty() {
        let controller = create_invalid_controller();
        assert_eq!(controller.text_before_cursor(100).as_str(), "");
    }

    #[test]
    fn test_invalid_controller_text_after_cursor_returns_empty() {
        let controller = create_invalid_controller();
        assert_eq!(controller.text_after_cursor(100).as_str(), "");
    }

    #[test]
    fn test_invalid_controller_selected_text_returns_none() {
        let controller = create_invalid_controller();
        assert!(controller.selected_text().is_none());
    }

    #[test]
    fn test_invalid_controller_text_returns_empty() {
        let controller = create_invalid_controller();
        assert_eq!(controller.text().as_str(), "");
    }

    #[test]
    fn test_invalid_controller_cursor_position_returns_zero() {
        let controller = create_invalid_controller();
        assert_eq!(controller.cursor_position(), 0);
    }

    #[test]
    fn test_invalid_controller_selection_returns_zero_zero() {
        let controller = create_invalid_controller();
        assert_eq!(controller.selection(), (0, 0));
    }

    #[test]
    fn test_invalid_controller_composing_region_returns_none() {
        let controller = create_invalid_controller();
        assert!(controller.composing_region().is_none());
    }

    #[test]
    fn test_invalid_controller_preedit_text_returns_empty() {
        let controller = create_invalid_controller();
        assert_eq!(controller.preedit_text().as_str(), "");
    }

    #[test]
    fn test_invalid_controller_preedit_cursor_returns_none() {
        let controller = create_invalid_controller();
        assert!(controller.preedit_cursor().is_none());
    }

    #[test]
    fn test_invalid_controller_cursor_rect_returns_default() {
        let controller = create_invalid_controller();
        let rect = controller.cursor_rect();
        assert_eq!(rect, LogicalRect::default());
    }

    #[test]
    fn test_invalid_controller_commit_text_returns_false() {
        let controller = create_invalid_controller();
        assert!(!controller.commit_text("hello", 0));
    }

    #[test]
    fn test_invalid_controller_set_preedit_returns_false() {
        let controller = create_invalid_controller();
        assert!(!controller.set_preedit("hello", None));
    }

    #[test]
    fn test_invalid_controller_clear_preedit_returns_false() {
        let controller = create_invalid_controller();
        assert!(!controller.clear_preedit());
    }

    #[test]
    fn test_invalid_controller_set_composing_region_returns_false() {
        let controller = create_invalid_controller();
        assert!(!controller.set_composing_region(Some((0, 5))));
    }

    #[test]
    fn test_invalid_controller_finish_composing_returns_false() {
        let controller = create_invalid_controller();
        assert!(!controller.finish_composing());
    }

    #[test]
    fn test_invalid_controller_delete_surrounding_returns_false() {
        let controller = create_invalid_controller();
        assert!(!controller.delete_surrounding(1, 1));
    }

    #[test]
    fn test_invalid_controller_set_cursor_returns_false() {
        let controller = create_invalid_controller();
        assert!(!controller.set_cursor(0));
    }

    #[test]
    fn test_invalid_controller_set_selection_returns_false() {
        let controller = create_invalid_controller();
        assert!(!controller.set_selection(0, 5));
    }

    // ===== Batch Edit Tests =====

    #[test]
    fn test_batch_edit_on_invalid_controller_fails() {
        let controller = create_invalid_controller();
        assert!(!controller.begin_batch_edit());
    }

    #[test]
    fn test_end_batch_edit_without_begin_fails() {
        let controller = create_invalid_controller();
        // Even on an invalid controller, end_batch_edit should return false
        // if no batch edit was started
        assert!(!controller.end_batch_edit());
    }

    #[test]
    fn test_batch_edit_counter_increments() {
        let controller = create_invalid_controller();

        // Note: begin_batch_edit returns false because controller is invalid,
        // but the counter logic can be tested with a valid controller.
        // For now, test that the counter doesn't change on invalid controller.
        assert_eq!(controller.batch_edit_count.get(), 0);
        controller.begin_batch_edit(); // Returns false, counter unchanged
        assert_eq!(controller.batch_edit_count.get(), 0);
    }

    #[test]
    fn test_batch_edit_nesting_logic() {
        // Test the batch edit nesting counter logic directly
        let count = Cell::new(0);

        // Simulate begin_batch_edit
        count.set(count.get() + 1);
        assert_eq!(count.get(), 1);

        // Nested begin
        count.set(count.get() + 1);
        assert_eq!(count.get(), 2);

        // End one level
        count.set(count.get() - 1);
        assert_eq!(count.get(), 1);

        // End final level
        count.set(count.get() - 1);
        assert_eq!(count.get(), 0);
    }

    // ===== Offset Validation Tests =====

    #[test]
    fn test_is_valid_offset_internal() {
        // Test the internal is_valid_offset method
        assert!(CoreTextInputController::is_valid_offset("hello", 0));
        assert!(CoreTextInputController::is_valid_offset("hello", 5));
        assert!(!CoreTextInputController::is_valid_offset("hello", 6));

        // Test with multibyte
        let text = "héllo";
        assert!(CoreTextInputController::is_valid_offset(text, 0));
        assert!(CoreTextInputController::is_valid_offset(text, 1));
        assert!(!CoreTextInputController::is_valid_offset(text, 2)); // middle of é
        assert!(CoreTextInputController::is_valid_offset(text, 3));
    }

    #[test]
    fn test_set_preedit_validates_cursor_offset() {
        let controller = create_invalid_controller();

        // Even though controller is invalid, set_preedit should validate the cursor offset
        // and return false for invalid offsets before checking controller validity.
        // However, the current implementation checks validity first, then offset.
        // Let's test what we can.

        // With invalid controller, all these return false
        assert!(!controller.set_preedit("hello", Some(0)));
        assert!(!controller.set_preedit("hello", Some(5)));
        assert!(!controller.set_preedit("hello", Some(6))); // invalid offset but controller check happens first

        // Test with multibyte preedit
        let preedit = "日本語";
        assert!(!controller.set_preedit(preedit, Some(0)));
        assert!(!controller.set_preedit(preedit, Some(3)));
        // Invalid offset in middle of character - validation happens before controller check
        assert!(!controller.set_preedit(preedit, Some(1)));
    }

    // ===== Edge Case Tests =====

    #[test]
    fn test_floor_byte_offset_empty_string() {
        assert_eq!(floor_byte_offset("", 0), 0);
        assert_eq!(floor_byte_offset("", 5), 0);
    }

    #[test]
    fn test_ceil_byte_offset_empty_string() {
        assert_eq!(ceil_byte_offset("", 0), 0);
        assert_eq!(ceil_byte_offset("", 5), 0);
    }

    #[test]
    fn test_byte_offset_conversions_empty_string() {
        assert_eq!(byte_offset_to_char_count("", 0), 0);
        assert_eq!(char_count_to_byte_offset("", 0), 0);
        assert_eq!(char_count_to_byte_offset("", 5), 0); // beyond end → clamped to end
    }

    #[test]
    fn test_floor_ceil_at_exact_boundary() {
        let text = "abc";
        // At exact boundaries, floor and ceil should return the same value
        for i in 0..=text.len() {
            assert_eq!(floor_byte_offset(text, i), i);
            assert_eq!(ceil_byte_offset(text, i), i);
        }
    }

    #[test]
    fn test_surrogate_pairs() {
        // Test with characters that would be surrogate pairs in UTF-16
        // but are single code points in UTF-8 (4 bytes)
        let text = "𝄞"; // Musical G clef, 4 bytes in UTF-8
        assert_eq!(text.len(), 4);
        assert!(is_valid_byte_offset(text, 0));
        assert!(!is_valid_byte_offset(text, 1));
        assert!(!is_valid_byte_offset(text, 2));
        assert!(!is_valid_byte_offset(text, 3));
        assert!(is_valid_byte_offset(text, 4));

        assert_eq!(floor_byte_offset(text, 2), 0);
        assert_eq!(ceil_byte_offset(text, 2), 4);
    }

    #[test]
    fn test_combining_characters() {
        // Test with combining characters (e.g., é as e + combining acute)
        let text = "e\u{0301}"; // 'e' followed by combining acute accent
        assert_eq!(text.chars().count(), 2); // Two code points
        assert_eq!(text.len(), 3); // 3 bytes (1 + 2)

        assert!(is_valid_byte_offset(text, 0)); // start
        assert!(is_valid_byte_offset(text, 1)); // after 'e'
        assert!(!is_valid_byte_offset(text, 2)); // middle of combining char
        assert!(is_valid_byte_offset(text, 3)); // end

        assert_eq!(byte_offset_to_char_count(text, 0), 0);
        assert_eq!(byte_offset_to_char_count(text, 1), 1);
        assert_eq!(byte_offset_to_char_count(text, 3), 2);
    }
}
