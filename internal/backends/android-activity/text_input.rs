// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Android text input handling scaffolding.
//!
//! This module provides the infrastructure for integrating with Android's IME system.
//! The full InputConnection implementation is tracked in SLINT-ANDROID-TXT-001 through TXT-015.
//!
//! ## Current Status
//!
//! This is scaffolding only. The current implementation:
//! - Stores the TextInputController when a TextInput gains focus
//! - Clears the controller when focus is lost
//! - Logs stub messages for unimplemented features
//!
//! ## Next Steps (from implementation plan)
//!
//! 1. **SLINT-ANDROID-TXT-001**: Create Java InputConnection implementation
//! 2. **SLINT-ANDROID-TXT-002**: JNI bindings for InputConnection ↔ Rust
//! 3. **SLINT-ANDROID-TXT-003**: Implement getTextBeforeCursor/getTextAfterCursor
//! 4. **SLINT-ANDROID-TXT-004**: Implement commitText
//! 5. **SLINT-ANDROID-TXT-005**: Implement setComposingText/setComposingRegion
//! 6. **SLINT-ANDROID-TXT-006**: Implement deleteSurroundingText
//! 7. **SLINT-ANDROID-TXT-007**: Implement getSelectedText/setSelection
//! 8. **SLINT-ANDROID-TXT-008**: Implement performEditorAction (for IME action button)
//! 9. **SLINT-ANDROID-TXT-009**: Handle keyboard visibility changes
//! 10. **SLINT-ANDROID-TXT-010**: Integrate with soft keyboard state API
//!
//! ## Architecture Notes
//!
//! The Android IME system works as follows:
//! 1. When TextInput gains focus, Slint calls `text_input_focused()` with a controller
//! 2. The controller provides sync access to text state (via TextInputController trait)
//! 3. Android's InputMethodManager creates an InputConnection to our view
//! 4. InputConnection methods call through JNI to the stored controller
//! 5. When TextInput loses focus, `text_input_unfocused()` clears the controller

use i_slint_core::text_input_controller::{
    byte_offset_to_utf16_offset, utf16_offset_to_byte_offset, TextInputController,
};
use std::cell::RefCell;
use std::rc::Rc;

/// Handler for Android text input / IME integration.
///
/// This struct holds the TextInputController and provides methods for
/// the Android InputConnection to interact with Slint's text input.
pub struct AndroidTextInputHandler {
    /// The current TextInputController, if a TextInput has focus.
    /// This is used by the Android InputConnection to query and modify text state.
    controller: RefCell<Option<Rc<dyn TextInputController>>>,
}

impl Default for AndroidTextInputHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl AndroidTextInputHandler {
    /// Creates a new AndroidTextInputHandler.
    pub fn new() -> Self {
        Self { controller: RefCell::new(None) }
    }

    /// Called when a TextInput gains focus.
    ///
    /// The controller provides synchronous access to the TextInput's state,
    /// which is needed for Android's InputConnection interface.
    pub fn on_text_input_focused(&self, controller: Rc<dyn TextInputController>) {
        log::debug!("AndroidTextInputHandler: text input focused");
        *self.controller.borrow_mut() = Some(controller);
    }

    /// Called when the focused TextInput loses focus or is destroyed.
    pub fn on_text_input_unfocused(&self) {
        log::debug!("AndroidTextInputHandler: text input unfocused");
        *self.controller.borrow_mut() = None;
    }

    /// Returns the current controller, if any.
    ///
    /// Returns None if no TextInput is focused or if the controller has been invalidated.
    pub fn controller(&self) -> Option<Rc<dyn TextInputController>> {
        let controller = self.controller.borrow();
        controller.as_ref().and_then(|c| if c.is_valid() { Some(c.clone()) } else { None })
    }

    /// Checks if there is a valid focused text input.
    pub fn has_focus(&self) -> bool {
        self.controller().is_some()
    }

    // ============================================================
    // InputConnection method stubs
    // These will be called from JNI when the full implementation is complete
    // ============================================================

    /// Gets text before the cursor. Called by InputConnection.getTextBeforeCursor().
    ///
    /// # Arguments
    /// * `max_chars` - Maximum number of characters to return
    ///
    /// # Returns
    /// The text before cursor, or empty string if no focus
    pub fn get_text_before_cursor(&self, max_chars: usize) -> String {
        if let Some(controller) = self.controller() {
            // Convert max chars to approximate max bytes (assume worst case UTF-8)
            let max_bytes = max_chars * 4;
            controller.text_before_cursor(max_bytes).to_string()
        } else {
            log::warn!("get_text_before_cursor called with no focused text input");
            String::new()
        }
    }

    /// Gets text after the cursor. Called by InputConnection.getTextAfterCursor().
    ///
    /// # Arguments
    /// * `max_chars` - Maximum number of characters to return
    ///
    /// # Returns
    /// The text after cursor, or empty string if no focus
    pub fn get_text_after_cursor(&self, max_chars: usize) -> String {
        if let Some(controller) = self.controller() {
            let max_bytes = max_chars * 4;
            controller.text_after_cursor(max_bytes).to_string()
        } else {
            log::warn!("get_text_after_cursor called with no focused text input");
            String::new()
        }
    }

    /// Gets the currently selected text. Called by InputConnection.getSelectedText().
    ///
    /// # Returns
    /// The selected text, or None if nothing selected or no focus
    pub fn get_selected_text(&self) -> Option<String> {
        self.controller()?.selected_text().map(|s| s.to_string())
    }

    /// Commits text at the cursor position. Called by InputConnection.commitText().
    ///
    /// # Arguments
    /// * `text` - The text to commit
    /// * `new_cursor_position` - Cursor position relative to text end (1 = after, 0 = at end)
    ///
    /// # Returns
    /// true if successful
    pub fn commit_text(&self, text: &str, new_cursor_position: i32) -> bool {
        if let Some(controller) = self.controller() {
            // Android uses 1-based position relative to committed text end
            // Convert to our offset from end of inserted text
            let cursor_offset = new_cursor_position - 1;
            controller.commit_text(text, cursor_offset)
        } else {
            log::warn!("commit_text called with no focused text input");
            false
        }
    }

    /// Sets composing (preedit) text. Called by InputConnection.setComposingText().
    ///
    /// # Arguments
    /// * `text` - The composing text
    /// * `new_cursor_position` - Cursor position within composing text
    ///
    /// # Returns
    /// true if successful
    pub fn set_composing_text(&self, text: &str, new_cursor_position: i32) -> bool {
        if let Some(controller) = self.controller() {
            // Convert Android's 1-based position to 0-based byte offset
            let cursor = if new_cursor_position > 0 {
                Some(text.len()) // Position after text
            } else {
                Some(0) // Position at start
            };
            controller.set_preedit(text, cursor)
        } else {
            log::warn!("set_composing_text called with no focused text input");
            false
        }
    }

    /// Finishes the composing text. Called by InputConnection.finishComposingText().
    ///
    /// # Returns
    /// true if successful
    pub fn finish_composing_text(&self) -> bool {
        if let Some(controller) = self.controller() {
            controller.finish_composing()
        } else {
            log::warn!("finish_composing_text called with no focused text input");
            false
        }
    }

    /// Deletes text around the cursor. Called by InputConnection.deleteSurroundingText().
    ///
    /// # Arguments
    /// * `before_length` - UTF-16 code units to delete before cursor
    /// * `after_length` - UTF-16 code units to delete after cursor
    ///
    /// # Returns
    /// true if successful
    pub fn delete_surrounding_text(&self, before_length: usize, after_length: usize) -> bool {
        if let Some(controller) = self.controller() {
            let text = controller.text();
            let cursor_bytes = controller.cursor_position();
            let cursor_utf16 = byte_offset_to_utf16_offset(&text, cursor_bytes);

            // Calculate the byte range to delete before cursor
            let before_bytes = if before_length > 0 {
                let target_utf16 = cursor_utf16.saturating_sub(before_length);
                let target_bytes = utf16_offset_to_byte_offset(&text, target_utf16).unwrap_or(0);
                cursor_bytes - target_bytes
            } else {
                0
            };

            // Calculate the byte range to delete after cursor
            let after_bytes = if after_length > 0 {
                let target_utf16 = cursor_utf16 + after_length;
                let target_bytes =
                    utf16_offset_to_byte_offset(&text, target_utf16).unwrap_or(text.len());
                target_bytes - cursor_bytes
            } else {
                0
            };

            controller.delete_surrounding(before_bytes, after_bytes)
        } else {
            log::warn!("delete_surrounding_text called with no focused text input");
            false
        }
    }

    /// Sets the selection range. Called by InputConnection.setSelection().
    ///
    /// # Arguments
    /// * `start` - Selection start (UTF-16 code unit offset)
    /// * `end` - Selection end (UTF-16 code unit offset)
    ///
    /// # Returns
    /// true if successful
    pub fn set_selection(&self, start: usize, end: usize) -> bool {
        if let Some(controller) = self.controller() {
            let text = controller.text();
            // Convert UTF-16 offsets to byte offsets
            let start_bytes = match utf16_offset_to_byte_offset(&text, start) {
                Some(offset) => offset,
                None => {
                    log::warn!("set_selection: invalid start offset {} (inside surrogate pair)", start);
                    return false;
                }
            };
            let end_bytes = match utf16_offset_to_byte_offset(&text, end) {
                Some(offset) => offset,
                None => {
                    log::warn!("set_selection: invalid end offset {} (inside surrogate pair)", end);
                    return false;
                }
            };
            controller.set_selection(start_bytes, end_bytes)
        } else {
            log::warn!("set_selection called with no focused text input");
            false
        }
    }

    /// Sets the composing region on existing text. Called by InputConnection.setComposingRegion().
    ///
    /// # Arguments
    /// * `start` - Region start (UTF-16 code unit offset)
    /// * `end` - Region end (UTF-16 code unit offset)
    ///
    /// # Returns
    /// true if successful
    pub fn set_composing_region(&self, start: usize, end: usize) -> bool {
        if let Some(controller) = self.controller() {
            let text = controller.text();
            // Convert UTF-16 offsets to byte offsets
            let start_bytes = match utf16_offset_to_byte_offset(&text, start) {
                Some(offset) => offset,
                None => {
                    log::warn!("set_composing_region: invalid start offset {}", start);
                    return false;
                }
            };
            let end_bytes = match utf16_offset_to_byte_offset(&text, end) {
                Some(offset) => offset,
                None => {
                    log::warn!("set_composing_region: invalid end offset {}", end);
                    return false;
                }
            };
            controller.set_composing_region(Some((start_bytes, end_bytes)))
        } else {
            log::warn!("set_composing_region called with no focused text input");
            false
        }
    }

    /// Gets the cursor position. Called by InputConnection.getExtractedText().
    ///
    /// # Returns
    /// (cursor_position, selection_start, selection_end) as UTF-16 code unit offsets,
    /// or None if no focus
    pub fn get_cursor_and_selection(&self) -> Option<(usize, usize, usize)> {
        let controller = self.controller()?;
        let text = controller.text();
        let cursor_bytes = controller.cursor_position();
        let (sel_start_bytes, sel_end_bytes) = controller.selection();

        // Convert byte offsets to UTF-16 offsets for Android
        let cursor = byte_offset_to_utf16_offset(&text, cursor_bytes);
        let sel_start = byte_offset_to_utf16_offset(&text, sel_start_bytes);
        let sel_end = byte_offset_to_utf16_offset(&text, sel_end_bytes);

        Some((cursor, sel_start, sel_end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_core::SharedString;
    use i_slint_core::lengths::LogicalRect;
    use std::cell::Cell;

    /// Mock TextInputController for testing the handler.
    struct MockTextInputController {
        valid: Cell<bool>,
        text: RefCell<String>,
        cursor: Cell<usize>,
        selection: Cell<(usize, usize)>,
        preedit: RefCell<String>,
        preedit_cursor: Cell<Option<usize>>,
        composing_region: Cell<Option<(usize, usize)>>,
        // Track method calls for verification
        commit_text_calls: RefCell<Vec<(String, i32)>>,
        set_preedit_calls: RefCell<Vec<(String, Option<usize>)>>,
        finish_composing_calls: Cell<u32>,
        delete_surrounding_calls: RefCell<Vec<(usize, usize)>>,
        set_selection_calls: RefCell<Vec<(usize, usize)>>,
        set_composing_region_calls: RefCell<Vec<Option<(usize, usize)>>>,
    }

    impl MockTextInputController {
        fn new() -> Self {
            Self {
                valid: Cell::new(true),
                text: RefCell::new(String::from("Hello World")),
                cursor: Cell::new(5),         // After "Hello"
                selection: Cell::new((5, 5)), // No selection, cursor at 5
                preedit: RefCell::new(String::new()),
                preedit_cursor: Cell::new(None),
                composing_region: Cell::new(None),
                commit_text_calls: RefCell::new(Vec::new()),
                set_preedit_calls: RefCell::new(Vec::new()),
                finish_composing_calls: Cell::new(0),
                delete_surrounding_calls: RefCell::new(Vec::new()),
                set_selection_calls: RefCell::new(Vec::new()),
                set_composing_region_calls: RefCell::new(Vec::new()),
            }
        }

        fn with_text(text: &str, cursor: usize) -> Self {
            let mock = Self::new();
            *mock.text.borrow_mut() = text.to_string();
            mock.cursor.set(cursor);
            mock.selection.set((cursor, cursor));
            mock
        }

        fn with_selection(text: &str, sel_start: usize, sel_end: usize) -> Self {
            let mock = Self::new();
            *mock.text.borrow_mut() = text.to_string();
            mock.cursor.set(sel_end);
            mock.selection.set((sel_start, sel_end));
            mock
        }

        fn set_invalid(&self) {
            self.valid.set(false);
        }
    }

    impl TextInputController for MockTextInputController {
        fn is_valid(&self) -> bool {
            self.valid.get()
        }

        fn text_before_cursor(&self, max_bytes: usize) -> SharedString {
            let text = self.text.borrow();
            let cursor = self.cursor.get().min(text.len());
            let start = cursor.saturating_sub(max_bytes);
            text[start..cursor].into()
        }

        fn text_after_cursor(&self, max_bytes: usize) -> SharedString {
            let text = self.text.borrow();
            let cursor = self.cursor.get().min(text.len());
            let end = (cursor + max_bytes).min(text.len());
            text[cursor..end].into()
        }

        fn selected_text(&self) -> Option<SharedString> {
            let (start, end) = self.selection.get();
            if start == end {
                None
            } else {
                let text = self.text.borrow();
                Some(text[start..end].into())
            }
        }

        fn text(&self) -> SharedString {
            self.text.borrow().as_str().into()
        }

        fn cursor_position(&self) -> usize {
            self.cursor.get()
        }

        fn selection(&self) -> (usize, usize) {
            self.selection.get()
        }

        fn composing_region(&self) -> Option<(usize, usize)> {
            self.composing_region.get()
        }

        fn preedit_text(&self) -> SharedString {
            self.preedit.borrow().as_str().into()
        }

        fn preedit_cursor(&self) -> Option<usize> {
            self.preedit_cursor.get()
        }

        fn cursor_rect(&self) -> LogicalRect {
            LogicalRect::default()
        }

        fn commit_text(&self, text: &str, cursor_offset: i32) -> bool {
            self.commit_text_calls.borrow_mut().push((text.to_string(), cursor_offset));
            true
        }

        fn set_preedit(&self, text: &str, cursor: Option<usize>) -> bool {
            self.set_preedit_calls.borrow_mut().push((text.to_string(), cursor));
            *self.preedit.borrow_mut() = text.to_string();
            self.preedit_cursor.set(cursor);
            true
        }

        fn clear_preedit(&self) -> bool {
            *self.preedit.borrow_mut() = String::new();
            self.preedit_cursor.set(None);
            true
        }

        fn set_composing_region(&self, region: Option<(usize, usize)>) -> bool {
            self.set_composing_region_calls.borrow_mut().push(region);
            self.composing_region.set(region);
            true
        }

        fn finish_composing(&self) -> bool {
            self.finish_composing_calls.set(self.finish_composing_calls.get() + 1);
            true
        }

        fn delete_surrounding(&self, before: usize, after: usize) -> bool {
            self.delete_surrounding_calls.borrow_mut().push((before, after));
            true
        }

        fn set_cursor(&self, position: usize) -> bool {
            self.cursor.set(position);
            self.selection.set((position, position));
            true
        }

        fn set_selection(&self, start: usize, end: usize) -> bool {
            self.set_selection_calls.borrow_mut().push((start, end));
            self.selection.set((start, end));
            self.cursor.set(end);
            true
        }

        fn begin_batch_edit(&self) -> bool {
            true
        }

        fn end_batch_edit(&self) -> bool {
            true
        }
    }

    // ===== Handler Lifecycle Tests =====

    #[test]
    fn test_new_handler_has_no_focus() {
        let handler = AndroidTextInputHandler::new();
        assert!(!handler.has_focus());
        assert!(handler.controller().is_none());
    }

    #[test]
    fn test_default_handler_has_no_focus() {
        let handler = AndroidTextInputHandler::default();
        assert!(!handler.has_focus());
    }

    #[test]
    fn test_focus_unfocus_cycle() {
        let handler = AndroidTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        // Initially no focus
        assert!(!handler.has_focus());

        // Focus
        handler.on_text_input_focused(mock.clone());
        assert!(handler.has_focus());
        assert!(handler.controller().is_some());

        // Unfocus
        handler.on_text_input_unfocused();
        assert!(!handler.has_focus());
        assert!(handler.controller().is_none());
    }

    #[test]
    fn test_controller_returns_none_when_invalid() {
        let handler = AndroidTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        handler.on_text_input_focused(mock.clone());
        assert!(handler.has_focus());

        // Invalidate the controller
        mock.set_invalid();
        assert!(!handler.has_focus());
        assert!(handler.controller().is_none());
    }

    #[test]
    fn test_replacing_focus() {
        let handler = AndroidTextInputHandler::new();
        let mock1 = Rc::new(MockTextInputController::with_text("First", 5));
        let mock2 = Rc::new(MockTextInputController::with_text("Second", 6));

        handler.on_text_input_focused(mock1);
        assert_eq!(handler.get_text_before_cursor(100), "First");

        // Replace with new controller
        handler.on_text_input_focused(mock2);
        assert_eq!(handler.get_text_before_cursor(100), "Second");
    }

    // ===== Text Query Tests =====

    #[test]
    fn test_get_text_before_cursor() {
        let handler = AndroidTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::with_text("Hello World", 5));

        handler.on_text_input_focused(mock);

        // Request more chars than available before cursor
        assert_eq!(handler.get_text_before_cursor(100), "Hello");

        // Request limited chars (max_chars * 4 = max_bytes)
        assert_eq!(handler.get_text_before_cursor(2), "lo"); // 2*4=8 bytes, but only 5 available
    }

    #[test]
    fn test_get_text_before_cursor_no_focus() {
        let handler = AndroidTextInputHandler::new();
        assert_eq!(handler.get_text_before_cursor(100), "");
    }

    #[test]
    fn test_get_text_after_cursor() {
        let handler = AndroidTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::with_text("Hello World", 5));

        handler.on_text_input_focused(mock);

        // Request more chars than available after cursor
        assert_eq!(handler.get_text_after_cursor(100), " World");
    }

    #[test]
    fn test_get_text_after_cursor_no_focus() {
        let handler = AndroidTextInputHandler::new();
        assert_eq!(handler.get_text_after_cursor(100), "");
    }

    #[test]
    fn test_get_selected_text_with_selection() {
        let handler = AndroidTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::with_selection("Hello World", 0, 5));

        handler.on_text_input_focused(mock);
        assert_eq!(handler.get_selected_text(), Some("Hello".to_string()));
    }

    #[test]
    fn test_get_selected_text_no_selection() {
        let handler = AndroidTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::with_text("Hello World", 5));

        handler.on_text_input_focused(mock);
        assert_eq!(handler.get_selected_text(), None);
    }

    #[test]
    fn test_get_selected_text_no_focus() {
        let handler = AndroidTextInputHandler::new();
        assert_eq!(handler.get_selected_text(), None);
    }

    #[test]
    fn test_get_cursor_and_selection() {
        let handler = AndroidTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::with_selection("Hello World", 2, 7));

        handler.on_text_input_focused(mock);

        let result = handler.get_cursor_and_selection();
        assert!(result.is_some());
        let (cursor, sel_start, sel_end) = result.unwrap();
        assert_eq!(cursor, 7); // Cursor at end of selection
        assert_eq!(sel_start, 2);
        assert_eq!(sel_end, 7);
    }

    #[test]
    fn test_get_cursor_and_selection_no_focus() {
        let handler = AndroidTextInputHandler::new();
        assert!(handler.get_cursor_and_selection().is_none());
    }

    // ===== Text Mutation Tests =====

    #[test]
    fn test_commit_text_cursor_offset_conversion() {
        let handler = AndroidTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        handler.on_text_input_focused(mock.clone());

        // Android uses 1-based position, we convert to 0-based offset
        // new_cursor_position = 1 means after text, offset = 0
        assert!(handler.commit_text("test", 1));
        let calls = mock.commit_text_calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], ("test".to_string(), 0)); // 1 - 1 = 0

        drop(calls);

        // new_cursor_position = 0 means at end of text, offset = -1
        assert!(handler.commit_text("more", 0));
        let calls = mock.commit_text_calls.borrow();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1], ("more".to_string(), -1)); // 0 - 1 = -1
    }

    #[test]
    fn test_commit_text_no_focus() {
        let handler = AndroidTextInputHandler::new();
        assert!(!handler.commit_text("test", 1));
    }

    #[test]
    fn test_set_composing_text_cursor_after() {
        let handler = AndroidTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        handler.on_text_input_focused(mock.clone());

        // new_cursor_position > 0 means cursor at end of preedit
        assert!(handler.set_composing_text("にほん", 1));
        let calls = mock.set_preedit_calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "にほん");
        assert_eq!(calls[0].1, Some(9)); // len of "にほん" in bytes
    }

    #[test]
    fn test_set_composing_text_cursor_at_start() {
        let handler = AndroidTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        handler.on_text_input_focused(mock.clone());

        // new_cursor_position <= 0 means cursor at start
        assert!(handler.set_composing_text("test", 0));
        let calls = mock.set_preedit_calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, Some(0));
    }

    #[test]
    fn test_set_composing_text_no_focus() {
        let handler = AndroidTextInputHandler::new();
        assert!(!handler.set_composing_text("test", 1));
    }

    #[test]
    fn test_finish_composing_text() {
        let handler = AndroidTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        handler.on_text_input_focused(mock.clone());

        assert!(handler.finish_composing_text());
        assert_eq!(mock.finish_composing_calls.get(), 1);
    }

    #[test]
    fn test_finish_composing_text_no_focus() {
        let handler = AndroidTextInputHandler::new();
        assert!(!handler.finish_composing_text());
    }

    #[test]
    fn test_delete_surrounding_text() {
        let handler = AndroidTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        handler.on_text_input_focused(mock.clone());

        assert!(handler.delete_surrounding_text(3, 2));
        let calls = mock.delete_surrounding_calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], (3, 2));
    }

    #[test]
    fn test_delete_surrounding_text_no_focus() {
        let handler = AndroidTextInputHandler::new();
        assert!(!handler.delete_surrounding_text(1, 1));
    }

    #[test]
    fn test_set_selection() {
        let handler = AndroidTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        handler.on_text_input_focused(mock.clone());

        assert!(handler.set_selection(2, 8));
        let calls = mock.set_selection_calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], (2, 8));
    }

    #[test]
    fn test_set_selection_no_focus() {
        let handler = AndroidTextInputHandler::new();
        assert!(!handler.set_selection(0, 5));
    }

    #[test]
    fn test_set_composing_region() {
        let handler = AndroidTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        handler.on_text_input_focused(mock.clone());

        assert!(handler.set_composing_region(3, 7));
        let calls = mock.set_composing_region_calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], Some((3, 7)));
    }

    #[test]
    fn test_set_composing_region_no_focus() {
        let handler = AndroidTextInputHandler::new();
        assert!(!handler.set_composing_region(0, 5));
    }
}
