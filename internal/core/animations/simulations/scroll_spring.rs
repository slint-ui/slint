// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Ported from Flutter's `SpringSimulation` (physics/spring_simulation.dart), which is:
//! Copyright 2014 The Flutter Authors. All rights reserved.
//!
//! Use of the original source is governed by a BSD-style license
//!
//! Original: <https://github.com/flutter/flutter/blob/d6bed8ff6135cdd414f14edc3063f761d47ca846/packages/flutter/lib/src/physics/spring_simulation.dart>

use core::time::Duration;

use crate::animations::Instant;
use crate::animations::simulations::spring::SpringRegime;
use crate::animations::simulations::{PositionSimulation, Simulation};

#[cfg(test)]
use crate::animations::simulations::test_limit_property;

const ZERO_TOLERANCE: f32 = 1e-3;

// The return constants are fitted to UIKit `UIScrollView` returns after a held pull from the top
// edge, captured on an iPhone 13 Pro Max with iOS 27.
/// The natural frequency of the critically damped return.
const RETURN_FREQUENCY: f32 = 10.67037;
/// How long after the release the return starts.
const RETURN_DELAY: Duration = Duration::from_millis(7);
/// The initial return speed relative to the overscroll, for a small overscroll.
const RETURN_RATE_MIN: f32 = 5.470122;
/// How much the initial return rate rises for a large overscroll.
const RETURN_RATE_RISE: f32 = 4.825567;
/// The overscroll at which the initial return rate has risen halfway.
const RETURN_RATE_HALF_DISTANCE: f32 = 165.8656;

// Fitted to UIKit releases while still dragging outward, at 130–2030 pointer points per second
// and 45–230 points past the edge.
/// Up to this pointer speed, a release keeps the full initial return rate.
const RETURN_RATE_FADE_START: f32 = 400.;
/// The pointer speed at which the initial return rate has faded out, for a large overscroll.
const RETURN_RATE_FADE_END_MAX: f32 = 3400.;
/// The overscroll at which the fade's end has risen halfway to [`RETURN_RATE_FADE_END_MAX`].
const RETURN_RATE_FADE_HALF_DISTANCE: f32 = 70.;

/// The share of the initial return rate that a release keeps, `distance` past the edge.
fn initial_return_rate_share(drag_speed: f32, distance: f32) -> f32 {
    let squared = distance * distance;
    let fade_length = (RETURN_RATE_FADE_END_MAX - RETURN_RATE_FADE_START) * squared
        / (RETURN_RATE_FADE_HALF_DISTANCE * RETURN_RATE_FADE_HALF_DISTANCE + squared);
    (1. - (drag_speed.abs() - RETURN_RATE_FADE_START) / fade_length.max(f32::EPSILON)).clamp(0., 1.)
}

fn initial_return_rate(distance: f32) -> f32 {
    let squared = distance * distance;
    RETURN_RATE_MIN
        + RETURN_RATE_RISE * squared
            / (RETURN_RATE_HALF_DISTANCE * RETURN_RATE_HALF_DISTANCE + squared)
}

#[derive(Debug)]
pub struct SpringSimulation {
    start_time: Instant,
    limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    limit: f32,
    last_position: f32,
    last_time: Duration,
    spring_time: Duration,
    data: SpringRegime,
    init_pos: f32,
    release_velocity: f32,
}

impl SpringSimulation {
    /// Springs from `start_value` back to `limit_value`, like UIKit's return after a pull.
    /// `velocity` is the content's velocity at the release; only motion away from
    /// `limit_value` carries over.
    /// A faster `drag_speed`, the pointer's speed at the release, weakens that motion's
    /// initial pull back.
    /// The content keeps that velocity until [`RETURN_DELAY`] after `start_time`, then the
    /// return starts.
    pub fn new_with_default_parameters(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        start_time: Instant,
        velocity: f32,
        drag_speed: f32,
    ) -> Self {
        let limit = limit_value.as_ref().get();
        let distance = limit - start_value;
        let release_velocity = if velocity * distance < 0. { velocity } else { 0. };
        let onset_distance = distance - release_velocity * RETURN_DELAY.as_secs_f32();
        let return_rate_share = if release_velocity == 0. {
            1.
        } else {
            initial_return_rate_share(drag_speed, distance)
        };
        let onset_velocity = -release_velocity
            - return_rate_share * onset_distance * initial_return_rate(onset_distance);
        Self {
            start_time,
            limit_value,
            limit,
            last_position: start_value,
            last_time: Duration::ZERO,
            spring_time: RETURN_DELAY,
            data: SpringRegime::new(onset_distance, onset_velocity, RETURN_FREQUENCY, 1.),
            init_pos: distance,
            release_velocity,
        }
    }

    /// The remaining distance to the limit and its rate of change.
    fn evaluate(&self, time_elapsed: Duration) -> (f32, f32) {
        match time_elapsed.checked_sub(self.spring_time) {
            Some(t) => self.data.evaluate(t.as_secs_f32()),
            None => (
                self.init_pos - self.release_velocity * time_elapsed.as_secs_f32(),
                -self.release_velocity,
            ),
        }
    }

    fn step_internal(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        let time_elapsed = new_tick.duration_since(self.start_time);
        let limit = self.limit_value.as_ref().get();
        if limit != self.limit || *current != self.last_position {
            let (_, velocity) = self.evaluate(self.last_time);
            self.data = SpringRegime::new(limit - *current, velocity, RETURN_FREQUENCY, 1.);
            self.spring_time = self.last_time;
            self.limit = limit;
        }
        let (new_pos, new_vel) = self.evaluate(time_elapsed);
        let finished = new_pos.abs() < ZERO_TOLERANCE && new_vel.abs() < ZERO_TOLERANCE;
        *current = if finished { self.limit } else { self.limit - new_pos };
        self.last_position = *current;
        self.last_time = time_elapsed;

        finished
    }
}

impl Simulation for SpringSimulation {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        self.step_internal(current, new_tick)
    }
}

impl PositionSimulation for SpringSimulation {
    fn remaining_distance(&self, time_elapsed: Duration) -> f32 {
        self.evaluate(time_elapsed).0
    }

    fn remaining_velocity(&self, time_elapsed: Duration) -> f32 {
        -self.evaluate(time_elapsed).1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::simulations::assert_approx_eq;
    use core::time::Duration;

    fn start_time() -> Instant {
        Instant::from_millis(1_000)
    }

    #[test]
    fn remaining_distance_and_velocity_settle_to_zero() {
        let simulation = SpringSimulation::new_with_default_parameters(
            30.,
            test_limit_property(20.),
            start_time(),
            0.,
            0.,
        );
        assert_approx_eq!(simulation.remaining_distance(core::time::Duration::from_secs(10)), 0.);
        assert_approx_eq!(simulation.remaining_velocity(core::time::Duration::from_secs(10)), 0.);
    }

    #[test]
    fn retargeting_preserves_position_and_velocity() {
        for sign in [-1., 1.] {
            for (new_limit, offset) in [(10., 0.), (20., 10.)] {
                let start = start_time();
                let mut simulation = SpringSimulation::new_with_default_parameters(
                    sign * 30.,
                    test_limit_property(sign * 20.),
                    start,
                    0.,
                    0.,
                );
                let elapsed = Duration::from_millis(50);
                let mut position = sign * 30.;
                assert!(!simulation.step(&mut position, start + elapsed));
                let velocity = simulation.remaining_velocity(elapsed);

                simulation.limit_value.as_ref().set(sign * new_limit);
                position += sign * offset;
                let before = position;
                assert!(!simulation.step(&mut position, start + elapsed));
                assert_approx_eq!(position, before);
                assert_approx_eq!(simulation.remaining_velocity(elapsed), velocity);
                assert_approx_eq!(
                    simulation.remaining_distance(elapsed),
                    sign * new_limit - position
                );

                assert!(simulation.step(&mut position, start + Duration::from_secs(10)));
                assert_approx_eq!(position, sign * new_limit);
            }
        }
    }

    /// `remaining_velocity` reports the velocity of the content position that `step` moves,
    /// not of the spring's internal, target-relative coordinate. Overscrolled past a lower
    /// limit (`start_value > limit`), the content must move down toward the limit, so its
    /// velocity is negative.
    #[test]
    fn remaining_velocity_points_toward_the_limit_from_above() {
        let simulation = SpringSimulation::new_with_default_parameters(
            30.,
            test_limit_property(20.),
            start_time(),
            0.,
            0.,
        );
        for millis in [50, 100, 300] {
            let t = core::time::Duration::from_millis(millis);
            assert!(simulation.remaining_velocity(t) < 0., "{millis}ms");
        }
    }

    /// Mirrors [`remaining_velocity_points_toward_the_limit_from_above`]: overscrolled past an
    /// upper limit (`start_value < limit`), the content must move up toward the limit, so its
    /// velocity is positive.
    #[test]
    fn remaining_velocity_points_toward_the_limit_from_below() {
        let simulation = SpringSimulation::new_with_default_parameters(
            10.,
            test_limit_property(20.),
            start_time(),
            0.,
            0.,
        );
        for millis in [50, 100, 300] {
            let t = core::time::Duration::from_millis(millis);
            assert!(simulation.remaining_velocity(t) > 0., "{millis}ms");
        }
    }

    #[test]
    fn remaining_velocity_matches_the_displayed_motion() {
        for (start, velocity) in
            [(-228.642, 0.), (-92.069, -600.), (21.392, 0.), (21.392, 600.), (228.642, 600.)]
        {
            let simulation = SpringSimulation::new_with_default_parameters(
                start,
                test_limit_property(0.),
                start_time(),
                velocity,
                0.,
            );
            assert_approx_eq!(simulation.remaining_distance(Duration::ZERO), -start);
            for millis in [50, 100, 200, 500] {
                let t = Duration::from_millis(millis);
                let dt = Duration::from_micros(100);
                let measured = -(simulation.remaining_distance(t + dt)
                    - simulation.remaining_distance(t - dt))
                    / (2. * dt.as_secs_f32());
                assert!((simulation.remaining_velocity(t) - measured).abs() < 0.2, "{start}");
            }
            assert!(simulation.remaining_distance(Duration::from_secs(10)).abs() < 0.001);
        }
    }

    #[test]
    fn does_not_move_before_the_return_delay() {
        let start = start_time();
        let mut simulation = SpringSimulation::new_with_default_parameters(
            30.,
            test_limit_property(20.),
            start,
            0.,
            0.,
        );
        let mut position = 30.;
        simulation.step(&mut position, start + RETURN_DELAY);
        assert_approx_eq!(position, 30.);
        assert_approx_eq!(simulation.remaining_distance(RETURN_DELAY), -10.);
        assert_approx_eq!(simulation.remaining_velocity(RETURN_DELAY / 2), 0.);
        simulation.step(&mut position, start + RETURN_DELAY + Duration::from_millis(8));
        assert!(position < 30.);
    }

    /// Samples of a UIKit return after a held 200-point pull, which ended 92 points past the top,
    /// timed by `CADisplayLink.targetTimestamp`; on that clock the return starts 16.8 ms after
    /// the release.
    /// UIKit reports positions in 1/3-point steps, with up to a frame of sampling jitter.
    #[test]
    fn follows_a_measured_uikit_return() {
        let uikit_onset = Duration::from_nanos(16_827_855);
        let start = start_time();
        let mut simulation = SpringSimulation::new_with_default_parameters(
            92.,
            test_limit_property(0.),
            start,
            0.,
            0.,
        );
        let mut position = 92.;
        for (millis, uikit) in
            [(17, 92.), (25, 87.), (50, 73.), (100, 50.), (150, 33.667), (200, 22.), (300, 10.)]
        {
            let since_onset = Duration::from_millis(millis).saturating_sub(uikit_onset);
            simulation.step(&mut position, start + RETURN_DELAY + since_onset);
            assert!((position - uikit).abs() < 1.5, "{millis} ms: {position} != {uikit}");
        }
    }

    #[test]
    fn keeps_moving_outward_after_a_moving_release() {
        let start = start_time();
        let mut simulation = SpringSimulation::new_with_default_parameters(
            45.,
            test_limit_property(0.),
            start,
            600.,
            0.,
        );
        assert_approx_eq!(simulation.remaining_velocity(Duration::ZERO), 600.);
        let mut position = 45.;
        let mut peak = position;
        for millis in (8..2_000).step_by(8) {
            simulation.step(&mut position, start + Duration::from_millis(millis));
            peak = peak.max(position);
        }
        assert!(peak > 50., "{peak}");
        assert!(position.abs() < 0.01, "{position}");
    }

    #[test]
    fn ignores_release_velocity_toward_the_limit() {
        let still = SpringSimulation::new_with_default_parameters(
            45.,
            test_limit_property(0.),
            start_time(),
            0.,
            0.,
        );
        let inward = SpringSimulation::new_with_default_parameters(
            45.,
            test_limit_property(0.),
            start_time(),
            -600.,
            0.,
        );
        for millis in [0, 5, 50, 200] {
            let t = Duration::from_millis(millis);
            assert_approx_eq!(inward.remaining_distance(t), still.remaining_distance(t));
        }
    }

    #[test]
    fn faster_drags_pull_back_less() {
        let peak = |drag_speed: f32| {
            let start = start_time();
            let mut simulation = SpringSimulation::new_with_default_parameters(
                45.,
                test_limit_property(0.),
                start,
                600.,
                drag_speed,
            );
            let mut position = 45.;
            let mut peak = position;
            for millis in (1..200).step_by(1) {
                simulation.step(&mut position, start + Duration::from_millis(millis));
                peak = peak.max(position);
            }
            peak
        };
        assert!(peak(1200.) > peak(400.) + 1., "{} <= {}", peak(1200.), peak(400.));
        assert_approx_eq!(peak(0.), peak(RETURN_RATE_FADE_START));
    }

    #[test]
    fn initial_return_rate_fades_later_for_a_larger_overscroll() {
        assert_approx_eq!(initial_return_rate_share(RETURN_RATE_FADE_START, 46.), 1.);
        assert!(initial_return_rate_share(1000., 46.) < 0.5);
        assert!(initial_return_rate_share(1000., 228.) > 0.75);
        assert_approx_eq!(initial_return_rate_share(1000., 0.), 0.);
    }

    /// Steps the spring against UIKit samples of a release 100 points into a pull, read in the
    /// `CADisplayLink` callback; on that clock the return starts about 11 ms after the release.
    fn assert_follows_uikit(
        start_value: f32,
        velocity: f32,
        drag_speed: f32,
        samples: &[(u64, f32)],
        tolerance: f32,
    ) {
        let uikit_onset = Duration::from_millis(11);
        let start = start_time();
        let mut simulation = SpringSimulation::new_with_default_parameters(
            start_value,
            test_limit_property(0.),
            start,
            velocity,
            drag_speed,
        );
        let mut position = start_value;
        for &(millis, uikit) in samples {
            let since_onset = Duration::from_millis(millis).saturating_sub(uikit_onset);
            simulation.step(&mut position, start + RETURN_DELAY + since_onset);
            assert!((position - uikit).abs() < tolerance, "{millis} ms: {position} != {uikit}");
        }
    }

    /// Released 45 points past the top with the pointer at 400 points per second and the content
    /// at 194.
    #[test]
    fn follows_a_measured_uikit_slow_moving_release() {
        let samples = [(25, 45.), (50, 41.333), (100, 32.), (200, 16.333), (300, 7.333)];
        assert_follows_uikit(45., 194., 400., &samples, 2.5);
    }

    /// Released 44.3 points past the top with the pointer at 1224 points per second and the
    /// content at 598.
    #[test]
    fn follows_a_measured_uikit_fast_moving_release() {
        let samples = [
            (25, 54.),
            (50, 60.333),
            (75, 61.),
            (100, 57.333),
            (150, 46.),
            (200, 34.),
            (300, 16.333),
        ];
        assert_follows_uikit(44.333, 598., 1224., &samples, 5.);
    }
}
