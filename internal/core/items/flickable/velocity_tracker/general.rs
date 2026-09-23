// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The default velocity tracking strategy: a least-squares quadratic fit
//! through recent position samples, evaluated at the latest sample. This is
//! a close port of Flutter's `VelocityTracker`.
//!
//! Original: <https://github.com/flutter/flutter/blob/d6bed8ff6135cdd414f14edc3063f761d47ca846/packages/flutter/lib/src/gestures/velocity_tracker.dart> (the `VelocityTracker` class)

use super::least_square::LeastSquaresSolver;
use super::ring_buffer::VelocityRingBuffer;
use super::{ASSUME_POINTER_MOVE_STOPPED, VelocityEstimate, VelocityEstimator, VelocityTracker};
use crate::animations::Instant;
use crate::lengths::{LogicalPx, LogicalVector};
use alloc::vec::Vec;
use core::time::Duration;
use euclid::Vector2D;

const HORIZON: Duration = Duration::from_millis(100);
const MIN_SAMPLE_SIZE: usize = 3;

#[derive(Default, Debug)]
pub(crate) struct GeneralVelocityTracker<const N: usize> {
    buffer: VelocityRingBuffer<N>,
}

impl<const N: usize> VelocityEstimator for GeneralVelocityTracker<N> {
    fn estimate_velocity_internal(&self) -> Option<VelocityEstimate> {
        let latest_time = self.buffer.last_time()?;

        let mut count = 0;

        let mut time = Vec::with_capacity(self.buffer.len());
        let mut x = Vec::with_capacity(self.buffer.len());
        let mut y = Vec::with_capacity(self.buffer.len());

        let mut previous: Option<&(Instant, Vector2D<f32, LogicalPx>)> = None;
        let mut iter = self.buffer.iter().rev(); // from newest to oldest
        let mut position = Vector2D::<f32, LogicalPx>::default(); // The entries are delta so we have to subtract
        while let Some(e) = iter.next() {
            let delta = previous
                .map(|p| {
                    position -= p.1;
                    p.0.0.saturating_sub(e.0.0)
                })
                .unwrap_or_default();
            let age = latest_time.0.saturating_sub(e.0.0);
            if delta > ASSUME_POINTER_MOVE_STOPPED || age > HORIZON {
                break;
            }

            let sample_time = -(age.as_secs_f32() * 1000.);
            if time.last() == Some(&sample_time) {
                previous = Some(e);
                continue;
            }
            time.push(sample_time);
            x.push(position.x);
            y.push(position.y);

            count += 1;
            previous = Some(e);
        }

        if count >= MIN_SAMPLE_SIZE {
            // We have a position fit
            // so deriving a second order function a*t^2 + b * t + c by x results in 2 * a * t + b
            // Evaluating at t = 0 leads to b. So the second coefficient is the velocity we are searching
            let degree = (count - 1).min(2);
            let res_x = LeastSquaresSolver::<'_, _, N>::new(&time, &x).solve::<3>(degree);
            let res_y = LeastSquaresSolver::<'_, _, N>::new(&time, &y).solve::<3>(degree);

            if let (Some(res_x), Some(res_y)) = (res_x, res_y) {
                // Convert values
                return Some(VelocityEstimate {
                    velocity: Vector2D::new(
                        res_x.coefficients()[1] * 1000.,
                        res_y.coefficients()[1] * 1000.,
                    ),
                    confidence: res_x.confidence * res_y.confidence,
                });
            }
        }

        return None;
    }
}

impl<const N: usize> VelocityTracker for GeneralVelocityTracker<N> {
    fn push(&mut self, time: Instant, position_delta: LogicalVector) {
        self.buffer.push(time, position_delta);
    }

    fn last_time(&self) -> Option<Instant> {
        self.buffer.last_time()
    }
}

#[cfg(test)]
mod tests_general_velocity_tracker {
    use alloc::vec;

    use super::*;
    use core::time::Duration;

    const EPSILON: f32 = 1e-2;

    macro_rules! values_equal {
        ($v1: expr, $exp: expr, $epsilon: expr) => {
            assert!(($v1 - $exp).abs() < $epsilon, "Received: {:?}, Expected: {:?}", $v1, $exp)
        };
        ($v1: expr, $exp: expr, $epsilon: expr, $name: expr) => {
            assert!(
                ($v1 - $exp).abs() < $epsilon,
                "Case '{:}': Received: {:?}, Expected: {:?}",
                $name,
                $v1,
                $exp
            )
        };
    }

    #[test]
    fn estimate_velocity_empty() {
        let tracker = GeneralVelocityTracker::<8>::default();
        assert!(tracker.estimate_velocity().is_none());
        assert_eq!(tracker.last_time(), None);
    }

    #[test]
    fn precise_samples_survive_buffer_wraparound() {
        let mut tracker = GeneralVelocityTracker::<3>::default();
        for i in 0..8 {
            tracker.push(Instant(Duration::from_micros(i * 3500)), LogicalVector::new(0., 21.));
        }
        assert_eq!(tracker.buffer.len(), 3);
        assert_eq!(tracker.last_sample_time(), Some(Duration::from_micros(24500)));
        let estimate = tracker.estimate_velocity_internal().unwrap();
        values_equal!(estimate.velocity.y, 6000., 0.1);
    }

    #[test]
    fn short_flick_preserves_submillisecond_timing() {
        let mut tracker = GeneralVelocityTracker::<8>::default();
        tracker.push(Instant(Duration::ZERO), LogicalVector::default());
        tracker.push(Instant(Duration::from_micros(7000)), LogicalVector::new(0., 42.));
        tracker.push(Instant(Duration::from_micros(10500)), LogicalVector::new(0., 21.));
        let estimate = tracker.estimate_velocity_internal().unwrap();
        values_equal!(estimate.velocity.y, 6000., 0.1);
    }

    #[test]
    fn short_flick_uses_two_distinct_samples() {
        let start = crate::animations::current_tick();
        let mut tracker = GeneralVelocityTracker::<8>::default();
        tracker.push(start, LogicalVector::default());
        tracker.push(start + Duration::from_millis(20), LogicalVector::new(0., 120.));
        let estimate = tracker.estimate_velocity_internal().unwrap();
        values_equal!(estimate.velocity.y, 6000., 0.1);
    }

    #[test]
    fn samples_at_the_same_time_preserve_distance() {
        let start = crate::animations::current_tick();
        let mut tracker = GeneralVelocityTracker::<8>::default();
        tracker.push(start, LogicalVector::default());
        tracker.push(start + Duration::from_millis(10), LogicalVector::new(0., 30.));
        tracker.push(start + Duration::from_millis(10), LogicalVector::new(0., 30.));
        tracker.push(start + Duration::from_millis(20), LogicalVector::new(0., 60.));
        let estimate = tracker.estimate_velocity_internal().unwrap();
        values_equal!(estimate.velocity.y, 6000., 0.1);
    }

    #[test]
    fn test_velocity_tracker_cases() {
        let base_time = crate::animations::current_tick();
        let test_cases = [
            (
                "x only",
                vec![
                    (base_time, LogicalVector::new(0.0, 0.0)),
                    (base_time + Duration::from_millis(10), LogicalVector::new(1.0, 0.0)),
                    (base_time + Duration::from_millis(20), LogicalVector::new(2.0, 0.0)),
                ],
                LogicalVector::new(2. / 20e-3, 0.),
            ),
            (
                "y only",
                vec![
                    (base_time, LogicalVector::new(0.0, 0.0)),
                    (base_time + Duration::from_millis(15), LogicalVector::new(0.0, 4.0)),
                    (base_time + Duration::from_millis(30), LogicalVector::new(0.0, 8.0)),
                ],
                LogicalVector::new(0., 8. / 30e-3),
            ),
            (
                "x and y",
                vec![
                    (base_time, LogicalVector::new(0.0, 0.0)),
                    (base_time + Duration::from_millis(15), LogicalVector::new(3., 4.0)),
                    (base_time + Duration::from_millis(30), LogicalVector::new(6., 8.0)),
                ],
                LogicalVector::new(6. / 30e-3, 8. / 30e-3),
            ),
            (
                // (x(t) = t^2 -> tau = t - 3 (age from newest) -> x(tau) = (tau + 3)^2 = tau ^2 + 6tau + 9
                // dx(tau)/d tau = 2 * tau + 6, at tau = 0 (newest point) -> dx(tau)/d tau = 6
                //
                // y(t) = 4*t^2 -> dy(t)/dt = 8 * t, at t = 3 (tau = 0): 24
                "square x and y",
                vec![
                    (base_time, LogicalVector::new(0.0, 0.0)),
                    (base_time + Duration::from_millis(10), LogicalVector::new(1. * 1., 4. * 1.)),
                    (base_time + Duration::from_millis(20), LogicalVector::new(1. * 4., 4. * 4.)),
                    (base_time + Duration::from_millis(30), LogicalVector::new(1. * 9., 4. * 9.)),
                ],
                LogicalVector::new(600., 2.4e3),
            ),
        ];

        for (name, test_values, expected) in test_cases {
            let mut tracker = GeneralVelocityTracker::<8>::default();
            let last_time = test_values.last().unwrap().0;
            let mut previous = LogicalVector::default();
            for (time, position) in test_values {
                tracker.push(time, position - previous);
                previous = position;
            }
            crate::animations::update_animations(last_time); // Otherwise the estimate_velocity might return None because time diff to large
            let res = tracker.estimate_velocity();
            assert_eq!(res.is_some(), true, "Case: {name}");
            let res = res.unwrap();
            values_equal!(res.velocity.x, expected.x, EPSILON, name);
            values_equal!(res.velocity.y, expected.y, EPSILON, name);
        }
    }
}
