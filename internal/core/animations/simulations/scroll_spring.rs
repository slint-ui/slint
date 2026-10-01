// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Ported from Flutter's `SpringSimulation` (physics/spring_simulation.dart), which is:
//! Copyright 2014 The Flutter Authors. All rights reserved.
//!
//! Use of the original source is governed by a BSD-style license
//!
//! Original: <https://github.com/flutter/flutter/blob/d6bed8ff6135cdd414f14edc3063f761d47ca846/packages/flutter/lib/src/physics/spring_simulation.dart>

use crate::animations::Instant;
use crate::animations::simulations::rubber_band;
use crate::animations::simulations::spring::{
    SpringParameters, SpringPhysicalParameters, SpringRegime,
};
use crate::animations::simulations::{PositionSimulation, Simulation};

#[cfg(test)]
use crate::animations::simulations::test_limit_property;

const DEFAULT_MASS: f32 = 0.5;
const DEFAULT_STIFFNESS: f32 = 100.;
const DEFAULT_RATIO: f32 = 1.1;

const ZERO_TOLERANCE: f32 = 1e-3;
/// The spring's initial return speed, relative to its raw distance, at zero overscroll.
/// Fitted to a 200-point pull on a UIKit `UIScrollView` (iPhone 13 Pro Max, iOS 27).
const RETURN_RATE: f32 = 2.4422646;

#[derive(Debug)]
pub struct SpringSimulation {
    start_time: Instant,
    traveled: f32,
    data: SpringRegime,
    init_pos: f32,
    viewport_length: f32,
}

impl SpringSimulation {
    /// Springs from `start_value` back to `limit_value`.
    pub fn new_with_default_parameters(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        viewport_length: f32,
    ) -> Self {
        let distance = limit_value.as_ref().get() - start_value;
        let (w_n, zeta) = SpringPhysicalParameters::new_with_damping_ratio(
            DEFAULT_MASS,
            DEFAULT_STIFFNESS,
            DEFAULT_RATIO,
        )
        .to_natural_frequency_and_damping_ratio();
        let (init_pos, velocity) = if viewport_length > 0. {
            let init_pos = rubber_band::uncompress(distance, viewport_length);
            let compression = (1. - distance.abs() / viewport_length).max(0.001);
            (init_pos, -init_pos * RETURN_RATE / compression)
        } else {
            (distance, 0.)
        };

        Self {
            start_time: crate::animations::current_tick(),
            traveled: 0.,
            data: SpringRegime::new(init_pos, velocity, w_n, zeta),
            init_pos,
            viewport_length,
        }
    }

    fn display_travel(&self, raw_travel: f32) -> f32 {
        if self.viewport_length <= 0. {
            return raw_travel;
        }
        let travel = raw_travel.clamp(self.init_pos.min(0.), self.init_pos.max(0.));
        rubber_band::compress(travel, self.viewport_length)
    }

    fn step_internal(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        let t = new_tick.duration_since(self.start_time).as_secs_f32();
        let (new_pos, new_vel) = self.data.evaluate(t);
        let new_traveled = self.display_travel(self.init_pos - new_pos);
        *current += new_traveled - self.traveled;
        self.traveled = new_traveled;

        new_pos.abs() < ZERO_TOLERANCE && new_vel.abs() < ZERO_TOLERANCE
    }
}

impl Simulation for SpringSimulation {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        self.step_internal(current, new_tick)
    }
}

impl PositionSimulation for SpringSimulation {
    fn remaining_distance(&self, time_elapsed: core::time::Duration) -> f32 {
        let position = self.data.current_position(time_elapsed.as_secs_f32());
        self.display_travel(self.init_pos) - self.display_travel(self.init_pos - position)
    }

    fn remaining_velocity(&self, time_elapsed: core::time::Duration) -> f32 {
        let t = time_elapsed.as_secs_f32();
        let velocity = -self.data.current_velocity(t);
        if self.viewport_length <= 0. {
            return velocity;
        }
        let progress = self.init_pos - self.data.current_position(t);
        if progress < self.init_pos.min(0.) || progress > self.init_pos.max(0.) {
            return 0.;
        }
        velocity * rubber_band::compress_slope(progress, self.viewport_length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::simulations::assert_approx_eq;
    use core::time::Duration;

    #[test]
    fn remaining_distance_and_velocity_settle_to_zero() {
        let simulation =
            SpringSimulation::new_with_default_parameters(30., test_limit_property(20.), 774.);
        assert_approx_eq!(simulation.remaining_distance(core::time::Duration::from_secs(10)), 0.);
        assert_approx_eq!(simulation.remaining_velocity(core::time::Duration::from_secs(10)), 0.);
    }

    /// `remaining_velocity` reports the velocity of the content position that `step` moves,
    /// not of the spring's internal, target-relative coordinate. Overscrolled past a lower
    /// limit (`start_value > limit`), the content must move down toward the limit, so its
    /// velocity is negative.
    #[test]
    fn remaining_velocity_points_toward_the_limit_from_above() {
        let simulation =
            SpringSimulation::new_with_default_parameters(30., test_limit_property(20.), 774.);
        for millis in [50, 100, 300] {
            let t = core::time::Duration::from_millis(millis);
            assert!(simulation.remaining_velocity(t) < 0., "{millis}ms");
        }
    }

    /// Mirrors [`remaining_velocity_points_toward_the_limit_from_above`]: overscrolled past an
    /// upper limit (`start_value < limit`), the content must move up toward the limit, so its
    /// velocity is positive.
    #[test]
    fn remaining_velocity_points_toward_the_limit_from_below() {
        let simulation =
            SpringSimulation::new_with_default_parameters(10., test_limit_property(20.), 774.);
        for millis in [50, 100, 300] {
            let t = core::time::Duration::from_millis(millis);
            assert!(simulation.remaining_velocity(t) > 0., "{millis}ms");
        }
    }

    #[test]
    fn remaining_velocity_matches_the_displayed_motion() {
        for start in [-228.642, -92.069, 21.392, 228.642] {
            let simulation =
                SpringSimulation::new_with_default_parameters(start, test_limit_property(0.), 774.);
            assert_approx_eq!(simulation.remaining_distance(Duration::ZERO), -start);
            for millis in [50, 100, 200, 500] {
                let t = Duration::from_millis(millis);
                let dt = Duration::from_micros(100);
                let measured = -(simulation.remaining_distance(t + dt)
                    - simulation.remaining_distance(t - dt))
                    / (2. * dt.as_secs_f32());
                assert!((simulation.remaining_velocity(t) - measured).abs() < 0.2, "{start}");
            }
            assert!(simulation.remaining_distance(Duration::from_secs(10)).abs() < 0.001);
        }
    }

    #[test]
    fn non_positive_viewport_springs_the_displayed_position() {
        for viewport_length in [0., -5.] {
            let simulation = SpringSimulation::new_with_default_parameters(
                30.,
                test_limit_property(20.),
                viewport_length,
            );
            assert_approx_eq!(simulation.remaining_distance(Duration::ZERO), -10.);
            assert_approx_eq!(simulation.remaining_velocity(Duration::ZERO), 0.);
            assert_approx_eq!(simulation.remaining_distance(Duration::from_secs(10)), 0.);
        }
    }
}
