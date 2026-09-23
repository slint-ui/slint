// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Velocity-driven flings use the portable AOSP spline in [`spline`].
//! Fixed-distance wheel motion retains the power curve from Flutter's `ClampingScrollSimulation`
//! (scroll_simulation.dart), which is:
//! Copyright 2014 The Flutter Authors. All rights reserved.
//!
//! Use of the original source is governed by a BSD-style license
//!
//! Original: <https://github.com/flutter/flutter/blob/d6bed8ff6135cdd414f14edc3063f761d47ca846/packages/flutter/lib/src/widgets/scroll_simulation.dart>
//!

use crate::animations::Instant;
use crate::animations::simulations::{Direction, Parameter, PositionSimulation, Simulation};
use core::time::Duration;
#[cfg(not(feature = "std"))]
use num_traits::Float;

mod spline;

const DEFAULT_FRICTION: f32 = 0.015;

fn deceleration_rate() -> f32 {
    f32::log10(0.78) / f32::log10(0.9)
}

/// Initial velocity in logical pixels per second and positive fling friction.
#[derive(Debug, Clone)]
pub struct AndroidFlickParameters {
    pub initial_velocity: f32,
    pub friction: f32,
}

impl AndroidFlickParameters {
    pub fn new_with_default_friction(initial_velocity: f32) -> Self {
        Self { initial_velocity, friction: DEFAULT_FRICTION }
    }
}

impl Parameter for AndroidFlickParameters {
    type Output = AndroidFlick;
    fn simulation(
        self,
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    ) -> Self::Output {
        AndroidFlick::new(start_value, limit_value, self)
    }
}

#[derive(Debug)]
pub struct AndroidFlick {
    /// If the limit is not reached, it is also fine. Also exceeding the limit can be ok,
    /// but at the end of the animation the limit shall not be exceeded
    limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    data: AndroidFlickParameters,
    direction: Direction,
    start_time: Instant,
    // Max duration until reaching the end
    duration: Duration,
    // Max distance to travel when running the simulation infinitely
    distance: f32,
    deceleration_rate: f32,
    traveled: f32,
    wheel: bool,
    finished: bool,
}

impl AndroidFlick {
    pub fn new(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        data: AndroidFlickParameters,
    ) -> Self {
        Self::new_internal(start_value, limit_value, data, crate::animations::current_tick())
    }

    fn new_internal(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        mut data: AndroidFlickParameters,
        start_time: Instant,
    ) -> Self {
        let direction = if start_value == limit_value.as_ref().get() {
            if data.initial_velocity >= 0. { Direction::Increasing } else { Direction::Decreasing }
        } else if start_value < limit_value.as_ref().get() {
            debug_assert!(data.initial_velocity >= 0.); // Makes no sense yet that the velocity goes into the other direction
            data.initial_velocity = f32::abs(data.initial_velocity);
            Direction::Increasing
        } else {
            data.initial_velocity = -f32::abs(data.initial_velocity);
            debug_assert!(data.initial_velocity <= 0.);
            Direction::Decreasing
        };
        let deceleration_rate = deceleration_rate();
        let (duration, max_distance) = spline::parameters(data.initial_velocity, data.friction);

        Self {
            limit_value,
            data,
            direction,
            start_time,
            duration,
            distance: max_distance,
            deceleration_rate,
            traveled: 0.,
            wheel: false,
            finished: false,
        }
    }

    /// Fixed-distance wheel scrolling retains the existing power curve.
    pub fn new_with_distance(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        distance: f32,
        duration: Duration,
    ) -> Self {
        let velocity = if duration.is_zero() {
            0.
        } else {
            distance * deceleration_rate() / duration.as_secs_f32()
        };
        let mut result = Self::new(
            start_value,
            limit_value,
            AndroidFlickParameters::new_with_default_friction(velocity),
        );
        result.wheel = true;
        result.distance = distance;
        result.duration = duration;
        result
    }

    fn sample(&self, elapsed: Duration) -> (f32, f32) {
        if self.finished || elapsed >= self.duration {
            return (self.distance, 0.);
        }
        if elapsed.is_zero() {
            return (0., self.data.initial_velocity);
        }
        let t = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        if self.wheel {
            (
                self.distance * (1. - (1. - t).powf(self.deceleration_rate)),
                self.data.initial_velocity * (1. - t).powf(self.deceleration_rate - 1.),
            )
        } else {
            let (position, slope) = spline::sample(t);
            (position * self.distance, slope * self.distance / self.duration.as_secs_f32())
        }
    }

    fn step_internal(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        if self.finished {
            return true;
        }
        let elapsed = new_tick.duration_since(self.start_time);
        let (position, _) = self.sample(elapsed);
        // Apply increments: virtualized views may move the content between frames.
        *current += position - self.traveled;
        self.traveled = position;
        let limit = self.limit_value.as_ref().get();
        let clamped = match self.direction {
            Direction::Increasing => *current >= limit,
            Direction::Decreasing => *current <= limit,
        };
        if clamped {
            *current = limit;
        }
        self.finished = clamped || elapsed >= self.duration;
        self.finished
    }
}

impl Simulation for AndroidFlick {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        self.step_internal(current, new_tick)
    }
}

impl PositionSimulation for AndroidFlick {
    fn remaining_distance(&self, time_elapsed: core::time::Duration) -> f32 {
        self.distance - self.sample(time_elapsed).0
    }

    fn remaining_velocity(&self, time_elapsed: core::time::Duration) -> f32 {
        self.sample(time_elapsed).1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::simulations::test_limit_property;

    #[test]
    fn aosp_reference_trajectories() {
        for line in include_str!("android/aosp_reference.csv").lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let values: alloc::vec::Vec<f32> =
                line.split(',').map(|s| s.parse().unwrap()).collect();
            let [velocity, friction, duration, distance, time, position, speed] = values.as_slice()
            else {
                panic!("Invalid reference row");
            };
            let simulation = AndroidFlick::new_internal(
                0.,
                test_limit_property(velocity.signum() * 1_000_000.),
                AndroidFlickParameters { initial_velocity: *velocity, friction: *friction },
                Instant::default(),
            );
            assert_eq!(simulation.duration.as_millis(), *duration as u128, "{line}");
            assert_eq!(simulation.distance, *distance, "{line}");
            let elapsed = Duration::from_millis(*time as u64);
            let (actual_position, actual_speed) = simulation.sample(elapsed);
            // Java rounds each position to an integer; Slint keeps subpixel positions.
            assert!((actual_position - position).abs() <= 0.51, "{line}: {actual_position}");
            assert!(
                (actual_speed - speed).abs() <= 0.02 + speed.abs() * 0.00001,
                "{line}: {actual_speed}"
            );
            assert!(
                (simulation.remaining_distance(elapsed) + actual_position - distance).abs() < 0.01
            );
        }
    }

    #[test]
    fn incremental_motion_and_dynamic_bounds() {
        for sign in [-1., 1.] {
            let start = Instant::default();
            let mut simulation = AndroidFlick::new_internal(
                0.,
                test_limit_property(sign * 10_000.),
                AndroidFlickParameters::new_with_default_friction(sign * 1504.),
                start,
            );
            let elapsed = Duration::from_millis(100);
            let tick = start + elapsed;
            let mut current = 0.;
            assert!(!simulation.step(&mut current, tick));
            let first = current;
            current += sign * 100.; // A virtualized view changes its content origin.
            assert!(!simulation.step(&mut current, tick));
            assert_eq!(current, first + sign * 100.);
            simulation.limit_value.as_ref().set(current + sign * 1.);
            assert!(simulation.step(&mut current, start + Duration::from_millis(200)));
            assert_eq!(current, first + sign * 101.);
            assert_eq!(simulation.remaining_distance(elapsed), 0.);
            assert_eq!(simulation.remaining_velocity(elapsed), 0.);
            let stopped = current;
            assert!(simulation.step(&mut current, start + Duration::from_secs(10)));
            assert_eq!(current, stopped);
        }
    }

    #[test]
    fn completion_at_exact_duration() {
        let start = Instant::default();
        let mut simulation = AndroidFlick::new_internal(
            0.,
            test_limit_property(100_000.),
            AndroidFlickParameters::new_with_default_friction(1504.),
            start,
        );
        let mut current = 0.;
        assert!(
            !simulation
                .step(&mut current, start + (simulation.duration - Duration::from_millis(1)))
        );
        assert!(simulation.step(&mut current, start + simulation.duration));
        assert_eq!(current, simulation.distance);
        assert_eq!(simulation.remaining_velocity(simulation.duration), 0.);
    }

    #[test]
    fn fixed_distance_wheel_curve() {
        for distance in [-100., 100.] {
            let duration = Duration::from_millis(250);
            let simulation = AndroidFlick::new_with_distance(
                0.,
                test_limit_property(distance * 10.),
                distance,
                duration,
            );
            for t in [0., 0.25, 0.5, 0.75, 1.] {
                let (position, velocity) = simulation.sample(duration.mul_f32(t));
                let rate = deceleration_rate();
                assert!((position - distance * (1. - (1. - t).powf(rate))).abs() < 0.001);
                assert!(
                    (velocity - distance * rate / 0.25 * (1. - t).powf(rate - 1.)).abs() < 0.001
                );
            }
        }
    }

    #[test]
    fn zero_init_velocity() {
        const START_VALUE: f32 = 10.;

        let parameters = AndroidFlickParameters::new_with_default_friction(0.);
        let time = Instant::default();
        let mut simulation =
            AndroidFlick::new_internal(START_VALUE, test_limit_property(100.), parameters, time);

        let mut current = START_VALUE;
        assert_eq!(
            simulation.step_internal(&mut current, time),
            true,
            "There is no velocity. So the simulation is must be finish"
        );
        assert_eq!(current, START_VALUE);
    }
}
