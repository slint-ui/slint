// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! iOS text input handling scaffolding.
//!
//! This module provides the infrastructure for integrating with iOS's UITextInput protocol.
//! The full UITextInput implementation is tracked in SLINT-IOS-TXT-001 through TXT-015.
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
//! 1. **SLINT-IOS-TXT-001**: Create SlintTextInputView (UIView subclass adopting UITextInput)
//! 2. **SLINT-IOS-TXT-002**: Implement UITextInput protocol methods
//! 3. **SLINT-IOS-TXT-003**: Bridge UITextInput calls to TextInputController
//! 4. **SLINT-IOS-TXT-004**: Handle marked text (preedit/composition)
//! 5. **SLINT-IOS-TXT-005**: Handle text position and text range conversions
//! 6. **SLINT-IOS-TXT-006**: Implement UITextInputTokenizer for word/paragraph navigation
//! 7. **SLINT-IOS-TXT-007**: Handle keyboard appearance notifications
//! 8. **SLINT-IOS-TXT-008**: Integrate with soft keyboard state API
//! 9. **SLINT-IOS-TXT-009**: Handle dictation and autocorrect
//! 10. **SLINT-IOS-TXT-010**: Support secure text entry for password fields
//!
//! ## Architecture Notes
//!
//! The iOS text input system works as follows:
//! 1. When TextInput gains focus, Slint calls `text_input_focused()` with a controller
//! 2. A hidden UIView (SlintTextInputView) becomes first responder
//! 3. iOS creates a UITextInputContext and queries the view via UITextInput protocol
//! 4. UITextInput method calls are bridged to the stored TextInputController
//! 5. When TextInput loses focus, the view resigns first responder
//!
//! ## UITextInput Protocol
//!
//! Key methods to implement:
//! - `textInRange:` / `replaceRange:withText:` - Text access and modification
//! - `selectedTextRange` / `setSelectedTextRange:` - Selection handling
//! - `markedTextRange` / `setMarkedText:selectedRange:` - Composition/preedit
//! - `positionFromPosition:offset:` - Cursor navigation
//! - `comparePosition:toPosition:` - Position comparison
//! - `textRangeFromPosition:toPosition:` - Range creation

use i_slint_core::text_input_controller::TextInputController;
use std::cell::RefCell;
use std::rc::Rc;

/// Handler for iOS text input / UITextInput integration.
///
/// This struct holds the TextInputController and provides methods for
/// the iOS UITextInput protocol to interact with Slint's text input.
pub struct IOSTextInputHandler {
    /// The current TextInputController, if a TextInput has focus.
    /// This is used by the UITextInput protocol implementation to query and modify text state.
    controller: RefCell<Option<Rc<dyn TextInputController>>>,
}

impl Default for IOSTextInputHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl IOSTextInputHandler {
    /// Creates a new IOSTextInputHandler.
    pub fn new() -> Self {
        Self { controller: RefCell::new(None) }
    }

    /// Called when a TextInput gains focus.
    ///
    /// The controller provides synchronous access to the TextInput's state,
    /// which is needed for iOS's UITextInput protocol.
    pub fn on_text_input_focused(&self, controller: Rc<dyn TextInputController>) {
        log::debug!("IOSTextInputHandler: text input focused");
        *self.controller.borrow_mut() = Some(controller);
        // TODO: Make SlintTextInputView become first responder
    }

    /// Called when the focused TextInput loses focus or is destroyed.
    pub fn on_text_input_unfocused(&self) {
        log::debug!("IOSTextInputHandler: text input unfocused");
        *self.controller.borrow_mut() = None;
        // TODO: Resign first responder from SlintTextInputView
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
    // UITextInput protocol method stubs
    // These will be called from Objective-C when the full implementation is complete
    // ============================================================

    /// Gets text in the specified range. Called by UITextInput textInRange:.
    ///
    /// # Arguments
    /// * `start` - Start position (UTF-16 offset in iOS)
    /// * `end` - End position (UTF-16 offset in iOS)
    ///
    /// # Returns
    /// The text in the range, or empty string if no focus
    pub fn text_in_range(&self, start: usize, end: usize) -> String {
        if let Some(controller) = self.controller() {
            let text = controller.text();
            // Convert UTF-16 offsets to byte offsets
            // For now, assume 1:1 mapping (ASCII). Full implementation needs proper conversion.
            let byte_start = start.min(text.len());
            let byte_end = end.min(text.len());
            if byte_start <= byte_end {
                text[byte_start..byte_end].to_string()
            } else {
                String::new()
            }
        } else {
            log::warn!("text_in_range called with no focused text input");
            String::new()
        }
    }

    /// Replaces text in the specified range. Called by UITextInput replaceRange:withText:.
    ///
    /// # Arguments
    /// * `start` - Start position
    /// * `end` - End position
    /// * `text` - Replacement text
    ///
    /// # Returns
    /// true if successful
    pub fn replace_range(&self, start: usize, end: usize, text: &str) -> bool {
        if let Some(controller) = self.controller() {
            // Set selection to the range, then commit the new text
            controller.set_selection(start, end);
            controller.commit_text(text, 0)
        } else {
            log::warn!("replace_range called with no focused text input");
            false
        }
    }

    /// Gets the selected text range. Called by UITextInput selectedTextRange.
    ///
    /// # Returns
    /// (start, end) of selection, or None if no focus
    pub fn selected_text_range(&self) -> Option<(usize, usize)> {
        let controller = self.controller()?;
        Some(controller.selection())
    }

    /// Sets the selected text range. Called by UITextInput setSelectedTextRange:.
    ///
    /// # Arguments
    /// * `start` - Selection start
    /// * `end` - Selection end
    ///
    /// # Returns
    /// true if successful
    pub fn set_selected_text_range(&self, start: usize, end: usize) -> bool {
        if let Some(controller) = self.controller() {
            controller.set_selection(start, end)
        } else {
            log::warn!("set_selected_text_range called with no focused text input");
            false
        }
    }

    /// Gets the marked (composition/preedit) text range. Called by UITextInput markedTextRange.
    ///
    /// # Returns
    /// (start, end) of marked text, or None if no marked text
    pub fn marked_text_range(&self) -> Option<(usize, usize)> {
        let controller = self.controller()?;
        let preedit = controller.preedit_text();
        if preedit.is_empty() {
            None
        } else {
            let cursor = controller.cursor_position();
            Some((cursor, cursor + preedit.len()))
        }
    }

    /// Sets the marked (composition/preedit) text. Called by UITextInput setMarkedText:selectedRange:.
    ///
    /// # Arguments
    /// * `text` - The marked text
    /// * `selected_start` - Selection start within marked text
    /// * `selected_end` - Selection end within marked text
    ///
    /// # Returns
    /// true if successful
    pub fn set_marked_text(&self, text: &str, selected_start: usize, selected_end: usize) -> bool {
        if let Some(controller) = self.controller() {
            // The selected range within the marked text indicates the cursor position
            let cursor = if selected_start == selected_end {
                Some(selected_start)
            } else {
                Some(selected_end)
            };
            controller.set_preedit(text, cursor)
        } else {
            log::warn!("set_marked_text called with no focused text input");
            false
        }
    }

    /// Unmarks the current marked text (commits it). Called by UITextInput unmarkText.
    ///
    /// # Returns
    /// true if successful
    pub fn unmark_text(&self) -> bool {
        if let Some(controller) = self.controller() {
            controller.finish_composing()
        } else {
            log::warn!("unmark_text called with no focused text input");
            false
        }
    }

    /// Gets the beginning of the document. Called by UITextInput beginningOfDocument.
    ///
    /// # Returns
    /// Position 0 (always)
    pub fn beginning_of_document(&self) -> usize {
        0
    }

    /// Gets the end of the document. Called by UITextInput endOfDocument.
    ///
    /// # Returns
    /// The text length, or 0 if no focus
    pub fn end_of_document(&self) -> usize {
        self.controller().map(|c| c.text().len()).unwrap_or(0)
    }

    /// Inserts text at the insertion point. Called by UITextInput insertText:.
    ///
    /// # Arguments
    /// * `text` - The text to insert
    ///
    /// # Returns
    /// true if successful
    pub fn insert_text(&self, text: &str) -> bool {
        if let Some(controller) = self.controller() {
            // First finish any composition
            controller.finish_composing();
            // Then commit the new text
            controller.commit_text(text, 0)
        } else {
            log::warn!("insert_text called with no focused text input");
            false
        }
    }

    /// Deletes backward from the insertion point. Called by UITextInput deleteBackward.
    ///
    /// # Returns
    /// true if successful
    pub fn delete_backward(&self) -> bool {
        if let Some(controller) = self.controller() {
            // If there's a selection, delete it
            let (start, end) = controller.selection();
            if start != end {
                controller.set_selection(start, end);
                controller.commit_text("", 0)
            } else {
                // Delete one character before cursor
                controller.delete_surrounding(1, 0)
            }
        } else {
            log::warn!("delete_backward called with no focused text input");
            false
        }
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

        fn with_preedit(text: &str, cursor: usize, preedit: &str) -> Self {
            let mock = Self::with_text(text, cursor);
            *mock.preedit.borrow_mut() = preedit.to_string();
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
        let handler = IOSTextInputHandler::new();
        assert!(!handler.has_focus());
        assert!(handler.controller().is_none());
    }

    #[test]
    fn test_default_handler_has_no_focus() {
        let handler = IOSTextInputHandler::default();
        assert!(!handler.has_focus());
    }

    #[test]
    fn test_focus_unfocus_cycle() {
        let handler = IOSTextInputHandler::new();
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
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        handler.on_text_input_focused(mock.clone());
        assert!(handler.has_focus());

        // Invalidate the controller
        mock.set_invalid();
        assert!(!handler.has_focus());
        assert!(handler.controller().is_none());
    }

    // ===== Text Query Tests =====

    #[test]
    fn test_text_in_range() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::with_text("Hello World", 5));

        handler.on_text_input_focused(mock);

        assert_eq!(handler.text_in_range(0, 5), "Hello");
        assert_eq!(handler.text_in_range(6, 11), "World");
        assert_eq!(handler.text_in_range(0, 11), "Hello World");
    }

    #[test]
    fn test_text_in_range_clamped() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::with_text("Hello", 5));

        handler.on_text_input_focused(mock);

        // Range beyond text length should be clamped
        assert_eq!(handler.text_in_range(0, 100), "Hello");
        assert_eq!(handler.text_in_range(3, 100), "lo");
    }

    #[test]
    fn test_text_in_range_reversed() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::with_text("Hello", 5));

        handler.on_text_input_focused(mock);

        // Reversed range should return empty
        assert_eq!(handler.text_in_range(5, 0), "");
    }

    #[test]
    fn test_text_in_range_no_focus() {
        let handler = IOSTextInputHandler::new();
        assert_eq!(handler.text_in_range(0, 5), "");
    }

    #[test]
    fn test_selected_text_range() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::with_selection("Hello World", 2, 7));

        handler.on_text_input_focused(mock);

        let range = handler.selected_text_range();
        assert_eq!(range, Some((2, 7)));
    }

    #[test]
    fn test_selected_text_range_no_selection() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::with_text("Hello", 3));

        handler.on_text_input_focused(mock);

        // No selection means start == end
        let range = handler.selected_text_range();
        assert_eq!(range, Some((3, 3)));
    }

    #[test]
    fn test_selected_text_range_no_focus() {
        let handler = IOSTextInputHandler::new();
        assert!(handler.selected_text_range().is_none());
    }

    #[test]
    fn test_marked_text_range_with_preedit() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::with_preedit("Hello", 5, "World"));

        handler.on_text_input_focused(mock);

        // Marked text range is (cursor, cursor + preedit.len())
        let range = handler.marked_text_range();
        assert_eq!(range, Some((5, 10))); // 5 + 5 bytes of "World"
    }

    #[test]
    fn test_marked_text_range_no_preedit() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::with_text("Hello", 5));

        handler.on_text_input_focused(mock);

        assert!(handler.marked_text_range().is_none());
    }

    #[test]
    fn test_marked_text_range_no_focus() {
        let handler = IOSTextInputHandler::new();
        assert!(handler.marked_text_range().is_none());
    }

    #[test]
    fn test_beginning_of_document() {
        let handler = IOSTextInputHandler::new();
        // Always returns 0, even without focus
        assert_eq!(handler.beginning_of_document(), 0);
    }

    #[test]
    fn test_end_of_document() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::with_text("Hello World", 5));

        handler.on_text_input_focused(mock);
        assert_eq!(handler.end_of_document(), 11);
    }

    #[test]
    fn test_end_of_document_no_focus() {
        let handler = IOSTextInputHandler::new();
        assert_eq!(handler.end_of_document(), 0);
    }

    // ===== Text Mutation Tests =====

    #[test]
    fn test_replace_range() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        handler.on_text_input_focused(mock.clone());

        assert!(handler.replace_range(0, 5, "Hi"));

        // Should first set selection, then commit
        let sel_calls = mock.set_selection_calls.borrow();
        assert_eq!(sel_calls.len(), 1);
        assert_eq!(sel_calls[0], (0, 5));

        let commit_calls = mock.commit_text_calls.borrow();
        assert_eq!(commit_calls.len(), 1);
        assert_eq!(commit_calls[0], ("Hi".to_string(), 0));
    }

    #[test]
    fn test_replace_range_no_focus() {
        let handler = IOSTextInputHandler::new();
        assert!(!handler.replace_range(0, 5, "test"));
    }

    #[test]
    fn test_set_selected_text_range() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        handler.on_text_input_focused(mock.clone());

        assert!(handler.set_selected_text_range(2, 8));
        let calls = mock.set_selection_calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], (2, 8));
    }

    #[test]
    fn test_set_selected_text_range_no_focus() {
        let handler = IOSTextInputHandler::new();
        assert!(!handler.set_selected_text_range(0, 5));
    }

    #[test]
    fn test_set_marked_text_cursor_at_end() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        handler.on_text_input_focused(mock.clone());

        // selected_start == selected_end means cursor at that position
        assert!(handler.set_marked_text("にほん", 3, 3));
        let calls = mock.set_preedit_calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "にほん");
        assert_eq!(calls[0].1, Some(3));
    }

    #[test]
    fn test_set_marked_text_with_selection() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        handler.on_text_input_focused(mock.clone());

        // When there's a selection in marked text, cursor goes to end
        assert!(handler.set_marked_text("test", 1, 3));
        let calls = mock.set_preedit_calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, Some(3)); // selected_end
    }

    #[test]
    fn test_set_marked_text_no_focus() {
        let handler = IOSTextInputHandler::new();
        assert!(!handler.set_marked_text("test", 0, 0));
    }

    #[test]
    fn test_unmark_text() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        handler.on_text_input_focused(mock.clone());

        assert!(handler.unmark_text());
        assert_eq!(mock.finish_composing_calls.get(), 1);
    }

    #[test]
    fn test_unmark_text_no_focus() {
        let handler = IOSTextInputHandler::new();
        assert!(!handler.unmark_text());
    }

    #[test]
    fn test_insert_text() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::new());

        handler.on_text_input_focused(mock.clone());

        assert!(handler.insert_text("Hello"));

        // Should finish composing first, then commit
        assert_eq!(mock.finish_composing_calls.get(), 1);

        let calls = mock.commit_text_calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], ("Hello".to_string(), 0));
    }

    #[test]
    fn test_insert_text_no_focus() {
        let handler = IOSTextInputHandler::new();
        assert!(!handler.insert_text("test"));
    }

    #[test]
    fn test_delete_backward_no_selection() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::with_text("Hello", 5));

        handler.on_text_input_focused(mock.clone());

        assert!(handler.delete_backward());

        // Should delete 1 character before cursor
        let calls = mock.delete_surrounding_calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], (1, 0));
    }

    #[test]
    fn test_delete_backward_with_selection() {
        let handler = IOSTextInputHandler::new();
        let mock = Rc::new(MockTextInputController::with_selection("Hello World", 0, 5));

        handler.on_text_input_focused(mock.clone());

        assert!(handler.delete_backward());

        // Should set selection and commit empty string
        let sel_calls = mock.set_selection_calls.borrow();
        assert_eq!(sel_calls.len(), 1);
        assert_eq!(sel_calls[0], (0, 5));

        let commit_calls = mock.commit_text_calls.borrow();
        assert_eq!(commit_calls.len(), 1);
        assert_eq!(commit_calls[0], ("".to_string(), 0));
    }

    #[test]
    fn test_delete_backward_no_focus() {
        let handler = IOSTextInputHandler::new();
        assert!(!handler.delete_backward());
    }
}
