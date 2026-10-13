// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::animations::Instant;
use crate::animations::simulations::{Direction, PositionSimulation, Simulation};
use core::f32::consts::PI;
use core::time::Duration;
#[cfg(not(feature = "std"))]
use num_traits::Float;

/// Moves a position by a fixed distance in a fixed duration along `(1 - cos(π t / duration)) / 2`.
///
/// UIKit's `setContentOffset(_:animated: true)` follows this curve, as measured in #13946.
#[derive(Debug)]
pub struct EaseInOutSine {
    limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    start_time: Instant,
    duration: Duration,
    distance: f32,
    traveled: f32,
    finished: bool,
    direction: Direction,
}

impl EaseInOutSine {
    pub fn new(
        distance: f32,
        duration: Duration,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    ) -> Self {
        Self::new_internal(distance, duration, limit_value, crate::animations::current_tick())
    }

    fn new_internal(
        distance: f32,
        duration: Duration,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        start_time: Instant,
    ) -> Self {
        Self {
            limit_value,
            start_time,
            duration,
            distance,
            traveled: 0.,
            finished: distance == 0. || duration.is_zero(),
            direction: if distance < 0. { Direction::Decreasing } else { Direction::Increasing },
        }
    }

    /// Returns the position relative to the start, and the velocity.
    fn sample(&self, elapsed: Duration) -> (f32, f32) {
        if self.finished || elapsed >= self.duration {
            return (self.distance, 0.);
        }
        let duration = self.duration.as_secs_f32();
        let phase = PI * elapsed.as_secs_f32() / duration;
        (
            self.distance * (1. - phase.cos()) / 2.,
            self.distance * PI / (2. * duration) * phase.sin(),
        )
    }
}

impl Simulation for EaseInOutSine {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        if self.finished {
            return true;
        }
        let elapsed = new_tick.duration_since(self.start_time);
        let (position, _) = self.sample(elapsed);

        // Apply increments: virtualized views may move the content between frames.
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
        self.finished = clamped || elapsed >= self.duration;
        self.finished
    }
}

impl PositionSimulation for EaseInOutSine {
    fn remaining_distance(&self, now: Instant) -> f32 {
        self.distance - self.sample(now.duration_since(self.start_time)).0
    }

    fn remaining_velocity(&self, now: Instant) -> f32 {
        self.sample(now.duration_since(self.start_time)).1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::simulations::{assert_approx_eq, test_limit_property};

    const DURATION: Duration = Duration::from_millis(300);

    #[test]
    fn follows_the_curve_and_ends_after_the_duration() {
        for sign in [-1., 1.] {
            let start = Instant::default();
            let mut simulation = EaseInOutSine::new_internal(
                sign * 400.,
                DURATION,
                test_limit_property(sign * 10_000.),
                start,
            );
            let mut position = 0.;
            for (millis, progress) in [(0, 0.), (75, 0.1464466), (150, 0.5), (225, 0.8535534)] {
                assert!(!simulation.step(&mut position, start + Duration::from_millis(millis)));
                assert!((position - sign * 400. * progress).abs() < 1e-2, "{millis}: {position}");
            }
            assert!(simulation.step(&mut position, start + DURATION));
            assert_approx_eq!(position, sign * 400.);
            assert_approx_eq!(simulation.remaining_distance(start + DURATION), 0.);
            assert_approx_eq!(simulation.remaining_velocity(start + DURATION), 0.);
        }
    }

    #[test]
    fn remaining_distance_and_velocity_match_the_motion() {
        let start = Instant::default();
        let mut simulation =
            EaseInOutSine::new_internal(-400., DURATION, test_limit_property(-10_000.), start);
        let mut position = 0.;
        let dt = Duration::from_millis(1);
        for millis in [10, 100, 150, 280] {
            let tick = start + Duration::from_millis(millis);
            simulation.step(&mut position, tick);
            assert_approx_eq!(simulation.remaining_distance(tick), -400. - position);

            let measured = (simulation.remaining_distance(tick - dt)
                - simulation.remaining_distance(tick + dt))
                / (2. * dt.as_secs_f32());
            let velocity = simulation.remaining_velocity(tick);
            assert!((velocity - measured).abs() < 1., "{millis}: {velocity} != {measured}");
        }
    }

    #[test]
    fn stops_at_the_limit() {
        for sign in [-1., 1.] {
            let start = Instant::default();
            let mut simulation = EaseInOutSine::new_internal(
                sign * 400.,
                DURATION,
                test_limit_property(sign * 100.),
                start,
            );
            let mut position = 0.;
            assert!(!simulation.step(&mut position, start + Duration::from_millis(50)));
            assert!(simulation.step(&mut position, start + Duration::from_millis(150)));
            assert_approx_eq!(position, sign * 100.);
        }
    }

    #[test]
    fn applies_increments_to_a_moved_position() {
        let start = Instant::default();
        let mut simulation =
            EaseInOutSine::new_internal(400., DURATION, test_limit_property(10_000.), start);
        let mut position = 0.;
        simulation.step(&mut position, start + Duration::from_millis(150));
        position += 1000.;
        assert!(simulation.step(&mut position, start + DURATION));
        assert_approx_eq!(position, 1400.);
    }

    #[test]
    fn zero_distance_is_finished() {
        let start = Instant::default();
        let mut simulation =
            EaseInOutSine::new_internal(0., DURATION, test_limit_property(0.), start);
        let mut position = 5.;
        assert!(simulation.step(&mut position, start));
        assert_approx_eq!(position, 5.);
    }
}
