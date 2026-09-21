// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Ported from Flutter's `ClampingScrollSimulation`
//! (scroll_simulation.dart.dart), which is:
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
use std::println;

const INFLEXION: f32 = 0.35;
const PHYSICAL_COEFFICIENT: f32 = 9.80665 // g, in meters per second^2
                                * 39.37 // 1 meter / 1 inch
                                * 160.0 // 1 inch / 1 logical pixel
                                * 0.84; // "look and feel tuning"
const DEFAULT_FRICTION: f32 = 0.015;

fn deceleration_rate() -> f32 {
    f32::log10(0.78) / f32::log10(0.9)
}

/// Input parameters for the `ConstantDeceleration` simulation
#[derive(Debug, Clone)]
pub struct AndroidFlickParameters {
    pub initial_velocity: f32,
    pub friction: f32,
}

impl AndroidFlickParameters {
    pub fn new_with_default_friction(initial_velocity: f32) -> Self {
        Self { initial_velocity, friction: DEFAULT_FRICTION }
    }

    pub fn new_with_distance(distance: f32, duration: Duration) -> Self {
        let duration = duration.as_secs_f32();
        // Calculate the friction
        // distance = init_vel * max_duration / Dec rate
        // --> init_vel = distance * dec_rate / max_duration
        let dec_rate = deceleration_rate();
        let initial_velocity = distance * dec_rate / duration;

        // extract friction from the fling_duration() function
        let f = f32::powf(duration / (dec_rate * INFLEXION), dec_rate - 1.);
        let friction = initial_velocity.abs() / (f * (PHYSICAL_COEFFICIENT / INFLEXION));
        Self { initial_velocity, friction }
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
        let duration = Self::fling_duration(deceleration_rate, &data);
        let max_distance = Self::fling_distance(duration, deceleration_rate, &data);

        Self {
            limit_value,
            data,
            direction,
            start_time,
            duration,
            distance: max_distance,
            deceleration_rate,
            traveled: 0.,
        }
    }

    fn step_internal(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        let max_duration = self.duration.as_secs_f32();
        if max_duration <= 0. {
            // Already finished
            println!("Simulation step: Finished because of time");
            return true;
        }
        let time_diff = new_tick.duration_since(self.start_time);
        let clamped = (time_diff.as_secs_f32() / max_duration).clamp(0., 1.);

        // This position is absolute, to get it relative, we have to subtract the previous value
        let new_traveled = self.distance * (1. - f32::powf(1. - clamped, self.deceleration_rate));
        *current += new_traveled - self.traveled;
        self.traveled = new_traveled;

        // Clamping to the limit
        let limit_value = self.limit_value.as_ref().get();
        let clamped = match self.direction {
            Direction::Increasing => {
                if *current >= limit_value {
                    *current = limit_value;
                    true
                } else {
                    false
                }
            }
            Direction::Decreasing => {
                if *current <= limit_value {
                    *current = limit_value;
                    true
                } else {
                    false
                }
            }
        };

        self.is_done(new_tick) || clamped
    }

    fn is_done(&mut self, new_tick: Instant) -> bool {
        new_tick.duration_since(self.start_time) > self.duration
    }

    fn fling_duration(deceleration_rate: f32, data: &AndroidFlickParameters) -> Duration {
        let reference_velocity = data.friction * PHYSICAL_COEFFICIENT / INFLEXION;

        let android_duration = f32::powf(
            data.initial_velocity.abs() / reference_velocity,
            1. / (deceleration_rate - 1.0),
        );

        Duration::from_secs_f32(deceleration_rate * INFLEXION * android_duration)
    }

    fn fling_distance(
        fling_duration: Duration,
        deceleration_rate: f32,
        data: &AndroidFlickParameters,
    ) -> f32 {
        data.initial_velocity * fling_duration.as_secs_f32() / deceleration_rate
    }

    fn clamped_time_diff(&self, duration: Duration) -> f32 {
        (duration.as_secs_f32() / self.duration.as_secs_f32()).clamp(0., 1.)
    }
}

impl Simulation for AndroidFlick {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        self.step_internal(current, new_tick)
    }
}

impl PositionSimulation for AndroidFlick {
    fn remaining_distance(&self, time_elapsed: core::time::Duration) -> f32 {
        let clamped = self.clamped_time_diff(time_elapsed);
        self.distance * (1. - f32::powf(1. - clamped, self.deceleration_rate))
    }

    fn remaining_velocity(&self, time_elapsed: core::time::Duration) -> f32 {
        let clamped = self.clamped_time_diff(time_elapsed);

        self.data.initial_velocity * f32::powf(1. - clamped, self.deceleration_rate - 1.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::simulations::test_limit_property;

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
