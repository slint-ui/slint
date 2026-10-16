// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Everything related to scrolling in a ListView

use crate::Coord;
use crate::animations::simulations::{PositionSimulation, Simulation};
use crate::item_tree::ItemWeak;
use crate::items::{Dimension, Flickable};
use crate::model::repeater::scroll::accelerate_jump_decelerate::AccelerateJumpDecelerate;
use crate::model::repeater::{RepeaterInstanceOps, RepeaterLayoutState};
use alloc::rc::Weak;
use alloc::vec::Vec;
use core::cell::RefCell;
pub mod accelerate_jump_decelerate;

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

pub(super) enum ScrollAnimation {
    /// The next layout pass puts the row at the edge.
    Instant(RowAlignment),
    AccelerateJumpDecelerate(AccelerateJumpDecelerate),
}

/// A request of `ensure-row-visible` that the ListView layout completes.
pub(super) struct ScrollToRowRequest {
    pub(super) row: usize,
    pub(super) flickable: ItemWeak,
    pub(super) animation: ScrollAnimation,
}

impl ScrollToRowRequest {
    pub(super) fn smooth_alignment(&self) -> Option<RowAlignment> {
        match &self.animation {
            ScrollAnimation::AccelerateJumpDecelerate(sim) => sim.alignment(),
            ScrollAnimation::Instant(_) => None,
        }
    }

    /// Whether another scroll replaced the smooth scroll, a gesture stopped it, or it finished.
    pub(super) fn is_stopped(&self) -> bool {
        match &self.animation {
            ScrollAnimation::AccelerateJumpDecelerate(sim) => sim.is_stopped(),
            ScrollAnimation::Instant(_) => false,
        }
    }
}

/// Starts `simulation` on the `content-y` of `flickable`, which owns it.
pub(super) fn start_simulation<S: Simulation + PositionSimulation + 'static>(
    flickable: &ItemWeak,
    simulation: S,
) -> Option<Weak<RefCell<S>>> {
    let flickable_rc = flickable.upgrade()?;
    let flickable = flickable_rc.downcast::<Flickable>()?;
    Some(flickable.as_pin_ref().start_content_simulation(&flickable_rc, simulation, Dimension::Y))
}

/// Determine the new request state of the scroll to state machine
pub(super) fn advance_scroll_to_row(
    mut request: ScrollToRowRequest,
    top: Coord,
    row_height: Coord,
    viewport_height: Coord,
    jumped: bool,
) -> Option<ScrollToRowRequest> {
    match &mut request.animation {
        ScrollAnimation::Instant(_) => return None,
        ScrollAnimation::AccelerateJumpDecelerate(sim) => {
            let alignment = RowAlignment::for_row(top, row_height, viewport_height)?;
            let distance = alignment.distance(top, row_height, viewport_height) as f32;
            sim.advance(
                viewport_height,
                top,
                row_height,
                alignment,
                distance,
                jumped,
                &request.flickable,
            )?;
        }
    };
    Some(request)
}

/// How a layout pass lays out the rows for a smooth scroll.
pub(super) struct SmoothScrollLayout {
    /// The edge that the row ends up at if the layout pass jumps, and its distance beyond that edge.
    pub(super) jump: Option<(RowAlignment, Coord)>,
    /// How far above the viewport to lay out rows.
    pub(super) extra_above: Coord,
    /// How far below the viewport's top edge to lay out rows.
    pub(super) layout_end: Coord,
}

/// If there is no scroll to request we handle it as a normal scroll
/// if one is ongoing, plan the steps for this layout pass for smooth scrolling (like determining
/// if we should jump and where to land)
pub(super) fn smooth_scroll_layout(
    request: Option<&ScrollToRowRequest>,
    state: &RepeaterLayoutState,
    ops: &impl RepeaterInstanceOps,
    content_y: Coord,
    viewport_height: Coord,
) -> SmoothScrollLayout {
    let Some((request, alignment)) = request.and_then(|r| Some((r, r.smooth_alignment()?))) else {
        // No scroll request ongoing or it is an instant jump
        return SmoothScrollLayout { jump: None, extra_above: 0., layout_end: viewport_height };
    };

    let heights = (0..ops.len())
        .map(|i| ops.height(i).unwrap_or(state.cached_item_height))
        .collect::<Vec<_>>();
    let (top_anchor, row_height) = row_geometry(state, &heights, request.row);
    let distance = alignment.distance(top_anchor + content_y, row_height, viewport_height);

    let (jump, lookahead_limit) = match &request.animation {
        ScrollAnimation::Instant(_) => {
            debug_assert!(false, "Is already captured above. So it should never executed");
            (None, 0 as Coord)
        }
        ScrollAnimation::AccelerateJumpDecelerate(sim) => (
            sim.should_jump(viewport_height, distance).map(|landing| (alignment, landing)),
            sim.lookahead_limit(viewport_height),
        ),
    };

    // lay out the rows up to the target, so that the scroll stops exactly on it.
    let (extra_above, extra_below) = match jump {
        Some((RowAlignment::Top, landing)) => (landing, 0 as Coord),
        Some((RowAlignment::Bottom, landing)) => (0 as Coord, landing + viewport_height),
        // If the distance is not too large
        None if distance.abs() <= lookahead_limit => {
            if distance > 0 as Coord {
                (distance + viewport_height, 0 as Coord)
            } else {
                (0 as Coord, viewport_height - distance)
            }
        }
        _ => (0 as Coord, 0 as Coord),
    };
    SmoothScrollLayout { jump, extra_above, layout_end: viewport_height + extra_below }
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
