// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// This enum represents different types of mouse cursors. It's a subset of the mouse cursors available in CSS.
/// For details and pictograms see the [MDN Documentation for cursor](https://developer.mozilla.org/en-US/docs/Web/CSS/cursor#values).
/// Depending on the backend and used OS unidirectional resize cursors may be replaced with bidirectional ones.
#[non_exhaustive]
#[repr(C, u32)]
#[derive(Debug, Clone, PartialEq, Default)]
pub enum MouseCursor {
    /// The systems default cursor.
    #[default]
    Default,
    /// No cursor is displayed.
    None,
    /// A cursor indicating help information.
    Help,
    /// A pointing hand indicating a link.
    Pointer,
    /// The program is busy but can still be interacted with.
    Progress,
    /// The program is busy.
    Wait,
    /// A crosshair.
    Crosshair,
    /// A cursor indicating selectable text.
    Text,
    /// An alias or shortcut is being created.
    Alias,
    /// A copy is being created.
    Copy,
    /// Something is to be moved.
    Move,
    /// Something can't be dropped here.
    NoDrop,
    /// An action isn't allowed
    NotAllowed,
    /// Something is grabbable.
    Grab,
    /// Something is being grabbed.
    Grabbing,
    /// Indicating that a column is resizable horizontally.
    ColResize,
    /// Indicating that a row is resizable vertically.
    RowResize,
    /// Unidirectional resize north.
    NResize,
    /// Unidirectional resize east.
    EResize,
    /// Unidirectional resize south.
    SResize,
    /// Unidirectional resize west.
    WResize,
    /// Unidirectional resize north-east.
    NeResize,
    /// Unidirectional resize north-west.
    NwResize,
    /// Unidirectional resize south-east.
    SeResize,
    /// Unidirectional resize south-west.
    SwResize,
    /// Bidirectional resize east-west.
    EwResize,
    /// Bidirectional resize north-south.
    NsResize,
    /// Bidirectional resize north-east-south-west.
    NeswResize,
    /// Bidirectional resize north-west-south-east.
    NwseResize,
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
