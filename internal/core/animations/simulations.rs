// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains various physics simulations which can be used as animation (internally only yet).
//! Currently it is used in the flickable to animate the viewport position
//!
//! Currently it contains two simulations:
//! - `ConstantDeceleration`
//! - `ConstantDecelerationSpringDamper` with spring damper simulation when reaching the limit

pub mod constant_deceleration;
pub mod constant_deceleration_spring_damper;
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
    ($a:expr, $b:expr) => {
        assert!(($a - $b).abs() < 1e-4, "{} != {}", $a, $b);
    };
}
#[cfg(test)]
pub(crate) use assert_approx_eq;

#[cfg(test)]
pub(crate) fn test_limit_property(
    value: f32,
) -> core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>> {
    alloc::boxed::Box::pin(crate::Property::new(value))
}
