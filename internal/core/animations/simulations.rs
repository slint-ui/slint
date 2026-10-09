// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Physics simulations that animate a Flickable's content position.
//!
//! `android` decelerates a fling that stops hard at its scroll limit.
//! `bounce` decelerates a fling that may run past its scroll limit, and springs content past the limit back to it.
//! `rubber_band` maps between the drag distance past a limit and the displayed overscroll.
//! `spring` holds the spring math `bounce` builds on.

pub mod android;
pub mod bounce;
pub mod rubber_band;
pub mod spring;

use crate::animations::Instant;

/// The direction the simulation is running
#[derive(Debug)]
enum Direction {
    /// The start value is smaller than the limit value
    Increasing,
    /// The start value is larger than the limit value
    Decreasing,
}

pub trait PositionSimulation {
    /// The signed distance the position still has to move at `now` until the simulation comes
    /// to rest. Not the distance it already moved.
    fn remaining_distance(&self, now: Instant) -> f32;
    fn remaining_velocity(&self, now: Instant) -> f32;
}

/// Common simulation trait
/// All simulations must implement this trait
pub trait Simulation {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool;
}

/// Trait to convert parameter objects into a simulation
/// All parameter objects must implement this trait!
pub trait Parameter {
    type Output;
    fn simulation(
        self,
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    ) -> Self::Output;
}

#[cfg(test)]
macro_rules! assert_approx_eq {
    ($a:expr, $b:expr) => {{
        let (a, b): (f32, f32) = ($a, $b);
        assert!((a - b).abs() < 1e-4 * a.abs().max(b.abs()).max(1.), "{a} != {b}");
    }};
}
#[cfg(test)]
pub(crate) use assert_approx_eq;

#[cfg(test)]
pub(crate) fn test_limit_property(
    value: f32,
) -> core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>> {
    alloc::boxed::Box::pin(crate::Property::new(value))
}
