// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::animations::simulations::Direction;
use crate::animations::simulations::spring::{SpringParameters, SpringPhysicalParameters};
use crate::animations::{
    Instant, SPRING_SETTLE_POSITION_EPSILON, SPRING_SETTLE_VELOCITY_EPSILON,
    simulations::{PositionSimulation, Simulation, spring::SpringRegime},
};
use core::time::Duration;

#[derive(Debug)]
pub struct SpringFlick {
    limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    start_time: Instant,
    traveled: f32,
    data: SpringRegime,
    initial_velocity: f32,
    initial_deflection: f32,
    finished: bool,
    direction: Direction,
}

impl SpringFlick {
    pub fn new(
        deflection: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        initial_velocity: f32,
        data: SpringPhysicalParameters,
    ) -> Self {
        Self::new_internal(
            deflection,
            limit_value,
            initial_velocity,
            data,
            crate::animations::current_tick(),
        )
    }

    fn new_internal(
        deflection: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        mut initial_velocity: f32,
        data: SpringPhysicalParameters,
        start_time: Instant,
    ) -> Self {
        let finished = deflection.abs() < SPRING_SETTLE_POSITION_EPSILON;
        let direction = if deflection < 0. {
            // We are behind the zero point so we will move forward
            debug_assert!(initial_velocity >= 0.); // Makes no sense yet that the velocity goes into the other direction
            initial_velocity = f32::abs(initial_velocity);
            Direction::Increasing
        } else {
            debug_assert!(initial_velocity <= 0.); // Makes no sense yet that the velocity goes into the other direction
            initial_velocity = -f32::abs(initial_velocity);
            Direction::Decreasing
        };
        let (w_n, zeta) = data.to_natural_frequency_and_damping_ratio();
        let data = SpringRegime::new(deflection, initial_velocity, w_n, zeta);

        Self {
            limit_value,
            start_time,
            traveled: 0.,
            data,
            initial_velocity,
            finished,
            initial_deflection: deflection,
            direction,
        }
    }

    fn sample(&self, elapsed: Duration) -> (f32, f32) {
        if self.finished {
            return (self.data.evaluate(Duration::from_secs(10000).as_secs_f32()).0, 0.);
        }
        if elapsed.is_zero() {
            return (self.initial_deflection, self.initial_velocity);
        }
        let t = elapsed.as_secs_f32();
        self.data.evaluate(t)
    }

    fn step_internal(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        if self.finished {
            return true;
        }
        let elapsed = new_tick.duration_since(self.start_time);
        // `displacement` is relative to the spring's rest point (goes to 0 when settled)
        let (displacement, velocity) = self.sample(elapsed);

        // Apply increments: virtualized views may move the content between frames.
        let position = displacement - self.initial_deflection;
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
        // This works only for critical damped spring simulations, otherwise
        // whether the simulation never ends or we don't even reach the position
        if clamped
            || (displacement.abs() < SPRING_SETTLE_POSITION_EPSILON
                && velocity.abs() < SPRING_SETTLE_VELOCITY_EPSILON)
        {
            self.finished = true;
        }
        self.finished
    }
}

impl Simulation for SpringFlick {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        self.step_internal(current, new_tick)
    }
}

impl PositionSimulation for SpringFlick {
    fn remaining_distance(&self, now: Instant) -> f32 {
        let elapsed = now.duration_since(self.start_time);
        self.sample(elapsed).0 - self.initial_deflection
    }

    fn remaining_velocity(&self, now: Instant) -> f32 {
        self.sample(now.duration_since(self.start_time)).1
    }
}
