// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::animations::Instant;
use crate::animations::simulations::{PositionSimulation, Simulation};

/// Speeds up at a constant rate to a maximum speed, and keeps that speed.
/// It stops when it reaches the target.
#[derive(Debug)]
pub struct Accelerate {
    remaining: f32,
    speed: f32,
    max_speed: f32,
    acceleration: f32,
    last_tick: Instant,
    finished: bool,
}

impl Accelerate {
    /// `distance` is the signed distance to the target.
    /// `max_speed` and `acceleration` are absolute values.
    pub fn new(distance: f32, max_speed: f32, acceleration: f32) -> Self {
        Self::new_internal(distance, max_speed, acceleration, crate::animations::current_tick())
    }

    fn new_internal(distance: f32, max_speed: f32, acceleration: f32, start_time: Instant) -> Self {
        Self {
            remaining: distance,
            speed: 0.,
            max_speed,
            acceleration,
            last_tick: start_time,
            finished: distance == 0.,
        }
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn is_at_max_speed(&self) -> bool {
        self.speed >= self.max_speed
    }

    /// The distance it takes to stop from the current speed at `deceleration`.
    pub fn braking_distance(&self, deceleration: f32) -> f32 {
        self.speed * self.speed / (2. * deceleration)
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// The tick of the last step, which shows whether a binding still drives the simulation.
    pub fn last_tick(&self) -> Instant {
        self.last_tick
    }

    /// Replaces the signed distance to the target.
    /// A target behind the position starts over from rest in the other direction.
    pub fn set_remaining_distance(&mut self, remaining: f32) {
        if remaining.signum() != self.remaining.signum() {
            self.speed = 0.;
        }
        self.remaining = remaining;
        self.finished |= remaining == 0.;
    }
}

impl Simulation for Accelerate {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        if self.finished {
            return true;
        }
        let dt = new_tick.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = new_tick;
        let new_speed = (self.speed + self.acceleration * dt).min(self.max_speed);
        let mut traveled = (self.speed + new_speed) / 2. * dt;
        self.speed = new_speed;
        if traveled >= self.remaining.abs() {
            traveled = self.remaining.abs();
            self.finished = true;
        }
        let direction = self.remaining.signum();
        // Apply increments: virtualized views may move the content between frames.
        *current += direction * traveled;
        self.remaining -= direction * traveled;
        self.finished
    }
}

impl PositionSimulation for Accelerate {
    fn remaining_distance(&self, _now: Instant) -> f32 {
        self.remaining
    }

    fn remaining_velocity(&self, _now: Instant) -> f32 {
        if self.finished { 0. } else { self.remaining.signum() * self.speed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::time::Duration;

    const MAX_SPEED: f32 = 4000.;
    const ACCELERATION: f32 = 16000.;
    const FRAME: Duration = Duration::from_millis(16);

    #[test]
    fn reaches_the_max_speed_and_stops_on_the_target() {
        for sign in [-1., 1.] {
            let start = Instant::default();
            let distance = sign * 10_000.;
            let mut simulation = Accelerate::new_internal(distance, MAX_SPEED, ACCELERATION, start);
            let mut position = 0.;
            let mut tick = start + FRAME;
            simulation.step(&mut position, tick);
            assert!(!simulation.is_at_max_speed());
            assert!(position * sign > 0.);

            tick = tick + Duration::from_millis(250);
            simulation.step(&mut position, tick);
            assert!(simulation.is_at_max_speed());
            assert_eq!(simulation.speed(), MAX_SPEED);
            assert_eq!(simulation.remaining_velocity(tick), sign * MAX_SPEED);
            assert_eq!(simulation.braking_distance(MAX_SPEED), MAX_SPEED / 2.);

            while !simulation.step(&mut position, tick) {
                tick = tick + FRAME;
            }
            assert!((position - distance).abs() < 1e-2);
            assert_eq!(simulation.remaining_distance(tick), 0.);
        }
    }

    #[test]
    fn retarget_behind_the_position_starts_from_rest() {
        let start = Instant::default();
        let mut simulation = Accelerate::new_internal(-10_000., MAX_SPEED, ACCELERATION, start);
        let mut position = 0.;
        simulation.step(&mut position, start + FRAME * 10);
        simulation.set_remaining_distance(100.);
        assert_eq!(simulation.speed(), 0.);
        let target = position + 100.;
        let mut tick = start + FRAME * 10;
        while !simulation.step(&mut position, tick) {
            tick = tick + FRAME;
        }
        assert!((position - target).abs() < 1e-2);
    }

    #[test]
    fn zero_distance_is_finished() {
        let mut position = 5.;
        let mut simulation =
            Accelerate::new_internal(0., MAX_SPEED, ACCELERATION, Instant::default());
        assert!(simulation.step(&mut position, Instant::default() + FRAME));
        assert_eq!(position, 5.);
    }
}
