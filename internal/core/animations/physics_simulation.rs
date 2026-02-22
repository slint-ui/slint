// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::animations::{self, Instant};
use core::f32;
use euclid::{Length, Scale};

pub enum Seconds {}
type Time = Length<f32, Seconds>;

#[derive(Debug)]
enum Direction {
    Increasing,
    Decreasing,
}

pub trait Simulation<Unit> {
    fn step(&mut self) -> (Length<f32, Unit>, bool);
    fn curr_value(&self) -> Length<f32, Unit>;
}

pub trait Parameter<Unit> {
    type Output;
    fn simulation(
        self,
        start_value: Length<f32, Unit>,
        limit_value: Length<f32, Unit>,
    ) -> Self::Output;
}

#[derive(Debug, Clone)]
pub struct ConstantDecelerationParameters<DestUnit> {
    pub initial_velocity: Length<f32, DestUnit>,
    pub deceleration: Scale<f32, Seconds, DestUnit>,
}

impl<DestUnit> Parameter<DestUnit> for ConstantDecelerationParameters<DestUnit> {
    type Output = ConstantDeceleration<DestUnit>;
    fn simulation(
        self,
        start_value: Length<f32, DestUnit>,
        limit_value: Length<f32, DestUnit>,
    ) -> Self::Output {
        let initial_velocity = self.initial_velocity.clone();
        ConstantDeceleration::new(start_value, limit_value, initial_velocity, self)
    }
}

#[derive(Debug)]
pub struct ConstantDeceleration<Unit> {
    /// If the limit is not reached, it is also fine. Also exceeding the limit can be ok,
    /// but at the end of the animation the limit shall not be exceeded
    limit_value: Length<f32, Unit>,
    curr_val: Length<f32, Unit>,
    velocity: Length<f32, Unit>,
    data: ConstantDecelerationParameters<Unit>,
    direction: Direction,
    start_time: Instant,
}

impl<Unit> ConstantDeceleration<Unit> {
    pub fn new(
        start_value: Length<f32, Unit>,
        limit_value: Length<f32, Unit>,
        initial_velocity: Length<f32, Unit>,
        data: ConstantDecelerationParameters<Unit>,
    ) -> Self {
        let direction =
            if start_value < limit_value { Direction::Increasing } else { Direction::Decreasing };

        Self {
            limit_value,
            curr_val: start_value,
            velocity: initial_velocity,
            data,
            direction,
            start_time: crate::animations::current_tick(),
        }
    }
}

impl<Unit> Simulation<Unit> for ConstantDeceleration<Unit> {
    fn curr_value(&self) -> Length<f32, Unit> {
        self.curr_val
    }

    fn step(&mut self) -> (Length<f32, Unit>, bool) {
        let new_tick = animations::current_tick();
        let duration = new_tick.duration_since(self.start_time);
        self.start_time = new_tick;

        let duration = Time::new(duration.as_secs_f32());
        let velocity_loss = f32::abs((duration * self.data.deceleration).0);
        if self.velocity.0 > 0. {
            self.velocity -= Length::<f32, Unit>::new(velocity_loss);
        } else {
            self.velocity += Length::<f32, Unit>::new(velocity_loss);
        }

        self.curr_val += duration * Scale::new(self.velocity.0);

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
