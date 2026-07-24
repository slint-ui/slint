// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore signum underdamped

use crate::animations::simulations::{Direction, Parameter, Simulation};
use crate::{Coord, animations::Instant};
#[cfg(not(feature = "std"))]
use num_traits::Float;

#[cfg(test)]
use crate::animations::simulations::{assert_approx_eq, test_limit_property};

/// Input parameters for the `ConstantDeceleration` simulation
#[derive(Debug, Clone)]
pub struct ConstantDecelerationParameters {
    pub initial_velocity: f32,
    pub deceleration: f32,
}

impl ConstantDecelerationParameters {
    pub fn new(initial_velocity: f32, deceleration: f32) -> Self {
        Self { initial_velocity, deceleration }
    }

    /// Creates a new `ConstantDecelerationParameters` parameter object based on the distance
    /// to travel and duration of the animation.
    /// The deceleration is chosen such that the animation covers the given distance at the end of
    /// the animation and the velocity becomes zero at the same time (after duration_secs).
    ///
    // * `distance` - the distance to cover with this animation
    // * `duration_secs` - the duration of the animation in seconds
    pub fn new_with_distance(distance: f32, duration_secs: f32) -> Self {
        debug_assert!(duration_secs > 0., "Duration must be greater than zero");

        // The initial velocity and deceleration are calculated based on the distance and duration to cover the given distance in the given time.
        //
        // The calculation is based on the equations of motion for constant acceleration:
        //      => v0 * t + 0.5 * a * t^2 = d
        //
        //
        // Where t = duration_secs, d = distance, v0 = initial_velocity and a = -deceleration
        // Warning! a is acceleration, not deceleration, so we need to flip the sign at the end
        //
        // We want to reach the limit value at the end of the animation, and the velocity should become zero at the same time, so we can determine `a` based on:
        //          v0 + a * t = 0
        //
        //      => a = -v0 / t
        //
        // Then we can solve for `v0` and `a`:
        //
        //     v0 * t + 0.5 * -(v0 / t) * t^2 = d
        //     v0 * t + 0.5 * -v0 * t = d
        //     v0 * (t + -0.5 * t) = d
        //     v0 * (0.5 * t) = d
        //     => v0 = d / (0.5 * t)
        //
        let d = distance;
        let t = duration_secs;
        let v0 = d / (0.5 * t);
        let a = -(v0 / t);
        // deceleration: therefore -a
        Self::new(v0, -a)
    }

    /// Calculates the remaining distance to the limit value at a given time based on the initial velocity and deceleration.
    pub fn remaining_distance(&self, time_elapsed: core::time::Duration) -> Coord {
        debug_assert!(self.deceleration != 0., "deceleration must not be zero");
        debug_assert!(
            self.deceleration.signum() == self.initial_velocity.signum(),
            "deceleration must actually decelerate the velocity"
        );

        // The animation stops if the velocity becomes zero.
        // Therefore we can calculate the animation duration based on the initial velocity and deceleration:
        //          v0 + a * t = 0
        //          => t = -v0 / a
        // Note: our deceleration is `-a` negated
        let total_duration = self.initial_velocity / self.deceleration;

        if time_elapsed.as_secs_f32() < total_duration {
            // Based on the equations of motion for constant acceleration we can calculate the remaining distance at a given time:
            (0.5 * (-self.deceleration)
                * (total_duration.powi(2) - time_elapsed.as_secs_f32().powi(2))
                + self.initial_velocity * (total_duration - time_elapsed.as_secs_f32()))
                as Coord
        } else {
            Coord::default()
        }
    }
}

impl Parameter for ConstantDecelerationParameters {
    type Output = ConstantDeceleration;
    fn simulation(
        self,
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    ) -> Self::Output {
        ConstantDeceleration::new(start_value, limit_value, self)
    }
}

/// This simulation simulates a constant deceleration of a point starting at position `start_value` with
/// an initial velocity of `initial_velocity`. When the point reaches the limit value `limit_value` it stops there
#[derive(Debug)]
pub struct ConstantDeceleration {
    /// If the limit is not reached, it is also fine. Also exceeding the limit can be ok,
    /// but at the end of the animation the limit shall not be exceeded
    limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    velocity: f32,
    data: ConstantDecelerationParameters,
    direction: Direction,
    start_time: Instant,
}

impl ConstantDeceleration {
    /// Create a new ConstantDeceleration simulation
    ///
    /// * `start_value` - start position
    /// * `limit_value` - value at which the simulation ends if the velocity did not get zero before
    /// * `initial_velocity` - the initial velocity of the point
    /// * `data` - the properties of this simulation
    pub fn new(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        data: ConstantDecelerationParameters,
    ) -> Self {
        Self::new_internal(start_value, limit_value, data, crate::animations::current_tick())
    }

    fn new_internal(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        mut data: ConstantDecelerationParameters,
        start_time: Instant,
    ) -> Self {
        let mut initial_velocity = data.initial_velocity;
        let direction = if start_value == limit_value.as_ref().get() {
            if initial_velocity >= 0. {
                data.deceleration = f32::abs(data.deceleration);
                Direction::Increasing
            } else {
                data.deceleration = -f32::abs(data.deceleration);
                Direction::Decreasing
            }
        } else if start_value < limit_value.as_ref().get() {
            data.deceleration = f32::abs(data.deceleration);
            assert!(initial_velocity >= 0.); // Makes no sense yet that the velocity goes into the other direction
            initial_velocity = f32::abs(initial_velocity);
            Direction::Increasing
        } else {
            data.deceleration = -f32::abs(data.deceleration);
            initial_velocity = -f32::abs(initial_velocity);
            assert!(initial_velocity <= 0.);
            Direction::Decreasing
        };

        Self { limit_value, velocity: initial_velocity, data, direction, start_time }
    }

    fn step_internal(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        let limit_value = self.limit_value.as_ref().get();

        // We have to prevent go go beyond the limit where velocity gets zero
        let duration = f32::min(
            new_tick.duration_since(self.start_time).as_secs_f32(),
            f32::abs(self.velocity / self.data.deceleration),
        );

        self.start_time = new_tick;

        let new_velocity = self.velocity - duration * self.data.deceleration;

        *current += duration * (self.velocity + new_velocity) / 2.; // Trapezoidal integration
        self.velocity = new_velocity;

        match self.direction {
            Direction::Increasing => {
                if *current >= limit_value {
                    *current = limit_value;
                    self.velocity = 0.;
                    return true;
                } else if self.velocity <= 0. {
                    return true;
                }
            }
            Direction::Decreasing => {
                if *current <= limit_value {
                    *current = limit_value;
                    self.velocity = 0.;
                    return true;
                } else if self.velocity >= 0. {
                    return true;
                }
            }
        }
        false
    }
}

impl Simulation for ConstantDeceleration {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        self.step_internal(current, new_tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::time::Duration;

    #[test]
    fn constant_deceleration_start_eq_limit() {
        const START_VALUE: f32 = 10.;
        const LIMIT_VALUE: f32 = 10.;
        const INITIAL_VELOCITY: f32 = 50.;
        const DECELERATION: f32 = 20.;
        let parameters = ConstantDecelerationParameters::new(INITIAL_VELOCITY, DECELERATION);

        let time = Instant::now();
        let mut simulation = ConstantDeceleration::new_internal(
            START_VALUE,
            test_limit_property(LIMIT_VALUE),
            parameters,
            time,
        );

        let mut current = START_VALUE;
        let finished = simulation.step(&mut current, time + Duration::from_hours(10));
        assert_eq!(finished, true);
        assert_eq!(current, START_VALUE);
    }

    /// The velocity becomes zero before we are reaching the limit
    /// start_value < limit_value
    #[test]
    fn constant_deceleration_increasing_limit_not_reached() {
        const START_VALUE: f32 = 10.;
        const LIMIT_VALUE: f32 = 2000.;
        const INITIAL_VELOCITY: f32 = 50.;
        const DECELERATION: f32 = 20.;
        let parameters = ConstantDecelerationParameters::new(INITIAL_VELOCITY, DECELERATION);

        let mut time = Instant::now();
        let mut simulation = ConstantDeceleration::new_internal(
            START_VALUE,
            test_limit_property(LIMIT_VALUE),
            parameters,
            time,
        );
        let mut current = START_VALUE;

        // Velocity does not become zero
        let mut duration = Duration::from_secs(1);
        assert!(DECELERATION * duration.as_secs_f32() < INITIAL_VELOCITY);
        time += duration;
        let finished = simulation.step(&mut current, time);
        assert_eq!(finished, false);
        assert_approx_eq!(
            current,
            START_VALUE + INITIAL_VELOCITY * duration.as_secs_f32()
                - 0.5 * DECELERATION * duration.as_secs_f32().powi(2)
        );

        // Now the velocity becomes zero and we don't do any further calculations
        duration = Duration::from_hours(10);
        assert!(Duration::from_secs((INITIAL_VELOCITY / DECELERATION) as u64) < duration);
        time += duration;
        let finished = simulation.step(&mut current, time);
        assert_eq!(finished, true);
        assert_approx_eq!(
            current,
            START_VALUE + INITIAL_VELOCITY * INITIAL_VELOCITY / DECELERATION
                - 0.5 * DECELERATION * (INITIAL_VELOCITY / DECELERATION).powi(2)
        );

        assert!(current < LIMIT_VALUE); // We reached velocity zero before we reached the position limit
    }

    /// We reach the position limit before the velocity got zero
    #[test]
    fn constant_deceleration_increasing_limit_reached() {
        const START_VALUE: f32 = 10.;
        const LIMIT_VALUE: f32 = 20.;
        const INITIAL_VELOCITY: f32 = 50.;
        const DECELERATION: f32 = 20.;
        let parameters = ConstantDecelerationParameters::new(INITIAL_VELOCITY, DECELERATION);

        let mut time = Instant::now();
        let mut simulation = ConstantDeceleration::new_internal(
            START_VALUE,
            test_limit_property(LIMIT_VALUE),
            parameters,
            time,
        );
        let mut current = START_VALUE;

        let duration = Duration::from_secs(1);
        assert!(f32::abs(DECELERATION * duration.as_secs_f32()) < f32::abs(INITIAL_VELOCITY)); // We don't reach the limit where the velocity gets zero
        time += duration;
        let finished = simulation.step(&mut current, time);
        assert_eq!(finished, true);
        assert_eq!(current, LIMIT_VALUE); // Limit reached
    }

    /// We don't reach the position limit. Before the velocity gets zero
    /// start_value > limit_value
    #[test]
    fn constant_deceleration_decreasing_limit_not_reached() {
        const START_VALUE: f32 = 2000.;
        const LIMIT_VALUE: f32 = 10.;
        const INITIAL_VELOCITY: f32 = -50.;
        const DECELERATION: f32 = 20.;

        let parameters = ConstantDecelerationParameters::new(INITIAL_VELOCITY, DECELERATION);

        let mut time = Instant::now();
        let mut simulation = ConstantDeceleration::new_internal(
            START_VALUE,
            test_limit_property(LIMIT_VALUE),
            parameters,
            time,
        );
        let mut current = START_VALUE;

        let mut duration = Duration::from_secs(1);
        assert!(f32::abs(DECELERATION * duration.as_secs_f32()) < f32::abs(INITIAL_VELOCITY));
        time += duration;
        let finished = simulation.step(&mut current, time);
        assert_eq!(finished, false);
        assert_eq!(
            current,
            START_VALUE + INITIAL_VELOCITY * duration.as_secs_f32()
                - INITIAL_VELOCITY.signum() * 0.5 * DECELERATION * duration.as_secs_f32().powi(2)
        );

        duration = Duration::from_hours(10);
        assert!(Duration::from_secs((INITIAL_VELOCITY / DECELERATION) as u64) < duration);
        time += duration;
        let finished = simulation.step(&mut current, time);
        assert_eq!(finished, true);
        assert_eq!(
            current,
            START_VALUE + INITIAL_VELOCITY * f32::abs(INITIAL_VELOCITY / DECELERATION)
                - 0.5
                    * INITIAL_VELOCITY.signum()
                    * DECELERATION
                    * (INITIAL_VELOCITY / DECELERATION).powi(2)
        );

        assert!(current > LIMIT_VALUE); // We reached velocity zero before we reached the position limit
    }

    /// We reach the position limit before the velocity got zero
    /// start_value > limit_value
    #[test]
    fn constant_deceleration_decreasing_limit_reached() {
        const START_VALUE: f32 = 20.;
        const LIMIT_VALUE: f32 = 10.;
        const INITIAL_VELOCITY: f32 = -50.;
        const DECELERATION: f32 = 20.;
        let parameters = ConstantDecelerationParameters::new(INITIAL_VELOCITY, DECELERATION);

        let mut time = Instant::now();
        let mut simulation = ConstantDeceleration::new_internal(
            START_VALUE,
            test_limit_property(LIMIT_VALUE),
            parameters,
            time,
        );
        let mut current = START_VALUE;

        let duration = Duration::from_secs(3);
        assert!(f32::abs(DECELERATION * duration.as_secs_f32()) > f32::abs(INITIAL_VELOCITY)); // We don't reach the limit where the velocity gets zero
        time += duration;
        let finished = simulation.step(&mut current, time);
        assert_eq!(finished, true);
        assert_eq!(current, LIMIT_VALUE); // Limit reached
    }
}
