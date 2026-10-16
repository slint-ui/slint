// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Everything related to scrolling in a ListView

// cSpell: ignore Jetpack

use crate::Coord;
use crate::animations::Instant;
use crate::animations::simulations::accelerate::Accelerate;
use crate::animations::simulations::decelerate::Decelerate;
use crate::animations::simulations::{PositionSimulation, Simulation};
use crate::item_tree::ItemWeak;
use crate::items::{Dimension, Flickable};
use crate::model::repeater::{RepeaterInstanceOps, RepeaterLayoutState};
use alloc::rc::Weak;
use alloc::vec::Vec;
use core::cell::RefCell;

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

pub(super) enum ScrollToRowState {
    /// The next layout pass puts the row at the edge.
    Instant(RowAlignment),
    /// The next layout pass starts the smooth scroll, because the ListView wasn't laid out yet.
    PendingSmooth,
    /// The Flickable owns the simulations.
    Accelerating(RowAlignment, Weak<RefCell<Accelerate>>),
    Decelerating(RowAlignment, Weak<RefCell<Decelerate>>),
}

/// The motion of a smooth scroll to a row, scaled to the viewport height.
///
/// The scroll accelerates to a maximum speed,
/// jumps to the landing distance before the row, and decelerates onto it.
/// Jetpack Compose's `animateScrollToItem` jumps too, which keeps the duration short for any distance.
pub(super) struct ScrollToRowMotion {
    pub(super) max_speed: f32,
    pub(super) acceleration: f32,
    pub(super) landing_distance: f32,
    pub(super) min_jump: f32,
}

impl ScrollToRowMotion {
    const MAX_SPEED_IN_VIEWPORTS_PER_SECOND: f32 = 8.;
    const ACCELERATION_DURATION_IN_SECONDS: f32 = 0.25;
    const LANDING_DISTANCE_IN_VIEWPORTS: f32 = 3.;
    const MIN_JUMP_IN_VIEWPORTS: f32 = 1.;

    pub(super) fn new(viewport_height: Coord) -> Self {
        let viewport_height = (viewport_height as f32).max(1.);
        let max_speed = Self::MAX_SPEED_IN_VIEWPORTS_PER_SECOND * viewport_height;
        Self {
            max_speed,
            acceleration: max_speed / Self::ACCELERATION_DURATION_IN_SECONDS,
            landing_distance: Self::LANDING_DISTANCE_IN_VIEWPORTS * viewport_height,
            min_jump: Self::MIN_JUMP_IN_VIEWPORTS * viewport_height,
        }
    }

    pub(super) fn accelerate(&self, distance: f32) -> Accelerate {
        Accelerate::new(distance, self.max_speed, self.acceleration)
    }

    /// The deceleration that stops from the maximum speed within the landing distance.
    pub(super) fn braking_deceleration(&self) -> f32 {
        self.max_speed * self.max_speed / (2. * self.landing_distance)
    }

    /// Whether to jump to the landing distance before a row that is `distance` away.
    pub(super) fn should_jump(&self, distance: f32, accelerating: &Accelerate) -> bool {
        accelerating.is_at_max_speed() && distance.abs() - self.landing_distance >= self.min_jump
    }
}

/// A request of `ensure-row-visible` that the ListView layout completes.
pub(super) struct ScrollToRowRequest {
    pub(super) row: usize,
    pub(super) flickable: ItemWeak,
    pub(super) state: ScrollToRowState,
}

impl ScrollToRowRequest {
    pub(super) fn smooth_alignment(&self) -> Option<RowAlignment> {
        match self.state {
            ScrollToRowState::Accelerating(alignment, _)
            | ScrollToRowState::Decelerating(alignment, _) => Some(alignment),
            ScrollToRowState::Instant(_) | ScrollToRowState::PendingSmooth => None,
        }
    }

    /// Whether another scroll replaced the smooth scroll, a gesture stopped it, or it finished.
    pub(super) fn is_stopped(&self) -> bool {
        /// `state` returns whether the simulation finished, and the tick of its last step.
        fn is_stopped<S>(
            simulation: &Weak<RefCell<S>>,
            state: impl Fn(&S) -> (bool, Instant),
        ) -> bool {
            simulation.upgrade().is_none_or(|simulation| {
                let (finished, last_tick) = state(&simulation.borrow());
                finished || last_tick != crate::animations::current_tick()
            })
        }
        match &self.state {
            ScrollToRowState::Accelerating(_, simulation) => {
                is_stopped(simulation, |s| (s.is_finished(), s.last_tick()))
            }
            ScrollToRowState::Decelerating(_, simulation) => {
                is_stopped(simulation, |s| (s.is_finished(), s.last_tick()))
            }
            ScrollToRowState::Instant(_) | ScrollToRowState::PendingSmooth => false,
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
/// Accelerating -> jumping -> Deccelerating
pub(super) fn advance_scroll_to_row(
    mut request: ScrollToRowRequest,
    top: Coord,
    height: Coord,
    viewport_height: Coord,
    jumped: bool,
) -> Option<ScrollToRowRequest> {
    request.state = match request.state {
        ScrollToRowState::Instant(_) => return None,
        ScrollToRowState::PendingSmooth => {
            let alignment = RowAlignment::for_row(top, height, viewport_height)?;
            let distance = alignment.distance(top, height, viewport_height) as f32;
            let simulation = ScrollToRowMotion::new(viewport_height).accelerate(distance);
            ScrollToRowState::Accelerating(
                alignment,
                start_simulation(&request.flickable, simulation)?,
            )
        }
        ScrollToRowState::Accelerating(alignment, simulation) => {
            let distance = alignment.distance(top, height, viewport_height) as f32;
            let accelerating = simulation.upgrade()?;
            let speed = {
                let mut accelerating = accelerating.borrow_mut();
                let braking_deceleration =
                    ScrollToRowMotion::new(viewport_height).braking_deceleration();
                if !jumped && distance.abs() > accelerating.braking_distance(braking_deceleration) {
                    // If we didn't jump and the target is still far, just update the remaining distance
                    // to go on with the accelerating phase
                    accelerating.set_remaining_distance(distance);
                    return Some(ScrollToRowRequest {
                        state: ScrollToRowState::Accelerating(alignment, simulation),
                        ..request
                    });
                }
                accelerating.speed()
            };
            let simulation = Decelerate::new(distance, speed);
            ScrollToRowState::Decelerating(
                alignment,
                start_simulation(&request.flickable, simulation)?,
            )
        }
        ScrollToRowState::Decelerating(alignment, simulation) => {
            simulation.upgrade()?.borrow_mut().set_remaining_distance(alignment.distance(
                top,
                height,
                viewport_height,
            ) as f32);
            ScrollToRowState::Decelerating(alignment, simulation)
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
    let Some((request, alignment)) = &request.and_then(|r| Some((r, r.smooth_alignment()?))) else {
        // No scroll request ongoing or it is an instant jump
        return SmoothScrollLayout { jump: None, extra_above: 0., layout_end: viewport_height };
    };

    let heights = (0..ops.len())
        .map(|i| ops.height(i).unwrap_or(state.cached_item_height))
        .collect::<Vec<_>>();
    let (top_anchor, row_height) = row_geometry(state, &heights, request.row);
    let distance = alignment.distance(top_anchor + content_y, row_height, viewport_height);

    // The distance of the scroll, estimated unless its row is laid out,
    // and the edge that the row ends up at if the layout pass jumps.
    // If the acceleration simulation reaches the maximum velocity we jump
    let motion = ScrollToRowMotion::new(viewport_height);
    let landing = motion.landing_distance as Coord;
    let jump = if let ScrollToRowState::Accelerating(_, simulation) = &request.state
        && let Some(simulation) = simulation.upgrade()
        && motion.should_jump(distance as f32, &simulation.borrow())
    {
        Some(*alignment)
    } else {
        None
    };

    // Like Android's LinearLayoutManager during a smooth scroll,
    // lay out the rows up to the target, so that the scroll stops exactly on it.
    let (extra_above, extra_below) = match jump {
        Some(RowAlignment::Top) => (landing, 0 as Coord),
        Some(RowAlignment::Bottom) => (0 as Coord, landing + viewport_height),
        None if distance.abs() <= landing + viewport_height => {
            if distance > 0 as Coord {
                (distance + viewport_height, 0 as Coord)
            } else {
                (0 as Coord, viewport_height - distance)
            }
        }
        _ => (0 as Coord, 0 as Coord),
    };
    SmoothScrollLayout {
        jump: jump.map(|alignment| (alignment, landing)),
        extra_above,
        layout_end: viewport_height + extra_below,
    }
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
