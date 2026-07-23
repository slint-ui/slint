// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

/// Fill a rectangle of the frame buffer, made of packed RGB triplets, with a
/// single color. The rectangle is clipped to the buffer, whose length must be
/// `buffer_size[0] * buffer_size[1] * 3`.
pub fn fill_rect(
    frame_buffer: &mut [u8],
    buffer_size: [u32; 2],
    position: [i32; 2],
    size: [i32; 2],
    color: [u8; 3],
) {
    // The drawn span on each axis is the intersection of
    // [position, position + size) with [0, buffer_size): a rectangle sticking
    // out of the buffer, including one at a negative position, is shortened
    let x0 = position[0].clamp(0, buffer_size[0] as i32) as usize;
    let x1 = position[0].saturating_add(size[0]).clamp(0, buffer_size[0] as i32) as usize;
    let y0 = position[1].clamp(0, buffer_size[1] as i32) as usize;
    let y1 = position[1].saturating_add(size[1]).clamp(0, buffer_size[1] as i32) as usize;
    let stride = buffer_size[0] as usize * 3;
    for row in y0..y1 {
        let row_range = row * stride + x0 * 3..row * stride + x1 * 3;
        for pixel in frame_buffer[row_range].chunks_exact_mut(3) {
            pixel.copy_from_slice(&color);
        }
    }
}

#[test]
fn test_fill_rect_negative_position() {
    // A 3x2 rectangle at (-2, -1) intersects a 4x4 buffer in the single
    // pixel (0, 0)
    let mut buffer = [0u8; 4 * 4 * 3];
    fill_rect(&mut buffer, [4, 4], [-2, -1], [3, 2], [1, 2, 3]);
    for y in 0..4 {
        for x in 0..4 {
            let expected = if (x, y) == (0, 0) { [1, 2, 3] } else { [0, 0, 0] };
            assert_eq!(buffer[(y * 4 + x) * 3..][..3], expected, "pixel ({x}, {y})");
        }
    }

    // A rectangle entirely outside the buffer paints nothing
    let mut buffer = [7u8; 4 * 4 * 3];
    fill_rect(&mut buffer, [4, 4], [-5, -5], [3, 2], [1, 2, 3]);
    assert_eq!(buffer, [7u8; 4 * 4 * 3]);
}
