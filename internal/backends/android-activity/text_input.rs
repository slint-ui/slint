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

use i_slint_core::text_input_controller::TextInputController;
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
    /// * `before_length` - Characters to delete before cursor
    /// * `after_length` - Characters to delete after cursor
    ///
    /// # Returns
    /// true if successful
    pub fn delete_surrounding_text(&self, before_length: usize, after_length: usize) -> bool {
        if let Some(controller) = self.controller() {
            // Convert character counts to byte counts (approximate)
            // The controller will handle boundary validation
            controller.delete_surrounding(before_length, after_length)
        } else {
            log::warn!("delete_surrounding_text called with no focused text input");
            false
        }
    }

    /// Sets the selection range. Called by InputConnection.setSelection().
    ///
    /// # Arguments
    /// * `start` - Selection start (character offset)
    /// * `end` - Selection end (character offset)
    ///
    /// # Returns
    /// true if successful
    pub fn set_selection(&self, start: usize, end: usize) -> bool {
        if let Some(controller) = self.controller() {
            controller.set_selection(start, end)
        } else {
            log::warn!("set_selection called with no focused text input");
            false
        }
    }

    /// Sets the composing region on existing text. Called by InputConnection.setComposingRegion().
    ///
    /// # Arguments
    /// * `start` - Region start (character offset)
    /// * `end` - Region end (character offset)
    ///
    /// # Returns
    /// true if successful
    pub fn set_composing_region(&self, start: usize, end: usize) -> bool {
        if let Some(controller) = self.controller() {
            controller.set_composing_region(Some((start, end)))
        } else {
            log::warn!("set_composing_region called with no focused text input");
            false
        }
    }

    /// Gets the cursor position. Called by InputConnection.getExtractedText().
    ///
    /// # Returns
    /// (cursor_position, selection_start, selection_end) or None if no focus
    pub fn get_cursor_and_selection(&self) -> Option<(usize, usize, usize)> {
        let controller = self.controller()?;
        let cursor = controller.cursor_position();
        let (sel_start, sel_end) = controller.selection();
        Some((cursor, sel_start, sel_end))
    }
}
