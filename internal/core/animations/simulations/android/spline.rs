// Copyright (C) 2010 The Android Open Source Project
// SPDX-License-Identifier: Apache-2.0

//! Unbounded spline fling from Android's OverScroller.java, android-15.0.0_r1.
//! https://android.googlesource.com/platform/frameworks/base/+/android-15.0.0_r1/core/java/android/widget/OverScroller.java
//! Coordinates are logical pixels (Android density 1). Bounds are handled by
//! the caller

use core::time::Duration;
#[cfg(not(feature = "std"))]
use num_traits::Float;

const RATE: f32 = 2.3582017; // ln(0.78) / ln(0.9)
const PHYSICAL_COEFFICIENT: f32 = 9.80665 * 39.37 * 160.0 * 0.84;

const POSITION: [f32; 101] = {
    let mut table = [0.; 101];
    let mut i = 0;
    let mut low = 0.;
    while i < 100 {
        let alpha = i as f32 / 100.;
        let mut high = 1.;
        loop {
            let x = (low + high) / 2.;
            let coefficient = 3. * x * (1. - x);
            let tx = coefficient * ((1. - x) * 0.175 + x * 0.35) + x * x * x;
            let error = tx - alpha;
            if error > -0.00001 && error < 0.00001 {
                table[i] = coefficient * ((1. - x) * 0.5 + x) + x * x * x;
                break;
            }
            if tx > alpha {
                high = x;
            } else {
                low = x;
            }
        }
        i += 1;
    }
    table[100] = 1.;
    table
};

pub(super) fn parameters(velocity: f32, friction: f32) -> (Duration, f32) {
    if velocity == 0. || !velocity.is_finite() || !friction.is_finite() || friction <= 0. {
        return (Duration::ZERO, 0.);
    }
    let deceleration = ((0.35 * velocity.abs() / (friction * PHYSICAL_COEFFICIENT)) as f64).ln();
    let rate = RATE as f64;
    let milliseconds = (1000. * (deceleration / (rate - 1.)).exp()) as u64;
    let distance =
        (friction * PHYSICAL_COEFFICIENT) as f64 * (rate / (rate - 1.) * deceleration).exp();
    // AOSP stores total distance as an integer. Positions between samples remain
    // fractional in Slint, avoiding rounding every rendered frame.
    (Duration::from_millis(milliseconds), (distance * velocity.signum() as f64) as i32 as f32)
}

pub(super) fn sample(t: f32) -> (f32, f32) {
    let index = (100. * t) as usize;
    if index >= 100 {
        return (1., 0.);
    }
    let lower = index as f32 / 100.;
    let upper = (index + 1) as f32 / 100.;
    let slope = (POSITION[index + 1] - POSITION[index]) / (upper - lower);
    (POSITION[index] + (t - lower) * slope, slope)
}
