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

/// iOS blends mostly the oldest segment: `[oldest, middle, newest]`.
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
