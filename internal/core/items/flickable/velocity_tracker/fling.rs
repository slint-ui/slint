// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Ported from Flutter's `IOSScrollViewFlingVelocityTracker._previousVelocityAt`
//! (velocity_tracker.dart), which is:
//! Copyright 2014 The Flutter Authors. All rights reserved.
//!
//! Use of the original source is governed by a BSD-style license
//!
//! Original: <https://github.com/flutter/flutter/blob/d6bed8ff6135cdd414f14edc3063f761d47ca846/packages/flutter/lib/src/gestures/velocity_tracker.dart>
//!
//! Shared machinery for the iOS/macOS fling velocity trackers: a weighted
//! blend of the last 3 point-to-point velocities, rather than a fit through
//! many samples. The two trackers differ only in their blend weights.

use super::Velocity;
use super::ring_buffer::VelocityRingBuffer;
use crate::animations::Instant;
use crate::lengths::LogicalPx;
use euclid::Vector2D;

pub(super) type BlendWeights = [f32; 3];

fn segment_velocity(
    sample: Option<&(Instant, Vector2D<f32, LogicalPx>)>,
    previous: Option<&(Instant, Vector2D<f32, LogicalPx>)>,
) -> Velocity {
    let (Some(sample), Some(previous)) = (sample, previous) else {
        return Velocity::default();
    };
    let dt = sample.0.duration_since(previous.0).as_secs_f32();
    if dt > 0.0 { sample.1 / dt } else { Velocity::default() }
}

/// Blends the velocities of the last 3 recorded segments in `buffer` using
/// `weights`.
pub(super) fn weighted_recent_velocity<const N: usize>(
    buffer: &VelocityRingBuffer<N>,
    weights: BlendWeights,
) -> Velocity {
    let mut recent = buffer.iter().rev();
    let newest = recent.next();
    let middle = recent.next();
    let oldest = recent.next();
    let before_oldest = recent.next();

    let newest_segment = segment_velocity(newest, middle);
    let middle_segment = segment_velocity(middle, oldest);
    let oldest_segment = segment_velocity(oldest, before_oldest);

    oldest_segment * weights[0] + middle_segment * weights[1] + newest_segment * weights[2]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lengths::LogicalVector;
    use core::time::Duration;

    #[test]
    fn blends_the_last_three_segments_by_weight() {
        let start = Instant::default();
        let mut buffer = VelocityRingBuffer::<4>::default();
        buffer.push(start, LogicalVector::default());
        assert_eq!(weighted_recent_velocity(&buffer, [1., 1., 1.]), Velocity::default());

        // The buffer drops the first sample; the rest form segments of 100, 200, and 300 px/s.
        for (millis, distance) in [(10, 9.), (20, 1.), (30, 2.), (40, 3.)] {
            buffer.push(
                start + Duration::from_millis(millis),
                LogicalVector::new(distance, -2. * distance),
            );
        }
        let velocity = weighted_recent_velocity(&buffer, [0.5, 0.3, 0.2]);
        let expected = 0.5 * 100. + 0.3 * 200. + 0.2 * 300.;
        assert!(
            (velocity - Velocity::new(expected, -2. * expected)).length() < 1e-3,
            "{velocity:?}"
        );
    }
}
