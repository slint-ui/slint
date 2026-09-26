// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Ported from Flutter's `VelocityTracker` (velocity_tracker.dart), which is:
//! Copyright 2014 The Flutter Authors. All rights reserved.
//!
//! Use of the original source is governed by a BSD-style license
//!
//! Original: <https://github.com/flutter/flutter/blob/d6bed8ff6135cdd414f14edc3063f761d47ca846/packages/flutter/lib/src/gestures/velocity_tracker.dart>
//!
//! Estimates a pointer's fling velocity from a short history of positions,
//! for use as the flickable's initial deceleration-animation velocity

#[cfg(any(test, target_os = "ios", target_os = "linux", target_os = "none", target_os = "macos"))]
mod fling;
#[cfg(any(
    test,
    not(any(target_os = "ios", target_os = "linux", target_os = "none", target_os = "macos"))
))]
mod general;
#[cfg(any(test, target_os = "ios", target_os = "linux", target_os = "none"))]
mod ios;
#[cfg(any(
    test,
    not(any(target_os = "ios", target_os = "linux", target_os = "none", target_os = "macos"))
))]
mod least_square;
#[cfg(any(test, target_os = "macos"))]
mod macos;
mod ring_buffer;

#[cfg(not(any(
    target_os = "ios",
    target_os = "linux",
    target_os = "none",
    target_os = "macos"
)))]
pub(crate) use general::GeneralVelocityTracker;
#[cfg(any(target_os = "ios", target_os = "linux", target_os = "none"))]
pub(crate) use ios::IOsVelocityTracker;
#[cfg(target_os = "macos")]
pub(crate) use macos::MacOsVelocityTracker;

use crate::animations::Instant;
use crate::lengths::{LogicalPx, LogicalVector};
use core::time::Duration;

// https://github.com/flutter/flutter/blob/d6bed8ff6135cdd414f14edc3063f761d47ca846/packages/flutter/lib/src/gestures/velocity_tracker.dart#L142-L145
//
// Shared by every tracking strategy: if the caller hasn't pushed a new
// sample within this long, the pointer is considered to have stopped.
const ASSUME_POINTER_MOVE_STOPPED: Duration = Duration::from_millis(40);

/// Logical pixels per second. Always `f32`: with an integer `Coord`, a rate would be
/// truncated to whole pixels per second.
pub(crate) type Velocity = euclid::Vector2D<f32, LogicalPx>;

pub(crate) struct VelocityEstimate {
    pub(crate) velocity: Velocity,
    #[cfg_attr(not(test), expect(unused, reason = "Confidence is not yet considered"))]
    pub(crate) confidence: f32,
}

trait VelocityEstimator {
    fn estimate_velocity_internal(&self) -> Option<VelocityEstimate>;
}

// VelocityEstimator stays module-private on purpose: it seals VelocityTracker so only the
// trackers defined in this module can implement it, while estimate_velocity()'s timeout check
// below remains the only entry point external callers get.
#[allow(private_bounds)]
pub(crate) trait VelocityTracker: VelocityEstimator {
    fn push(&mut self, time: Instant, position_delta: LogicalVector);
    fn last_time(&self) -> Option<Instant>;
    fn estimate_velocity(&self) -> Option<VelocityEstimate> {
        if crate::animations::current_tick().0.saturating_sub(self.last_time()?.0)
            > ASSUME_POINTER_MOVE_STOPPED
        {
            return None;
        }
        self.estimate_velocity_internal()
    }
}
