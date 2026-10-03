// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Ported from Flutter's `IOSScrollViewFlingVelocityTracker` (velocity_tracker.dart), which is:
//! Copyright 2014 The Flutter Authors. All rights reserved.
//!
//! Use of the original source is governed by a BSD-style license
//!
//! Original: <https://github.com/flutter/flutter/blob/d6bed8ff6135cdd414f14edc3063f761d47ca846/packages/flutter/lib/src/gestures/velocity_tracker.dart>
//!
//! A close approximation of iOS scroll view's fling velocity estimation
//! strategy: a weighted average of the last few point-to-point velocities

use super::fling::{BlendWeights, weighted_recent_velocity};
use super::ring_buffer::VelocityRingBuffer;
use super::{VelocityEstimate, VelocityEstimator, VelocityTracker};
use crate::animations::Instant;
use crate::lengths::LogicalVector;

///                        [oldest, middle, newest]`.
const WEIGHTS: BlendWeights = [0.6, 0.35, 0.05];
/// `weighted_recent_velocity` reads one more sample than there are weights
/// (each weight blends a segment between two consecutive samples).
const REQUIRED_SAMPLES: usize = WEIGHTS.len() + 1;

#[derive(Default, Debug)]
pub(crate) struct IOsVelocityTracker {
    buffer: VelocityRingBuffer<REQUIRED_SAMPLES>,
}

impl VelocityTracker for IOsVelocityTracker {
    fn push(&mut self, time: Instant, position_delta: LogicalVector) {
        self.buffer.push(time, position_delta);
    }

    fn last_time(&self) -> Option<Instant> {
        self.buffer.last_time()
    }
}

impl VelocityEstimator for IOsVelocityTracker {
    fn estimate_velocity_internal(&self) -> Option<VelocityEstimate> {
        Some(VelocityEstimate {
            velocity: weighted_recent_velocity(&self.buffer, WEIGHTS),
            confidence: 1.0,
        })
    }
}

#[cfg(test)]
mod tests_ios_velocity_tracker {
    use super::super::Velocity;
    use super::*;
    use core::time::Duration;

    #[test]
    fn estimate_velocity_is_none_when_empty() {
        let tracker = IOsVelocityTracker::default();
        assert!(tracker.estimate_velocity().is_none());
        assert_eq!(tracker.last_time(), None);
    }

    #[test]
    fn estimate_velocity_is_zero_with_a_single_sample() {
        let mut tracker = IOsVelocityTracker::default();
        tracker.push(Instant::default(), LogicalVector::new(5.0, 5.0));

        let estimate = tracker.estimate_velocity().unwrap();
        assert_eq!(estimate.velocity, Velocity::default());
        assert_eq!(estimate.confidence, 1.0);
    }

    #[test]
    fn estimate_velocity_blends_the_last_three_segments() {
        let mut tracker = IOsVelocityTracker::default();
        let base_time = crate::animations::current_tick();

        // 4 samples, 10ms apart; the first sample's delta is never used
        // (there's no earlier sample to pair it with), leaving 3 segments
        // of 1.0, 2.0, and 3.0 px per 10ms, i.e. 100, 200, 300 px/s.
        tracker.push(base_time, LogicalVector::new(0.0, 0.0));
        tracker.push(base_time + Duration::from_millis(10), LogicalVector::new(1.0, 0.0));
        tracker.push(base_time + Duration::from_millis(20), LogicalVector::new(2.0, 0.0));
        tracker.push(base_time + Duration::from_millis(30), LogicalVector::new(3.0, 0.0));
        crate::animations::update_animations(base_time + Duration::from_millis(30));

        let estimate = tracker.estimate_velocity().unwrap();
        let [oldest, middle, newest] = [100.0, 200.0, 300.0];
        let expected = oldest * WEIGHTS[0] + middle * WEIGHTS[1] + newest * WEIGHTS[2];
        assert!((estimate.velocity.x - expected).abs() < 1e-3);
        assert_eq!(estimate.velocity.y, 0.0);
        assert_eq!(estimate.confidence, 1.0);
    }
}
