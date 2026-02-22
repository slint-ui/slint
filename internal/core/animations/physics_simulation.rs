// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::{
    Coord,
    animations::{self, Instant},
};
use euclid::{Length, Scale};

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

#[derive(Debug, Clone)]
pub struct ConstantDecelerationParameters<DestUnit> {
    pub initial_velocity: Length<f32, DestUnit>,
    pub deceleration: Scale<f32, Seconds, DestUnit>,
}

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
        initial_velocity: Length<f32, Unit>,
        data: ConstantDecelerationParameters<Unit>,
        start_time: Instant,
    ) -> Self {
        let direction =
            if start_value < limit_value { Direction::Increasing } else { Direction::Decreasing };

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

        let velocity_loss = f32::abs((duration * self.data.deceleration).0);
        let new_velocity = if self.velocity.0 > 0. {
            self.velocity.0 - velocity_loss
        } else {
            self.velocity.0 + velocity_loss
        };

        self.curr_val += Length::new(
            (duration * Scale::<f32, Seconds, Unit>::new((self.velocity.0 + new_velocity) / 2.)).0
                as Coord,
        ); // Trapezoidal integration
        self.velocity = Length::new(new_velocity);

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

    /// We don't reach the position limit. Before the velocity gets zero
    /// start_value < limit_value
    #[test]
    fn constant_deceleration_increasing_limit_not_reached() {
        let initial_velocity = 50.;
        let deceleration = 20.;
        let parameters = ConstantDecelerationParameters::<LogicalPx> {
            initial_velocity: Length::new(initial_velocity),
            deceleration: Scale::new(deceleration),
        };

        let mut time = Instant::now();
        let mut simulation = ConstantDeceleration::new_internal(
            Length::new(10.),
            Length::new(2000.),
            parameters.initial_velocity,
            parameters,
            time.clone(),
        );

        let mut duration = Duration::from_secs(1);
        assert!(deceleration * duration.as_secs_f32() < initial_velocity);
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, false);
        assert_eq!(
            res.0,
            10. + 50. * duration.as_secs_f32()
                - 0.5 * deceleration * duration.as_secs_f32().powi(2)
        );

        duration = Duration::from_hours(10);
        assert!(Duration::from_secs((initial_velocity / deceleration) as u64) < duration);
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, true);
        assert_eq!(
            res.0,
            10. + 50. * initial_velocity / deceleration
                - 0.5 * deceleration * (initial_velocity / deceleration).powi(2)
        );

        assert!(res.0 < 2000.); // We reached velocity zero before we reached the position limit
    }

    /// We reach the position limit before the velocity got zero
    #[test]
    fn constant_deceleration_increasing_limit_reached() {
        let initial_velocity = 50.;
        let deceleration = 20.;
        let parameters = ConstantDecelerationParameters::<LogicalPx> {
            initial_velocity: Length::new(initial_velocity),
            deceleration: Scale::new(deceleration),
        };

        let mut time = Instant::now();
        let mut simulation = ConstantDeceleration::new_internal(
            Length::new(10.),
            Length::new(20.),
            parameters.initial_velocity,
            parameters,
            time.clone(),
        );

        let duration = Duration::from_secs(3);
        assert!(deceleration * duration.as_secs_f32() > initial_velocity); // We don't reach the limit where the velocity gets zero
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, true);
        assert_eq!(res.0, 20.); // Limit reached
    }

    /// We don't reach the position limit. Before the velocity gets zero
    /// start_value > limit_value
    #[test]
    fn constant_deceleration_decreasing_limit_not_reached() {
        let start_value = 2000.;
        let limit_value = 10.;
        let initial_velocity = -50.;
        let deceleration = 20.;

        let parameters = ConstantDecelerationParameters::<LogicalPx> {
            initial_velocity: Length::new(initial_velocity),
            deceleration: Scale::new(deceleration),
        };

        let mut time = Instant::now();
        let mut simulation = ConstantDeceleration::new_internal(
            Length::new(start_value),
            Length::new(limit_value),
            parameters.initial_velocity,
            parameters,
            time.clone(),
        );

        let mut duration = Duration::from_secs(1);
        assert!(f32::abs(deceleration * duration.as_secs_f32()) < f32::abs(initial_velocity));
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, false);
        assert_eq!(
            res.0,
            start_value + initial_velocity * duration.as_secs_f32()
                - initial_velocity.signum() * 0.5 * deceleration * duration.as_secs_f32().powi(2)
        );

        duration = Duration::from_hours(10);
        assert!(Duration::from_secs((initial_velocity / deceleration) as u64) < duration);
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, true);
        assert_eq!(
            res.0,
            start_value + initial_velocity * f32::abs(initial_velocity / deceleration)
                - 0.5
                    * initial_velocity.signum()
                    * deceleration
                    * (initial_velocity / deceleration).powi(2)
        );

        assert!(res.0 > limit_value); // We reached velocity zero before we reached the position limit
    }

    /// We reach the position limit before the velocity got zero
    /// start_value > limit_value
    #[test]
    fn constant_deceleration_decreasing_limit_reached() {
        let start_value = 20.;
        let limit_value = 10.;
        let initial_velocity = -50.;
        let deceleration = 20.;
        let parameters = ConstantDecelerationParameters::<LogicalPx> {
            initial_velocity: Length::new(initial_velocity),
            deceleration: Scale::new(deceleration),
        };

        let mut time = Instant::now();
        let mut simulation = ConstantDeceleration::new_internal(
            Length::new(start_value),
            Length::new(limit_value),
            parameters.initial_velocity,
            parameters,
            time.clone(),
        );

        let duration = Duration::from_secs(3);
        assert!(f32::abs(deceleration * duration.as_secs_f32()) > f32::abs(initial_velocity)); // We don't reach the limit where the velocity gets zero
        time += duration;
        let (res, finished) = simulation.step_internal(time);
        assert_eq!(finished, true);
        assert_eq!(res.0, limit_value); // Limit reached
    }
}
