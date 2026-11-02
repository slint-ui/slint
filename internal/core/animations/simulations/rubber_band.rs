// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! UIKit's rubber-band curve, `exposure = c * distance * viewport_length / (viewport_length + c * |distance|)`.
//! `distance` is how far the content would be past its edge without resistance;
//! `exposure` is how far it's displayed past the edge.
//! All functions require `viewport_length > 0`.

const COEFFICIENT: f32 = 0.55;

pub fn compress(distance: f32, viewport_length: f32) -> f32 {
    COEFFICIENT * distance * viewport_length / (viewport_length + COEFFICIENT * distance.abs())
}

pub fn uncompress(exposure: f32, viewport_length: f32) -> f32 {
    exposure * viewport_length / (COEFFICIENT * (viewport_length - exposure.abs()).max(0.001))
}

/// The derivative of [`compress`] with respect to `distance`.
pub fn compress_slope(distance: f32, viewport_length: f32) -> f32 {
    let scale = viewport_length / (viewport_length + COEFFICIENT * distance.abs());
    COEFFICIENT * scale * scale
}
