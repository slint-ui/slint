// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::animations::simulations::Direction;
use crate::animations::simulations::spring::{SpringParameters, SpringPhysicalParameters};
use crate::animations::{
    Instant, SPRING_SETTLE_POSITION_EPSILON, SPRING_SETTLE_VELOCITY_EPSILON,
    simulations::{PositionSimulation, Simulation, spring::SpringRegime},
};
use core::time::Duration;

/// Moves a position by `-deflection` with a spring that starts at rest.
///
/// Use a critically damped spring: an underdamped one overshoots past the limit,
/// and an overdamped one takes long to settle.
#[derive(Debug)]
pub struct SpringFlick {
    limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    start_time: Instant,
    traveled: f32,
    regime: SpringRegime,
    deflection: f32,
    finished: bool,
    direction: Direction,
}

impl SpringFlick {
    pub fn new(
        deflection: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        parameters: SpringPhysicalParameters,
    ) -> Self {
        Self::new_internal(deflection, limit_value, parameters, crate::animations::current_tick())
    }

    fn new_internal(
        deflection: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        parameters: SpringPhysicalParameters,
        start_time: Instant,
    ) -> Self {
        let (w_n, zeta) = parameters.to_natural_frequency_and_damping_ratio();
        Self {
            limit_value,
            start_time,
            traveled: 0.,
            regime: SpringRegime::new(deflection, 0., w_n, zeta),
            deflection,
            finished: deflection.abs() < SPRING_SETTLE_POSITION_EPSILON,
            direction: if deflection < 0. { Direction::Increasing } else { Direction::Decreasing },
        }
    }

    /// Returns the displacement from the target, and the velocity.
    fn sample(&self, elapsed: Duration) -> (f32, f32) {
        if self.finished {
            return (0., 0.);
        }
        let (displacement, velocity) = self.regime.evaluate(elapsed.as_secs_f32());
        if displacement.abs() < SPRING_SETTLE_POSITION_EPSILON
            && velocity.abs() < SPRING_SETTLE_VELOCITY_EPSILON
        {
            (0., 0.)
        } else {
            (displacement, velocity)
        }
    }
}

impl Simulation for SpringFlick {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        if self.finished {
            return true;
        }
        let (displacement, velocity) = self.sample(new_tick.duration_since(self.start_time));

        // Apply increments: virtualized views may move the content between frames.
        let position = displacement - self.deflection;
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
        self.finished = clamped || (displacement == 0. && velocity == 0.);
        self.finished
    }
}

impl PositionSimulation for SpringFlick {
    fn remaining_distance(&self, now: Instant) -> f32 {
        -self.sample(now.duration_since(self.start_time)).0
    }

    fn remaining_velocity(&self, now: Instant) -> f32 {
        self.sample(now.duration_since(self.start_time)).1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::simulations::{assert_approx_eq, test_limit_property};

    const STIFFNESS: f32 = 1500.;

    fn spring_flick(deflection: f32, limit: f32, start: Instant) -> SpringFlick {
        SpringFlick::new_internal(
            deflection,
            test_limit_property(limit),
            SpringPhysicalParameters::new_critical_damped(STIFFNESS),
            start,
        )
    }

    #[test]
    fn settles_exactly_on_the_target() {
        for sign in [-1., 1.] {
            let start = Instant::default();
            let mut simulation = spring_flick(sign * 400., -sign * 10_000., start);
            let mut position = 0.;
            let mut previous = 0.;
            let mut millis = 0;
            while !simulation.step(&mut position, start + Duration::from_millis(millis)) {
                assert!(-sign * position >= -sign * previous, "{millis}: {position} < {previous}");
                assert!(-sign * position <= 400., "{millis}: {position} overshoots");
                previous = position;
                millis += 16;
                assert!(millis < 2000, "doesn't settle: {position}");
            }
            assert_approx_eq!(position, -sign * 400.);
            assert_eq!(simulation.remaining_distance(start + Duration::from_millis(millis)), 0.);
            assert_eq!(simulation.remaining_velocity(start + Duration::from_millis(millis)), 0.);
        }
    }

    #[test]
    fn remaining_distance_and_velocity_match_the_motion() {
        let start = Instant::default();
        let mut simulation = spring_flick(400., -10_000., start);
        let mut position = 0.;
        let dt = Duration::from_millis(1);
        for millis in [10, 50, 100, 200] {
            let tick = start + Duration::from_millis(millis);
            simulation.step(&mut position, tick);
            assert_approx_eq!(simulation.remaining_distance(tick), -400. - position);

            let measured = (simulation.remaining_distance(tick - dt)
                - simulation.remaining_distance(tick + dt))
                / (2. * dt.as_secs_f32());
            let velocity = simulation.remaining_velocity(tick);
            assert!(
                (velocity - measured).abs() < 1e-2 * velocity.abs().max(1.),
                "{millis}: {velocity} != {measured}"
            );
        }
    }

    #[test]
    fn stops_at_the_limit() {
        for sign in [-1., 1.] {
            let start = Instant::default();
            let mut simulation = spring_flick(sign * 400., -sign * 100., start);
            let mut position = 0.;
            assert!(!simulation.step(&mut position, start + Duration::from_millis(5)));
            assert!(simulation.step(&mut position, start + Duration::from_millis(200)));
            assert_eq!(position, -sign * 100.);
        }
    }

    #[test]
    fn applies_increments_to_a_moved_position() {
        let start = Instant::default();
        let mut simulation = spring_flick(-400., 10_000., start);
        let mut position = 0.;
        simulation.step(&mut position, start + Duration::from_millis(50));
        position += 1000.;
        assert!(simulation.step(&mut position, start + Duration::from_secs(10)));
        assert_approx_eq!(position, 1400.);
    }

    #[test]
    fn zero_deflection_is_finished() {
        let start = Instant::default();
        let mut simulation = spring_flick(0., 0., start);
        let mut position = 5.;
        assert!(simulation.step(&mut position, start));
        assert_eq!(position, 5.);
        assert_eq!(simulation.remaining_distance(start), 0.);
    }
}
