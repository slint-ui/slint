// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore signum underdamped

use crate::animations::Instant;
use crate::animations::simulations::spring::{SpringParameters, SpringRegime};
use crate::animations::simulations::{Direction, Parameter, Simulation};
#[cfg(not(feature = "std"))]
use num_traits::Float;

#[cfg(test)]
use crate::animations::simulations::{assert_approx_eq, test_limit_property};

/// Position epsilon (in the simulated property's units) below which the spring phase is
/// considered settled at the limit.
const SPRING_POSITION_EPSILON: f32 = 0.5;
/// Velocity epsilon below which the spring phase is considered settled at the limit.
const SPRING_VELOCITY_EPSILON: f32 = 5.0;
/// Safety cap on the spring phase's duration, in case the settling checks above never trigger
const SPRING_MAX_DURATION: f32 = 2.0;

/// Input parameters for the `ConstantDecelerationSpringDamper` simulation.
#[derive(Debug, Clone)]
pub struct ConstantDecelerationSpringDamperParameters {
    pub initial_velocity: f32,
    pub deceleration: f32,
    /// Whether the simulation starts already past `limit_value`, rather than approaching it.
    pub already_out_of_bounds: bool,
    w_n: f32,
    zeta: f32,
}

impl ConstantDecelerationSpringDamperParameters {
    pub fn new(
        initial_velocity: f32,
        deceleration: f32,
        already_out_of_bounds: bool,
        spring: impl SpringParameters,
    ) -> Self {
        let (w_n, zeta) = spring.to_natural_frequency_and_damping_ratio();
        Self { initial_velocity, deceleration, already_out_of_bounds, w_n, zeta }
    }
}

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

#[derive(Debug, PartialEq)]
enum State {
    Deceleration,
    Spring,
    Done,
}

/// Simulates a constant deceleration of a point starting at `start_value` with an initial
/// velocity, like `ConstantDeceleration`. But instead of clamping at `limit_value`, it lets the
/// point cross it and springs it back, so it settles at `limit_value` rather than stopping dead
/// against it.
#[derive(Debug)]
pub struct ConstantDecelerationSpringDamper {
    /// The value the simulation settles at; may keep changing over the simulation's lifetime,
    /// e.g. if the content driving it is resized mid-flight.
    limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    velocity: f32,
    deceleration: f32,
    direction: Direction,
    start_time: Instant,
    state: State,
    w_n: f32,
    zeta: f32,
    /// Only set once `state` is `Spring`.
    spring: Option<SpringRegime>,
    /// The sign the spring phase's relative position started on; it's finished once that sign
    /// flips (or the position/velocity settle near zero, for a spring that never crosses back).
    spring_away_sign: f32,
}

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
        data: ConstantDecelerationSpringDamperParameters,
        start_time: Instant,
    ) -> Self {
        let limit = limit_value.as_ref().get();

        let mut this = Self {
            limit_value,
            velocity: data.initial_velocity,
            deceleration: data.deceleration,
            direction: Direction::Increasing,
            start_time,
            state: State::Deceleration,
            w_n: data.w_n,
            zeta: data.zeta,
            spring: None,
            spring_away_sign: 0.,
        };

        if start_value == limit {
            this.state = State::Done;
        } else if data.already_out_of_bounds {
            this.enter_spring(start_value - limit, data.initial_velocity);
        } else {
            debug_assert!(
                data.initial_velocity != 0.,
                "a simulation that starts in bounds and isn't moving has nothing to animate"
            );
            if data.initial_velocity >= 0. {
                this.direction = Direction::Increasing;
                this.deceleration = f32::abs(data.deceleration);
            } else {
                this.direction = Direction::Decreasing;
                this.deceleration = -f32::abs(data.deceleration);
            }
        }

        this
    }

    /// Switches to the spring phase, with `x_rel` and `velocity` relative to `limit_value`.
    fn enter_spring(&mut self, x_rel: f32, velocity: f32) {
        self.spring_away_sign = if x_rel != 0. { x_rel.signum() } else { velocity.signum() };
        self.spring = Some(SpringRegime::new(x_rel, velocity, self.w_n, self.zeta));
        self.state = State::Spring;
    }

    fn step_internal(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        match self.state {
            State::Deceleration => self.state_deceleration(current, new_tick),
            State::Spring => self.state_spring(current, new_tick),
            State::Done => {
                *current = self.limit_value.as_ref().get();
                true
            }
        }
    }

    fn state_deceleration(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        let limit_value = self.limit_value.as_ref().get();
        let duration_unlimited = new_tick.duration_since(self.start_time);

        // We have to prevent going beyond the limit where velocity gets zero.
        let duration =
            f32::min(duration_unlimited.as_secs_f32(), f32::abs(self.velocity / self.deceleration));

        self.start_time = new_tick;

        let new_velocity = self.velocity - duration * self.deceleration;
        let new_val = *current + duration * (self.velocity + new_velocity) / 2.; // Trapezoidal integration

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
                // Solve for the time within this step at which the limit was crossed:
                // p_limit = p_old + v_old * dt - 0.5 * a * dt^2
                let root = f32::sqrt(
                    self.velocity.powi(2) - 2. * self.deceleration * (limit_value - *current),
                );
                // The smaller root is the relevant crossing; the larger one is where the
                // (unclamped) parabola would turn around and cross again.
                let dt = f32::min(
                    (self.velocity - root) / self.deceleration,
                    (self.velocity + root) / self.deceleration,
                )
                .clamp(0., duration_unlimited.as_secs_f32());

                let velocity_at_crossing = self.velocity - dt * self.deceleration;
                *current = limit_value;
                self.enter_spring(0., velocity_at_crossing);

                // Simulate the remainder of this tick's duration in the spring phase right away,
                // instead of losing it until the next tick.
                self.start_time =
                    new_tick - (duration_unlimited - core::time::Duration::from_secs_f32(dt));
                self.state_spring(current, new_tick)
            }
            S::VelocityZero => {
                self.velocity = 0.;
                *current = new_val;
                self.state = State::Done;
                true
            }
            S::None => {
                self.velocity = new_velocity;
                *current = new_val;
                false
            }
        }
    }

    fn state_spring(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        let limit_value = self.limit_value.as_ref().get();
        let t = new_tick.duration_since(self.start_time).as_secs_f32();
        let spring = self.spring.as_ref().expect("state_spring requires an active spring");
        let (x_rel, velocity) = spring.evaluate(t);

        let crossed_back = x_rel * self.spring_away_sign <= 0.;
        let settled = f32::abs(x_rel) < SPRING_POSITION_EPSILON
            && f32::abs(velocity) < SPRING_VELOCITY_EPSILON;
        let timed_out = t > SPRING_MAX_DURATION;

        if crossed_back || settled || timed_out {
            *current = limit_value;
            self.velocity = 0.;
            self.state = State::Done;
            true
        } else {
            *current = limit_value + x_rel;
            self.velocity = velocity;
            false
        }
    }
}

impl Simulation for ConstantDecelerationSpringDamper {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        self.step_internal(current, new_tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::simulations::spring::SpringDurationBounceParameters;
    use core::time::Duration;

    const SPRING: SpringDurationBounceParameters =
        SpringDurationBounceParameters { duration_secs: 0.3, bounce: 0.15 };

    #[test]
    fn start_eq_limit_is_done_immediately() {
        let parameters = ConstantDecelerationSpringDamperParameters::new(50., 20., false, SPRING);
        let time = Instant::default();
        let mut simulation = ConstantDecelerationSpringDamper::new_internal(
            10.,
            test_limit_property(10.),
            parameters,
            time,
        );
        let mut current = 10.;
        let finished = simulation.step(&mut current, time + Duration::from_hours(10));
        assert!(finished);
        assert_eq!(current, 10.);
    }

    #[test]
    fn velocity_zero_before_limit_reached_stays_in_bound() {
        let parameters = ConstantDecelerationSpringDamperParameters::new(50., 20., false, SPRING);
        let time = Instant::default();
        let mut simulation = ConstantDecelerationSpringDamper::new_internal(
            10.,
            test_limit_property(2000.),
            parameters,
            time,
        );
        let mut current = 10.;
        let finished = simulation.step(&mut current, time + Duration::from_hours(10));
        assert!(finished);
        assert!(current < 2000.);
    }

    #[test]
    fn overshoot_bounces_back_to_the_limit() {
        let parameters = ConstantDecelerationSpringDamperParameters::new(50., 20., false, SPRING);
        let time = Instant::default();
        let mut simulation = ConstantDecelerationSpringDamper::new_internal(
            10.,
            test_limit_property(20.),
            parameters,
            time,
        );
        let mut current = 10.;

        // Crosses the limit (10 units away, decelerating from 50 units/s at 20 units/s^2) well
        // before the velocity would naturally reach zero (at 2.5s): step in small increments and
        // check it passes through the spring phase, overshooting past 20, before settling there.
        let mut saw_spring_overshoot = false;
        let mut t = Duration::ZERO;
        loop {
            t += Duration::from_millis(5);
            let finished = simulation.step(&mut current, time + t);
            if simulation.state == State::Spring && current > 20. {
                saw_spring_overshoot = true;
            }
            if finished {
                break;
            }
            assert!(t < Duration::from_secs(10), "simulation should have settled by now");
        }
        assert!(saw_spring_overshoot);
        assert_approx_eq!(current, 20.);
    }

    #[test]
    fn already_out_of_bounds_springs_back_regardless_of_velocity_direction() {
        // Released while still drifting further out of bounds: the spring must still pull it back.
        let parameters = ConstantDecelerationSpringDamperParameters::new(-5., 20., true, SPRING);
        let time = Instant::default();
        let mut simulation = ConstantDecelerationSpringDamper::new_internal(
            -120.,
            test_limit_property(-100.),
            parameters,
            time,
        );
        assert_eq!(simulation.state, State::Spring);
        let mut current = -120.;
        let finished = simulation.step(&mut current, time + Duration::from_secs(10));
        assert!(finished);
        assert_approx_eq!(current, -100.);
    }

    #[test]
    fn already_out_of_bounds_at_rest_springs_back() {
        let parameters = ConstantDecelerationSpringDamperParameters::new(0., 20., true, SPRING);
        let time = Instant::default();
        let mut simulation = ConstantDecelerationSpringDamper::new_internal(
            -120.,
            test_limit_property(-100.),
            parameters,
            time,
        );
        let mut current = -120.;
        let finished = simulation.step(&mut current, time + Duration::from_secs(10));
        assert!(finished);
        assert_approx_eq!(current, -100.);
    }
}
