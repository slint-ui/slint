// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::items::BuiltInMouseCursor;

/// This enum represents different types of mouse cursors. It's a subset of the mouse cursors available in CSS.
/// For details and pictograms see the [MDN Documentation for cursor](https://developer.mozilla.org/en-US/docs/Web/CSS/cursor#values).
/// Depending on the backend and used OS unidirectional resize cursors may be replaced with bidirectional ones.
#[repr(C, u32)]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum MouseCursorInner {
    /// One of the built-in mouse cursors.
    BuiltIn(BuiltInMouseCursor),
    /// Custom cursor from an `Image`.
    CustomMouseCursor {
        /// Image backing for this cursor.
        image: crate::graphics::Image,
        /// X pixel coordinate of the image relative to where the cursor is, starting from the left.
        ///
        /// If this value is negative, the hotspot is horizontally centered in the image.
        hotspot_x: i32,
        /// Y pixel coordinate of the image relative to where the cursor is, starting from the top.
        ///
        /// If this value is negative, the hotspot is vertically centered in the image.
        hotspot_y: i32,
    },
}

impl Default for MouseCursorInner {
    fn default() -> Self {
        Self::BuiltIn(BuiltInMouseCursor::Default)
    }
}

