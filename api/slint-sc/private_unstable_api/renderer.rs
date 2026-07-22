// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

/// Fill the whole frame buffer, made of packed RGB triplets, with a single color.
pub fn fill_rgb8(frame_buffer: &mut [u8], red: u8, green: u8, blue: u8) {
    let color = [red, green, blue];
    for pixel in frame_buffer.chunks_exact_mut(3) {
        pixel.copy_from_slice(&color);
    }
}
