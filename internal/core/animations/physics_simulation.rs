// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore signum underdamped
//! This module contains various physics simulations which can be used as animation (internally only yet).
//! Currently it is used in the flickable to animate the viewport position
//!
//! Currently it contains two simulations:
//! - `ConstantDeceleration`
//! - `ConstantDecelerationSpringDamper` with spring damper simulation when reaching the limit

use crate::{Coord, animations::Instant};
#[cfg(not(feature = "std"))]
use num_traits::Float;

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
    /// The simulation's current velocity, used to hand off motion to a replacement simulation.
    fn velocity(&self) -> f32;
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

    fn velocity(&self) -> f32 {
        self.velocity
    }
}

#[cfg(test)]
macro_rules! assert_approx_eq {
    ($a:expr, $b:expr) => {
        assert!(($a - $b).abs() < 1e-4, "{} != {}", $a, $b);
    };
}
#[cfg(test)]
fn test_limit_property(value: f32) -> core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>> {
    alloc::boxed::Box::pin(crate::Property::new(value))
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

/// Input parameters for the `ConstantDecelerationSpringDamper` simulation
/// [1] https://www.maplesoft.com/content/EngineeringFundamentals/6/MapleDocument_32/Free%20Response%20Part%202.pdf
#[cfg(test)]
#[derive(Debug, Clone)]
pub struct ConstantDecelerationSpringDamperParameters {
    pub initial_velocity: f32,
    pub deceleration: f32,
    pub mass: f32,                // [1] parameter m
    pub spring_constant: f32,     // [1] parameter k
    pub damping_coefficient: f32, // [1] parameter c
}

#[cfg(test)]
impl ConstantDecelerationSpringDamperParameters {
    /// Creates a new `ConstantDecelerationSpringDamperParameters` parameter object
    /// It is more comfortable to use than specifying the parameters manually because here the parameter calculation
    /// is done based on the `half_period_time` parameter
    ///
    /// * `initial_velocity` - the initial velocity of the point
    /// * `deceleration` - the constant deceleration of the point
    /// * `half_period_time` - the time of the simulation when the limit value got exceeded to return back to it
    pub fn new(initial_velocity: f32, deceleration: f32, half_period_time: f32) -> Self {
        let (mass, spring_constant, damping_coefficient) =
            Self::calculate_parameters(half_period_time);

        Self { initial_velocity, deceleration, mass, spring_constant, damping_coefficient }
    }

    fn calculate_parameters(half_period_time: f32) -> (f32, f32, f32) {
        // [1] eq 13
        const MASS: f32 = 1.;
        const DAMPING_COEFFICIENT: f32 = 1.;
        let w_d = 2. * core::f32::consts::PI * 1. / (2. * half_period_time);
        let spring_constant = w_d.powi(2) + DAMPING_COEFFICIENT.powi(2) / (4. * MASS.powi(2));

        (MASS, spring_constant, DAMPING_COEFFICIENT)
    }
}

#[cfg(test)]
impl Parameter for ConstantDecelerationSpringDamperParameters {
    type Output = ConstantDecelerationSpringDamper;
    fn simulation(
        self,
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    ) -> Self::Output {
        ConstantDecelerationSpringDamper::new(start_value, limit_value, self)
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
enum State {
    Deceleration,
    SpringDamper,
    Done,
}

/// This simulation simulates a constant deceleration of a point starting at position `start_value` with
/// an initial velocity of `initial_velocity`. When the point reaches the limit value `limit_value` before
/// the velocity reaches zero, the system simulates a spring damper system to go shortly beyond the limit
/// value and returning then back
#[cfg(test)]
#[derive(Debug)]
pub struct ConstantDecelerationSpringDamper {
    /// If the limit is not reached, it is also fine. Also exceeding the limit can be ok,
    /// but at the end of the animation the limit shall not be exceeded
    limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    curr_val_zeroed: f32,
    velocity: f32,
    data: ConstantDecelerationSpringDamperParameters,
    direction: Direction,
    start_time: Instant,
    state: State,
    damping_ratio: f32,
    /// Undamped natural frequency
    w_n: f32,
    /// Damped natural frequency
    w_d: f32,
    constant_a: f32,
    constant_phi: f32,
}

#[cfg(test)]
impl ConstantDecelerationSpringDamper {
    pub fn new(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        data: ConstantDecelerationSpringDamperParameters,
    ) -> Self {
        Self::new_internal(start_value, limit_value, data, crate::animations::current_tick())
    }

    fn new_internal(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        mut data: ConstantDecelerationSpringDamperParameters,
        start_time: Instant,
    ) -> Self {
        let mut initial_velocity = data.initial_velocity;
        let mut state = State::Deceleration;
        let direction = if start_value == limit_value.as_ref().get() {
            state = State::Done;
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

        assert!(data.mass > 0.);
        assert!(data.spring_constant >= 0.);

        let c_cr = 2. * f32::sqrt(data.mass * data.spring_constant); // Critical damping coefficient
        let damping_ratio = data.damping_coefficient / c_cr;
        assert!(damping_ratio > 0.);
        assert!(damping_ratio < 1.); // Currently we support only the underdamped motion, because we wanna return to the `limit_value`

        let w_n = c_cr / (2. * data.mass);
        let w_d = w_n * f32::sqrt(1. - damping_ratio.powi(2));

        Self {
            limit_value,
            curr_val_zeroed: 0.,
            velocity: initial_velocity,
            data,
            direction,
            start_time,
            state,
            damping_ratio,
            w_n,
            w_d,
            constant_a: 0., // Calculated when transitioning to the damper spring state
            constant_phi: 0., // Calculated when transitioning to the damper spring state
        }
    }

    fn new_value(&self) -> f32 {
        self.limit_value.as_ref().get() + self.curr_val_zeroed
    }

    fn step_internal(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        match self.state {
            State::Deceleration => self.state_deceleration(current, new_tick),
            State::SpringDamper => self.state_spring_damper(current, new_tick),
            State::Done => {
                *current = self.new_value();
                true
            }
        }
    }

    fn state_deceleration(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        let limit_value = self.limit_value.as_ref().get();
        let duration_unlimited = new_tick.duration_since(self.start_time);
        // We have to prevent go go beyond the limit where velocity gets zero
        let duration = f32::min(
            duration_unlimited.as_secs_f32(),
            f32::abs(self.velocity / self.data.deceleration),
        );

        self.start_time = new_tick;

        let new_velocity = self.velocity - (duration * self.data.deceleration);
        let new_val = *current + (duration * (self.velocity + new_velocity) / 2.); // Trapezoidal integration

        enum S {
            LimitReached,
            VelocityZero,
            None,
        }

        let s = match self.direction {
            Direction::Increasing if new_val > limit_value => S::LimitReached,
            Direction::Increasing if new_velocity <= 0. => S::VelocityZero,
            Direction::Decreasing if new_val < limit_value => S::LimitReached,
            Direction::Decreasing if new_velocity >= 0. => S::VelocityZero,
            _ => S::None,
        };
        match s {
            S::LimitReached => {
                self.state = State::SpringDamper;

                // time when reaching the limit
                // solving p_limit = p_old + v_old * dt - 0.5 * a * dt^2
                let root = f32::sqrt(
                    self.velocity.powi(2) - self.data.deceleration * (limit_value - *current),
                );
                // The smaller is the relevant. The larger is when the initial velocity got zero and due to the constant acceleration we turn
                let dt = f32::min(
                    (self.velocity - root) / self.data.deceleration,
                    (self.velocity + root) / self.data.deceleration,
                );

                self.velocity -= dt * self.data.deceleration; // Velocity at limit value point. Solved `new_val` equation for new_velocity
                self.curr_val_zeroed = 0.;
                *current = limit_value;

                const X0: f32 = 0.; // Relative point
                self.constant_a = self.velocity.signum()
                    * f32::sqrt(
                        (self.w_d.powi(2) * X0.powi(2)
                            + (self.velocity + self.damping_ratio * self.w_n * 0.).powi(2))
                            / self.w_d.powi(2),
                    );
                self.constant_phi =
                    f32::atan(self.w_d * X0 / (self.velocity + self.damping_ratio * self.w_n * X0));
                self.state_spring_damper(
                    current,
                    new_tick
                        + (duration_unlimited
                            - core::time::Duration::from_millis((dt * 1000.) as u64)),
                )
            }
            S::VelocityZero => {
                self.velocity = 0.;
                *current = new_val;
                true
            }
            S::None => {
                self.velocity = new_velocity;
                *current = new_val;
                false
            }
        }
    }

    fn state_spring_damper(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        // Here we use absolute time because it simplifies the equation
        let t = (new_tick - self.start_time).as_secs_f32();
        // Underdamped spring damper equation
        assert!(self.damping_ratio < 1.);
        let new_val = self.constant_a
            * f32::exp(-self.damping_ratio * self.w_n * t)
            * f32::sin(self.w_d * t + self.constant_phi);
        self.curr_val_zeroed = new_val; // relative value

        let limit_value = self.limit_value.as_ref().get();
        let max_time = 2. * core::f32::consts::PI / self.w_d;
        let current_val = self.new_value();
        *current = current_val;
        let finished = match self.direction {
            Direction::Increasing => {
                // We are coming back from a value higher than the limit
                current_val < limit_value || t > max_time
            }
            Direction::Decreasing => {
                // We are coming back from a value lower than the limit
                current_val > limit_value || t > max_time
            }
        };
        if finished {
            self.velocity = 0.;
            *current = limit_value;
            self.curr_val_zeroed = 0.;
            self.state = State::Done;
        }
        finished
    }
}

#[cfg(test)]
impl Simulation for ConstantDecelerationSpringDamper {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        self.step_internal(current, new_tick)
    }

    fn velocity(&self) -> f32 {
        self.velocity
    }
}

#[cfg(test)]
mod tests_spring_damper {
    use super::*;
    use core::{f32::consts::PI, time::Duration};

    #[test]
    fn calculate_parameters() {
        const INITIAL_VELOCITY: f32 = 50.;
        const DECELERATION: f32 = 20.;
        const HALF_PERIOD_TIME: f32 = 100e-3;
        let res = super::ConstantDecelerationSpringDamperParameters::new(
            INITIAL_VELOCITY,
            DECELERATION,
            HALF_PERIOD_TIME,
        );

        let w_n = f32::sqrt(res.spring_constant * res.mass) / res.mass;
        let damping_ratio = res.damping_coefficient / (2. * res.mass * w_n);
        let w_d = w_n * f32::sqrt(1. - damping_ratio.powi(2));
        assert_approx_eq!(w_d, 2. * PI * 1. / (2. * HALF_PERIOD_TIME));
    }

    #[test]
    fn constant_deceleration_start_eq_limit() {
        const START_VALUE: f32 = 10.;
        const LIMIT_VALUE: f32 = 10.;
        const INITIAL_VELOCITY: f32 = 50.;
        const DECELERATION: f32 = 20.;
        const HALF_PERIOD_TIME: f32 = 100e-3;
        let parameters = ConstantDecelerationSpringDamperParameters::new(
            INITIAL_VELOCITY,
            DECELERATION,
            HALF_PERIOD_TIME,
        );

        assert_eq!(START_VALUE, LIMIT_VALUE);
        let time = Instant::now();
        let mut simulation = ConstantDecelerationSpringDamper::new_internal(
            START_VALUE,
            test_limit_property(LIMIT_VALUE),
            parameters,
            time,
        );
        let mut current = START_VALUE;
        let finished = simulation.step(&mut current, time);
        assert_eq!(current, START_VALUE);
        assert_eq!(finished, true);
        assert_eq!(simulation.state, State::Done);
    }

    /// The velocity becomes zero before we are reaching the limit
    /// start_value < limit_value
    #[test]
    fn constant_deceleration_increasing_limit_not_reached() {
        const INITIAL_VELOCITY: f32 = 50.;
        const DECELERATION: f32 = 20.;
        const HALF_PERIOD_TIME: f32 = 100e-3;
        let parameters = ConstantDecelerationSpringDamperParameters::new(
            INITIAL_VELOCITY,
            DECELERATION,
            HALF_PERIOD_TIME,
        );

        let mut time = Instant::now();
        let mut simulation = ConstantDecelerationSpringDamper::new_internal(
            10.,
            test_limit_property(2000.),
            parameters,
            time,
        );
        let mut current = 10.;

        // Velocity does not become zero
        let mut duration = Duration::from_secs(1);
        assert!(DECELERATION * duration.as_secs_f32() < INITIAL_VELOCITY);
        time += duration;
        let finished = simulation.step(&mut current, time);
        assert_eq!(finished, false);
        assert_approx_eq!(
            current,
            10. + 50. * duration.as_secs_f32()
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
            10. + 50. * INITIAL_VELOCITY / DECELERATION
                - 0.5 * DECELERATION * (INITIAL_VELOCITY / DECELERATION).powi(2)
        );

        assert!(current < 2000.); // We reached velocity zero before we reached the position limit
    }

    /// We don't reach the position limit. Before the velocity gets zero
    /// start_value > limit_value
    #[test]
    fn constant_deceleration_decreasing_limit_not_reached() {
        const START_VALUE: f32 = 2000.;
        const LIMIT_VALUE: f32 = 10.;
        const INITIAL_VELOCITY: f32 = -50.;
        const DECELERATION: f32 = 20.;
        const HALF_PERIOD_TIME: f32 = 100e-3;

        let parameters = ConstantDecelerationSpringDamperParameters::new(
            INITIAL_VELOCITY,
            DECELERATION,
            HALF_PERIOD_TIME,
        );

        let mut time = Instant::now();
        let mut simulation = ConstantDecelerationSpringDamper::new_internal(
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

    /// We reach the position limit before the velocity got zero and so we run into the spring damper system
    /// Increasing case: start_value < limit_value
    #[test]
    fn constant_deceleration_spring_damper_increasing_limit_reached() {
        const INITIAL_VELOCITY: f32 = 50.;
        const DECELERATION: f32 = 20.;
        const HALF_PERIOD_TIME: f32 = 10.;
        const START_VALUE: f32 = 10.;
        const LIMIT_VALUE: f32 = 70.;
        let parameters = super::ConstantDecelerationSpringDamperParameters::new(
            INITIAL_VELOCITY,
            DECELERATION,
            HALF_PERIOD_TIME,
        );

        let mut time = Instant::now();
        let mut simulation = ConstantDecelerationSpringDamper::new_internal(
            START_VALUE,
            test_limit_property(LIMIT_VALUE),
            parameters,
            time,
        );
        let mut current = START_VALUE;

        let duration = Duration::from_secs(1);
        assert!(f32::abs(DECELERATION) * duration.as_secs_f32() < f32::abs(INITIAL_VELOCITY)); // We don't reach the limit where the velocity gets zero
        time += duration;
        let finished = simulation.step(&mut current, time);
        assert_eq!(finished, false);
        assert_eq!(simulation.state, State::Deceleration);
        assert!(current < LIMIT_VALUE); // We are still in the constant deceleration state

        time += Duration::from_secs((HALF_PERIOD_TIME / 2.) as u64);
        let finished = simulation.step(&mut current, time);
        assert_eq!(finished, false);
        assert_eq!(simulation.state, State::SpringDamper);
        assert!(current > LIMIT_VALUE);

        time += Duration::from_hours(10);
        let finished = simulation.step(&mut current, time);
        assert_eq!(finished, true);
        assert_eq!(simulation.state, State::Done);
        assert_eq!(current, LIMIT_VALUE);
    }

    /// We reach the position limit before the velocity got zero and so we run into the spring damper system
    /// Decreasing case. limit_value < start_value
    #[test]
    fn constant_deceleration_spring_damper_decreasing_limit_reached() {
        const INITIAL_VELOCITY: f32 = -50.;
        const DECELERATION: f32 = 20.;
        const HALF_PERIOD_TIME: f32 = 10.;
        const START_VALUE: f32 = 70.;
        const LIMIT_VALUE: f32 = 10.;
        let parameters = super::ConstantDecelerationSpringDamperParameters::new(
            INITIAL_VELOCITY,
            DECELERATION,
            HALF_PERIOD_TIME,
        );

        let mut time = Instant::now();
        let mut simulation = ConstantDecelerationSpringDamper::new_internal(
            START_VALUE,
            test_limit_property(LIMIT_VALUE),
            parameters,
            time,
        );
        let mut current = START_VALUE;

        let duration = Duration::from_secs(1);
        assert!(f32::abs(DECELERATION) * duration.as_secs_f32() < f32::abs(INITIAL_VELOCITY)); // We don't reach the limit where the velocity gets zero
        time += duration;
        let finished = simulation.step(&mut current, time);
        assert_eq!(finished, false);
        assert_eq!(simulation.state, State::Deceleration);
        assert!(current > LIMIT_VALUE); // We are still in the constant deceleration state

        time += Duration::from_secs((HALF_PERIOD_TIME / 2.) as u64);
        let finished = simulation.step(&mut current, time);
        assert_eq!(finished, false);
        assert_eq!(simulation.state, State::SpringDamper);
        assert!(current < LIMIT_VALUE);

        time += Duration::from_hours(10);
        let finished = simulation.step(&mut current, time);
        assert_eq!(finished, true);
        assert_eq!(simulation.state, State::Done);
        assert_eq!(current, LIMIT_VALUE);
    }
}

/// How close to the target position/velocity a `SpringSimulation` must get before it is
/// considered settled and snaps to rest. This is the "settling duration" and is distinct from the
/// user-facing `duration` that fixes the natural frequency
const SPRING_SETTLE_POSITION_EPSILON: f32 = 0.01;
const SPRING_SETTLE_VELOCITY_EPSILON: f32 = 0.01;

/// Converts a springs configuration into the `(natural_frequency, damping_ratio)` pair
/// that the `SpringSimulation` solves the ODE with.
pub trait SpringParameters {
    /// Returns `(w_n, zeta)`
    fn to_natural_frequency_and_damping_ratio(&self) -> (f32, f32);
}

/// `duration` decides the natural frequency and bounce decides the damping
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct SpringDurationBounceParameters {
    pub duration_secs: f32,
    /// Expected range `-1.0..=1.0`, but not clamped here.
    pub bounce: f32,
}

impl SpringDurationBounceParameters {
    #[allow(dead_code)]
    pub fn new(duration_secs: f32, bounce: f32) -> Self {
        Self { duration_secs, bounce }
    }
}

impl SpringParameters for SpringDurationBounceParameters {
    fn to_natural_frequency_and_damping_ratio(&self) -> (f32, f32) {
        debug_assert!(self.duration_secs > 0., "duration must be greater than zero");
        let w_n = 2. * core::f32::consts::PI / self.duration_secs;
        let zeta = 1. - self.bounce;
        (w_n, zeta)
    }
}

/// `mass`/`stiffness`/`damping`-style spring configuration
#[derive(Debug, Clone, Copy)]
pub struct SpringPhysicalParameters {
    pub mass: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl SpringPhysicalParameters {
    #[allow(dead_code)]
    pub fn new(mass: f32, stiffness: f32, damping: f32) -> Self {
        Self { mass, stiffness, damping }
    }
}

impl SpringParameters for SpringPhysicalParameters {
    fn to_natural_frequency_and_damping_ratio(&self) -> (f32, f32) {
        debug_assert!(self.mass > 0., "mass must be greater than zero");
        debug_assert!(self.stiffness >= 0., "stiffness must not be negative");
        let w_n = f32::sqrt(self.stiffness / self.mass);
        let critical_damping = 2. * f32::sqrt(self.mass * self.stiffness);
        let zeta = if critical_damping > 0. { self.damping / critical_damping } else { 0. };
        (w_n, zeta)
    }
}

/// Precomputed coefficients for a spring, one varient per damping regime
/// All are relative to `target` (`x_rel = x - target`)
/// `x_rel(0) = start_value - target`
/// `vel(0) = initial_velocity`
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
enum SpringRegime {
    /// `zeta < 1`: oscillates while decaying. `x_rel(t) = e^(-zeta*w_n*t) * (c1*cos(w_d*t) + c2*sin(w_d*t))`
    Underdamped { w_d: f32, c1: f32, c2: f32 },
    /// `zeta == 1`: fastest non-oscillating approach. `x_rel(t) = (c1 + c2*t) * e^(-w_n*t)`
    Critical { c1: f32, c2: f32 },
    /// `zeta > 1`: slow, non-oscillating approach. `x_rel(t) = c1*e^(r1*t) + c2*e^(r2*t)`
    Overdamped { r1: f32, r2: f32, c1: f32, c2: f32 },
}

impl SpringRegime {
    /// `zeta` values within this distance of `1.0` are treated as critically damped, to avoid
    /// `w_d` (underdamped) or `sqrt(zeta^2 - 1)` (overdamped) blowing up near the boundary.
    #[allow(dead_code)]
    const CRITICAL_ZETA_EPSILON: f32 = 1e-3;

    #[allow(dead_code)]
    fn new(x0: f32, v0: f32, w_n: f32, zeta: f32) -> Self {
        if (zeta - 1.).abs() < Self::CRITICAL_ZETA_EPSILON {
            Self::Critical { c1: x0, c2: v0 + w_n * x0 }
        } else if zeta < 1. {
            let w_d = w_n * f32::sqrt(1. - zeta * zeta);
            Self::Underdamped { w_d, c1: x0, c2: (v0 + zeta * w_n * x0) / w_d }
        } else {
            let disc = f32::sqrt(zeta * zeta - 1.);
            let r1 = w_n * (-zeta + disc);
            let r2 = w_n * (-zeta - disc);
            let c1 = (v0 - r2 * x0) / (r1 - r2);
            Self::Overdamped { r1, r2, c1, c2: x0 - c1 }
        }
    }

    /// Evaluates the closed form at elapsed time `t`, returning `(x_rel, vel)`.
    fn evaluate(&self, w_n: f32, zeta: f32, t: f32) -> (f32, f32) {
        match *self {
            Self::Underdamped { w_d, c1, c2 } => {
                let decay = f32::exp(-zeta * w_n * t);
                let (s, c) = f32::sin_cos(w_d * t);
                let pos = decay * (c1 * c + c2 * s);
                let vel = decay
                    * ((-zeta * w_n * c1 + w_d * c2) * c + (-zeta * w_n * c2 - w_d * c1) * s);
                (pos, vel)
            }
            Self::Critical { c1, c2 } => {
                let decay = f32::exp(-w_n * t);
                let pos = decay * (c1 + c2 * t);
                let vel = decay * (c2 - w_n * (c1 + c2 * t));
                (pos, vel)
            }
            Self::Overdamped { r1, r2, c1, c2 } => {
                let pos = c1 * f32::exp(r1 * t) + c2 * f32::exp(r2 * t);
                let vel = c1 * r1 * f32::exp(r1 * t) + c2 * r2 * f32::exp(r2 * t);
                (pos, vel)
            }
        }
    }
}

/// This simulation moves a point from `start_value` toward a fixed `target`, driven by a spring
/// with natural frequency `w_n` and damping ratio `zeta`, starting with `initial_velocity`.
/// This simulates a spring towards a fixed target unlike `ConstantDecelerationSpringDamper`
#[allow(dead_code)]
#[derive(Debug)]
pub struct SpringSimulation {
    target: f32,
    w_n: f32,
    zeta: f32,
    regime: SpringRegime,
    start_time: Instant,
    last_velocity: f32,
}

impl SpringSimulation {
    /// Creates a new spring simulation moving from `start_value` toward `target`, starting with
    /// `initial_velocity`
    #[allow(dead_code)]
    pub fn new(
        start_value: f32,
        initial_velocity: f32,
        target: f32,
        parameters: impl SpringParameters,
    ) -> Self {
        Self::new_internal(
            start_value,
            initial_velocity,
            target,
            parameters,
            crate::animations::current_tick(),
        )
    }

    #[allow(dead_code)]
    fn new_internal(
        start_value: f32,
        initial_velocity: f32,
        target: f32,
        parameters: impl SpringParameters,
        start_time: Instant,
    ) -> Self {
        let (w_n, zeta) = parameters.to_natural_frequency_and_damping_ratio();
        debug_assert!(w_n > 0., "natural frequency must be greater than zero");
        debug_assert!(zeta >= 0., "damping ratio must not be negative");
        let x0 = start_value - target;
        let regime = SpringRegime::new(x0, initial_velocity, w_n, zeta);
        Self {
            target,
            w_n,
            zeta,
            regime,
            start_time,
            last_velocity: initial_velocity,
        }
    }

    fn step_internal(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        let t = new_tick.duration_since(self.start_time).as_secs_f32();
        let (rel_pos, rel_vel) = self.regime.evaluate(self.w_n, self.zeta, t);
        self.last_velocity = rel_vel;
        *current = self.target + rel_pos;

        let settled = f32::abs(rel_pos) < SPRING_SETTLE_POSITION_EPSILON
            && f32::abs(rel_vel) < SPRING_SETTLE_VELOCITY_EPSILON;
        if settled {
            *current = self.target;
            self.last_velocity = 0.;
        }
        settled
    }
}

impl Simulation for SpringSimulation {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        self.step_internal(current, new_tick)
    }

    fn velocity(&self) -> f32 {
        self.last_velocity
    }
}

#[cfg(test)]
mod tests_spring_simulation {
    use super::*;
    use core::time::Duration;

    #[test]
    fn duration_bounce_conversion_edge_cases() {
        let (_, zeta) = SpringDurationBounceParameters::new(1., 0.)
            .to_natural_frequency_and_damping_ratio();
        assert_approx_eq!(zeta, 1.); // critically damped

        let (_, zeta) = SpringDurationBounceParameters::new(1., 1.)
            .to_natural_frequency_and_damping_ratio();
        assert_approx_eq!(zeta, 0.); // undamped boundary of underdamped

        let (_, zeta) = SpringDurationBounceParameters::new(1., -1.)
            .to_natural_frequency_and_damping_ratio();
        assert_approx_eq!(zeta, 2.); // overdamped

        let (w_n, _) = SpringDurationBounceParameters::new(2., 0.)
            .to_natural_frequency_and_damping_ratio();
        assert_approx_eq!(w_n, core::f32::consts::PI);
    }

    #[test]
    fn physical_conversion_matches_textbook_formula() {
        let (w_n, zeta) =
            SpringPhysicalParameters::new(2., 8., 4.).to_natural_frequency_and_damping_ratio();
        assert_approx_eq!(w_n, 2.); // sqrt(8/2)
        assert_approx_eq!(zeta, 4. / (2. * f32::sqrt(2. * 8.)));
    }

    #[test]
    fn underdamped_matches_closed_form() {
        const START: f32 = 0.;
        const TARGET: f32 = 100.;
        const INITIAL_VELOCITY: f32 = 0.;
        let (w_n, zeta) = SpringDurationBounceParameters::new(1., 0.5)
            .to_natural_frequency_and_damping_ratio();
        assert!(zeta < 1.);

        let time = Instant::now();
        let mut sim = SpringSimulation::new_internal(
            START,
            INITIAL_VELOCITY,
            TARGET,
            SpringPhysicalParameters::new(1., w_n * w_n, 2. * zeta * w_n),
            time,
        );
        let mut current = START;
        let t = Duration::from_millis(50);
        let finished = sim.step(&mut current, time + t);
        assert_eq!(finished, false);

        // Independently re-derive the expected value/velocity from the same closed form.
        let x0 = START - TARGET;
        let w_d = w_n * f32::sqrt(1. - zeta * zeta);
        let c1 = x0;
        let c2 = (INITIAL_VELOCITY + zeta * w_n * x0) / w_d;
        let tt = t.as_secs_f32();
        let decay = f32::exp(-zeta * w_n * tt);
        let (s, c) = f32::sin_cos(w_d * tt);
        let expected_pos = TARGET + decay * (c1 * c + c2 * s);
        let expected_vel = decay
            * ((-zeta * w_n * c1 + w_d * c2) * c + (-zeta * w_n * c2 - w_d * c1) * s);

        assert_approx_eq!(current, expected_pos);
        assert_approx_eq!(sim.velocity(), expected_vel);
    }

    #[test]
    fn critically_damped_matches_closed_form() {
        const START: f32 = 0.;
        const TARGET: f32 = 100.;
        const INITIAL_VELOCITY: f32 = 0.;
        let (w_n, zeta) = SpringDurationBounceParameters::new(1., 0.)
            .to_natural_frequency_and_damping_ratio();
        assert_approx_eq!(zeta, 1.);

        let time = Instant::now();
        let mut sim = SpringSimulation::new_internal(
            START,
            INITIAL_VELOCITY,
            TARGET,
            SpringDurationBounceParameters::new(1., 0.),
            time,
        );
        let mut current = START;
        let t = Duration::from_millis(50);
        let finished = sim.step(&mut current, time + t);
        assert_eq!(finished, false);

        let x0 = START - TARGET;
        let c1 = x0;
        let c2 = INITIAL_VELOCITY + w_n * x0;
        let tt = t.as_secs_f32();
        let decay = f32::exp(-w_n * tt);
        let expected_pos = TARGET + decay * (c1 + c2 * tt);
        let expected_vel = decay * (c2 - w_n * (c1 + c2 * tt));

        assert_approx_eq!(current, expected_pos);
        assert_approx_eq!(sim.velocity(), expected_vel);
    }

    #[test]
    fn overdamped_matches_closed_form() {
        const START: f32 = 0.;
        const TARGET: f32 = 100.;
        const INITIAL_VELOCITY: f32 = 0.;
        let (w_n, zeta) = SpringDurationBounceParameters::new(1., -0.5)
            .to_natural_frequency_and_damping_ratio();
        assert!(zeta > 1.);

        let time = Instant::now();
        let mut sim = SpringSimulation::new_internal(
            START,
            INITIAL_VELOCITY,
            TARGET,
            SpringDurationBounceParameters::new(1., -0.5),
            time,
        );
        let mut current = START;
        let t = Duration::from_millis(50);
        let finished = sim.step(&mut current, time + t);
        assert_eq!(finished, false);

        let x0 = START - TARGET;
        let disc = f32::sqrt(zeta * zeta - 1.);
        let r1 = w_n * (-zeta + disc);
        let r2 = w_n * (-zeta - disc);
        let c1 = (INITIAL_VELOCITY - r2 * x0) / (r1 - r2);
        let c2 = x0 - c1;
        let tt = t.as_secs_f32();
        let expected_pos = TARGET + c1 * f32::exp(r1 * tt) + c2 * f32::exp(r2 * tt);
        let expected_vel = c1 * r1 * f32::exp(r1 * tt) + c2 * r2 * f32::exp(r2 * tt);

        assert_approx_eq!(current, expected_pos);
        assert_approx_eq!(sim.velocity(), expected_vel);
    }

    #[test]
    fn settles_near_target_and_not_before() {
        const START: f32 = 0.;
        const TARGET: f32 = 100.;
        let time = Instant::now();
        let mut sim = SpringSimulation::new_internal(
            START,
            0.,
            TARGET,
            SpringDurationBounceParameters::new(0.3, 0.),
            time,
        );
        let mut current = START;

        // Not settled almost immediately.
        let finished = sim.step(&mut current, time + Duration::from_millis(1));
        assert_eq!(finished, false);

        // Settled well after the spring's characteristic duration.
        let finished = sim.step(&mut current, time + Duration::from_secs(10));
        assert_eq!(finished, true);
        assert_eq!(current, TARGET);
        assert_eq!(sim.velocity(), 0.);
    }

    /// Retargeting mid-flight: seed a fresh simulation with the outgoing one's position/velocity
    /// at the splice point and assert there is no discontinuity
    #[test]
    fn velocity_handoff_has_no_discontinuity() {
        const START: f32 = 0.;
        const TARGET_A: f32 = 100.;
        const TARGET_B: f32 = -50.;
        let time = Instant::now();
        let mut sim_a = SpringSimulation::new_internal(
            START,
            0.,
            TARGET_A,
            SpringDurationBounceParameters::new(1., 0.5),
            time,
        );
        let mut current_a = START;
        let splice = time + Duration::from_millis(120);
        sim_a.step(&mut current_a, splice);

        // Build the replacement "in place": same position and velocity, new target.
        let mut sim_b = SpringSimulation::new_internal(
            current_a,
            sim_a.velocity(),
            TARGET_B,
            SpringDurationBounceParameters::new(1., 0.5),
            splice,
        );
        let mut current_b = current_a;
        // Step by (almost) zero time: should reproduce the same starting position/velocity.
        sim_b.step(&mut current_b, splice + Duration::from_micros(1));

        assert_approx_eq!(current_b, current_a);
        assert_approx_eq!(sim_b.velocity(), sim_a.velocity());
    }
}
