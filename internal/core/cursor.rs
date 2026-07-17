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
        /// X pixel coordinate of the hotspot from the left edge of the image.
        ///
        /// The value is clamped to the image bounds.
        hotspot_x: i32,
        /// Y pixel coordinate of the hotspot from the top edge of the image.
        ///
        /// The value is clamped to the image bounds.
        hotspot_y: i32,
    },
}

impl Default for MouseCursorInner {
    fn default() -> Self {
        Self::BuiltIn(BuiltInMouseCursor::Default)
    }
}

/// Maps a custom cursor's hotspot from the source image into a buffer rendered at
/// `rendered_size` pixels, clamped to stay inside it.
pub fn scaled_hotspot(hotspot: i32, source_size: u32, rendered_size: u32) -> u32 {
    let scaled = if source_size == 0 {
        0
    } else {
        hotspot as i64 * rendered_size as i64 / source_size as i64
    };
    scaled.clamp(0, rendered_size.saturating_sub(1) as i64) as u32
}
