// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Ported from Flutter's `SpringSimulation` (physics/spring_simulation.dart), which is:
//! Copyright 2014 The Flutter Authors. All rights reserved.
//!
//! Use of the original source is governed by a BSD-style license
//!
//! Original: <https://github.com/flutter/flutter/blob/d6bed8ff6135cdd414f14edc3063f761d47ca846/packages/flutter/lib/src/physics/spring_simulation.dart>

use core::time::Duration;

use crate::animations::Instant;
use crate::animations::simulations::spring::SpringRegime;
use crate::animations::simulations::{PositionSimulation, Simulation};

#[cfg(test)]
use crate::animations::simulations::test_limit_property;

const ZERO_TOLERANCE: f32 = 1e-3;

// The return constants are fitted to UIKit `UIScrollView` returns after a held pull from the top
// edge, captured on an iPhone 13 Pro Max with iOS 27.
/// The natural frequency of the critically damped return.
const RETURN_FREQUENCY: f32 = 10.67037;
/// How long after the release UIKit's return starts moving.
const RETURN_DELAY: Duration = Duration::from_nanos(16_827_855);
/// The initial return speed relative to the overscroll, for a small overscroll.
const RETURN_RATE_MIN: f32 = 5.470122;
/// How much the initial return rate rises for a large overscroll.
const RETURN_RATE_RISE: f32 = 4.825567;
/// The overscroll at which the initial return rate has risen halfway.
const RETURN_RATE_HALF_DISTANCE: f32 = 165.8656;

fn initial_return_rate(distance: f32) -> f32 {
    let squared = distance * distance;
    RETURN_RATE_MIN
        + RETURN_RATE_RISE * squared
            / (RETURN_RATE_HALF_DISTANCE * RETURN_RATE_HALF_DISTANCE + squared)
}

#[derive(Debug)]
pub struct SpringSimulation {
    start_time: Instant,
    traveled: f32,
    data: SpringRegime,
    init_pos: f32,
}

impl SpringSimulation {
    /// Springs from `start_value` back to `limit_value`, like UIKit's return after a held pull.
    pub fn new_with_default_parameters(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    ) -> Self {
        let distance = limit_value.as_ref().get() - start_value;
        let velocity = -distance * initial_return_rate(distance);
        Self {
            start_time: crate::animations::current_tick(),
            traveled: 0.,
            data: SpringRegime::new(distance, velocity, RETURN_FREQUENCY, 1.),
            init_pos: distance,
        }
    }

    fn spring_time(time_elapsed: Duration) -> f32 {
        time_elapsed.saturating_sub(RETURN_DELAY).as_secs_f32()
    }

    fn step_internal(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        let t = Self::spring_time(new_tick.duration_since(self.start_time));
        let (new_pos, new_vel) = self.data.evaluate(t);
        let new_traveled = self.init_pos - new_pos;
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
    fn remaining_distance(&self, time_elapsed: Duration) -> f32 {
        self.data.current_position(Self::spring_time(time_elapsed))
    }

    fn remaining_velocity(&self, time_elapsed: Duration) -> f32 {
        if time_elapsed < RETURN_DELAY {
            return 0.;
        }
        -self.data.current_velocity(Self::spring_time(time_elapsed))
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
            SpringSimulation::new_with_default_parameters(30., test_limit_property(20.));
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
            SpringSimulation::new_with_default_parameters(30., test_limit_property(20.));
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
            SpringSimulation::new_with_default_parameters(10., test_limit_property(20.));
        for millis in [50, 100, 300] {
            let t = core::time::Duration::from_millis(millis);
            assert!(simulation.remaining_velocity(t) > 0., "{millis}ms");
        }
    }

    #[test]
    fn remaining_velocity_matches_the_displayed_motion() {
        for start in [-228.642, -92.069, 21.392, 228.642] {
            let simulation =
                SpringSimulation::new_with_default_parameters(start, test_limit_property(0.));
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
    fn does_not_move_before_the_return_delay() {
        let start = crate::animations::current_tick();
        let mut simulation =
            SpringSimulation::new_with_default_parameters(30., test_limit_property(20.));
        let mut position = 30.;
        simulation.step(&mut position, start + RETURN_DELAY);
        assert_approx_eq!(position, 30.);
        assert_approx_eq!(simulation.remaining_distance(RETURN_DELAY), -10.);
        assert_approx_eq!(simulation.remaining_velocity(RETURN_DELAY / 2), 0.);
        simulation.step(&mut position, start + RETURN_DELAY + Duration::from_millis(8));
        assert!(position < 30.);
    }

    /// Samples of a UIKit return after a held 200-point pull, which ended 92 points past the top.
    /// UIKit reports positions in 1/3-point steps, with up to a frame of sampling jitter.
    #[test]
    fn follows_a_measured_uikit_return() {
        let start = crate::animations::current_tick();
        let mut simulation =
            SpringSimulation::new_with_default_parameters(92., test_limit_property(0.));
        let mut position = 92.;
        for (millis, uikit) in
            [(17, 92.), (25, 87.), (50, 73.), (100, 50.), (150, 33.667), (200, 22.), (300, 10.)]
        {
            simulation.step(&mut position, start + Duration::from_millis(millis));
            assert!((position - uikit).abs() < 1.5, "{millis} ms: {position} != {uikit}");
        }
    }
}
