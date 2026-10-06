// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Ported from Flutter's `BouncingScrollSimulation` (and the `FrictionSimulation`/
//! `SpringSimulation` it composes), in scroll_simulation.dart and physics/*.dart, which are:
//! Copyright 2014 The Flutter Authors. All rights reserved.
//!
//! Use of the original source is governed by a BSD-style license
//!
//! Original: <https://github.com/flutter/flutter/blob/d6bed8ff6135cdd414f14edc3063f761d47ca846/packages/flutter/lib/src/widgets/scroll_simulation.dart>
//!
//! The scroll physics of a Flickable that bounces: the position decelerates under friction, and
//! once it's past `limit_value`, a spring pulls it back to the boundary.
//! A release past the boundary starts with the spring.
//! The constants are fitted to UIKit's `UIScrollView`.

use core::time::Duration;

use crate::animations::Instant;
use crate::animations::simulations::spring::SpringRegime;
use crate::animations::simulations::{Direction, Parameter, PositionSimulation, Simulation};
#[cfg(not(feature = "std"))]
use num_traits::Float;

#[cfg(test)]
use crate::animations::simulations::test_limit_property;

/// iOS's `UIScrollView.decelerationRate` (`.normal`), expressed the way a
/// friction simulation wants it: `0.998^1000 ≈ 0.135`, the fraction of
/// velocity retained after one second.
const DRAG: f32 = 0.135;

/// The largest velocity that can be carried over from the friction phase
/// into the spring phase.
const MAX_SPRING_TRANSFER_VELOCITY: f32 = 5000.0;

/// A velocity at or below this magnitude (logical px/s) is considered stopped.
const VELOCITY_TOLERANCE: f32 = 1.0;
/// UIKit ends a deceleration where the content is once its speed falls below this, in logical
/// px/s, measured on an iPhone 13 Pro Max with iOS 27.
const DECELERATION_STOP_VELOCITY: f32 = 10.0;
/// A spring within this distance (logical px) of `limit_value` is considered settled.
const DISTANCE_TOLERANCE: f32 = 0.5;

// The spring constants are fitted to UIKit returns after a held pull past the top edge and after
// flings into it, captured on an iPhone 13 Pro Max with iOS 27.
/// The natural frequency of the critically damped spring past the limit.
const SPRING_FREQUENCY: f32 = 10.67037;
/// How long after a release past the limit the return starts.
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

/// Input parameters for the `BounceFlick` simulation.
#[derive(Debug, Clone)]
pub struct BounceFlickParameters {
    pub initial_velocity: f32,
}

impl BounceFlickParameters {
    pub fn new(initial_velocity: f32) -> Self {
        Self { initial_velocity }
    }
    pub fn new_with_distance(distance: f32, _duration: Duration) -> Self {
        let drag_log = f32::ln(DRAG);
        // finalX - x0 = (v0 - v_stop) / -drag_log  =>  v0 = -distance * drag_log + v_stop
        let stop_velocity =
            if distance == 0. { 0. } else { distance.signum() * DECELERATION_STOP_VELOCITY };
        Self { initial_velocity: -distance * drag_log + stop_velocity }
    }
}

impl Parameter for BounceFlickParameters {
    type Output = BounceFlick;
    fn simulation(
        self,
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    ) -> Self::Output {
        BounceFlick::new(start_value, limit_value, self)
    }
}

/// How the position moves until the spring takes over.
#[derive(Debug, Clone, Copy)]
enum Approach {
    /// Decelerates under [`DRAG`], from `start_value` with `velocity` at `since` seconds.
    Friction { start_value: f32, velocity: f32, since: f32 },
    /// Keeps `velocity` from `start_value`, during the delay after a release past the limit.
    Coast { start_value: f32, velocity: f32 },
}

#[derive(Debug)]
pub struct BounceFlick {
    /// Limit property to determine when to bounce back
    limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    limit: f32,
    direction: Direction,
    start_time: Instant,
    approach: Approach,
    /// Seconds after `start_time` at which the spring takes over, or infinity if it never does.
    spring_time: f32,
    /// The spring that pulls the position, relative to `limit`, back to it.
    spring: SpringRegime,
    last_position: f32,
    last_time: f32,
}

impl BounceFlick {
    pub fn new(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        data: BounceFlickParameters,
    ) -> Self {
        Self::new_internal(start_value, limit_value, data, crate::animations::current_tick())
    }

    fn new_internal(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        data: BounceFlickParameters,
        start_time: Instant,
    ) -> Self {
        let limit = limit_value.as_ref().get();
        let mut velocity = data.initial_velocity;
        let direction = if start_value == limit {
            if velocity >= 0. { Direction::Increasing } else { Direction::Decreasing }
        } else if start_value < limit {
            debug_assert!(velocity >= 0.); // Makes no sense yet that the velocity goes into the other direction
            velocity = f32::abs(velocity);
            Direction::Increasing
        } else {
            velocity = -f32::abs(velocity);
            debug_assert!(velocity <= 0.);
            Direction::Decreasing
        };

        let (spring_time, spring) =
            Self::spring_parameters(start_value, velocity, limit, &direction);

        Self {
            limit_value,
            limit,
            direction,
            start_time,
            approach: Approach::Friction { start_value, velocity, since: 0. },
            spring_time,
            spring,
            last_position: start_value,
            last_time: 0.,
        }
    }

    /// Springs from `start_value`, past `limit_value`, back to it, like UIKit after a pull is
    /// released past the edge.
    /// `velocity` is the content's velocity at the release; only motion away from
    /// `limit_value` carries over.
    /// A faster `drag_speed`, the pointer's speed at the release, weakens that motion's
    /// initial pull back.
    /// The content keeps that velocity until [`RETURN_DELAY`] after `start_time`, then the
    /// return starts.
    pub fn new_overscroll_release(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        start_time: Instant,
        velocity: f32,
        drag_speed: f32,
    ) -> Self {
        let limit = limit_value.as_ref().get();
        let overscroll = start_value - limit;
        let velocity = if velocity * overscroll > 0. { velocity } else { 0. };
        let delay = RETURN_DELAY.as_secs_f32();
        let onset_overscroll = overscroll + velocity * delay;
        let return_rate_share =
            if velocity == 0. { 1. } else { initial_return_rate_share(drag_speed, overscroll) };
        let onset_velocity =
            velocity - return_rate_share * onset_overscroll * initial_return_rate(onset_overscroll);
        Self {
            limit_value,
            limit,
            direction: if overscroll >= 0. { Direction::Increasing } else { Direction::Decreasing },
            start_time,
            approach: Approach::Coast { start_value, velocity },
            spring_time: delay,
            spring: SpringRegime::new(onset_overscroll, onset_velocity, SPRING_FREQUENCY, 1.),
            last_position: start_value,
            last_time: 0.,
        }
    }

    /// When the friction curve from `start_value` with `velocity` reaches `limit`, and the spring
    /// that takes over there. Past the limit already, the spring takes over at once.
    fn spring_parameters(
        start_value: f32,
        velocity: f32,
        limit: f32,
        direction: &Direction,
    ) -> (f32, SpringRegime) {
        let outside_bounds = match direction {
            Direction::Increasing => start_value > limit,
            Direction::Decreasing => start_value < limit,
        };
        if outside_bounds {
            return (0., SpringRegime::new(start_value - limit, velocity, SPRING_FREQUENCY, 1.));
        }

        let final_x = Self::friction_end(start_value, velocity);
        let needs_spring = match direction {
            Direction::Increasing => final_x > limit,
            Direction::Decreasing => final_x < limit,
        };
        let spring_time = if needs_spring {
            Self::time_at_x(start_value, velocity, f32::ln(DRAG), limit).unwrap_or(f32::INFINITY)
        } else {
            f32::INFINITY
        };

        let v_at_spring =
            if spring_time.is_finite() { velocity * f32::powf(DRAG, spring_time) } else { 0. };
        // See BouncingScrollSimulation's `maxSpringTransferVelocity` clamp: only ever
        // caps an excessively fast *positive* handoff velocity, same as upstream.
        let spring_velocity = f32::min(v_at_spring, MAX_SPRING_TRANSFER_VELOCITY);
        (spring_time, SpringRegime::new(0., spring_velocity, SPRING_FREQUENCY, 1.))
    }

    /// The elapsed time at which the friction curve (starting at `x0` with
    /// velocity `v0`) reaches `target`, or `None` if it never does.
    fn time_at_x(x0: f32, v0: f32, drag_log: f32, target: f32) -> Option<f32> {
        if v0 == 0. {
            return None;
        }
        // x(t) - x0 = v0 * (drag^t - 1) / drag_log  =>  solve for t.
        let ratio = 1. + (target - x0) * drag_log / v0;
        if ratio <= 0. {
            return None;
        }
        Some(f32::ln(ratio) / drag_log)
    }

    /// The elapsed time at which the friction curve's speed falls to [`DECELERATION_STOP_VELOCITY`].
    fn friction_stop_time(v0: f32) -> f32 {
        if f32::abs(v0) <= DECELERATION_STOP_VELOCITY {
            0.
        } else {
            f32::ln(DECELERATION_STOP_VELOCITY / f32::abs(v0)) / f32::ln(DRAG)
        }
    }

    /// The position the friction curve (starting at `x0` with velocity `v0`) comes to rest at.
    fn friction_end(x0: f32, v0: f32) -> f32 {
        let drag_log = f32::ln(DRAG);
        x0 + v0 * f32::powf(DRAG, Self::friction_stop_time(v0)) / drag_log - v0 / drag_log
    }

    /// Position and velocity of the friction curve from `x0` with velocity `v0`, `t` seconds in.
    fn friction_at(x0: f32, v0: f32, t: f32) -> (f32, f32) {
        if t >= Self::friction_stop_time(v0) {
            return (Self::friction_end(x0, v0), 0.);
        }
        let drag_log = f32::ln(DRAG);
        let position = x0 + v0 * f32::powf(DRAG, t) / drag_log - v0 / drag_log;
        (position, v0 * f32::powf(DRAG, t))
    }

    /// The position the simulation comes to rest at: the limit if the spring takes over,
    /// the end of the friction curve otherwise.
    fn final_position(&self) -> f32 {
        match self.approach {
            Approach::Friction { start_value, velocity, .. } if self.spring_time.is_infinite() => {
                Self::friction_end(start_value, velocity)
            }
            _ => self.limit,
        }
    }

    /// Position, velocity, and whether the simulation has settled, at
    /// elapsed time `t` since `start_time`.
    fn evaluate(&self, t: f32) -> (f32, f32, bool) {
        if t < self.spring_time {
            match self.approach {
                Approach::Friction { start_value, velocity, since } => {
                    let (position, velocity) = Self::friction_at(start_value, velocity, t - since);
                    (position, velocity, f32::abs(velocity) < VELOCITY_TOLERANCE)
                }
                Approach::Coast { start_value, velocity } => {
                    (start_value + velocity * t, velocity, false)
                }
            }
        } else {
            let (x_rel, velocity) = self.spring.evaluate(t - self.spring_time);
            let position = self.limit + x_rel;
            let done =
                f32::abs(x_rel) < DISTANCE_TOLERANCE && f32::abs(velocity) < VELOCITY_TOLERANCE;
            (position, velocity, done)
        }
    }

    fn step_internal(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        let t = new_tick.duration_since(self.start_time).as_secs_f32();
        let limit = self.limit_value.as_ref().get();
        if limit != self.limit || *current != self.last_position {
            let (_, velocity, _) = self.evaluate(self.last_time);
            let (spring_time, spring) =
                Self::spring_parameters(*current, velocity, limit, &self.direction);
            self.limit = limit;
            self.approach =
                Approach::Friction { start_value: *current, velocity, since: self.last_time };
            self.spring_time = self.last_time + spring_time;
            self.spring = spring;
        }
        let (mut position, _velocity, done) = self.evaluate(t);

        if done && t >= self.spring_time {
            position = self.limit;
        }

        *current = position;
        self.last_position = position;
        self.last_time = t;
        done
    }
}

impl Simulation for BounceFlick {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        self.step_internal(current, new_tick)
    }
}

impl PositionSimulation for BounceFlick {
    fn remaining_distance(&self, time_elapsed: Duration) -> f32 {
        let (position, _velocity, _done) = self.evaluate(time_elapsed.as_secs_f32());
        self.final_position() - position
    }

    fn remaining_velocity(&self, time_elapsed: Duration) -> f32 {
        let (_position, velocity, _done) = self.evaluate(time_elapsed.as_secs_f32());
        velocity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::simulations::assert_approx_eq;

    #[test]
    fn starts_at_initial_position_and_velocity() {
        let time = Instant::default();
        let mut simulation = BounceFlick::new_internal(
            10.,
            test_limit_property(2000.),
            BounceFlickParameters::new(500.),
            time,
        );
        let mut current = 10.;
        simulation.step(&mut current, time);
        assert_approx_eq!(current, 10.);
    }

    /// A gentle fling that settles under friction alone, well short of the limit.
    #[test]
    fn gentle_fling_never_reaches_limit() {
        let time = Instant::default();
        let mut simulation = BounceFlick::new_internal(
            10.,
            test_limit_property(2000.),
            BounceFlickParameters::new(500.),
            time,
        );
        let mut current = 10.;
        let finished = simulation.step(&mut current, time + Duration::from_secs(30));
        assert_eq!(finished, true);
        assert!(current < 2000.);
        assert!(current > 10.);
        assert!(simulation.spring_time.is_infinite());
    }

    /// A hard fling toward a nearby limit overshoots, then springs back exactly to it.
    #[test]
    fn hard_fling_overshoots_then_springs_back() {
        let time = Instant::default();
        let mut simulation = BounceFlick::new_internal(
            10.,
            test_limit_property(20.),
            BounceFlickParameters::new(5000.),
            time,
        );
        assert!(simulation.spring_time.is_finite());

        let mut current = 10.;
        // Shortly after the fling starts, it should already be past the limit
        // (that's the whole point of the rubber-band effect).
        let finished = simulation.step(&mut current, time + Duration::from_millis(50));
        assert_eq!(finished, false);
        assert!(current > 20.);

        // Long after, the spring must have pulled it back exactly to the limit.
        let finished = simulation.step(&mut current, time + Duration::from_secs(10));
        assert_eq!(finished, true);
        assert_approx_eq!(current, 20.);
    }

    #[test]
    fn decreasing_direction_mirrors_increasing() {
        let time = Instant::default();
        let mut simulation = BounceFlick::new_internal(
            20.,
            test_limit_property(10.),
            BounceFlickParameters::new(-5000.),
            time,
        );
        assert!(simulation.spring_time.is_finite());

        let mut current = 20.;
        let finished = simulation.step(&mut current, time + Duration::from_millis(50));
        assert_eq!(finished, false);
        assert!(current < 10.);

        let finished = simulation.step(&mut current, time + Duration::from_secs(10));
        assert_eq!(finished, true);
        assert_approx_eq!(current, 10.);
    }

    #[test]
    fn retargeting_preserves_position_and_velocity() {
        for sign in [-1., 1.] {
            for (limit, initial_velocity, new_limit, offset) in
                [(2000., 500., 10., 0.), (20., 5000., 10., 0.), (20., 5000., 20., 50.)]
            {
                let start = Instant::default();
                let mut simulation = BounceFlick::new_internal(
                    0.,
                    test_limit_property(sign * limit),
                    BounceFlickParameters::new(sign * initial_velocity),
                    start,
                );
                let elapsed = Duration::from_millis(50);
                let mut position = 0.;
                assert!(!simulation.step(&mut position, start + elapsed));
                let velocity = simulation.remaining_velocity(elapsed);

                simulation.limit_value.as_ref().set(sign * new_limit);
                position += sign * offset;
                let before = position;
                assert!(!simulation.step(&mut position, start + elapsed));
                assert_approx_eq!(position, before);
                assert!((simulation.remaining_velocity(elapsed) - velocity).abs() < 0.01);
                assert_approx_eq!(
                    simulation.remaining_distance(elapsed),
                    sign * new_limit - position
                );

                assert!(simulation.step(&mut position, start + Duration::from_secs(10)));
                assert_approx_eq!(position, sign * new_limit);
            }
        }
    }

    #[test]
    fn growing_bounds_cancels_the_pending_bounce() {
        for sign in [-1., 1.] {
            let start = Instant::default();
            let mut simulation = BounceFlick::new_internal(
                0.,
                test_limit_property(sign * 100.),
                BounceFlickParameters::new(sign * 500.),
                start,
            );
            let mut unbounded = BounceFlick::new_internal(
                0.,
                test_limit_property(sign * 1000.),
                BounceFlickParameters::new(sign * 500.),
                start,
            );
            let mut position = 0.;
            let mut expected = 0.;
            assert!(!simulation.step(&mut position, start + Duration::from_millis(50)));
            simulation.limit_value.as_ref().set(sign * 1000.);
            for millis in [50, 100, 300, 1000, 10_000] {
                let tick = start + Duration::from_millis(millis);
                let finished = simulation.step(&mut position, tick);
                assert_eq!(finished, unbounded.step(&mut expected, tick));
                assert_approx_eq!(position, expected);
            }
            assert!(sign * position > 100.);
        }
    }

    /// The distance left shrinks to zero, and together with the distance moved so far it is
    /// always the whole distance of the friction curve.
    #[test]
    fn remaining_distance_is_the_distance_left_to_rest() {
        let time = Instant::default();
        let simulation = BounceFlick::new_internal(
            10.,
            test_limit_property(2000.),
            BounceFlickParameters::new(500.),
            time,
        );
        let total = -(500. - DECELERATION_STOP_VELOCITY) / f32::ln(DRAG);
        assert!((simulation.remaining_distance(Duration::ZERO) - total).abs() < 1e-2);

        let mut previous = total;
        for millis in [50, 300, 1000, 1900] {
            let elapsed = Duration::from_millis(millis);
            let remaining = simulation.remaining_distance(elapsed);
            let (position, _, _) = simulation.evaluate(elapsed.as_secs_f32());
            assert!(remaining > 0. && remaining < previous, "{millis}ms: {remaining}");
            assert!((position - 10. + remaining - total).abs() < 1e-2, "{millis}ms");
            previous = remaining;
        }
        assert!(simulation.remaining_distance(Duration::from_secs(60)).abs() < 1e-2);
    }

    /// With the spring, the position comes to rest at the limit, so the distance left is measured
    /// to it. Beyond the limit that is the way back.
    #[test]
    fn remaining_distance_with_the_spring_is_measured_to_the_limit() {
        let time = Instant::default();
        let increasing = BounceFlick::new_internal(
            10.,
            test_limit_property(20.),
            BounceFlickParameters::new(5000.),
            time,
        );
        assert!((increasing.remaining_distance(Duration::ZERO) - 10.).abs() < 1e-2);
        assert!(increasing.remaining_distance(Duration::from_millis(50)) < 0.);
        assert!(increasing.remaining_distance(Duration::from_secs(10)).abs() < 1e-2);

        let decreasing = BounceFlick::new_internal(
            20.,
            test_limit_property(10.),
            BounceFlickParameters::new(-5000.),
            time,
        );
        assert!((decreasing.remaining_distance(Duration::ZERO) + 10.).abs() < 1e-2);
        assert!(decreasing.remaining_distance(Duration::from_millis(50)) > 0.);
        assert!(decreasing.remaining_distance(Duration::from_secs(10)).abs() < 1e-2);
    }

    #[test]
    fn zero_init_velocity() {
        const START_VALUE: f32 = 10.;

        let parameters = BounceFlickParameters::new(0.);
        let time = Instant::default();
        let mut simulation =
            BounceFlick::new_internal(START_VALUE, test_limit_property(100.), parameters, time);

        let mut current = START_VALUE;
        assert_eq!(
            simulation.step(&mut current, time),
            true,
            "There is no velocity. So the simulation must be finish"
        );
        assert_eq!(current, START_VALUE);
    }

    /// UIKit stops a 656 pt/s fling about 2.1 s after release, 5 points before its friction
    /// curve would come to rest.
    #[test]
    fn stops_where_the_speed_falls_to_the_stop_velocity() {
        let time = Instant::default();
        let mut simulation = BounceFlick::new_internal(
            0.,
            test_limit_property(5000.),
            BounceFlickParameters::new(656.),
            time,
        );
        let stop_time = BounceFlick::friction_stop_time(656.);
        assert!((stop_time - 2.09).abs() < 0.01, "{stop_time}");
        let (_, velocity, done) = simulation.evaluate(stop_time - 0.01);
        assert!(!done && (velocity - DECELERATION_STOP_VELOCITY).abs() < 0.5, "{velocity}");

        let mut current = 0.;
        assert!(simulation.step(&mut current, time + Duration::from_secs_f32(stop_time + 0.01)));
        let unstopped_travel = -656. / f32::ln(DRAG);
        assert!((unstopped_travel - current - 5.).abs() < 0.05, "{current}");
        let stopped = current;
        simulation.step(&mut current, time + Duration::from_secs(5));
        assert_eq!(current, stopped);
    }

    #[test]
    fn distance_parameters_still_cover_the_distance() {
        for distance in [-120., 40., 300.] {
            let params = BounceFlickParameters::new_with_distance(distance, Duration::ZERO);
            let end = BounceFlick::friction_end(10., params.initial_velocity);
            assert!((end - 10. - distance).abs() < 1e-3, "{distance}: {end}");
        }
    }

    fn start_time() -> Instant {
        Instant::from_millis(1_000)
    }

    #[test]
    fn remaining_distance_and_velocity_settle_to_zero() {
        let simulation = BounceFlick::new_overscroll_release(
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
    fn retargeting_a_release_past_the_limit_preserves_position_and_velocity() {
        for sign in [-1., 1.] {
            for (new_limit, offset) in [(10., 0.), (20., 10.)] {
                let start = start_time();
                let mut simulation = BounceFlick::new_overscroll_release(
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
        let simulation = BounceFlick::new_overscroll_release(
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
        let simulation = BounceFlick::new_overscroll_release(
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
            let simulation = BounceFlick::new_overscroll_release(
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
        let mut simulation =
            BounceFlick::new_overscroll_release(30., test_limit_property(20.), start, 0., 0.);
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
        let mut simulation =
            BounceFlick::new_overscroll_release(92., test_limit_property(0.), start, 0., 0.);
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
        let mut simulation =
            BounceFlick::new_overscroll_release(45., test_limit_property(0.), start, 600., 0.);
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
        let still =
            BounceFlick::new_overscroll_release(45., test_limit_property(0.), start_time(), 0., 0.);
        let inward = BounceFlick::new_overscroll_release(
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
            let mut simulation = BounceFlick::new_overscroll_release(
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
        let mut simulation = BounceFlick::new_overscroll_release(
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
