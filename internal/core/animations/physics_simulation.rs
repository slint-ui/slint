// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
use crate::{
    Coord,
    animations::{self, Instant},
};
use core::{f32::consts::PI, time::Duration};
use euclid::{Length, Scale};
#[cfg(not(feature = "std"))]
use num_traits::Float;

pub enum Seconds {}
type Time = Length<f32, Seconds>;

#[derive(Debug)]
enum Direction {
    Increasing,
    Decreasing,
}

pub trait Simulation<Unit> {
    fn step(&mut self) -> (Length<Coord, Unit>, bool);
    fn curr_value(&self) -> Length<Coord, Unit>;
}

pub trait Parameter<Unit> {
    type Output;
    fn simulation(
        self,
        start_value: Length<Coord, Unit>,
        limit_value: Length<Coord, Unit>,
    ) -> Self::Output;
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ConstantDecelerationParameters<DestUnit> {
    pub initial_velocity: Length<f32, DestUnit>,
    pub deceleration: Scale<f32, Seconds, DestUnit>,
}

#[allow(dead_code)]
impl<DestUnit> Parameter<DestUnit> for ConstantDecelerationParameters<DestUnit> {
    type Output = ConstantDeceleration<DestUnit>;
    fn simulation(
        self,
        start_value: Length<Coord, DestUnit>,
        limit_value: Length<Coord, DestUnit>,
    ) -> Self::Output {
        let initial_velocity = self.initial_velocity.clone();
        ConstantDeceleration::new(start_value, limit_value, initial_velocity, self)
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ConstantDeceleration<Unit> {
    /// If the limit is not reached, it is also fine. Also exceeding the limit can be ok,
    /// but at the end of the animation the limit shall not be exceeded
    limit_value: Length<Coord, Unit>,
    curr_val: Length<Coord, Unit>,
    velocity: Length<f32, Unit>,
    data: ConstantDecelerationParameters<Unit>,
    direction: Direction,
    start_time: Instant,
}

#[allow(dead_code)]
impl<Unit> ConstantDeceleration<Unit> {
    pub fn new(
        start_value: Length<Coord, Unit>,
        limit_value: Length<Coord, Unit>,
        initial_velocity: Length<f32, Unit>,
        data: ConstantDecelerationParameters<Unit>,
    ) -> Self {
        Self::new_internal(
            start_value,
            limit_value,
            initial_velocity,
            data,
            crate::animations::current_tick(),
        )
    }

    fn new_internal(
        start_value: Length<Coord, Unit>,
        limit_value: Length<Coord, Unit>,
        mut initial_velocity: Length<f32, Unit>,
        mut data: ConstantDecelerationParameters<Unit>,
        start_time: Instant,
    ) -> Self {
        let direction = if start_value == limit_value {
            if initial_velocity.0 >= 0. {
                data.deceleration = Scale::new(f32::abs(data.deceleration.0));
                Direction::Increasing
            } else {
                data.deceleration = Scale::new(-f32::abs(data.deceleration.0));
                Direction::Decreasing
            }
        } else if start_value < limit_value {
            data.deceleration = Scale::new(f32::abs(data.deceleration.0));
            assert!(initial_velocity.0 >= 0.); // Makes no sense yet that the velocity goes into the other direction
            initial_velocity = Length::new(f32::abs(initial_velocity.0));
            Direction::Increasing
        } else {
            data.deceleration = Scale::new(-f32::abs(data.deceleration.0));
            initial_velocity = Length::new(-f32::abs(initial_velocity.0));
            assert!(initial_velocity.0 <= 0.);
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

    fn step_internal(&mut self, new_tick: Instant) -> (Length<Coord, Unit>, bool) {
        // We have to prevent go go beyond the limit where velocity gets zero
        let duration = Time::new(f32::min(
            new_tick.duration_since(self.start_time).as_secs_f32(),
            f32::abs((self.velocity / self.data.deceleration).0),
        ));

        self.start_time = new_tick;

        let new_velocity = self.velocity - duration * self.data.deceleration;

        self.curr_val += Length::new(
            (duration * Scale::<f32, Seconds, Unit>::new((self.velocity + new_velocity).0 / 2.)).0
                as Coord,
        ); // Trapezoidal integration
        self.velocity = new_velocity;

        match self.direction {
            Direction::Increasing => {
                if self.curr_val >= self.limit_value {
                    self.curr_val = self.limit_value;
                    self.velocity = Length::new(0.);
                    return (self.curr_val, true);
                } else if self.velocity.0 <= 0. {
                    return (self.curr_val, true);
                }
            }
            Direction::Decreasing => {
                if self.curr_val <= self.limit_value {
                    self.curr_val = self.limit_value;
                    self.velocity = Length::new(0.);
                    return (self.curr_val, true);
                } else if self.velocity.0 >= 0. {
                    return (self.curr_val, true);
                }
            }
        }
        (self.curr_val, false)
    }
}

#[allow(dead_code)]
impl<Unit> Simulation<Unit> for ConstantDeceleration<Unit> {
    fn curr_value(&self) -> Length<Coord, Unit> {
        self.curr_val
    }

    fn step(&mut self) -> (Length<Coord, Unit>, bool) {
        let new_tick = animations::current_tick();
        self.step_internal(new_tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lengths::LogicalPx;
    use core::time::Duration;

    #[test]
    fn constant_deceleration_start_eq_limit() {
        const START_VALUE: f32 = 10.;
        const LIMIT_VALUE: f32 = 10.;
        const INITIAL_VELOCITY: f32 = 50.;
        const DECELERATION: f32 = 20.;
        let parameters = ConstantDecelerationParameters::<LogicalPx> {
            initial_velocity: Length::new(INITIAL_VELOCITY),
            deceleration: Scale::new(DECELERATION),
        };

        let time = Instant::now();
        let mut simulation = ConstantDeceleration::new_internal(
            Length::new(START_VALUE),
            Length::new(LIMIT_VALUE),
            parameters.initial_velocity,
            parameters,
            time.clone(),
        );

        let res = simulation.step_internal(time + Duration::from_hours(10));
        assert_eq!(res.1, true);
        assert_eq!(res.0, Length::new(START_VALUE));
    }

    /// The velocity becomes zero before we are reaching the limit
    /// start_value < limit_value
    #[test]
    fn constant_deceleration_increasing_limit_not_reached() {
        const START_VALUE: f32 = 10.;
        const LIMIT_VALUE: f32 = 2000.;
        const INITIAL_VELOCITY: f32 = 50.;
        const DECELERATION: f32 = 20.;
        let parameters = ConstantDecelerationParameters::<LogicalPx> {
            initial_velocity: Length::new(INITIAL_VELOCITY),
            deceleration: Scale::new(DECELERATION),
        };

        let mut time = Instant::now();
        let mut simulation = ConstantDeceleration::new_internal(
            Length::new(START_VALUE),
            Length::new(LIMIT_VALUE),
            parameters.initial_velocity,
            parameters,
            time.clone(),
        );

        // Velocity does not become zero
        let mut duration = Duration::from_secs(1);
        assert!(DECELERATION * duration.as_secs_f32() < INITIAL_VELOCITY);
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, false);
        assert_eq!(
            res.0,
            START_VALUE + INITIAL_VELOCITY * duration.as_secs_f32()
                - 0.5 * DECELERATION * duration.as_secs_f32().powi(2)
        );

        // Now the velocity becomes zero and we don't do any further calculations
        duration = Duration::from_hours(10);
        assert!(Duration::from_secs((INITIAL_VELOCITY / DECELERATION) as u64) < duration);
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, true);
        assert_eq!(
            res.0,
            START_VALUE + INITIAL_VELOCITY * INITIAL_VELOCITY / DECELERATION
                - 0.5 * DECELERATION * (INITIAL_VELOCITY / DECELERATION).powi(2)
        );

        assert!(res.0 < LIMIT_VALUE); // We reached velocity zero before we reached the position limit
    }

    /// We reach the position limit before the velocity got zero
    #[test]
    fn constant_deceleration_increasing_limit_reached() {
        const START_VALUE: f32 = 10.;
        const LIMIT_VALUE: f32 = 20.;
        const INITIAL_VELOCITY: f32 = 50.;
        const DECELERATION: f32 = 20.;
        let parameters = ConstantDecelerationParameters::<LogicalPx> {
            initial_velocity: Length::new(INITIAL_VELOCITY),
            deceleration: Scale::new(DECELERATION),
        };

        let mut time = Instant::now();
        let mut simulation = ConstantDeceleration::new_internal(
            Length::new(START_VALUE),
            Length::new(LIMIT_VALUE),
            parameters.initial_velocity,
            parameters,
            time.clone(),
        );

        let duration = Duration::from_secs(1);
        assert!(f32::abs(DECELERATION * duration.as_secs_f32()) < f32::abs(INITIAL_VELOCITY)); // We don't reach the limit where the velocity gets zero
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, true);
        assert_eq!(res.0, LIMIT_VALUE); // Limit reached
    }

    /// We don't reach the position limit. Before the velocity gets zero
    /// start_value > limit_value
    #[test]
    fn constant_deceleration_decreasing_limit_not_reached() {
        const START_VALUE: f32 = 2000.;
        const LIMIT_VALUE: f32 = 10.;
        const INITIAL_VELOCITY: f32 = -50.;
        const DECELERATION: f32 = 20.;

        let parameters = ConstantDecelerationParameters::<LogicalPx> {
            initial_velocity: Length::new(INITIAL_VELOCITY),
            deceleration: Scale::new(DECELERATION),
        };

        let mut time = Instant::now();
        let mut simulation = ConstantDeceleration::new_internal(
            Length::new(START_VALUE),
            Length::new(LIMIT_VALUE),
            parameters.initial_velocity,
            parameters,
            time.clone(),
        );

        let mut duration = Duration::from_secs(1);
        assert!(f32::abs(DECELERATION * duration.as_secs_f32()) < f32::abs(INITIAL_VELOCITY));
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, false);
        assert_eq!(
            res.0,
            START_VALUE + INITIAL_VELOCITY * duration.as_secs_f32()
                - INITIAL_VELOCITY.signum() * 0.5 * DECELERATION * duration.as_secs_f32().powi(2)
        );

        duration = Duration::from_hours(10);
        assert!(Duration::from_secs((INITIAL_VELOCITY / DECELERATION) as u64) < duration);
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, true);
        assert_eq!(
            res.0,
            START_VALUE + INITIAL_VELOCITY * f32::abs(INITIAL_VELOCITY / DECELERATION)
                - 0.5
                    * INITIAL_VELOCITY.signum()
                    * DECELERATION
                    * (INITIAL_VELOCITY / DECELERATION).powi(2)
        );

        assert!(res.0 > LIMIT_VALUE); // We reached velocity zero before we reached the position limit
    }

    /// We reach the position limit before the velocity got zero
    /// start_value > limit_value
    #[test]
    fn constant_deceleration_decreasing_limit_reached() {
        const START_VALUE: f32 = 20.;
        const LIMIT_VALUE: f32 = 10.;
        const INITIAL_VELOCITY: f32 = -50.;
        const DECELERATION: f32 = 20.;
        let parameters = ConstantDecelerationParameters::<LogicalPx> {
            initial_velocity: Length::new(INITIAL_VELOCITY),
            deceleration: Scale::new(DECELERATION),
        };

        let mut time = Instant::now();
        let mut simulation = ConstantDeceleration::new_internal(
            Length::new(START_VALUE),
            Length::new(LIMIT_VALUE),
            parameters.initial_velocity,
            parameters,
            time.clone(),
        );

        let duration = Duration::from_secs(3);
        assert!(f32::abs(DECELERATION * duration.as_secs_f32()) > f32::abs(INITIAL_VELOCITY)); // We don't reach the limit where the velocity gets zero
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, true);
        assert_eq!(res.0, LIMIT_VALUE); // Limit reached
    }
}

/// [1] https://www.maplesoft.com/content/EngineeringFundamentals/6/MapleDocument_32/Free%20Response%20Part%202.pdf
#[derive(Debug, Clone)]
pub struct ConstantDecelerationSpringDamperParameters<DestUnit> {
    pub initial_velocity: Length<f32, DestUnit>,
    pub deceleration: Scale<f32, Seconds, DestUnit>,
    pub mass: f32,                // Scale<f32, Seconds, DestUnit>, [1] parameter m
    pub spring_constant: f32,     // Scale<f32, Seconds, DestUnit>, [1] parameter k
    pub damping_coefficient: f32, // Scale<f32, Seconds, DestUnit>, [1] parameter c
}

impl<DestUnit> ConstantDecelerationSpringDamperParameters<DestUnit> {
    pub fn new(
        initial_velocity: Length<f32, DestUnit>,
        deceleration: Scale<f32, Seconds, DestUnit>,
        half_period_time: f32,
    ) -> Self {
        let (mass, spring_constant, damping_coefficient) =
            Self::calculate_parameters(half_period_time);

        Self { initial_velocity, deceleration, mass, spring_constant, damping_coefficient }
    }

    fn calculate_parameters(half_period_time: f32) -> (f32, f32, f32) {
        // [1] eq 13
        const MASS: f32 = 1.;
        const DAMPING_COEFFICIENT: f32 = 1.;
        let w_d = 2. * PI * 1. / (2. * half_period_time);
        let spring_constant = w_d.powi(2) + DAMPING_COEFFICIENT.powi(2) / (4. * MASS.powi(2));

        (MASS, spring_constant, DAMPING_COEFFICIENT)
    }
}

impl<DestUnit> Parameter<DestUnit> for ConstantDecelerationSpringDamperParameters<DestUnit> {
    type Output = ConstantDecelerationSpringDamper<DestUnit>;
    fn simulation(
        self,
        start_value: Length<Coord, DestUnit>,
        limit_value: Length<Coord, DestUnit>,
    ) -> Self::Output {
        let initial_velocity = self.initial_velocity.clone();
        ConstantDecelerationSpringDamper::new(start_value, limit_value, initial_velocity, self)
    }
}

#[derive(Debug, PartialEq)]
enum State {
    Deceleration,
    SpringDamper,
    Done,
}

#[derive(Debug)]
pub struct ConstantDecelerationSpringDamper<Unit> {
    /// If the limit is not reached, it is also fine. Also exceeding the limit can be ok,
    /// but at the end of the animation the limit shall not be exceeded
    limit_value: Length<Coord, Unit>,
    curr_val_zeroed: Length<Coord, Unit>,
    curr_val: Length<Coord, Unit>,
    velocity: Length<f32, Unit>,
    data: ConstantDecelerationSpringDamperParameters<Unit>,
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

impl<Unit> ConstantDecelerationSpringDamper<Unit> {
    pub fn new(
        start_value: Length<Coord, Unit>,
        limit_value: Length<Coord, Unit>,
        initial_velocity: Length<f32, Unit>,
        data: ConstantDecelerationSpringDamperParameters<Unit>,
    ) -> Self {
        Self::new_internal(
            start_value,
            limit_value,
            initial_velocity,
            data,
            crate::animations::current_tick(),
        )
    }

    fn new_internal(
        start_value: Length<Coord, Unit>,
        limit_value: Length<Coord, Unit>,
        mut initial_velocity: Length<f32, Unit>,
        mut data: ConstantDecelerationSpringDamperParameters<Unit>,
        start_time: Instant,
    ) -> Self {
        let mut state = State::Deceleration;
        let direction = if start_value == limit_value {
            state = State::Done;
            if initial_velocity.0 >= 0. {
                data.deceleration = Scale::new(f32::abs(data.deceleration.0));
                Direction::Increasing
            } else {
                data.deceleration = Scale::new(-f32::abs(data.deceleration.0));
                Direction::Decreasing
            }
        } else if start_value < limit_value {
            data.deceleration = Scale::new(f32::abs(data.deceleration.0));
            assert!(initial_velocity.0 >= 0.); // Makes no sense yet that the velocity goes into the other direction
            initial_velocity = Length::new(f32::abs(initial_velocity.0));
            Direction::Increasing
        } else {
            data.deceleration = Scale::new(-f32::abs(data.deceleration.0));
            initial_velocity = Length::new(-f32::abs(initial_velocity.0));
            assert!(initial_velocity.0 <= 0.);
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
            curr_val_zeroed: Length::new(0. as Coord),
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

    fn step_internal(&mut self, new_tick: Instant) -> (Length<Coord, Unit>, bool) {
        match self.state {
            State::Deceleration => self.state_deceleration(new_tick),
            State::SpringDamper => self.state_spring_damper(new_tick),
            State::Done => (self.curr_value(), true),
        }
    }

    fn state_deceleration(&mut self, new_tick: Instant) -> (Length<Coord, Unit>, bool) {
        let duration_unlimited = new_tick.duration_since(self.start_time);
        // We have to prevent go go beyond the limit where velocity gets zero
        let duration = Time::new(f32::min(
            duration_unlimited.as_secs_f32(),
            f32::abs((self.velocity / self.data.deceleration).0),
        ));

        self.start_time = new_tick;

        let new_velocity = self.velocity - (duration * self.data.deceleration);
        let new_val = self.curr_val
            + Length::new(
                (duration
                    * Scale::<f32, Seconds, Unit>::new((self.velocity.0 + new_velocity.0) / 2.))
                .0 as Coord,
            ); // Trapezoidal integration

        enum S {
            LimitReached,
            VelocityZero,
            None,
        }

        let s = match self.direction {
            Direction::Increasing if new_val > self.limit_value => S::LimitReached,
            Direction::Increasing if new_velocity.0 <= 0. => S::VelocityZero,
            Direction::Decreasing if new_val < self.limit_value => S::LimitReached,
            Direction::Decreasing if new_velocity.0 >= 0. => S::VelocityZero,
            _ => S::None,
        };
        match s {
            S::LimitReached => {
                self.state = State::SpringDamper;

                // time when reaching the limit
                // solving p_limit = p_old + v_old * dt - 0.5 * a * dt^2
                let root = f32::sqrt(
                    self.velocity.0.powi(2)
                        - self.data.deceleration.0 * (self.limit_value - self.curr_val).0 as f32,
                );
                // The smaller is the relevant. The larger is when the initial velocity got zero and due to the constant acceleration we turn
                let dt = f32::min(
                    (self.velocity.0 - root) / self.data.deceleration.0,
                    (self.velocity.0 + root) / self.data.deceleration.0,
                );

                self.velocity = self.velocity - Time::new(dt) * self.data.deceleration; // Velocity at limit value point. Solved `new_val` equation for new_velocity
                self.curr_val_zeroed = Length::new(0. as Coord);
                self.curr_val = self.limit_value;

                const X0: f32 = 0.; // Relative point
                self.constant_a = self.velocity.0.signum()
                    * f32::sqrt(
                        (self.w_d.powi(2) * X0.powi(2)
                            + (self.velocity.0 + self.damping_ratio * self.w_n * 0.).powi(2))
                            / self.w_d.powi(2),
                    );
                self.constant_phi = f32::atan(
                    self.w_d * X0 / (self.velocity.0 + self.damping_ratio * self.w_n * X0),
                );
                return self.state_spring_damper(
                    new_tick + (duration_unlimited - Duration::from_millis((dt * 1000.) as u64)),
                );
            }
            S::VelocityZero => {
                self.velocity = Length::new(0.);
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

    fn state_spring_damper(&mut self, new_tick: Instant) -> (Length<Coord, Unit>, bool) {
        // Here we use absolute time because it simplifies the equation
        let t = (new_tick - self.start_time).as_secs_f32();
        // Underdamped spring damper equation
        assert!(self.damping_ratio < 1.);
        let new_val = self.constant_a
            * f32::exp(-self.damping_ratio * self.w_n * t)
            * f32::sin(self.w_d * t + self.constant_phi);
        self.curr_val_zeroed = Length::new(new_val as Coord); // relative value

        let max_time = 2. * PI / self.w_d;
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
            self.velocity = Length::new(0.);
            self.curr_val = self.limit_value;
            self.curr_val_zeroed = Length::new(0. as Coord);
            self.state = State::Done;
        }
        (current_val, finished)
    }
}

impl<Unit> Simulation<Unit> for ConstantDecelerationSpringDamper<Unit> {
    fn curr_value(&self) -> Length<Coord, Unit> {
        self.curr_val + self.curr_val_zeroed
    }

    fn step(&mut self) -> (Length<Coord, Unit>, bool) {
        let new_tick = animations::current_tick();
        self.step_internal(new_tick)
    }
}

#[cfg(test)]
mod tests_spring_damper {
    use super::*;
    use crate::lengths::LogicalPx;
    use core::{f32::consts::PI, time::Duration};

    #[test]
    fn calculate_parameters() {
        const INITIAL_VELOCITY: Length<f32, LogicalPx> = Length::new(50.);
        const DECELERATION: Scale<f32, Seconds, LogicalPx> = Scale::new(20.);
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
        let parameters = ConstantDecelerationSpringDamperParameters::<LogicalPx>::new(
            Length::new(INITIAL_VELOCITY),
            Scale::new(DECELERATION),
            HALF_PERIOD_TIME,
        );

        assert_eq!(START_VALUE, LIMIT_VALUE);
        let time = Instant::now();
        let mut simulation = ConstantDecelerationSpringDamper::new_internal(
            Length::new(START_VALUE),
            Length::new(LIMIT_VALUE),
            parameters.initial_velocity,
            parameters,
            time.clone(),
        );
        let res = simulation.step_internal(time);
        assert_eq!(res.0, Length::new(START_VALUE));
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
        let parameters = ConstantDecelerationSpringDamperParameters::<LogicalPx>::new(
            Length::new(INITIAL_VELOCITY),
            Scale::new(DECELERATION),
            HALF_PERIOD_TIME,
        );

        let mut time = Instant::now();
        let mut simulation = ConstantDecelerationSpringDamper::new_internal(
            Length::new(10.),
            Length::new(2000.),
            parameters.initial_velocity,
            parameters,
            time.clone(),
        );

        // Velocity does not become zero
        let mut duration = Duration::from_secs(1);
        assert!(DECELERATION * duration.as_secs_f32() < INITIAL_VELOCITY);
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, false);
        assert_eq!(
            res.0,
            10. + 50. * duration.as_secs_f32()
                - 0.5 * DECELERATION * duration.as_secs_f32().powi(2)
        );

        // Now the velocity becomes zero and we don't do any further calculations
        duration = Duration::from_hours(10);
        assert!(Duration::from_secs((INITIAL_VELOCITY / DECELERATION) as u64) < duration);
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, true);
        assert_eq!(
            res.0,
            10. + 50. * INITIAL_VELOCITY / DECELERATION
                - 0.5 * DECELERATION * (INITIAL_VELOCITY / DECELERATION).powi(2)
        );

        assert!(res.0 < 2000.); // We reached velocity zero before we reached the position limit
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

        let parameters = ConstantDecelerationSpringDamperParameters::<LogicalPx>::new(
            Length::new(INITIAL_VELOCITY),
            Scale::new(DECELERATION),
            HALF_PERIOD_TIME,
        );

        let mut time = Instant::now();
        let mut simulation = ConstantDecelerationSpringDamper::new_internal(
            Length::new(START_VALUE),
            Length::new(LIMIT_VALUE),
            parameters.initial_velocity,
            parameters,
            time.clone(),
        );

        let mut duration = Duration::from_secs(1);
        assert!(f32::abs(DECELERATION * duration.as_secs_f32()) < f32::abs(INITIAL_VELOCITY));
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, false);
        assert_eq!(
            res.0,
            START_VALUE + INITIAL_VELOCITY * duration.as_secs_f32()
                - INITIAL_VELOCITY.signum() * 0.5 * DECELERATION * duration.as_secs_f32().powi(2)
        );

        duration = Duration::from_hours(10);
        assert!(Duration::from_secs((INITIAL_VELOCITY / DECELERATION) as u64) < duration);
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, true);
        assert_eq!(
            res.0,
            START_VALUE + INITIAL_VELOCITY * f32::abs(INITIAL_VELOCITY / DECELERATION)
                - 0.5
                    * INITIAL_VELOCITY.signum()
                    * DECELERATION
                    * (INITIAL_VELOCITY / DECELERATION).powi(2)
        );

        assert!(res.0 > LIMIT_VALUE); // We reached velocity zero before we reached the position limit
    }

    /// We reach the position limit before the velocity got zero and so we run into the spring damper system
    /// Increasing case: start_value < limit_value
    #[test]
    fn constant_deceleration_spring_damper_increasing_limit_reached() {
        const INITIAL_VELOCITY: Length<f32, LogicalPx> = Length::new(50.);
        const DECELERATION: Scale<f32, Seconds, LogicalPx> = Scale::new(20.);
        const HALF_PERIOD_TIME: f32 = 10.;
        const START_VALUE: f32 = 10.;
        const LIMIT_VALUE: f32 = 70.;
        let parameters = super::ConstantDecelerationSpringDamperParameters::<LogicalPx>::new(
            INITIAL_VELOCITY,
            DECELERATION,
            HALF_PERIOD_TIME,
        );

        let mut time = Instant::now();
        let mut simulation = ConstantDecelerationSpringDamper::new_internal(
            Length::new(START_VALUE),
            Length::new(LIMIT_VALUE),
            parameters.initial_velocity,
            parameters,
            time.clone(),
        );

        let duration = Duration::from_secs(1);
        assert!(f32::abs(DECELERATION.0) * duration.as_secs_f32() < f32::abs(INITIAL_VELOCITY.0)); // We don't reach the limit where the velocity gets zero
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, false);
        assert_eq!(simulation.state, State::Deceleration);
        assert!(res.0 < LIMIT_VALUE); // We are still in the constant deceleration state

        time += Duration::from_secs((HALF_PERIOD_TIME / 2.) as u64);
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, false);
        assert_eq!(simulation.state, State::SpringDamper);
        assert!(res.0 > LIMIT_VALUE);

        time += Duration::from_hours(10);
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, true);
        assert_eq!(simulation.state, State::Done);
        assert_eq!(res.0, LIMIT_VALUE);
    }

    /// We reach the position limit before the velocity got zero and so we run into the spring damper system
    /// Decreasing case. limit_value < start_value
    #[test]
    fn constant_deceleration_spring_damper_decreasing_limit_reached() {
        const INITIAL_VELOCITY: Length<f32, LogicalPx> = Length::new(-50.);
        const DECELERATION: Scale<f32, Seconds, LogicalPx> = Scale::new(20.);
        const HALF_PERIOD_TIME: f32 = 10.;
        const START_VALUE: f32 = 70.;
        const LIMIT_VALUE: f32 = 10.;
        let parameters = super::ConstantDecelerationSpringDamperParameters::<LogicalPx>::new(
            INITIAL_VELOCITY,
            DECELERATION,
            HALF_PERIOD_TIME,
        );

        let mut time = Instant::now();
        let mut simulation = ConstantDecelerationSpringDamper::new_internal(
            Length::new(START_VALUE),
            Length::new(LIMIT_VALUE),
            parameters.initial_velocity,
            parameters,
            time.clone(),
        );

        let duration = Duration::from_secs(1);
        assert!(f32::abs(DECELERATION.0) * duration.as_secs_f32() < f32::abs(INITIAL_VELOCITY.0)); // We don't reach the limit where the velocity gets zero
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, false);
        assert_eq!(simulation.state, State::Deceleration);
        assert!(res.0 > LIMIT_VALUE); // We are still in the constant deceleration state

        time += Duration::from_secs((HALF_PERIOD_TIME / 2.) as u64);
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, false);
        assert_eq!(simulation.state, State::SpringDamper);
        assert!(res.0 < LIMIT_VALUE);

        time += Duration::from_hours(10);
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, true);
        assert_eq!(simulation.state, State::Done);
        assert_eq!(res.0, LIMIT_VALUE);
    }
}
