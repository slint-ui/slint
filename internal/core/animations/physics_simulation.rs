// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains varios physics simulations which can be used as animation (internally only yet).
//! Currently it is used in the flickable to animate the viewport position
//!
//! Currently it contains two simulations:
//! - `ConstantDeceleration`
//! - `ConstantDecelerationSpringDamper` with spring damper simulation when reaching the limit

use crate::animations::Instant;
#[cfg(all(not(feature = "std"), test))]
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
    fn step(&mut self, new_tick: Instant) -> (f32, bool);
    fn curr_value(&self) -> f32;
}

/// Trait to convert parameter objects into a simulation
/// All parameter objects must implement this trait!
pub trait Parameter {
    type Output;
    fn simulation(self, start_value: f32, limit_value: f32) -> Self::Output;
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
}

impl Parameter for ConstantDecelerationParameters {
    type Output = ConstantDeceleration;
    fn simulation(self, start_value: f32, limit_value: f32) -> Self::Output {
        ConstantDeceleration::new(start_value, limit_value, self)
    }
}

/// This simulation simulates a constant deceleration of a point starting at position `start_value` with
/// an initial velocity of `initial_velocity`. When the point reaches the limit value `limit_value` it stops there
#[derive(Debug)]
pub struct ConstantDeceleration {
    /// If the limit is not reached, it is also fine. Also exceeding the limit can be ok,
    /// but at the end of the animation the limit shall not be exceeded
    limit_value: f32,
    curr_val: f32,
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
    pub fn new(start_value: f32, limit_value: f32, data: ConstantDecelerationParameters) -> Self {
        Self::new_internal(start_value, limit_value, data, crate::animations::current_tick())
    }

    fn new_internal(
        start_value: f32,
        limit_value: f32,
        mut data: ConstantDecelerationParameters,
        start_time: Instant,
    ) -> Self {
        let mut initial_velocity = data.initial_velocity;
        let direction = if start_value == limit_value {
            if initial_velocity >= 0. {
                data.deceleration = f32::abs(data.deceleration);
                Direction::Increasing
            } else {
                data.deceleration = -f32::abs(data.deceleration);
                Direction::Decreasing
            }
        } else if start_value < limit_value {
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

        Self {
            limit_value,
            curr_val: start_value,
            velocity: initial_velocity,
            data,
            direction,
            start_time,
        }
    }

    fn step_internal(&mut self, new_tick: Instant) -> (f32, bool) {
        // We have to prevent go go beyond the limit where velocity gets zero
        let duration = f32::min(
            new_tick.duration_since(self.start_time).as_secs_f32(),
            f32::abs(self.velocity / self.data.deceleration),
        );

        self.start_time = new_tick;

        let new_velocity = self.velocity - duration * self.data.deceleration;

        self.curr_val += duration * (self.velocity + new_velocity) / 2.; // Trapezoidal integration
        self.velocity = new_velocity;

        match self.direction {
            Direction::Increasing => {
                if self.curr_val >= self.limit_value {
                    self.curr_val = self.limit_value;
                    self.velocity = 0.;
                    return (self.curr_val, true);
                } else if self.velocity <= 0. {
                    return (self.curr_val, true);
                }
            }
            Direction::Decreasing => {
                if self.curr_val <= self.limit_value {
                    self.curr_val = self.limit_value;
                    self.velocity = 0.;
                    return (self.curr_val, true);
                } else if self.velocity >= 0. {
                    return (self.curr_val, true);
                }
            }
        }
        (self.curr_val, false)
    }
}

impl Simulation for ConstantDeceleration {
    fn curr_value(&self) -> f32 {
        self.curr_val
    }

    fn step(&mut self, new_tick: Instant) -> (f32, bool) {
        self.step_internal(new_tick)
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
        let parameters = ConstantDecelerationParameters {
            initial_velocity: INITIAL_VELOCITY,
            deceleration: DECELERATION,
        };

        let time = Instant::now();
        let mut simulation =
            ConstantDeceleration::new_internal(START_VALUE, LIMIT_VALUE, parameters, time.clone());

        let res = simulation.step(time + Duration::from_hours(10));
        assert_eq!(res.1, true);
        assert_eq!(res.0, START_VALUE);
    }

    /// The velocity becomes zero before we are reaching the limit
    /// start_value < limit_value
    #[test]
    fn constant_deceleration_increasing_limit_not_reached() {
        const START_VALUE: f32 = 10.;
        const LIMIT_VALUE: f32 = 2000.;
        const INITIAL_VELOCITY: f32 = 50.;
        const DECELERATION: f32 = 20.;
        let parameters = ConstantDecelerationParameters {
            initial_velocity: INITIAL_VELOCITY,
            deceleration: DECELERATION,
        };

        let mut time = Instant::now();
        let mut simulation =
            ConstantDeceleration::new_internal(START_VALUE, LIMIT_VALUE, parameters, time.clone());

        // Velocity does not become zero
        let mut duration = Duration::from_secs(1);
        assert!(DECELERATION * duration.as_secs_f32() < INITIAL_VELOCITY);
        time += duration;
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, false);
        assert_eq!(
            res,
            START_VALUE + INITIAL_VELOCITY * duration.as_secs_f32()
                - 0.5 * DECELERATION * duration.as_secs_f32().powi(2)
        );

        // Now the velocity becomes zero and we don't do any further calculations
        duration = Duration::from_hours(10);
        assert!(Duration::from_secs((INITIAL_VELOCITY / DECELERATION) as u64) < duration);
        time += duration;
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, true);
        assert_eq!(
            res,
            START_VALUE + INITIAL_VELOCITY * INITIAL_VELOCITY / DECELERATION
                - 0.5 * DECELERATION * (INITIAL_VELOCITY / DECELERATION).powi(2)
        );

        assert!(res < LIMIT_VALUE); // We reached velocity zero before we reached the position limit
    }

    /// We reach the position limit before the velocity got zero
    #[test]
    fn constant_deceleration_increasing_limit_reached() {
        const START_VALUE: f32 = 10.;
        const LIMIT_VALUE: f32 = 20.;
        const INITIAL_VELOCITY: f32 = 50.;
        const DECELERATION: f32 = 20.;
        let parameters = ConstantDecelerationParameters {
            initial_velocity: INITIAL_VELOCITY,
            deceleration: DECELERATION,
        };

        let mut time = Instant::now();
        let mut simulation =
            ConstantDeceleration::new_internal(START_VALUE, LIMIT_VALUE, parameters, time.clone());

        let duration = Duration::from_secs(1);
        assert!(f32::abs(DECELERATION * duration.as_secs_f32()) < f32::abs(INITIAL_VELOCITY)); // We don't reach the limit where the velocity gets zero
        time += duration;
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, true);
        assert_eq!(res, LIMIT_VALUE); // Limit reached
    }

    /// We don't reach the position limit. Before the velocity gets zero
    /// start_value > limit_value
    #[test]
    fn constant_deceleration_decreasing_limit_not_reached() {
        const START_VALUE: f32 = 2000.;
        const LIMIT_VALUE: f32 = 10.;
        const INITIAL_VELOCITY: f32 = -50.;
        const DECELERATION: f32 = 20.;

        let parameters = ConstantDecelerationParameters {
            initial_velocity: INITIAL_VELOCITY,
            deceleration: DECELERATION,
        };

        let mut time = Instant::now();
        let mut simulation =
            ConstantDeceleration::new_internal(START_VALUE, LIMIT_VALUE, parameters, time.clone());

        let mut duration = Duration::from_secs(1);
        assert!(f32::abs(DECELERATION * duration.as_secs_f32()) < f32::abs(INITIAL_VELOCITY));
        time += duration;
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, false);
        assert_eq!(
            res,
            START_VALUE + INITIAL_VELOCITY * duration.as_secs_f32()
                - INITIAL_VELOCITY.signum() * 0.5 * DECELERATION * duration.as_secs_f32().powi(2)
        );

        duration = Duration::from_hours(10);
        assert!(Duration::from_secs((INITIAL_VELOCITY / DECELERATION) as u64) < duration);
        time += duration;
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, true);
        assert_eq!(
            res,
            START_VALUE + INITIAL_VELOCITY * f32::abs(INITIAL_VELOCITY / DECELERATION)
                - 0.5
                    * INITIAL_VELOCITY.signum()
                    * DECELERATION
                    * (INITIAL_VELOCITY / DECELERATION).powi(2)
        );

        assert!(res > LIMIT_VALUE); // We reached velocity zero before we reached the position limit
    }

    /// We reach the position limit before the velocity got zero
    /// start_value > limit_value
    #[test]
    fn constant_deceleration_decreasing_limit_reached() {
        const START_VALUE: f32 = 20.;
        const LIMIT_VALUE: f32 = 10.;
        const INITIAL_VELOCITY: f32 = -50.;
        const DECELERATION: f32 = 20.;
        let parameters = ConstantDecelerationParameters {
            initial_velocity: INITIAL_VELOCITY,
            deceleration: DECELERATION,
        };

        let mut time = Instant::now();
        let mut simulation =
            ConstantDeceleration::new_internal(START_VALUE, LIMIT_VALUE, parameters, time.clone());

        let duration = Duration::from_secs(3);
        assert!(f32::abs(DECELERATION * duration.as_secs_f32()) > f32::abs(INITIAL_VELOCITY)); // We don't reach the limit where the velocity gets zero
        time += duration;
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, true);
        assert_eq!(res, LIMIT_VALUE); // Limit reached
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
    /// It is more comfortable to use than specifiying the parameters manually because here the parameter calculation
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
    fn simulation(self, start_value: f32, limit_value: f32) -> Self::Output {
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
    limit_value: f32,
    curr_val_zeroed: f32,
    curr_val: f32,
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
        limit_value: f32,
        data: ConstantDecelerationSpringDamperParameters,
    ) -> Self {
        Self::new_internal(start_value, limit_value, data, crate::animations::current_tick())
    }

    fn new_internal(
        start_value: f32,
        limit_value: f32,
        mut data: ConstantDecelerationSpringDamperParameters,
        start_time: Instant,
    ) -> Self {
        let mut initial_velocity = data.initial_velocity;
        let mut state = State::Deceleration;
        let direction = if start_value == limit_value {
            state = State::Done;
            if initial_velocity >= 0. {
                data.deceleration = f32::abs(data.deceleration);
                Direction::Increasing
            } else {
                data.deceleration = -f32::abs(data.deceleration);
                Direction::Decreasing
            }
        } else if start_value < limit_value {
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
            curr_val: start_value,
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

    fn step_internal(&mut self, new_tick: Instant) -> (f32, bool) {
        match self.state {
            State::Deceleration => self.state_deceleration(new_tick),
            State::SpringDamper => self.state_spring_damper(new_tick),
            State::Done => (self.curr_value(), true),
        }
    }

    fn state_deceleration(&mut self, new_tick: Instant) -> (f32, bool) {
        let duration_unlimited = new_tick.duration_since(self.start_time);
        // We have to prevent go go beyond the limit where velocity gets zero
        let duration = f32::min(
            duration_unlimited.as_secs_f32(),
            f32::abs(self.velocity / self.data.deceleration),
        );

        self.start_time = new_tick;

        let new_velocity = self.velocity - (duration * self.data.deceleration);
        let new_val = self.curr_val + (duration * (self.velocity + new_velocity) / 2.); // Trapezoidal integration

        enum S {
            LimitReached,
            VelocityZero,
            None,
        }

        let s = match self.direction {
            Direction::Increasing if new_val > self.limit_value => S::LimitReached,
            Direction::Increasing if new_velocity <= 0. => S::VelocityZero,
            Direction::Decreasing if new_val < self.limit_value => S::LimitReached,
            Direction::Decreasing if new_velocity >= 0. => S::VelocityZero,
            _ => S::None,
        };
        match s {
            S::LimitReached => {
                self.state = State::SpringDamper;

                // time when reaching the limit
                // solving p_limit = p_old + v_old * dt - 0.5 * a * dt^2
                let root = f32::sqrt(
                    self.velocity.powi(2)
                        - self.data.deceleration * (self.limit_value - self.curr_val) as f32,
                );
                // The smaller is the relevant. The larger is when the initial velocity got zero and due to the constant acceleration we turn
                let dt = f32::min(
                    (self.velocity - root) / self.data.deceleration,
                    (self.velocity + root) / self.data.deceleration,
                );

                self.velocity = self.velocity - dt * self.data.deceleration; // Velocity at limit value point. Solved `new_val` equation for new_velocity
                self.curr_val_zeroed = 0.;
                self.curr_val = self.limit_value;

                const X0: f32 = 0.; // Relative point
                self.constant_a = self.velocity.signum()
                    * f32::sqrt(
                        (self.w_d.powi(2) * X0.powi(2)
                            + (self.velocity + self.damping_ratio * self.w_n * 0.).powi(2))
                            / self.w_d.powi(2),
                    );
                self.constant_phi =
                    f32::atan(self.w_d * X0 / (self.velocity + self.damping_ratio * self.w_n * X0));
                return self.state_spring_damper(
                    new_tick
                        + (duration_unlimited
                            - core::time::Duration::from_millis((dt * 1000.) as u64)),
                );
            }
            S::VelocityZero => {
                self.velocity = 0.;
                self.curr_val = new_val;
                return (self.curr_val, true);
            }
            S::None => {
                self.velocity = new_velocity;
                self.curr_val = new_val;
                (self.curr_val, false)
            }
        }
    }

    fn state_spring_damper(&mut self, new_tick: Instant) -> (f32, bool) {
        // Here we use absolute time because it simplifies the equation
        let t = (new_tick - self.start_time).as_secs_f32();
        // Underdamped spring damper equation
        assert!(self.damping_ratio < 1.);
        let new_val = self.constant_a
            * f32::exp(-self.damping_ratio * self.w_n * t)
            * f32::sin(self.w_d * t + self.constant_phi);
        self.curr_val_zeroed = new_val; // relative value

        let max_time = 2. * core::f32::consts::PI / self.w_d;
        let current_val = self.curr_value();
        let finished = match self.direction {
            Direction::Increasing => {
                // We are comming back from a value higher than the limit
                if current_val < self.limit_value || t > max_time { true } else { false }
            }
            Direction::Decreasing => {
                // We are comming back from a value lower than the limit
                if current_val > self.limit_value || t > max_time { true } else { false }
            }
        };
        if finished {
            self.velocity = 0.;
            self.curr_val = self.limit_value;
            self.curr_val_zeroed = 0.;
            self.state = State::Done;
        }
        (current_val, finished)
    }
}

#[cfg(test)]
impl Simulation for ConstantDecelerationSpringDamper {
    fn curr_value(&self) -> f32 {
        self.curr_val + self.curr_val_zeroed
    }

    fn step(&mut self, new_tick: Instant) -> (f32, bool) {
        self.step_internal(new_tick)
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
        assert_eq!(w_d, 2. * PI * 1. / (2. * HALF_PERIOD_TIME));
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
            LIMIT_VALUE,
            parameters,
            time.clone(),
        );
        let res = simulation.step(time);
        assert_eq!(res.0, START_VALUE);
        assert_eq!(res.1, true);
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
        let mut simulation =
            ConstantDecelerationSpringDamper::new_internal(10., 2000., parameters, time.clone());

        // Velocity does not become zero
        let mut duration = Duration::from_secs(1);
        assert!(DECELERATION * duration.as_secs_f32() < INITIAL_VELOCITY);
        time += duration;
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, false);
        assert_eq!(
            res,
            10. + 50. * duration.as_secs_f32()
                - 0.5 * DECELERATION * duration.as_secs_f32().powi(2)
        );

        // Now the velocity becomes zero and we don't do any further calculations
        duration = Duration::from_hours(10);
        assert!(Duration::from_secs((INITIAL_VELOCITY / DECELERATION) as u64) < duration);
        time += duration;
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, true);
        assert_eq!(
            res,
            10. + 50. * INITIAL_VELOCITY / DECELERATION
                - 0.5 * DECELERATION * (INITIAL_VELOCITY / DECELERATION).powi(2)
        );

        assert!(res < 2000.); // We reached velocity zero before we reached the position limit
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
            LIMIT_VALUE,
            parameters,
            time.clone(),
        );

        let mut duration = Duration::from_secs(1);
        assert!(f32::abs(DECELERATION * duration.as_secs_f32()) < f32::abs(INITIAL_VELOCITY));
        time += duration;
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, false);
        assert_eq!(
            res,
            START_VALUE + INITIAL_VELOCITY * duration.as_secs_f32()
                - INITIAL_VELOCITY.signum() * 0.5 * DECELERATION * duration.as_secs_f32().powi(2)
        );

        duration = Duration::from_hours(10);
        assert!(Duration::from_secs((INITIAL_VELOCITY / DECELERATION) as u64) < duration);
        time += duration;
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, true);
        assert_eq!(
            res,
            START_VALUE + INITIAL_VELOCITY * f32::abs(INITIAL_VELOCITY / DECELERATION)
                - 0.5
                    * INITIAL_VELOCITY.signum()
                    * DECELERATION
                    * (INITIAL_VELOCITY / DECELERATION).powi(2)
        );

        assert!(res > LIMIT_VALUE); // We reached velocity zero before we reached the position limit
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
            LIMIT_VALUE,
            parameters,
            time.clone(),
        );

        let duration = Duration::from_secs(1);
        assert!(f32::abs(DECELERATION) * duration.as_secs_f32() < f32::abs(INITIAL_VELOCITY)); // We don't reach the limit where the velocity gets zero
        time += duration;
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, false);
        assert_eq!(simulation.state, State::Deceleration);
        assert!(res < LIMIT_VALUE); // We are still in the constant deceleration state

        time += Duration::from_secs((HALF_PERIOD_TIME / 2.) as u64);
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, false);
        assert_eq!(simulation.state, State::SpringDamper);
        assert!(res > LIMIT_VALUE);

        time += Duration::from_hours(10);
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, true);
        assert_eq!(simulation.state, State::Done);
        assert_eq!(res, LIMIT_VALUE);
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
            LIMIT_VALUE,
            parameters,
            time.clone(),
        );

        let duration = Duration::from_secs(1);
        assert!(f32::abs(DECELERATION) * duration.as_secs_f32() < f32::abs(INITIAL_VELOCITY)); // We don't reach the limit where the velocity gets zero
        time += duration;
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, false);
        assert_eq!(simulation.state, State::Deceleration);
        assert!(res > LIMIT_VALUE); // We are still in the constant deceleration state

        time += Duration::from_secs((HALF_PERIOD_TIME / 2.) as u64);
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, false);
        assert_eq!(simulation.state, State::SpringDamper);
        assert!(res < LIMIT_VALUE);

        time += Duration::from_hours(10);
        let (res, finished) = simulation.step(time);
        assert_eq!(finished, true);
        assert_eq!(simulation.state, State::Done);
        assert_eq!(res, LIMIT_VALUE);
    }
}
