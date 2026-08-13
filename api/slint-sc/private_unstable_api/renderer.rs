// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

/// The pixels painted on one axis: the intersection of
/// [position, position + extent) with [0, buffer_extent). What sticks out of
/// the buffer, including at a negative position, is cut off.
fn span(position: i32, extent: i32, buffer_extent: u32) -> (usize, usize) {
    let start = position.clamp(0, buffer_extent as i32) as usize;
    let end = position.saturating_add(extent).clamp(0, buffer_extent as i32) as usize;
    (start, end)
}

/// Composite `color` over a pixel of the frame buffer, which carries no alpha
/// channel: the destination is opaque by construction, and so is the result.
fn blend_pixel(pixel: &mut [u8; 3], color: crate::Color) {
    let destination = crate::Color::from_rgb_u8(pixel[0], pixel[1], pixel[2]);
    let blended = color.composite_over(destination);
    *pixel = [blended.red(), blended.green(), blended.blue()];
}

/// Fill a rectangle of the frame buffer, made of packed RGB triplets, with a
/// single color, composited over what the buffer already holds: a translucent
/// color lets the pixels underneath show through, and a fully transparent one
/// paints nothing. The rectangle is clipped to the buffer, whose length must
/// be `buffer_size.width * buffer_size.height * 3`.
pub fn fill_rect(
    frame_buffer: &mut [u8],
    buffer_size: crate::Size,
    position: [i32; 2],
    size: [i32; 2],
    color: crate::Color,
) {
    let alpha = color.alpha();
    // A fully transparent color leaves every pixel as it was
    if alpha == 0 {
        return;
    }
    let (x0, x1) = span(position[0], size[0], buffer_size.width);
    let (y0, y1) = span(position[1], size[1], buffer_size.height);
    let stride = buffer_size.width as usize * 3;
    let rgb = [color.red(), color.green(), color.blue()];
    for row in y0..y1 {
        let row_range = row * stride + x0 * 3..row * stride + x1 * 3;
        let pixels = frame_buffer[row_range].as_chunks_mut::<3>().0;
        if alpha == 0xff {
            pixels.fill(rgb);
        } else {
            for pixel in pixels {
                blend_pixel(pixel, color);
            }
        }
    }
}

/// Draw an image into the frame buffer of packed RGB triplets, one image
/// pixel per frame-buffer pixel, each composited over what the buffer already
/// holds. The image is clipped to the buffer, whose length must be
/// `buffer_size.width * buffer_size.height * 3`. An [`Image::None`](crate::Image)
/// draws nothing.
pub fn draw_image(
    frame_buffer: &mut [u8],
    buffer_size: crate::Size,
    position: [i32; 2],
    image: crate::Image,
) {
    let crate::Image::StaticArgb { argb, width: image_width } = image else {
        return;
    };
    // The image pixels, four packed bytes each, alpha first like the ARGB
    // encoding, counted row by row from the top-left. An incomplete trailing
    // pixel falls out of the chunking, like an incomplete row falls out of
    // the height.
    let pixels = argb.as_chunks::<4>().0;
    let pixel_at =
        |index: usize| crate::Color::from_argb_encoded(u32::from_be_bytes(pixels[index]));
    // The image is clipped to the buffer like a fill_rect rectangle. A width
    // or height beyond i32 saturates: the span stays within the buffer and
    // the source offsets below stay exact, so nothing is lost.
    let extent = |dimension: usize| i32::try_from(dimension).unwrap_or(i32::MAX);
    let (x0, x1) = span(position[0], extent(image.width()), buffer_size.width);
    let (y0, y1) = span(position[1], extent(image.height()), buffer_size.height);
    // How far into an image row the drawn span starts: 0 unless the position
    // is negative, then the columns sticking out are skipped
    let source_x0 = (x0 as i32 - position[0]) as usize;
    let stride = buffer_size.width as usize * 3;
    for row in y0..y1 {
        // The image row drawn on buffer row `row`; rows above the buffer are
        // skipped like the columns
        let source_row = (row as i32 - position[1]) as usize * image_width + source_x0;
        let row_range = row * stride + x0 * 3..row * stride + x1 * 3;
        let destination = frame_buffer[row_range].as_chunks_mut::<3>().0;
        for (column, pixel) in destination.iter_mut().enumerate() {
            blend_pixel(pixel, pixel_at(source_row + column));
        }
    }
}

#[test]
fn test_fill_rect_negative_position() {
    // A 3x2 rectangle at (-2, -1) intersects a 4x4 buffer in the single
    // pixel (0, 0)
    let mut buffer = [0u8; 4 * 4 * 3];
    fill_rect(
        &mut buffer,
        crate::Size::new(4, 4),
        [-2, -1],
        [3, 2],
        crate::Color::from_rgb_u8(1, 2, 3),
    );
    for y in 0..4 {
        for x in 0..4 {
            let expected = if (x, y) == (0, 0) { [1, 2, 3] } else { [0, 0, 0] };
            assert_eq!(buffer[(y * 4 + x) * 3..][..3], expected, "pixel ({x}, {y})");
        }
    }

    // A rectangle entirely outside the buffer paints nothing
    let mut buffer = [7u8; 4 * 4 * 3];
    fill_rect(
        &mut buffer,
        crate::Size::new(4, 4),
        [-5, -5],
        [3, 2],
        crate::Color::from_rgb_u8(1, 2, 3),
    );
    assert_eq!(buffer, [7u8; 4 * 4 * 3]);
}

#[test]
fn test_fill_rect_blends_with_the_destination() {
    // A 2x2 rectangle of half-transparent red over a buffer of a single color
    let destination = [200, 100, 50];
    let mut buffer = [0u8; 4 * 4 * 3];
    buffer.as_chunks_mut::<3>().0.fill(destination);
    fill_rect(
        &mut buffer,
        crate::Size::new(4, 4),
        [1, 1],
        [2, 2],
        crate::Color::from_argb_encoded(0x80ff0000),
    );
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
    fill_rect(
        &mut buffer,
        crate::Size::new(4, 4),
        [-1, -1],
        [2, 2],
        crate::Color::from_argb_encoded(0x80ff0000),
    );
    for y in 0..4 {
        for x in 0..4 {
            let expected = if (x, y) == (0, 0) { blended } else { destination };
            assert_eq!(buffer[(y * 4 + x) * 3..][..3], expected, "pixel ({x}, {y})");
        }
    }
}

/// A 2x2 test image: opaque red and half-transparent green on the top row,
/// opaque blue and fully transparent on the bottom row.
#[cfg(test)]
static TEST_IMAGE_ARGB: [u8; 16] = [
    0xff, 0xff, 0x00, 0x00, // opaque red
    0x80, 0x00, 0xff, 0x00, // half-transparent green
    0xff, 0x00, 0x00, 0xff, // opaque blue
    0x00, 0x00, 0x00, 0x00, // fully transparent
];

#[cfg(test)]
const TEST_IMAGE: crate::Image = crate::Image::StaticArgb { argb: &TEST_IMAGE_ARGB, width: 2 };

#[test]
fn test_draw_image() {
    // The image is drawn pixel for pixel at its position, over the buffer:
    // opaque pixels replace, the half-transparent green blends with the white
    // underneath, and the transparent pixel leaves the buffer as it was
    //#sls.paint.image
    //#sls.paint.image.blend
    let mut buffer = [0xff; 4 * 4 * 3];
    draw_image(&mut buffer, crate::Size::new(4, 4), [1, 2], TEST_IMAGE);
    // 0 * 128 + 255 * 127 + 127 == 32512, and 32512 / 255 == 127; green stays 255
    let blended_green = [127, 255, 127];
    for y in 0..4 {
        for x in 0..4 {
            let expected = match (x, y) {
                (1, 2) => [255, 0, 0],
                (2, 2) => blended_green,
                (1, 3) => [0, 0, 255],
                _ => [255, 255, 255],
            };
            assert_eq!(buffer[(y * 4 + x) * 3..][..3], expected, "pixel ({x}, {y})");
        }
    }
}

#[test]
fn test_draw_image_clipped() {
    // At (-1, -1) only the transparent bottom-right image pixel lands on the
    // buffer pixel (0, 0), which it leaves untouched
    //#sls.paint.clip
    let mut buffer = [7u8; 2 * 2 * 3];
    draw_image(&mut buffer, crate::Size::new(2, 2), [-1, -1], TEST_IMAGE);
    assert_eq!(buffer, [7u8; 2 * 2 * 3]);

    // At (-1, 0) the right image column lands on the buffer's left column:
    // half-transparent green over 7 gray, and the transparent pixel below
    let mut buffer = [7u8; 2 * 2 * 3];
    draw_image(&mut buffer, crate::Size::new(2, 2), [-1, 0], TEST_IMAGE);
    // 0 * 128 + 7 * 127 + 127 == 1016, and 1016 / 255 == 3;
    // 255 * 128 + 7 * 127 + 127 == 33656, and 33656 / 255 == 131
    assert_eq!(buffer[0..3], [3, 131, 3]);
    assert_eq!(buffer[3..6], [7, 7, 7]);
    assert_eq!(buffer[6..12], [7u8; 6]);

    // At (1, 1) of a 2x2 buffer only the top-left image pixel is inside
    let mut buffer = [7u8; 2 * 2 * 3];
    draw_image(&mut buffer, crate::Size::new(2, 2), [1, 1], TEST_IMAGE);
    assert_eq!(buffer[0..9], [7u8; 9]);
    assert_eq!(buffer[9..12], [255, 0, 0]);

    // Entirely outside the buffer, nothing is drawn
    let mut buffer = [7u8; 2 * 2 * 3];
    draw_image(&mut buffer, crate::Size::new(2, 2), [2, 0], TEST_IMAGE);
    draw_image(&mut buffer, crate::Size::new(2, 2), [0, -2], TEST_IMAGE);
    assert_eq!(buffer, [7u8; 2 * 2 * 3]);
}

#[test]
fn test_draw_image_nothing_to_draw() {
    // No image, and a static image without pixels, draw nothing
    //#sls.paint.image.none
    let mut buffer = [7u8; 2 * 2 * 3];
    draw_image(&mut buffer, crate::Size::new(2, 2), [0, 0], crate::Image::None);
    draw_image(
        &mut buffer,
        crate::Size::new(2, 2),
        [0, 0],
        crate::Image::StaticArgb { argb: &[], width: 0 },
    );
    // An incomplete last row is not drawn: with a width of 4, three pixels
    // of bytes make no complete row
    draw_image(
        &mut buffer,
        crate::Size::new(2, 2),
        [0, 0],
        crate::Image::StaticArgb { argb: &TEST_IMAGE_ARGB[..12], width: 4 },
    );
    // Neither is an incomplete packed pixel: three bytes make no pixel
    draw_image(
        &mut buffer,
        crate::Size::new(2, 2),
        [0, 0],
        crate::Image::StaticArgb { argb: &[0xff, 0xff, 0xff], width: 1 },
    );
    assert_eq!(buffer, [7u8; 2 * 2 * 3]);
}

#[test]
fn test_fill_rect_transparent() {
    // A fully transparent color paints nothing, whatever its other channels
    let mut buffer = [7u8; 4 * 4 * 3];
    fill_rect(
        &mut buffer,
        crate::Size::new(4, 4),
        [0, 0],
        [4, 4],
        crate::Color::from_argb_encoded(0x00ff8040),
    );
    assert_eq!(buffer, [7u8; 4 * 4 * 3]);
    // The default color is transparent
    fill_rect(&mut buffer, crate::Size::new(4, 4), [0, 0], [4, 4], crate::Color::default());
    assert_eq!(buffer, [7u8; 4 * 4 * 3]);
}
