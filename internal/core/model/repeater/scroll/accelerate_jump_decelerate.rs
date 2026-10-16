// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A simulation which simulates a scroll to with an acceleration at the beginning then a jump and then
//! a deceleration to keep the simulation short even when the simulation is to a far away row

// cSpell: ignore Jetpack

use crate::Coord;
use crate::animations::simulations::accelerate::Accelerate;
use crate::animations::simulations::decelerate::Decelerate;
use crate::item_tree::ItemWeak;
use crate::model::repeater::scroll::RowAlignment;
use alloc::rc::Weak;
use core::cell::RefCell;

#[derive(Debug, Clone)]
enum Phase {
    /// The next layout pass starts the smooth scroll, because the ListView wasn't laid out yet.
    PendingSmooth,
    /// The Flickable owns the simulations.
    Accelerating(RowAlignment, Weak<RefCell<Accelerate>>),
    Decelerating(RowAlignment, Weak<RefCell<Decelerate>>),
}

pub struct AccelerateJumpDecelerate {
    phase: Phase,
}

impl AccelerateJumpDecelerate {
    pub fn new() -> Self {
        Self { phase: Phase::PendingSmooth }
    }

    pub fn alignment(&self) -> Option<RowAlignment> {
        match self.phase {
            Phase::Accelerating(alignment, _) | Phase::Decelerating(alignment, _) => {
                Some(alignment)
            }
            _ => None,
        }
    }

    pub(super) fn is_stopped(&self) -> bool {
        match &self.phase {
            Phase::Accelerating(_, simulation) => {
                simulation.upgrade().is_none_or(|s| s.borrow().is_finished())
            }
            Phase::Decelerating(_, simulation) => {
                simulation.upgrade().is_none_or(|s| s.borrow().is_finished())
            }
            Phase::PendingSmooth => false,
        }
    }

    /// Whether the Flickable still owns a simulation of this scroll.
    pub(super) fn has_simulation(&self) -> bool {
        match &self.phase {
            Phase::Accelerating(_, simulation) => simulation.strong_count() > 0,
            Phase::Decelerating(_, simulation) => simulation.strong_count() > 0,
            Phase::PendingSmooth => false,
        }
    }

    pub fn should_jump(&self, viewport_height: Coord, distance: Coord) -> Option<Coord> {
        // The distance of the scroll, estimated unless its row is laid out,
        // and the edge that the row ends up at if the layout pass jumps.
        // If the acceleration simulation reaches the maximum velocity we jump
        let motion = ScrollToRowMotion::new(viewport_height);
        let landing = motion.landing_distance as Coord;
        let jump = if let Phase::Accelerating(_, simulation) = &self.phase
            && let Some(simulation) = simulation.upgrade()
            && motion.should_jump(distance as f32, &simulation.borrow())
        {
            Some(landing)
        } else {
            None
        };
        jump
    }

    /// Is used to determine how large is the instantiation span.
    /// If this value is large a lot of instances get instantiated even
    /// they are not visible.
    /// When jumping, this is the start point of the deceleration before the desired row
    pub fn lookahead_limit(&self, viewport_height: Coord) -> Coord {
        ScrollToRowMotion::new(viewport_height).landing_distance as Coord + viewport_height
    }

    /// Accelerating -> jumping -> Decelerating
    #[must_use]
    pub fn advance(
        &mut self,
        viewport_height: Coord,
        top: Coord,
        row_height: Coord,
        alignment: RowAlignment,
        distance: Coord,

        jumped: bool,
        flickable: &ItemWeak,
    ) -> Option<()> {
        match self.phase.clone() {
            Phase::PendingSmooth => {
                let simulation = ScrollToRowMotion::new(viewport_height).accelerate(distance);
                self.phase =
                    Phase::Accelerating(alignment, super::start_simulation(flickable, simulation)?);
            }
            Phase::Accelerating(alignment, simulation) => {
                let accelerating = simulation.upgrade()?;
                let speed = {
                    let mut accelerating = accelerating.borrow_mut();
                    let braking_deceleration =
                        ScrollToRowMotion::new(viewport_height).braking_deceleration();
                    if !jumped
                        && distance.abs() > accelerating.braking_distance(braking_deceleration)
                    {
                        // If we didn't jump and the target is still far, just update the remaining distance
                        // to go on with the accelerating phase
                        accelerating.set_remaining_distance(distance);
                        return Some(());
                    }
                    accelerating.speed()
                };
                let simulation = Decelerate::new(distance, speed);
                self.phase =
                    Phase::Decelerating(alignment, super::start_simulation(flickable, simulation)?);
            }
            Phase::Decelerating(alignment, simulation) => {
                simulation.upgrade()?.borrow_mut().set_remaining_distance(alignment.distance(
                    top,
                    row_height,
                    viewport_height,
                ) as f32);
                self.phase = Phase::Decelerating(alignment, simulation);
            }
        }
        Some(())
    }
}

/// The motion of a smooth scroll to a row, scaled to the viewport height.
///
/// The scroll accelerates to a maximum speed,
/// jumps to the landing distance before the row, and decelerates onto it.
/// Jetpack Compose's `animateScrollToItem` jumps too, which keeps the duration short for any distance.
pub(super) struct ScrollToRowMotion {
    pub(super) max_speed: f32,
    pub(super) acceleration: f32,
    /// The distance before the desired row to land and starting the deceleration (when scrolling up).
    /// For scrolling down it is the distance after.
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
