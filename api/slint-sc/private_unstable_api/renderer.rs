// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

/// Fill a rectangle of the frame buffer, made of packed RGB triplets, with a
/// single color, composited over what the buffer already holds: a translucent
/// color lets the pixels underneath show through, and a fully transparent one
/// paints nothing. The rectangle is clipped to the buffer, whose length must
/// be `buffer_size[0] * buffer_size[1] * 3`.
pub fn fill_rect(
    frame_buffer: &mut [u8],
    buffer_size: [u32; 2],
    position: [i32; 2],
    size: [i32; 2],
    color: crate::Color,
) {
    let alpha = color.alpha();
    // A fully transparent color leaves every pixel as it was
    if alpha == 0 {
        return;
    }
    // The drawn span on each axis is the intersection of
    // [position, position + size) with [0, buffer_size): a rectangle sticking
    // out of the buffer, including one at a negative position, is shortened
    let x0 = position[0].clamp(0, buffer_size[0] as i32) as usize;
    let x1 = position[0].saturating_add(size[0]).clamp(0, buffer_size[0] as i32) as usize;
    let y0 = position[1].clamp(0, buffer_size[1] as i32) as usize;
    let y1 = position[1].saturating_add(size[1]).clamp(0, buffer_size[1] as i32) as usize;
    let stride = buffer_size[0] as usize * 3;
    let rgb = [color.red(), color.green(), color.blue()];
    for row in y0..y1 {
        let row_range = row * stride + x0 * 3..row * stride + x1 * 3;
        let pixels = frame_buffer[row_range].as_chunks_mut::<3>().0;
        if alpha == 0xff {
            pixels.fill(rgb);
        } else {
            for pixel in pixels {
                // The buffer carries no alpha channel, so the destination is
                // opaque by construction, and the result is opaque in turn
                let destination = crate::Color::from_rgb_u8(pixel[0], pixel[1], pixel[2]);
                let blended = color.composite_over(destination);
                *pixel = [blended.red(), blended.green(), blended.blue()];
            }
        }
    }
}

#[test]
fn test_fill_rect_negative_position() {
    // A 3x2 rectangle at (-2, -1) intersects a 4x4 buffer in the single
    // pixel (0, 0)
    let mut buffer = [0u8; 4 * 4 * 3];
    fill_rect(&mut buffer, [4, 4], [-2, -1], [3, 2], crate::Color::from_rgb_u8(1, 2, 3));
    for y in 0..4 {
        for x in 0..4 {
            let expected = if (x, y) == (0, 0) { [1, 2, 3] } else { [0, 0, 0] };
            assert_eq!(buffer[(y * 4 + x) * 3..][..3], expected, "pixel ({x}, {y})");
        }
    }

    // A rectangle entirely outside the buffer paints nothing
    let mut buffer = [7u8; 4 * 4 * 3];
    fill_rect(&mut buffer, [4, 4], [-5, -5], [3, 2], crate::Color::from_rgb_u8(1, 2, 3));
    assert_eq!(buffer, [7u8; 4 * 4 * 3]);
}

#[test]
fn test_fill_rect_blends_with_the_destination() {
    // A 2x2 rectangle of half-transparent red over a buffer of a single color
    let destination = [200, 100, 50];
    let mut buffer = [0u8; 4 * 4 * 3];
    buffer.as_chunks_mut::<3>().0.fill(destination);
    fill_rect(&mut buffer, [4, 4], [1, 1], [2, 2], crate::Color::from_argb_encoded(0x80ff0000));
    // 255 * 128 + 200 * 127 + 127 == 58167, and 58167 / 255 == 228; the green
    // and blue channels keep 127/255 of the destination alone
    let blended = [228, 50, 25];
    for y in 0..4 {
        for x in 0..4 {
            let expected =
                if (1..3).contains(&x) && (1..3).contains(&y) { blended } else { destination };
            assert_eq!(buffer[(y * 4 + x) * 3..][..3], expected, "pixel ({x}, {y})");
        }
    }

    // Blending is clipped like any other fill: at (-1, -1) only the pixel
    // (0, 0) is blended
    let mut buffer = [0u8; 4 * 4 * 3];
    buffer.as_chunks_mut::<3>().0.fill(destination);
    fill_rect(&mut buffer, [4, 4], [-1, -1], [2, 2], crate::Color::from_argb_encoded(0x80ff0000));
    for y in 0..4 {
        for x in 0..4 {
            let expected = if (x, y) == (0, 0) { blended } else { destination };
            assert_eq!(buffer[(y * 4 + x) * 3..][..3], expected, "pixel ({x}, {y})");
        }
    }
}

#[test]
fn test_fill_rect_transparent() {
    // A fully transparent color paints nothing, whatever its other channels
    let mut buffer = [7u8; 4 * 4 * 3];
    fill_rect(&mut buffer, [4, 4], [0, 0], [4, 4], crate::Color::from_argb_encoded(0x00ff8040));
    assert_eq!(buffer, [7u8; 4 * 4 * 3]);
    // The default color is transparent
    fill_rect(&mut buffer, [4, 4], [0, 0], [4, 4], crate::Color::default());
    assert_eq!(buffer, [7u8; 4 * 4 * 3]);
}
