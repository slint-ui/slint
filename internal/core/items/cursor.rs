// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::items::MouseCursor;

/// This enum represents different types of mouse cursors. It's a subset of the mouse cursors available in CSS.
/// For details and pictograms see the [MDN Documentation for cursor](https://developer.mozilla.org/en-US/docs/Web/CSS/cursor#values).
/// Depending on the backend and used OS unidirectional resize cursors may be replaced with bidirectional ones.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum MouseCursorInner {
    BuiltIn(MouseCursor),
    /// Custom cursor from an `Image`.
    CustomCursor {
        /// Image backing for this cursor.
        image: crate::graphics::Image,
        /// Hotspot X.
        hotspot_x: i32,
        /// Hotspot Y.
        hotspot_y: i32,
    },
}

impl Default for MouseCursorInner {
    fn default() -> Self {
        Self::BuiltIn(MouseCursor::Default)
    }
}
