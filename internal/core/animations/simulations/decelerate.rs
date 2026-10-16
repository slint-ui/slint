// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::animations::Instant;
use crate::animations::simulations::{PositionSimulation, Simulation};
#[cfg(not(feature = "std"))]
use num_traits::Float;

/// Slows down from a speed, and stops after a distance.
/// Without a retarget, the speed follows a smoothstep curve down to zero.
#[derive(Debug)]
pub struct Decelerate {
    remaining: f32,
    speed: f32,
    deceleration: f32,
    last_tick: Instant,
    finished: bool,
}

impl Decelerate {
    /// `distance` is the signed distance to the target, `speed` the absolute start speed.
    pub fn new(distance: f32, speed: f32) -> Self {
        Self::new_internal(distance, speed, crate::animations::current_tick())
    }

    fn new_internal(distance: f32, speed: f32, start_time: Instant) -> Self {
        Self {
            remaining: distance,
            speed,
            deceleration: 0.,
            last_tick: start_time,
            finished: distance == 0.,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// The tick of the last step, which shows whether a binding still drives the simulation.
    pub fn last_tick(&self) -> Instant {
        self.last_tick
    }

    /// Replaces the signed distance to the target, keeping the speed and deceleration.
    /// A target behind the position is reached in the next step.
    pub fn set_remaining_distance(&mut self, remaining: f32) {
        if remaining.signum() != self.remaining.signum() {
            self.speed = 0.;
            self.deceleration = 0.;
        }
        self.remaining = remaining;
        self.finished |= remaining == 0.;
    }
}

impl Simulation for Decelerate {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        if self.finished {
            return true;
        }
        let dt = new_tick.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = new_tick;
        let distance = self.remaining.abs();
        let speed = self.speed;
        // The speed follows the cubic that starts at the current speed and deceleration,
        // and ends with zero speed and deceleration after `distance`.
        // Its duration solves `distance = duration * speed / 2 - duration² * deceleration / 12`.
        let mut discriminant = speed * speed / 4. - self.deceleration * distance / 3.;
        if discriminant < 0. {
            // The target moved beyond where the current deceleration stops.
            self.deceleration = 0.;
            discriminant = speed * speed / 4.;
        }
        let duration = 2. * distance / (speed / 2. + discriminant.sqrt());
        let u = dt / duration;
        let mut traveled = distance;
        if speed > 0. && u < 1. {
            let m = self.deceleration * duration;
            let u2 = u * u;
            traveled = duration
                * (speed * u * (1. - u2 + u2 * u / 2.)
                    - m * u2 * (1. / 2. - 2. * u / 3. + u2 / 4.));
            self.speed = (1. - u) * (1. - u) * (speed * (1. + 2. * u) - m * u);
            self.deceleration =
                (1. - u) * (6. * speed * u / duration + self.deceleration * (1. - 3. * u));
        }
        if traveled >= distance {
            traveled = distance;
            self.speed = 0.;
            self.deceleration = 0.;
            self.finished = true;
        }
        let direction = self.remaining.signum();
        // Apply increments: virtualized views may move the content between frames.
        *current += direction * traveled;
        self.remaining -= direction * traveled;
        self.finished
    }
}

impl PositionSimulation for Decelerate {
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
    use crate::animations::simulations::assert_approx_eq;
    use alloc::vec::Vec;
    use core::time::Duration;

    const FRAME: Duration = Duration::from_millis(16);

    #[test]
    fn stops_on_the_target_while_slowing_down() {
        for sign in [-1., 1.] {
            let distance = sign * 1800.;
            let start = Instant::default();
            let mut simulation = Decelerate::new_internal(distance, 4800., start);
            let mut position = 0.;
            let mut steps = Vec::new();
            let mut tick = start;
            loop {
                tick = tick + FRAME;
                let before = position;
                let finished = simulation.step(&mut position, tick);
                steps.push(position - before);
                if finished {
                    break;
                }
            }
            assert!((position - distance).abs() < 1e-2);
            assert!(steps.iter().all(|s| s * sign >= 0.));
            assert!(steps.windows(2).all(|w| w[1].abs() <= w[0].abs() + 1e-3));
            assert_eq!(simulation.remaining_velocity(tick), 0.);
        }
    }

    #[test]
    fn follows_a_smoothstep_curve() {
        let (distance, speed) = (1800., 4800.);
        // Twice as long as at full speed, like a constant deceleration.
        let duration = 2. * distance / speed;
        let start = Instant::default();
        let mut simulation = Decelerate::new_internal(distance, speed, start);
        let mut position = 0.;
        let mut tick = start;
        loop {
            tick = tick + FRAME;
            if simulation.step(&mut position, tick) {
                break;
            }
            let u = tick.duration_since(start).as_secs_f32() / duration;
            let expected = speed * duration * u * (1. - u * u + u * u * u / 2.);
            assert!((position - expected).abs() < 0.1, "{u}: {position} != {expected}");
            if tick == start + FRAME {
                assert!(simulation.remaining_velocity(tick) > 0.99 * speed);
            }
        }
        let elapsed = tick.duration_since(start).as_secs_f32();
        assert!(elapsed >= duration && elapsed < duration + FRAME.as_secs_f32(), "{elapsed}");
        assert_approx_eq!(position, distance);
    }

    #[test]
    fn retarget_keeps_the_speed() {
        for factor in [0.5, 4.] {
            let start = Instant::default();
            let mut simulation = Decelerate::new_internal(-1000., 4000., start);
            let mut position = 0.;
            let mut tick = start;
            let mut previous = 0.;
            while tick.duration_since(start).as_millis() < 250 {
                tick = tick + FRAME;
                let before = position;
                simulation.step(&mut position, tick);
                previous = position - before;
            }
            let target = position + factor * simulation.remaining_distance(tick);
            simulation.set_remaining_distance(target - position);
            let mut steps = Vec::new();
            loop {
                tick = tick + FRAME;
                let before = position;
                let finished = simulation.step(&mut position, tick);
                steps.push(position - before);
                if finished {
                    break;
                }
            }
            assert!((position - target).abs() < 1e-2, "{factor}: {position} != {target}");
            assert!(
                steps[0] / previous > 0.8 && steps[0] / previous <= 1.,
                "{factor}: {previous} {steps:?}"
            );
            assert!(steps.iter().all(|s| *s <= 0.), "{factor}: {steps:?}");
            assert!(
                steps.windows(2).all(|w| w[1].abs() <= w[0].abs() + 1e-3),
                "{factor}: {steps:?}"
            );
        }
    }

    #[test]
    fn retarget_lands_on_the_new_target() {
        let start = Instant::default();
        let mut simulation = Decelerate::new_internal(-1000., 4000., start);
        let mut position = 0.;
        simulation.step(&mut position, start + FRAME);
        assert_eq!(simulation.remaining_distance(start + FRAME), -1000. - position);
        let target = -1200.;
        simulation.set_remaining_distance(target - position);
        let mut tick = start + FRAME;
        while !simulation.step(&mut position, tick) {
            tick = tick + FRAME;
        }
        assert!((position - target).abs() < 1e-2);
    }

    #[test]
    fn zero_distance_is_finished() {
        let mut position = 5.;
        let mut simulation = Decelerate::new_internal(0., 100., Instant::default());
        assert!(simulation.step(&mut position, Instant::default() + FRAME));
        assert_eq!(position, 5.);
    }
}
