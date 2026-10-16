// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Everything related to scrolling in a ListView

use crate::Coord;
use crate::model::repeater::RepeaterLayoutState;

/// The ListView viewport edge that a scrolled-to row ends up at.
#[repr(u8)]
#[derive(Debug, Default, Clone, Copy)]
pub(crate) enum RowAlignment {
    #[default]
    Top,
    Bottom,
}

impl RowAlignment {
    /// The edge for a row at `top` in the viewport, or `None` if the row is fully visible.
    /// If `None` we don't need to scroll because it is fully in view
    pub(super) fn for_row(top: Coord, height: Coord, viewport_height: Coord) -> Option<Self> {
        if top < 0 as Coord || height > viewport_height {
            // 1) Row begins before the viewport begins
            // 2) Row is taller than the viewport height
            (top != 0 as Coord).then_some(Self::Top)
        } else if top + height > viewport_height {
            // Row bottom beyond the viewport height
            Some(Self::Bottom)
        } else {
            None
        }
    }

    /// The signed distance `content-y` has to move to put a row at `top` in the viewport at this edge.
    pub(super) fn distance(self, top: Coord, height: Coord, viewport_height: Coord) -> Coord {
        match self {
            Self::Top => -top,
            Self::Bottom => viewport_height - (top + height),
        }
    }
}

/// A request of `ensure-row-visible` that the next ListView layout pass completes.
#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub(super) struct ScrollToRowRequest {
    pub(super) row: usize,
    pub(super) alignment: RowAlignment,
}

/// The top and height of `row` in content coordinates.
/// `heights` are the heights of the instances from `state.offset` on.
/// Both are estimates for a row without an instance.
pub(super) fn row_geometry(
    state: &RepeaterLayoutState,
    heights: &[Coord],
    row: usize,
) -> (Coord, Coord) {
    // The instance at `offset` is laid out at `anchor_y`, see `update_visible_instances`.
    if row < state.offset {
        (
            state.anchor_y - state.cached_item_height * (state.offset - row) as Coord,
            state.cached_item_height,
        )
    } else if let Some(height) = heights.get(row - state.offset) {
        (state.anchor_y + heights[..row - state.offset].iter().copied().sum::<Coord>(), *height)
    } else {
        let skipped = row - state.offset - heights.len();
        (
            state.anchor_y
                + heights.iter().copied().sum::<Coord>()
                + state.cached_item_height * skipped as Coord,
            state.cached_item_height,
        )
    }
}
