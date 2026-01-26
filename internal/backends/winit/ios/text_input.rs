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
        controller.as_ref().and_then(|c| {
            if c.is_valid() {
                Some(c.clone())
            } else {
                None
            }
        })
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
