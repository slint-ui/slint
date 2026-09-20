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

use super::ring_buffer::VelocityRingBuffer;
use crate::Coord;
use crate::animations::Instant;
use crate::lengths::LogicalVector;

pub(super) type BlendWeights = [Coord; 3];

fn segment_velocity(
    sample: Option<&(Instant, LogicalVector)>,
    previous: Option<&(Instant, LogicalVector)>,
) -> LogicalVector {
    let (Some(sample), Some(previous)) = (sample, previous) else {
        return LogicalVector::default();
    };
    let dt = sample.0.duration_since(previous.0).as_millis() as Coord;
    if dt > 0.0 { sample.1 * (1000.0 / dt) } else { LogicalVector::default() }
}

/// Blends the velocities of the last 3 recorded segments in `buffer` using
/// `weights`.
pub(super) fn weighted_recent_velocity<const N: usize>(
    buffer: &VelocityRingBuffer<N>,
    weights: BlendWeights,
) -> LogicalVector {
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
