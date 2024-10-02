// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pupup window handling helpers

use crate::lengths::LogicalRect;

/// A collection of data that might influence the palcement of a `Popup`.
pub struct PlacementData {
    /// The requested popup area
    target_rect: LogicalRect,
}

/// Find a placement for the `Popup`, using the provided `PlacementData`.
pub fn place_popup(data: PlacementData) -> LogicalRect {
    data.target_rect
}

#[cfg(test)]
use crate::lengths::{LogicalPoint, LogicalSize};

#[test]
fn test_place_popup() {
    let rect = LogicalRect::new(LogicalPoint::new(50.0, 100.0), LogicalSize::new(23.0, 42.0));

    let result = place_popup(PlacementData { target_rect: rect.clone() });

    assert_eq!(result, rect);
}
