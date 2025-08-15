// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![allow(clippy::identity_op)] // We use x + 0 a lot here for symmetry

//! This is the module for the functions that are drawing the pixels
//! on the line buffer

use super::{Fixed, PhysicalLength, PhysicalRect};
use crate::graphics::{Rgb8Pixel, TexturePixelFormat};
use crate::lengths::{PointLengths, SizeLengths};
use crate::Color;
use derive_more::{Add, Sub};
use integer_sqrt::IntegerSquareRoot;
#[allow(unused_imports)]
use num_traits::Float;

/// Draw one line of the texture in the line buffer
///
pub(super) fn draw_texture_line(
    span: &PhysicalRect,
    line: PhysicalLength,
    texture: &super::SceneTexture,
    line_buffer: &mut [impl TargetPixel],
    extra_clip_begin: i16,
    extra_clip_end: i16,
) {
    let super::SceneTexture {
        data,
        format,
        pixel_stride,
        extra: super::SceneTextureExtra { colorize, alpha, rotation, dx, dy, off_x, off_y },
    } = *texture;

    let source_size = texture.source_size().cast::<i32>();
    let len = line_buffer.len();
    let y = line - span.origin.y_length();
    let y = if rotation.mirror_width() { span.size.height - y.get() - 1 } else { y.get() } as i32;

    let off_y = Fixed::<i32, 8>::from_fixed(off_y);
    let dx = Fixed::<i32, 8>::from_fixed(dx);
    let dy = Fixed::<i32, 8>::from_fixed(dy);
    let off_x = Fixed::<i32, 8>::from_fixed(off_x);

    if !rotation.is_transpose() {
        let mut delta = dx;
        let row = off_y + dy * y;
        // The position where to start in the image array for a this row
        let mut init =
            Fixed::from_integer(row.truncate() % source_size.height) * pixel_stride as i32;

        // the size of the tile in physical pixels in the target
        let tile_len = (Fixed::from_integer(source_size.width) / delta) as usize;
        // the amount of missing image pixel on one tile
        let mut remainder = Fixed::from_integer(source_size.width) % delta;
        // The position in image pixel where to get the image
        let mut pos;
        // the end index in the target buffer
        let mut end;
        // the accumulated error in image pixels
        let mut acc_err;
        if rotation.mirror_height() {
            let o = (off_x + (delta * (extra_clip_end as i32 + len as i32 - 1)))
                % Fixed::from_integer(source_size.width);
            pos = init + o;
            init += Fixed::from_integer(source_size.width);
            end = (o / delta) as usize + 1;
            acc_err = -delta + o % delta;
            delta = -delta;
            remainder = -remainder;
        } else {
            let o =
                (off_x + delta * extra_clip_begin as i32) % Fixed::from_integer(source_size.width);
            pos = init + o;
            end = ((Fixed::from_integer(source_size.width) - o) / delta) as usize;
            acc_err = (Fixed::from_integer(source_size.width) - o) % delta;
            if acc_err != Fixed::default() {
                acc_err = delta - acc_err;
                end += 1;
            }
        }
        end = end.min(len);
        let mut begin = 0;
        let row_fract = row.fract();
        while begin < len {
            fetch_blend_pixel(
                &mut line_buffer[begin..end],
                format,
                data,
                alpha,
                colorize,
                (pixel_stride as usize, dy),
                #[inline(always)]
                |bpp| {
                    let p = (pos.truncate() as usize * bpp, pos.fract(), row_fract);
                    pos += delta;
                    p
                },
            );
            begin = end;
            end += tile_len;
            pos = init;
            pos += acc_err;
            if remainder != Fixed::from_integer(0) {
                acc_err -= remainder;
                let wrap = if rotation.mirror_height() {
                    acc_err >= Fixed::from_integer(0)
                } else {
                    acc_err < Fixed::from_integer(0)
                };
                if wrap {
                    acc_err += delta;
                    end += 1;
                }
            };
            end = end.min(len);
        }
    } else {
        let bpp = format.bpp();
        let col = off_x + dx * y;
        let col_fract = col.fract();
        let col = (col.truncate() % source_size.width) as usize * bpp;
        let stride = pixel_stride as usize * bpp;
        let mut row_delta = dy;
        let tile_len = (Fixed::from_integer(source_size.height) / row_delta) as usize;
        let mut remainder = Fixed::from_integer(source_size.height) % row_delta;
        let mut end;
        let mut row_init = Fixed::default();
        let mut row;
        let mut acc_err;
        if rotation.mirror_height() {
            row_init = Fixed::from_integer(source_size.height);
            row = (off_y + (row_delta * (extra_clip_end as i32 + len as i32 - 1)))
                % Fixed::from_integer(source_size.height);
            end = (row / row_delta) as usize + 1;
            acc_err = -row_delta + row % row_delta;
            row_delta = -row_delta;
            remainder = -remainder;
        } else {
            row = (off_y + row_delta * extra_clip_begin as i32)
                % Fixed::from_integer(source_size.height);
            end = ((Fixed::from_integer(source_size.height) - row) / row_delta) as usize;
            acc_err = (Fixed::from_integer(source_size.height) - row) % row_delta;
            if acc_err != Fixed::default() {
                acc_err = row_delta - acc_err;
                end += 1;
            }
        };
        end = end.min(len);
        let mut begin = 0;
        while begin < len {
            fetch_blend_pixel(
                &mut line_buffer[begin..end],
                format,
                data,
                alpha,
                colorize,
                (stride, dy),
                #[inline(always)]
                |_| {
                    let pos = (row.truncate() as usize * stride + col, col_fract, row.fract());
                    row += row_delta;
                    pos
                },
            );
            begin = end;
            end += tile_len;
            row = row_init;
            row += acc_err;
            if remainder != Fixed::from_integer(0) {
                acc_err -= remainder;
                let wrap = if rotation.mirror_height() {
                    acc_err >= Fixed::from_integer(0)
                } else {
                    acc_err < Fixed::from_integer(0)
                };
                if wrap {
                    acc_err += row_delta;
                    end += 1;
                }
            };
            end = end.min(len);
        }
    };

    fn fetch_blend_pixel(
        line_buffer: &mut [impl TargetPixel],
        format: TexturePixelFormat,
        data: &[u8],
        alpha: u8,
        color: Color,
        (stride, delta): (usize, Fixed<i32, 8>),
        mut pos: impl FnMut(usize) -> (usize, u8, u8),
    ) {
        match format {
            TexturePixelFormat::Rgb => {
                for pix in line_buffer {
                    let pos = pos(3).0;
                    let p: &[u8] = &data[pos..pos + 3];
                    if alpha == 0xff {
                        *pix = TargetPixel::from_rgb(p[0], p[1], p[2]);
                    } else {
                        pix.blend(PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                            alpha, p[0], p[1], p[2],
                        )))
                    }
                }
            }
            TexturePixelFormat::Rgba => {
                if color.alpha() == 0 {
                    for pix in line_buffer {
                        let pos = pos(4).0;
                        let alpha = ((data[pos + 3] as u16 * alpha as u16) / 255) as u8;
                        let c = PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                            alpha,
                            data[pos + 0],
                            data[pos + 1],
                            data[pos + 2],
                        ));
                        pix.blend(c);
                    }
                } else {
                    for pix in line_buffer {
                        let pos = pos(4).0;
                        let alpha = ((data[pos + 3] as u16 * alpha as u16) / 255) as u8;
                        let c = PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                            alpha,
                            color.red(),
                            color.green(),
                            color.blue(),
                        ));
                        pix.blend(c);
                    }
                }
            }
            TexturePixelFormat::RgbaPremultiplied => {
                if color.alpha() > 0 {
                    for pix in line_buffer {
                        let pos = pos(4).0;
                        let c = PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                            ((data[pos + 3] as u16 * alpha as u16) / 255) as u8,
                            color.red(),
                            color.green(),
                            color.blue(),
                        ));
                        pix.blend(c);
                    }
                } else if alpha == 0xff {
                    for pix in line_buffer {
                        let pos = pos(4).0;
                        let c = PremultipliedRgbaColor {
                            alpha: data[pos + 3],
                            red: data[pos + 0],
                            green: data[pos + 1],
                            blue: data[pos + 2],
                        };
                        pix.blend(c);
                    }
                } else {
                    for pix in line_buffer {
                        let pos = pos(4).0;
                        let c = PremultipliedRgbaColor {
                            alpha: (data[pos + 3] as u16 * alpha as u16 / 255) as u8,
                            red: (data[pos + 0] as u16 * alpha as u16 / 255) as u8,
                            green: (data[pos + 1] as u16 * alpha as u16 / 255) as u8,
                            blue: (data[pos + 2] as u16 * alpha as u16 / 255) as u8,
                        };
                        pix.blend(c);
                    }
                }
            }
            TexturePixelFormat::AlphaMap => {
                for pix in line_buffer {
                    let pos = pos(1).0;
                    let c = PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                        ((data[pos] as u16 * alpha as u16) / 255) as u8,
                        color.red(),
                        color.green(),
                        color.blue(),
                    ));
                    pix.blend(c);
                }
            }
            TexturePixelFormat::SignedDistanceField => {
                const RANGE: i32 = 6;
                let factor = (362 * 256 / delta.0) * RANGE; // 362 ≃ 255 * sqrt(2)
                for pix in line_buffer {
                    let (pos, col_f, row_f) = pos(1);
                    let (col_f, row_f) = (col_f as i32, row_f as i32);
                    let mut dist = ((data[pos] as i8 as i32) * (256 - col_f)
                        + (data[pos + 1] as i8 as i32) * col_f)
                        * (256 - row_f);
                    if pos + stride + 1 < data.len() {
                        dist += ((data[pos + stride] as i8 as i32) * (256 - col_f)
                            + (data[pos + stride + 1] as i8 as i32) * col_f)
                            * row_f
                    } else {
                        debug_assert_eq!(row_f, 0);
                    }
                    let a = ((((dist >> 8) * factor) >> 16) + 128).clamp(0, 255) * alpha as i32;
                    let c = PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                        (a / 255) as u8,
                        color.red(),
                        color.green(),
                        color.blue(),
                    ));
                    pix.blend(c);
                }
            }
        };
    }
}

/// draw one line of the rounded rectangle in the line buffer
#[allow(clippy::unnecessary_cast)] // Coord
pub(super) fn draw_rounded_rectangle_line(
    span: &PhysicalRect,
    line: PhysicalLength,
    rr: &super::RoundedRectangle,
    line_buffer: &mut [impl TargetPixel],
    extra_left_clip: i16,
    extra_right_clip: i16,
) {
    let width = line_buffer.len();
    let y1 = (line - span.origin.y_length()) + rr.top_clip;
    let y2 = (span.origin.y_length() + span.size.height_length() - line) + rr.bottom_clip
        - PhysicalLength::new(1);
    let y = y1.min(y2);
    debug_assert!(y.get() >= 0,);
    let border = Shifted::new(rr.width.get());
    const ONE: Shifted = Shifted::ONE;
    const ZERO: Shifted = Shifted(0);
    let anti_alias = |x1: Shifted, x2: Shifted, process_pixel: &mut dyn FnMut(usize, u32)| {
        // x1 and x2 are the coordinate on the top and bottom of the intersection of the pixel
        // line and the curve.
        // `process_pixel` be called for the coordinate in the array and a coverage between 0..255
        // This algorithm just go linearly which is not perfect, but good enough.
        for x in x1.floor()..x2.ceil() {
            // the coverage is basically how much of the pixel should be used
            let cov = ((ONE + Shifted::new(x) - x1).0 << 8) / (ONE + x2 - x1).0;
            process_pixel(x as usize, cov);
        }
    };
    let rev = |x: Shifted| {
        (Shifted::new(width) + Shifted::new(rr.right_clip.get() + extra_right_clip))
            .saturating_sub(x)
    };
    let calculate_xxxx = |r: i16, y: i16| {
        let r = Shifted::new(r);
        // `y` is how far away from the center of the circle the current line is.
        let y = r - Shifted::new(y);
        // Circle equation: x = √(r² - y²)
        // Coordinate from the left edge: x' = r - x
        let x2 = r - (r * r).saturating_sub(y * y).sqrt();
        let x1 = r - (r * r).saturating_sub((y - ONE) * (y - ONE)).sqrt();
        let r2 = r.saturating_sub(border);
        let x4 = r - (r2 * r2).saturating_sub(y * y).sqrt();
        let x3 = r - (r2 * r2).saturating_sub((y - ONE) * (y - ONE)).sqrt();
        (x1, x2, x3, x4)
    };

    let (x1, x2, x3, x4, x5, x6, x7, x8) = if let Some(r) = rr.radius.as_uniform() {
        let (x1, x2, x3, x4) =
            if y.get() < r { calculate_xxxx(r, y.get()) } else { (ZERO, ZERO, border, border) };
        (x1, x2, x3, x4, rev(x4), rev(x3), rev(x2), rev(x1))
    } else {
        let (x1, x2, x3, x4) = if y1 < PhysicalLength::new(rr.radius.top_left) {
            calculate_xxxx(rr.radius.top_left, y.get())
        } else if y2 < PhysicalLength::new(rr.radius.bottom_left) {
            calculate_xxxx(rr.radius.bottom_left, y.get())
        } else {
            (ZERO, ZERO, border, border)
        };
        let (x5, x6, x7, x8) = if y1 < PhysicalLength::new(rr.radius.top_right) {
            let x = calculate_xxxx(rr.radius.top_right, y.get());
            (x.3, x.2, x.1, x.0)
        } else if y2 < PhysicalLength::new(rr.radius.bottom_right) {
            let x = calculate_xxxx(rr.radius.bottom_right, y.get());
            (x.3, x.2, x.1, x.0)
        } else {
            (border, border, ZERO, ZERO)
        };
        (x1, x2, x3, x4, rev(x5), rev(x6), rev(x7), rev(x8))
    };
    anti_alias(
        x1.saturating_sub(Shifted::new(rr.left_clip.get() + extra_left_clip)),
        x2.saturating_sub(Shifted::new(rr.left_clip.get() + extra_left_clip)),
        &mut |x, cov| {
            if x >= width {
                return;
            }
            let c = if border == ZERO { rr.inner_color } else { rr.border_color };
            let col = PremultipliedRgbaColor {
                alpha: (((c.alpha as u32) * cov as u32) / 255) as u8,
                red: (((c.red as u32) * cov as u32) / 255) as u8,
                green: (((c.green as u32) * cov as u32) / 255) as u8,
                blue: (((c.blue as u32) * cov as u32) / 255) as u8,
            };
            line_buffer[x].blend(col);
        },
    );
    if y < rr.width {
        // up or down border (x2 .. x7)
        let l = x2
            .ceil()
            .saturating_sub((rr.left_clip.get() + extra_left_clip) as u32)
            .min(width as u32) as usize;
        let r = x7.floor().min(width as u32) as usize;
        if l < r {
            TargetPixel::blend_slice(&mut line_buffer[l..r], rr.border_color)
        }
    } else {
        if border > ZERO {
            // 3. draw the border (between x2 and x3)
            if ONE + x2 <= x3 {
                TargetPixel::blend_slice(
                    &mut line_buffer[x2
                        .ceil()
                        .saturating_sub((rr.left_clip.get() + extra_left_clip) as u32)
                        .min(width as u32) as usize
                        ..x3.floor()
                            .saturating_sub((rr.left_clip.get() + extra_left_clip) as u32)
                            .min(width as u32) as usize],
                    rr.border_color,
                )
            }
            // 4. anti-aliasing for the contents (x3 .. x4)
            anti_alias(
                x3.saturating_sub(Shifted::new(rr.left_clip.get() + extra_left_clip)),
                x4.saturating_sub(Shifted::new(rr.left_clip.get() + extra_left_clip)),
                &mut |x, cov| {
                    if x >= width {
                        return;
                    }
                    let col = interpolate_color(cov, rr.border_color, rr.inner_color);
                    line_buffer[x].blend(col);
                },
            );
        }
        if rr.inner_color.alpha > 0 {
            // 5. inside (x4 .. x5)
            let begin = x4
                .ceil()
                .saturating_sub((rr.left_clip.get() + extra_left_clip) as u32)
                .min(width as u32);
            let end = x5.floor().min(width as u32);
            if begin < end {
                TargetPixel::blend_slice(
                    &mut line_buffer[begin as usize..end as usize],
                    rr.inner_color,
                )
            }
        }
        if border > ZERO {
            // 6. border anti-aliasing: x5..x6
            anti_alias(x5, x6, &mut |x, cov| {
                if x >= width {
                    return;
                }
                let col = interpolate_color(cov, rr.inner_color, rr.border_color);
                line_buffer[x].blend(col)
            });
            // 7. border x6 .. x7
            if ONE + x6 <= x7 {
                TargetPixel::blend_slice(
                    &mut line_buffer[x6.ceil().min(width as u32) as usize
                        ..x7.floor().min(width as u32) as usize],
                    rr.border_color,
                )
            }
        }
    }
    anti_alias(x7, x8, &mut |x, cov| {
        if x >= width {
            return;
        }
        let c = if border == ZERO { rr.inner_color } else { rr.border_color };
        let col = PremultipliedRgbaColor {
            alpha: (((c.alpha as u32) * (255 - cov) as u32) / 255) as u8,
            red: (((c.red as u32) * (255 - cov) as u32) / 255) as u8,
            green: (((c.green as u32) * (255 - cov) as u32) / 255) as u8,
            blue: (((c.blue as u32) * (255 - cov) as u32) / 255) as u8,
        };
        line_buffer[x].blend(col);
    });
}

// Border radius clipping utility functions
// Extracted from draw_rounded_rectangle_line for reuse in gradient functions

/// This is an integer shifted by 4 bits for efficient fixed-point math.
/// Used for border radius boundary calculations with anti-aliasing.
#[derive(Clone, Copy, PartialEq, Ord, PartialOrd, Eq, Add, Sub)]
struct Shifted(u32);

impl Shifted {
    const ONE: Self = Shifted(1 << 4);

    #[track_caller]
    #[inline]
    pub fn new(value: impl TryInto<u32> + core::fmt::Debug + Copy) -> Self {
        Self(value.try_into().unwrap_or_else(|_| panic!("Overflow {value:?}")) << 4)
    }

    #[inline(always)]
    pub fn floor(self) -> u32 {
        self.0 >> 4
    }

    #[inline(always)]
    pub fn ceil(self) -> u32 {
        (self.0 + Self::ONE.0 - 1) >> 4
    }

    #[inline(always)]
    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    #[inline(always)]
    pub fn sqrt(self) -> Self {
        Self(self.0.integer_sqrt())
    }
}

impl core::ops::Mul for Shifted {
    type Output = Shifted;
    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

/// Check if a point is inside a rounded rectangle boundary
/// Returns true if the point is inside the rounded shape
pub fn is_point_inside_rounded_rect(
    x: i16,
    y: i16,
    rect: &super::PhysicalRect,
    radius: &super::PhysicalBorderRadius,
    border_width: super::PhysicalLength,
) -> bool {
    // Quick check for points clearly inside the rectangle
    let rect_x = x - rect.origin.x_length().get() as i16;
    let rect_y = y - rect.origin.y_length().get() as i16;

    if rect_x < 0 || rect_y < 0 {
        return false;
    }
    if rect_x >= rect.size.width_length().get() as i16
        || rect_y >= rect.size.height_length().get() as i16
    {
        return false;
    }

    let width = rect.size.width_length().get() as i16;
    let height = rect.size.height_length().get() as i16;

    // Handle uniform radius case (optimization)
    if let Some(r) = radius.as_uniform() {
        // Check if we're in a corner region
        let in_top_left_corner = rect_x < r && rect_y < r;
        let in_top_right_corner = rect_x >= width - r && rect_y < r;
        let in_bottom_left_corner = rect_x < r && rect_y >= height - r;
        let in_bottom_right_corner = rect_x >= width - r && rect_y >= height - r;

        if in_top_left_corner {
            // Distance from top-left corner center
            let dx = r - rect_x;
            let dy = r - rect_y;
            let dist_sq = dx * dx + dy * dy;
            let radius_sq = (r - border_width.get() as i16).max(0);
            return dist_sq <= radius_sq * radius_sq;
        }

        if in_top_right_corner {
            // Distance from top-right corner center
            let dx = rect_x - (width - r);
            let dy = r - rect_y;
            let dist_sq = dx * dx + dy * dy;
            let radius_sq = (r - border_width.get() as i16).max(0);
            return dist_sq <= radius_sq * radius_sq;
        }

        if in_bottom_left_corner {
            // Distance from bottom-left corner center
            let dx = r - rect_x;
            let dy = rect_y - (height - r);
            let dist_sq = dx * dx + dy * dy;
            let radius_sq = (r - border_width.get() as i16).max(0);
            return dist_sq <= radius_sq * radius_sq;
        }

        if in_bottom_right_corner {
            // Distance from bottom-right corner center
            let dx = rect_x - (width - r);
            let dy = rect_y - (height - r);
            let dist_sq = dx * dx + dy * dy;
            let radius_sq = (r - border_width.get() as i16).max(0);
            return dist_sq <= radius_sq * radius_sq;
        }

        // Not in any corner region, so it's inside
        return true;
    }

    // Handle different radii for each corner
    let border = border_width.get() as i16;

    // Top-left corner
    if rect_x < radius.top_left && rect_y < radius.top_left {
        let dx = radius.top_left - rect_x;
        let dy = radius.top_left - rect_y;
        let dist_sq = dx * dx + dy * dy;
        let inner_radius = (radius.top_left - border).max(0);
        return dist_sq <= inner_radius * inner_radius;
    }

    // Top-right corner
    if rect_x >= width - radius.top_right && rect_y < radius.top_right {
        let dx = rect_x - (width - radius.top_right);
        let dy = radius.top_right - rect_y;
        let dist_sq = dx * dx + dy * dy;
        let inner_radius = (radius.top_right - border).max(0);
        return dist_sq <= inner_radius * inner_radius;
    }

    // Bottom-left corner
    if rect_x < radius.bottom_left && rect_y >= height - radius.bottom_left {
        let dx = radius.bottom_left - rect_x;
        let dy = rect_y - (height - radius.bottom_left);
        let dist_sq = dx * dx + dy * dy;
        let inner_radius = (radius.bottom_left - border).max(0);
        return dist_sq <= inner_radius * inner_radius;
    }

    // Bottom-right corner
    if rect_x >= width - radius.bottom_right && rect_y >= height - radius.bottom_right {
        let dx = rect_x - (width - radius.bottom_right);
        let dy = rect_y - (height - radius.bottom_right);
        let dist_sq = dx * dx + dy * dy;
        let inner_radius = (radius.bottom_right - border).max(0);
        return dist_sq <= inner_radius * inner_radius;
    }

    // Not in any corner region, so it's inside
    true
}

/// Get alpha value (0-255) for anti-aliasing at rounded rectangle boundary
/// Returns 255 if fully inside, 0 if fully outside, and intermediate values for smooth edges
pub fn get_rounded_rect_alpha(
    x: i16,
    y: i16,
    rect: &super::PhysicalRect,
    radius: &super::PhysicalBorderRadius,
    border_width: super::PhysicalLength,
) -> u8 {
    // Quick bounds check
    let rect_x = x - rect.origin.x_length().get() as i16;
    let rect_y = y - rect.origin.y_length().get() as i16;

    if rect_x < -1 || rect_y < -1 {
        return 0;
    }
    if rect_x > rect.size.width_length().get() as i16
        || rect_y > rect.size.height_length().get() as i16
    {
        return 0;
    }

    let width = rect.size.width_length().get() as i16;
    let height = rect.size.height_length().get() as i16;

    // Handle uniform radius case (optimization)
    if let Some(r) = radius.as_uniform() {
        let inner_radius = (r - border_width.get() as i16).max(0);

        // Check corner regions for anti-aliasing
        let check_corner = |center_x: i16, center_y: i16| -> u8 {
            let dx = rect_x - center_x;
            let dy = rect_y - center_y;
            let dist = ((dx * dx + dy * dy) as f32).sqrt();

            if dist <= inner_radius as f32 - 0.5 {
                255 // Fully inside
            } else if dist >= inner_radius as f32 + 0.5 {
                0 // Fully outside
            } else {
                // Anti-aliasing zone
                let alpha = (inner_radius as f32 + 0.5 - dist) * 255.0;
                alpha.max(0.0).min(255.0) as u8
            }
        };

        // Top-left corner
        if rect_x < r && rect_y < r {
            return check_corner(r, r);
        }

        // Top-right corner
        if rect_x >= width - r && rect_y < r {
            return check_corner(width - r, r);
        }

        // Bottom-left corner
        if rect_x < r && rect_y >= height - r {
            return check_corner(r, height - r);
        }

        // Bottom-right corner
        if rect_x >= width - r && rect_y >= height - r {
            return check_corner(width - r, height - r);
        }
    }

    // For non-corner regions or different corner radii, return full alpha
    // (More complex anti-aliasing for different corner radii can be added later)
    if is_point_inside_rounded_rect(x, y, rect, radius, border_width) {
        255
    } else {
        0
    }
}

// a is between 0 and 255. When 0, we get color1, when 255 we get color2
fn interpolate_color(
    a: u32,
    color1: PremultipliedRgbaColor,
    color2: PremultipliedRgbaColor,
) -> PremultipliedRgbaColor {
    let b = 255 - a;

    let al1 = color1.alpha as u32;
    let al2 = color2.alpha as u32;

    let a_ = a * al2;
    let b_ = b * al1;
    let m = a_ + b_;

    if m == 0 {
        return PremultipliedRgbaColor::default();
    }

    PremultipliedRgbaColor {
        alpha: (m / 255) as u8,
        red: ((b * color1.red as u32 + a * color2.red as u32) / 255) as u8,
        green: ((b * color1.green as u32 + a * color2.green as u32) / 255) as u8,
        blue: ((b * color1.blue as u32 + a * color2.blue as u32) / 255) as u8,
    }
}

pub(super) fn draw_linear_gradient(
    rect: &PhysicalRect,
    line: PhysicalLength,
    g: &super::LinearGradientCommand,
    mut buffer: &mut [impl TargetPixel],
    extra_left_clip: i16,
) {
    let fill_col1 = g.flags & 0b010 != 0;
    let fill_col2 = g.flags & 0b100 != 0;
    let invert_slope = g.flags & 0b1 != 0;

    let y = (line.get() - rect.min_y() + g.top_clip.get()) as i32;
    let size_y = (rect.height() + g.top_clip.get() + g.bottom_clip.get()) as i32;
    let start = g.start as i32;

    let (mut color1, mut color2) = (g.color1, g.color2);

    if g.start == 0 {
        let p = if invert_slope {
            (255 - start) * y / size_y
        } else {
            start + (255 - start) * y / size_y
        };
        if (fill_col1 || p >= 0) && (fill_col2 || p < 255) {
            let col = interpolate_color(p.clamp(0, 255) as u32, color1, color2);
            TargetPixel::blend_slice(buffer, col);
        }
        return;
    }

    let size_x = (rect.width() + g.left_clip.get() + g.right_clip.get()) as i32;

    let mut x = if invert_slope {
        (y * size_x * (255 - start)) / (size_y * start)
    } else {
        (size_y - y) * size_x * (255 - start) / (size_y * start)
    } + g.left_clip.get() as i32
        + extra_left_clip as i32;

    let len = ((255 * size_x) / start) as usize;

    if x < 0 {
        let l = (-x as usize).min(buffer.len());
        if invert_slope {
            if fill_col1 {
                TargetPixel::blend_slice(&mut buffer[..l], g.color1);
            }
        } else if fill_col2 {
            TargetPixel::blend_slice(&mut buffer[..l], g.color2);
        }
        buffer = &mut buffer[l..];
        x = 0;
    }

    if buffer.len() + x as usize > len {
        let l = len.saturating_sub(x as usize);
        if invert_slope {
            if fill_col2 {
                TargetPixel::blend_slice(&mut buffer[l..], g.color2);
            }
        } else if fill_col1 {
            TargetPixel::blend_slice(&mut buffer[l..], g.color1);
        }
        buffer = &mut buffer[..l];
    }

    if buffer.is_empty() {
        return;
    }

    if !invert_slope {
        core::mem::swap(&mut color1, &mut color2);
    }

    let dr = (((color2.red as i32 - color1.red as i32) * start) << 15) / (255 * size_x);
    let dg = (((color2.green as i32 - color1.green as i32) * start) << 15) / (255 * size_x);
    let db = (((color2.blue as i32 - color1.blue as i32) * start) << 15) / (255 * size_x);
    let da = (((color2.alpha as i32 - color1.alpha as i32) * start) << 15) / (255 * size_x);

    let mut r = ((color1.red as u32) << 15).wrapping_add((x * dr) as _);
    let mut g = ((color1.green as u32) << 15).wrapping_add((x * dg) as _);
    let mut b = ((color1.blue as u32) << 15).wrapping_add((x * db) as _);
    let mut a = ((color1.alpha as u32) << 15).wrapping_add((x * da) as _);

    if color1.alpha == 255 && color2.alpha == 255 {
        buffer.fill_with(|| {
            let pix = TargetPixel::from_rgb((r >> 15) as u8, (g >> 15) as u8, (b >> 15) as u8);
            r = r.wrapping_add(dr as _);
            g = g.wrapping_add(dg as _);
            b = b.wrapping_add(db as _);
            pix
        })
    } else {
        for pix in buffer {
            pix.blend(PremultipliedRgbaColor {
                red: (r >> 15) as u8,
                green: (g >> 15) as u8,
                blue: (b >> 15) as u8,
                alpha: (a >> 15) as u8,
            });
            r = r.wrapping_add(dr as _);
            g = g.wrapping_add(dg as _);
            b = b.wrapping_add(db as _);
            a = a.wrapping_add(da as _);
        }
    }
}

/// Enhanced version of draw_linear_gradient with border radius clipping support
pub(super) fn draw_linear_gradient_with_clipping(
    rect: &PhysicalRect,
    line: PhysicalLength,
    g: &super::LinearGradientCommand,
    buffer: &mut [impl TargetPixel],
    extra_left_clip: i16,
    rounded_rect: Option<&super::RoundedRectangle>,
) {
    // If no border radius clipping needed, use the fast path
    if rounded_rect.is_none() || rounded_rect.as_ref().unwrap().radius.is_zero() {
        draw_linear_gradient(rect, line, g, buffer, extra_left_clip);
        return;
    }

    let rr = rounded_rect.unwrap();

    // Same gradient calculation logic as original function
    let fill_col1 = g.flags & 0b010 != 0;
    let fill_col2 = g.flags & 0b100 != 0;
    let invert_slope = g.flags & 0b1 != 0;

    let y = (line.get() - rect.min_y() + g.top_clip.get()) as i32;
    let size_y = (rect.height() + g.top_clip.get() + g.bottom_clip.get()) as i32;
    let start = g.start as i32;
    let (mut color1, mut color2) = (g.color1, g.color2);

    // Handle vertical gradients (start == 0 case)
    if g.start == 0 {
        let p = if invert_slope {
            (255 - start) * y / size_y
        } else {
            start + (255 - start) * y / size_y
        };

        if (fill_col1 || p >= 0) && (fill_col2 || p < 255) {
            let gradient_color = interpolate_color(p.clamp(0, 255) as u32, color1, color2);

            // Apply gradient with border radius clipping
            for (x_offset, pixel) in buffer.iter_mut().enumerate() {
                let pixel_x = rect.min_x() + extra_left_clip + x_offset as i16;
                let pixel_y = line.get();

                if is_point_inside_rounded_rect(pixel_x, pixel_y, rect, &rr.radius, rr.width) {
                    let alpha =
                        get_rounded_rect_alpha(pixel_x, pixel_y, rect, &rr.radius, rr.width);
                    if alpha > 0 {
                        let mut clipped_color = gradient_color;
                        clipped_color.alpha =
                            ((clipped_color.alpha as u16 * alpha as u16) / 255) as u8;
                        pixel.blend(clipped_color);
                    }
                }
            }
        }
        return;
    }

    // Handle diagonal gradients with border radius clipping
    let size_x = (rect.width() + g.left_clip.get() + g.right_clip.get()) as i32;

    if !invert_slope {
        core::mem::swap(&mut color1, &mut color2);
    }

    let dr = (((color2.red as i32 - color1.red as i32) * start) << 15) / (255 * size_x);
    let dg = (((color2.green as i32 - color1.green as i32) * start) << 15) / (255 * size_x);
    let db = (((color2.blue as i32 - color1.blue as i32) * start) << 15) / (255 * size_x);
    let da = (((color2.alpha as i32 - color1.alpha as i32) * start) << 15) / (255 * size_x);

    // Calculate initial gradient position for this line
    let gradient_x = if invert_slope {
        (y * size_x * (255 - start)) / (size_y * start)
    } else {
        (size_y - y) * size_x * (255 - start) / (size_y * start)
    } + g.left_clip.get() as i32
        + extra_left_clip as i32;

    // Process each pixel with border radius clipping
    for (x_offset, pixel) in buffer.iter_mut().enumerate() {
        let pixel_x = rect.min_x() + extra_left_clip + x_offset as i16;
        let pixel_y = line.get();

        // Check if pixel is inside the rounded rectangle
        if is_point_inside_rounded_rect(pixel_x, pixel_y, rect, &rr.radius, rr.width) {
            let alpha = get_rounded_rect_alpha(pixel_x, pixel_y, rect, &rr.radius, rr.width);
            if alpha > 0 {
                // Calculate gradient color for this pixel
                let current_x = gradient_x + x_offset as i32;

                // Determine gradient factor based on position
                let gradient_factor = if current_x < 0 {
                    // Before gradient start
                    if invert_slope && fill_col1 {
                        255
                    } else if !invert_slope && fill_col2 {
                        255
                    } else {
                        0
                    }
                } else {
                    let len = ((255 * size_x) / start) as i32;
                    if current_x >= len {
                        // After gradient end
                        if invert_slope && fill_col2 {
                            0
                        } else if !invert_slope && fill_col1 {
                            0
                        } else {
                            255
                        }
                    } else {
                        // Within gradient range
                        ((current_x * 255) / len).clamp(0, 255) as u32
                    }
                };

                if gradient_factor > 0 {
                    // Calculate color components with fixed-point math
                    let r = ((color1.red as u32) << 15).wrapping_add((current_x * dr) as u32);
                    let g = ((color1.green as u32) << 15).wrapping_add((current_x * dg) as u32);
                    let b = ((color1.blue as u32) << 15).wrapping_add((current_x * db) as u32);
                    let a = ((color1.alpha as u32) << 15).wrapping_add((current_x * da) as u32);

                    let mut gradient_color = PremultipliedRgbaColor {
                        red: (r >> 15) as u8,
                        green: (g >> 15) as u8,
                        blue: (b >> 15) as u8,
                        alpha: (a >> 15) as u8,
                    };

                    // Apply border radius anti-aliasing
                    gradient_color.alpha =
                        ((gradient_color.alpha as u16 * alpha as u16) / 255) as u8;

                    if gradient_color.alpha > 0 {
                        pixel.blend(gradient_color);
                    }
                }
            }
        }
    }
}

/// Draw a radial gradient on a line
pub(super) fn draw_radial_gradient(
    rect: &PhysicalRect,
    line: PhysicalLength,
    g: &super::RadialGradientCommand,
    buffer: &mut [impl TargetPixel],
    extra_left_clip: i16,
    _extra_right_clip: i16,
) {
    if g.stops.is_empty() {
        return;
    }

    let center_x = (rect.min_x() + g.center_x.get()) as i32;
    let center_y = (rect.min_y() + g.center_y.get()) as i32;

    // Calculate the maximum radius (distance from center to corner)
    let max_radius = {
        let dx1 = ((rect.min_x() as i32) - center_x).abs();
        let dx2 = ((rect.max_x() as i32) - center_x).abs();
        let dy1 = ((rect.min_y() as i32) - center_y).abs();
        let dy2 = ((rect.max_y() as i32) - center_y).abs();
        let max_dx = dx1.max(dx2) as f32;
        let max_dy = dy1.max(dy2) as f32;
        (max_dx * max_dx + max_dy * max_dy).sqrt()
    };

    let start_x = rect.min_x() + extra_left_clip;
    // Use the absolute line position for distance calculation
    let dy = (line.get() as i32 - center_y) as f32;
    let dy_squared = dy * dy;

    for (i, pixel) in buffer.iter_mut().enumerate() {
        let x = start_x + i as i16;
        let dx = (x as i32 - center_x) as f32;
        let distance = (dx * dx + dy_squared).sqrt();
        let position = (distance / max_radius).clamp(0.0, 1.0);

        // Find the two gradient stops to interpolate between
        let mut color = g.stops.first().map(|s| s.color).unwrap_or_default();

        for window in g.stops.windows(2) {
            let stop1 = &window[0];
            let stop2 = &window[1];

            if position >= stop1.position && position <= stop2.position {
                // Interpolate between the two stops
                let t = if stop2.position == stop1.position {
                    0.0
                } else {
                    (position - stop1.position) / (stop2.position - stop1.position)
                };

                let c1 = stop1.color.to_argb_u8();
                let c2 = stop2.color.to_argb_u8();

                let alpha = ((1.0 - t) * c1.alpha as f32 + t * c2.alpha as f32) as u8;
                let red = ((1.0 - t) * c1.red as f32 + t * c2.red as f32) as u8;
                let green = ((1.0 - t) * c1.green as f32 + t * c2.green as f32) as u8;
                let blue = ((1.0 - t) * c1.blue as f32 + t * c2.blue as f32) as u8;

                color = Color::from_argb_u8(alpha, red, green, blue);
                break;
            } else if position > stop2.position {
                color = stop2.color;
            }
        }

        pixel.blend(super::PremultipliedRgbaColor::from(color));
    }
}

/// Enhanced version of draw_radial_gradient with border radius clipping support
pub(super) fn draw_radial_gradient_with_clipping(
    rect: &PhysicalRect,
    line: PhysicalLength,
    g: &super::RadialGradientCommand,
    buffer: &mut [impl TargetPixel],
    extra_left_clip: i16,
    _extra_right_clip: i16,
    rounded_rect: Option<&super::RoundedRectangle>,
) {
    if g.stops.is_empty() {
        return;
    }

    // If no border radius clipping needed, use the fast path
    if rounded_rect.is_none() || rounded_rect.as_ref().unwrap().radius.is_zero() {
        draw_radial_gradient(rect, line, g, buffer, extra_left_clip, _extra_right_clip);
        return;
    }

    let rr = rounded_rect.unwrap();

    let center_x = (rect.min_x() + g.center_x.get()) as i32;
    let center_y = (rect.min_y() + g.center_y.get()) as i32;

    // Calculate the maximum radius (distance from center to corner)
    let max_radius = {
        let dx1 = ((rect.min_x() as i32) - center_x).abs();
        let dx2 = ((rect.max_x() as i32) - center_x).abs();
        let dy1 = ((rect.min_y() as i32) - center_y).abs();
        let dy2 = ((rect.max_y() as i32) - center_y).abs();
        let max_dx = dx1.max(dx2) as f32;
        let max_dy = dy1.max(dy2) as f32;
        (max_dx * max_dx + max_dy * max_dy).sqrt()
    };

    let start_x = rect.min_x() + extra_left_clip;
    let dy = (line.get() as i32 - center_y) as f32;
    let dy_squared = dy * dy;

    for (i, pixel) in buffer.iter_mut().enumerate() {
        let x = start_x + i as i16;
        let pixel_x = x;
        let pixel_y = line.get();

        // Check if pixel is inside the rounded rectangle
        if is_point_inside_rounded_rect(pixel_x, pixel_y, rect, &rr.radius, rr.width) {
            let alpha = get_rounded_rect_alpha(pixel_x, pixel_y, rect, &rr.radius, rr.width);
            if alpha > 0 {
                // Calculate radial gradient color
                let dx = (x as i32 - center_x) as f32;
                let distance = (dx * dx + dy_squared).sqrt();
                let position = (distance / max_radius).clamp(0.0, 1.0);

                // Find the two gradient stops to interpolate between
                let mut color = g.stops.first().map(|s| s.color).unwrap_or_default();

                for window in g.stops.windows(2) {
                    let stop1 = &window[0];
                    let stop2 = &window[1];

                    if position >= stop1.position && position <= stop2.position {
                        // Interpolate between the two stops
                        let t = if stop2.position == stop1.position {
                            0.0
                        } else {
                            (position - stop1.position) / (stop2.position - stop1.position)
                        };

                        let c1 = stop1.color.to_argb_u8();
                        let c2 = stop2.color.to_argb_u8();

                        let stop_alpha = ((1.0 - t) * c1.alpha as f32 + t * c2.alpha as f32) as u8;
                        let red = ((1.0 - t) * c1.red as f32 + t * c2.red as f32) as u8;
                        let green = ((1.0 - t) * c1.green as f32 + t * c2.green as f32) as u8;
                        let blue = ((1.0 - t) * c1.blue as f32 + t * c2.blue as f32) as u8;

                        color = Color::from_argb_u8(stop_alpha, red, green, blue);
                        break;
                    } else if position > stop2.position {
                        color = stop2.color;
                    }
                }

                // Apply border radius anti-aliasing
                let mut gradient_color = PremultipliedRgbaColor::from(color);
                gradient_color.alpha = ((gradient_color.alpha as u16 * alpha as u16) / 255) as u8;

                if gradient_color.alpha > 0 {
                    pixel.blend(gradient_color);
                }
            }
        }
    }
}

/// Draw a conic gradient on a line
pub(super) fn draw_conic_gradient(
    rect: &PhysicalRect,
    line: PhysicalLength,
    g: &super::ConicGradientCommand,
    buffer: &mut [impl TargetPixel],
    extra_left_clip: i16,
    _extra_right_clip: i16,
) {
    if g.stops.is_empty() {
        return;
    }

    // Center is always the center of the rectangle
    let center_x = (rect.min_x() + rect.width() / 2) as f32;
    let center_y = (rect.min_y() + rect.height() / 2) as f32;

    let start_x = rect.min_x() + extra_left_clip;
    let y = line.get() as f32;

    for (i, pixel) in buffer.iter_mut().enumerate() {
        let x = (start_x + i as i16) as f32;

        // Calculate angle from center to current pixel
        let dx = x - center_x;
        let dy = y - center_y;

        // atan2 returns angle in radians from -π to π
        // For 0deg at north (12 o'clock), we need to rotate by -90 degrees
        let mut angle = dy.atan2(dx) + core::f32::consts::FRAC_PI_2;

        // Normalize angle to [0, 2π]
        while angle < 0.0 {
            angle += 2.0 * core::f32::consts::PI;
        }
        while angle >= 2.0 * core::f32::consts::PI {
            angle -= 2.0 * core::f32::consts::PI;
        }

        // Convert to position in [0, 1]
        let position = angle / (2.0 * core::f32::consts::PI);

        // Find the two gradient stops to interpolate between
        let mut color = g.stops.first().map(|s| s.color).unwrap_or_default();

        for window in g.stops.windows(2) {
            let stop1 = &window[0];
            let stop2 = &window[1];

            if position >= stop1.position && position <= stop2.position {
                // Interpolate between the two stops
                let t = if stop2.position == stop1.position {
                    0.0
                } else {
                    (position - stop1.position) / (stop2.position - stop1.position)
                };

                let c1 = stop1.color.to_argb_u8();
                let c2 = stop2.color.to_argb_u8();

                let alpha = ((1.0 - t) * c1.alpha as f32 + t * c2.alpha as f32) as u8;
                let red = ((1.0 - t) * c1.red as f32 + t * c2.red as f32) as u8;
                let green = ((1.0 - t) * c1.green as f32 + t * c2.green as f32) as u8;
                let blue = ((1.0 - t) * c1.blue as f32 + t * c2.blue as f32) as u8;

                color = Color::from_argb_u8(alpha, red, green, blue);
                break;
            } else if position > stop2.position {
                color = stop2.color;
            }
        }

        pixel.blend(super::PremultipliedRgbaColor::from(color));
    }
}

/// Enhanced version of draw_conic_gradient with border radius clipping support
pub(super) fn draw_conic_gradient_with_clipping(
    rect: &PhysicalRect,
    line: PhysicalLength,
    g: &super::ConicGradientCommand,
    buffer: &mut [impl TargetPixel],
    extra_left_clip: i16,
    _extra_right_clip: i16,
    rounded_rect: Option<&super::RoundedRectangle>,
) {
    if g.stops.is_empty() {
        return;
    }

    // If no border radius clipping needed, use the fast path
    if rounded_rect.is_none() || rounded_rect.as_ref().unwrap().radius.is_zero() {
        draw_conic_gradient(rect, line, g, buffer, extra_left_clip, _extra_right_clip);
        return;
    }

    let rr = rounded_rect.unwrap();

    // Center is always the center of the rectangle
    let center_x = (rect.min_x() + rect.width() / 2) as f32;
    let center_y = (rect.min_y() + rect.height() / 2) as f32;

    let start_x = rect.min_x() + extra_left_clip;
    let y = line.get() as f32;

    for (i, pixel) in buffer.iter_mut().enumerate() {
        let x = start_x + i as i16;
        let pixel_x = x;
        let pixel_y = line.get();

        // Check if pixel is inside the rounded rectangle
        if is_point_inside_rounded_rect(pixel_x, pixel_y, rect, &rr.radius, rr.width) {
            let alpha = get_rounded_rect_alpha(pixel_x, pixel_y, rect, &rr.radius, rr.width);
            if alpha > 0 {
                // Calculate conic gradient color
                let x_f = x as f32;
                let dx = x_f - center_x;
                let dy = y - center_y;

                // atan2 returns angle in radians from -π to π
                // For 0deg at north (12 o'clock), we need to rotate by -90 degrees
                let mut angle = dy.atan2(dx) + core::f32::consts::FRAC_PI_2;

                // Normalize angle to [0, 2π]
                while angle < 0.0 {
                    angle += 2.0 * core::f32::consts::PI;
                }
                while angle >= 2.0 * core::f32::consts::PI {
                    angle -= 2.0 * core::f32::consts::PI;
                }

                // Convert to position in [0, 1]
                let position = angle / (2.0 * core::f32::consts::PI);

                // Find the two gradient stops to interpolate between
                let mut color = g.stops.first().map(|s| s.color).unwrap_or_default();

                for window in g.stops.windows(2) {
                    let stop1 = &window[0];
                    let stop2 = &window[1];

                    if position >= stop1.position && position <= stop2.position {
                        // Interpolate between the two stops
                        let t = if stop2.position == stop1.position {
                            0.0
                        } else {
                            (position - stop1.position) / (stop2.position - stop1.position)
                        };

                        let c1 = stop1.color.to_argb_u8();
                        let c2 = stop2.color.to_argb_u8();

                        let stop_alpha = ((1.0 - t) * c1.alpha as f32 + t * c2.alpha as f32) as u8;
                        let red = ((1.0 - t) * c1.red as f32 + t * c2.red as f32) as u8;
                        let green = ((1.0 - t) * c1.green as f32 + t * c2.green as f32) as u8;
                        let blue = ((1.0 - t) * c1.blue as f32 + t * c2.blue as f32) as u8;

                        color = Color::from_argb_u8(stop_alpha, red, green, blue);
                        break;
                    } else if position > stop2.position {
                        color = stop2.color;
                    }
                }

                // Apply border radius anti-aliasing
                let mut gradient_color = PremultipliedRgbaColor::from(color);
                gradient_color.alpha = ((gradient_color.alpha as u16 * alpha as u16) / 255) as u8;

                if gradient_color.alpha > 0 {
                    pixel.blend(gradient_color);
                }
            }
        }
    }
}

/// A color whose component have been pre-multiplied by alpha
///
/// The renderer operates faster on pre-multiplied color since it
/// caches the multiplication of its component
///
/// PremultipliedRgbaColor can be constructed from a [`Color`] with
/// the [`From`] trait. This conversion will pre-multiply the color
/// components
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PremultipliedRgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

/// Convert a non-premultiplied color to a premultiplied one
impl From<Color> for PremultipliedRgbaColor {
    fn from(col: Color) -> Self {
        Self::premultiply(col)
    }
}

impl PremultipliedRgbaColor {
    /// Convert a non premultiplied color to a premultiplied one
    fn premultiply(col: Color) -> Self {
        let a = col.alpha() as u16;
        Self {
            alpha: col.alpha(),
            red: (col.red() as u16 * a / 255) as u8,
            green: (col.green() as u16 * a / 255) as u8,
            blue: (col.blue() as u16 * a / 255) as u8,
        }
    }
}

/// Trait for the pixels in the buffer
pub trait TargetPixel: Sized + Copy {
    /// Blend a single pixel with a color
    fn blend(&mut self, color: PremultipliedRgbaColor);
    /// Blend a color to all the pixel in the slice.
    fn blend_slice(slice: &mut [Self], color: PremultipliedRgbaColor) {
        if color.alpha == u8::MAX {
            slice.fill(Self::from_rgb(color.red, color.green, color.blue))
        } else {
            for x in slice {
                Self::blend(x, color);
            }
        }
    }
    /// Create a pixel from the red, gree, blue component in the range 0..=255
    fn from_rgb(red: u8, green: u8, blue: u8) -> Self;

    /// Pixel which will be filled as the background in case the slint view has transparency
    fn background() -> Self {
        Self::from_rgb(0, 0, 0)
    }
}

impl TargetPixel for crate::graphics::image::Rgb8Pixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let a = (u8::MAX - color.alpha) as u16;
        self.r = (self.r as u16 * a / 255) as u8 + color.red;
        self.g = (self.g as u16 * a / 255) as u8 + color.green;
        self.b = (self.b as u16 * a / 255) as u8 + color.blue;
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b)
    }
}

impl TargetPixel for PremultipliedRgbaColor {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let a = (u8::MAX - color.alpha) as u16;
        self.red = ((self.red as u16 * a / 255) + color.red as u16).min(255) as u8;
        self.green = ((self.green as u16 * a / 255) + color.green as u16).min(255) as u8;
        self.blue = ((self.blue as u16 * a / 255) + color.blue as u16).min(255) as u8;
        self.alpha = (self.alpha as u16 + color.alpha as u16
            - (self.alpha as u16 * color.alpha as u16) / 255) as u8;
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { red: r, green: g, blue: b, alpha: 255 }
    }

    fn background() -> Self {
        Self { red: 0, green: 0, blue: 0, alpha: 0 }
    }
}

/// A 16bit pixel that has 5 red bits, 6 green bits and  5 blue bits
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Rgb565Pixel(pub u16);

impl Rgb565Pixel {
    const R_MASK: u16 = 0b1111_1000_0000_0000;
    const G_MASK: u16 = 0b0000_0111_1110_0000;
    const B_MASK: u16 = 0b0000_0000_0001_1111;

    /// Return the red component as a u8.
    ///
    /// The bits are shifted so that the result is between 0 and 255
    fn red(self) -> u8 {
        ((self.0 & Self::R_MASK) >> 8) as u8
    }
    /// Return the green component as a u8.
    ///
    /// The bits are shifted so that the result is between 0 and 255
    fn green(self) -> u8 {
        ((self.0 & Self::G_MASK) >> 3) as u8
    }
    /// Return the blue component as a u8.
    ///
    /// The bits are shifted so that the result is between 0 and 255
    fn blue(self) -> u8 {
        ((self.0 & Self::B_MASK) << 3) as u8
    }
}

impl TargetPixel for Rgb565Pixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let a = (u8::MAX - color.alpha) as u32;
        // convert to 5 bits
        let a = (a + 4) >> 3;

        // 00000ggg_ggg00000_rrrrr000_000bbbbb
        let expanded = (self.0 & (Self::R_MASK | Self::B_MASK)) as u32
            | (((self.0 & Self::G_MASK) as u32) << 16);

        // gggggggg_000rrrrr_rrr000bb_bbbbbb00
        let c =
            ((color.red as u32) << 13) | ((color.green as u32) << 24) | ((color.blue as u32) << 2);
        // gggggg00_000rrrrr_000000bb_bbb00000
        let c = c & 0b11111100_00011111_00000011_11100000;

        let res = expanded * a + c;

        self.0 = ((res >> 21) as u16 & Self::G_MASK)
            | ((res >> 5) as u16 & (Self::R_MASK | Self::B_MASK));
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self(((r as u16 & 0b11111000) << 8) | ((g as u16 & 0b11111100) << 3) | (b as u16 >> 3))
    }
}

impl From<Rgb8Pixel> for Rgb565Pixel {
    fn from(p: Rgb8Pixel) -> Self {
        Self::from_rgb(p.r, p.g, p.b)
    }
}

impl From<Rgb565Pixel> for Rgb8Pixel {
    fn from(p: Rgb565Pixel) -> Self {
        Rgb8Pixel { r: p.red(), g: p.green(), b: p.blue() }
    }
}

#[test]
fn rgb565() {
    let pix565 = Rgb565Pixel::from_rgb(0xff, 0x25, 0);
    let pix888: Rgb8Pixel = pix565.into();
    assert_eq!(pix565, pix888.into());

    let pix565 = Rgb565Pixel::from_rgb(0x56, 0x42, 0xe3);
    let pix888: Rgb8Pixel = pix565.into();
    assert_eq!(pix565, pix888.into());
}

#[test]
fn test_border_radius_clipping_basic() {
    use crate::software_renderer::PhysicalBorderRadius;
    use crate::software_renderer::{PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalSize};

    // Create a 100x100 rectangle at (0,0) with uniform 20px radius
    let rect = PhysicalRect::new(
        PhysicalPoint::from_lengths(PhysicalLength::new(0), PhysicalLength::new(0)),
        PhysicalSize::from_lengths(PhysicalLength::new(100), PhysicalLength::new(100)),
    );
    let radius = PhysicalBorderRadius {
        top_left: 20,
        top_right: 20,
        bottom_right: 20,
        bottom_left: 20,
        _unit: Default::default(),
    };
    let border_width = PhysicalLength::new(0);

    // Test center point (should be inside)
    assert!(is_point_inside_rounded_rect(50, 50, &rect, &radius, border_width));

    // Test corner points (should be outside)
    assert!(!is_point_inside_rounded_rect(0, 0, &rect, &radius, border_width));
    assert!(!is_point_inside_rounded_rect(99, 0, &rect, &radius, border_width));
    assert!(!is_point_inside_rounded_rect(0, 99, &rect, &radius, border_width));
    assert!(!is_point_inside_rounded_rect(99, 99, &rect, &radius, border_width));

    // Test points on rounded corners boundary (should be approximately inside)
    assert!(is_point_inside_rounded_rect(35, 35, &rect, &radius, border_width)); // Inside corner curve

    // Test rectangular edges (should be inside)
    assert!(is_point_inside_rounded_rect(50, 0, &rect, &radius, border_width)); // Top edge
    assert!(is_point_inside_rounded_rect(0, 50, &rect, &radius, border_width)); // Left edge
    assert!(is_point_inside_rounded_rect(99, 50, &rect, &radius, border_width)); // Right edge
    assert!(is_point_inside_rounded_rect(50, 99, &rect, &radius, border_width));
    // Bottom edge
}

#[test]
fn test_border_radius_clipping_with_border() {
    use crate::software_renderer::PhysicalBorderRadius;
    use crate::software_renderer::{PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalSize};

    // Create a 100x100 rectangle with 20px radius and 5px border
    let rect = PhysicalRect::new(
        PhysicalPoint::from_lengths(PhysicalLength::new(0), PhysicalLength::new(0)),
        PhysicalSize::from_lengths(PhysicalLength::new(100), PhysicalLength::new(100)),
    );
    let radius = PhysicalBorderRadius {
        top_left: 20,
        top_right: 20,
        bottom_right: 20,
        bottom_left: 20,
        _unit: Default::default(),
    };
    let border_width = PhysicalLength::new(5);

    // Center should still be inside
    assert!(is_point_inside_rounded_rect(50, 50, &rect, &radius, border_width));

    // Points near the inner boundary should be inside due to reduced effective radius
    assert!(is_point_inside_rounded_rect(30, 30, &rect, &radius, border_width));
    // Was outside without border
}

#[test]
fn test_border_radius_clipping_different_corners() {
    use crate::software_renderer::PhysicalBorderRadius;
    use crate::software_renderer::{PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalSize};

    // Create rectangle with different corner radii
    let rect = PhysicalRect::new(
        PhysicalPoint::from_lengths(PhysicalLength::new(0), PhysicalLength::new(0)),
        PhysicalSize::from_lengths(PhysicalLength::new(100), PhysicalLength::new(100)),
    );
    let radius = PhysicalBorderRadius {
        top_left: 30,     // Large radius
        top_right: 0,     // No radius
        bottom_right: 15, // Medium radius
        bottom_left: 5,   // Small radius
        _unit: Default::default(),
    };
    let border_width = PhysicalLength::new(0);

    // Top-right corner should be sharp (no radius)
    assert!(is_point_inside_rounded_rect(99, 0, &rect, &radius, border_width));

    // Top-left corner should be rounded
    assert!(!is_point_inside_rounded_rect(0, 0, &rect, &radius, border_width));
    assert!(is_point_inside_rounded_rect(35, 35, &rect, &radius, border_width));

    // Bottom-left corner has small radius
    assert!(!is_point_inside_rounded_rect(0, 99, &rect, &radius, border_width));
    assert!(is_point_inside_rounded_rect(8, 92, &rect, &radius, border_width));
}

#[test]
fn test_border_radius_alpha_values() {
    use crate::software_renderer::PhysicalBorderRadius;
    use crate::software_renderer::{PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalSize};

    let rect = PhysicalRect::new(
        PhysicalPoint::from_lengths(PhysicalLength::new(0), PhysicalLength::new(0)),
        PhysicalSize::from_lengths(PhysicalLength::new(100), PhysicalLength::new(100)),
    );
    let radius = PhysicalBorderRadius {
        top_left: 20,
        top_right: 20,
        bottom_right: 20,
        bottom_left: 20,
        _unit: Default::default(),
    };
    let border_width = PhysicalLength::new(0);

    // Center should have full alpha
    assert_eq!(get_rounded_rect_alpha(50, 50, &rect, &radius, border_width), 255);

    // Far outside should have zero alpha
    assert_eq!(get_rounded_rect_alpha(-10, -10, &rect, &radius, border_width), 0);
    assert_eq!(get_rounded_rect_alpha(110, 110, &rect, &radius, border_width), 0);

    // Points inside but near corner should have full alpha
    assert_eq!(get_rounded_rect_alpha(35, 35, &rect, &radius, border_width), 255);
}

#[test]
fn test_linear_gradient_with_border_radius_clipping() {
    use crate::software_renderer::scene::{LinearGradientCommand, RoundedRectangle};
    use crate::software_renderer::{
        PhysicalBorderRadius, PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalSize,
    };
    use alloc::vec;
    use alloc::vec::Vec;

    // Create a test rectangle and rounded rectangle setup
    let rect = PhysicalRect::new(
        PhysicalPoint::from_lengths(PhysicalLength::new(0), PhysicalLength::new(0)),
        PhysicalSize::from_lengths(PhysicalLength::new(100), PhysicalLength::new(100)),
    );
    let radius = PhysicalBorderRadius {
        top_left: 20,
        top_right: 20,
        bottom_right: 20,
        bottom_left: 20,
        _unit: Default::default(),
    };
    let rr = RoundedRectangle {
        radius,
        width: PhysicalLength::new(0), // No border for gradient test
        border_color: PremultipliedRgbaColor::from(crate::Color::from_rgb_u8(0, 0, 0)),
        inner_color: PremultipliedRgbaColor::from(crate::Color::from_rgb_u8(255, 255, 255)),
        left_clip: PhysicalLength::new(0),
        right_clip: PhysicalLength::new(0),
        top_clip: PhysicalLength::new(0),
        bottom_clip: PhysicalLength::new(0),
    };

    // Create a simple linear gradient (red to blue)
    let gradient = LinearGradientCommand {
        color1: PremultipliedRgbaColor::from(crate::Color::from_rgb_u8(255, 0, 0)), // Red
        color2: PremultipliedRgbaColor::from(crate::Color::from_rgb_u8(0, 0, 255)), // Blue
        start: 128,                                                                 // 50% gradient
        flags: 0,
        left_clip: PhysicalLength::new(0),
        right_clip: PhysicalLength::new(0),
        top_clip: PhysicalLength::new(0),
        bottom_clip: PhysicalLength::new(0),
    };

    // Test rendering to a line buffer at the top edge where corners are
    let mut buffer: Vec<PremultipliedRgbaColor> =
        vec![PremultipliedRgbaColor::from(crate::Color::from_argb_u8(0, 0, 0, 0)); 100];
    let line = PhysicalLength::new(5); // Near the top edge where corners matter

    // Test with border radius clipping
    draw_linear_gradient_with_clipping(&rect, line, &gradient, &mut buffer, 0, Some(&rr));

    // Check that pixels in corners are not rendered (should remain transparent)
    // At y=5, with radius=20, the corners at x=0 and x=99 should be clipped
    assert_eq!(buffer[0].alpha, 0, "Top-left corner pixel should remain transparent");
    assert_eq!(buffer[99].alpha, 0, "Top-right corner pixel should remain transparent");

    // Check that center pixels have gradient applied
    assert!(buffer[50].alpha > 0, "Center pixel should have gradient applied");

    // Test at middle line where no clipping should occur
    let mut buffer2: Vec<PremultipliedRgbaColor> =
        vec![PremultipliedRgbaColor::from(crate::Color::from_argb_u8(0, 0, 0, 0)); 100];
    let line2 = PhysicalLength::new(50); // Middle of the rectangle
    draw_linear_gradient_with_clipping(&rect, line2, &gradient, &mut buffer2, 0, Some(&rr));

    // At y=50, edges should not be clipped
    assert!(buffer2[0].alpha > 0, "Left edge at middle should have gradient");
    assert!(buffer2[99].alpha > 0, "Right edge at middle should have gradient");
}

#[test]
fn test_radial_gradient_with_border_radius_clipping() {
    use crate::graphics::{Color, GradientStop};
    use crate::software_renderer::scene::{RadialGradientCommand, RoundedRectangle};
    use crate::software_renderer::{
        PhysicalBorderRadius, PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalSize,
    };
    use crate::SharedVector;
    use alloc::vec;
    use alloc::vec::Vec;

    let rect = PhysicalRect::new(
        PhysicalPoint::from_lengths(PhysicalLength::new(0), PhysicalLength::new(0)),
        PhysicalSize::from_lengths(PhysicalLength::new(100), PhysicalLength::new(100)),
    );
    let radius = PhysicalBorderRadius {
        top_left: 25,
        top_right: 25,
        bottom_right: 25,
        bottom_left: 25,
        _unit: Default::default(),
    };
    let rr = RoundedRectangle {
        radius,
        width: PhysicalLength::new(0),
        border_color: PremultipliedRgbaColor::from(Color::from_rgb_u8(0, 0, 0)),
        inner_color: PremultipliedRgbaColor::from(Color::from_rgb_u8(255, 255, 255)),
        left_clip: PhysicalLength::new(0),
        right_clip: PhysicalLength::new(0),
        top_clip: PhysicalLength::new(0),
        bottom_clip: PhysicalLength::new(0),
    };

    // Create radial gradient (yellow center to purple edge)
    let gradient = RadialGradientCommand {
        stops: SharedVector::from(&[
            GradientStop { position: 0.0, color: Color::from_rgb_u8(255, 255, 0) }, // Yellow
            GradientStop { position: 1.0, color: Color::from_rgb_u8(128, 0, 128) }, // Purple
        ] as &[_]),
        center_x: PhysicalLength::new(50), // Center of rectangle
        center_y: PhysicalLength::new(50),
    };

    let mut buffer: Vec<PremultipliedRgbaColor> =
        vec![PremultipliedRgbaColor::from(crate::Color::from_argb_u8(0, 0, 0, 0)); 100];
    let line = PhysicalLength::new(5); // Near top edge where corners matter

    draw_radial_gradient_with_clipping(&rect, line, &gradient, &mut buffer, 0, 0, Some(&rr));

    // At y=5 with radius=25, corner pixels should be transparent
    assert_eq!(buffer[0].alpha, 0, "Top-left corner should be clipped");
    assert_eq!(buffer[99].alpha, 0, "Top-right corner should be clipped");

    // Center region should have gradient
    assert!(buffer[50].alpha > 0, "Center should have radial gradient");
}

#[test]
fn test_conic_gradient_with_border_radius_clipping() {
    use crate::graphics::{Color, GradientStop};
    use crate::software_renderer::scene::{ConicGradientCommand, RoundedRectangle};
    use crate::software_renderer::{
        PhysicalBorderRadius, PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalSize,
    };
    use crate::SharedVector;
    use alloc::vec;
    use alloc::vec::Vec;

    let rect = PhysicalRect::new(
        PhysicalPoint::from_lengths(PhysicalLength::new(0), PhysicalLength::new(0)),
        PhysicalSize::from_lengths(PhysicalLength::new(80), PhysicalLength::new(80)),
    );
    let radius = PhysicalBorderRadius {
        top_left: 15,
        top_right: 15,
        bottom_right: 15,
        bottom_left: 15,
        _unit: Default::default(),
    };
    let rr = RoundedRectangle {
        radius,
        width: PhysicalLength::new(0),
        border_color: PremultipliedRgbaColor::from(Color::from_rgb_u8(0, 0, 0)),
        inner_color: PremultipliedRgbaColor::from(Color::from_rgb_u8(255, 255, 255)),
        left_clip: PhysicalLength::new(0),
        right_clip: PhysicalLength::new(0),
        top_clip: PhysicalLength::new(0),
        bottom_clip: PhysicalLength::new(0),
    };

    // Create conic gradient (rainbow)
    let gradient = ConicGradientCommand {
        stops: SharedVector::from(&[
            GradientStop { position: 0.0, color: Color::from_rgb_u8(255, 0, 0) }, // Red
            GradientStop { position: 0.33, color: Color::from_rgb_u8(0, 255, 0) }, // Green
            GradientStop { position: 0.66, color: Color::from_rgb_u8(0, 0, 255) }, // Blue
            GradientStop { position: 1.0, color: Color::from_rgb_u8(255, 0, 0) }, // Red
        ] as &[_]),
    };

    let mut buffer: Vec<PremultipliedRgbaColor> =
        vec![PremultipliedRgbaColor::from(crate::Color::from_argb_u8(0, 0, 0, 0)); 100];
    let line = PhysicalLength::new(5); // Near top edge where corners matter

    draw_conic_gradient_with_clipping(&rect, line, &gradient, &mut buffer, 0, 0, Some(&rr));

    // Corner areas should be clipped at y=5 with radius=30
    assert_eq!(buffer[0].alpha, 0, "Top-left corner should be clipped");
    assert_eq!(buffer[99].alpha, 0, "Top-right corner should be clipped");

    // Interior should have conic gradient applied
    assert!(buffer[40].alpha > 0, "Center area should have conic gradient");
}

#[test]
fn test_gradient_clipping_performance_comparison() {
    use crate::software_renderer::scene::{LinearGradientCommand, RoundedRectangle};
    use crate::software_renderer::{
        PhysicalBorderRadius, PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalSize,
    };
    use alloc::vec;
    use alloc::vec::Vec;

    let rect = PhysicalRect::new(
        PhysicalPoint::from_lengths(PhysicalLength::new(0), PhysicalLength::new(0)),
        PhysicalSize::from_lengths(PhysicalLength::new(1000), PhysicalLength::new(1000)),
    );

    // Test with no border radius (should use fast path)
    let no_radius = PhysicalBorderRadius {
        top_left: 0,
        top_right: 0,
        bottom_right: 0,
        bottom_left: 0,
        _unit: Default::default(),
    };
    let rr_no_radius = RoundedRectangle {
        radius: no_radius,
        width: PhysicalLength::new(0),
        border_color: PremultipliedRgbaColor::from(crate::Color::from_rgb_u8(0, 0, 0)),
        inner_color: PremultipliedRgbaColor::from(crate::Color::from_rgb_u8(255, 255, 255)),
        left_clip: PhysicalLength::new(0),
        right_clip: PhysicalLength::new(0),
        top_clip: PhysicalLength::new(0),
        bottom_clip: PhysicalLength::new(0),
    };

    let gradient = LinearGradientCommand {
        color1: PremultipliedRgbaColor::from(crate::Color::from_rgb_u8(255, 0, 0)),
        color2: PremultipliedRgbaColor::from(crate::Color::from_rgb_u8(0, 0, 255)),
        start: 128,
        flags: 0,
        left_clip: PhysicalLength::new(0),
        right_clip: PhysicalLength::new(0),
        top_clip: PhysicalLength::new(0),
        bottom_clip: PhysicalLength::new(0),
    };

    let mut buffer: Vec<PremultipliedRgbaColor> =
        vec![PremultipliedRgbaColor::from(crate::Color::from_argb_u8(0, 0, 0, 0)); 100];
    let line = PhysicalLength::new(500);

    // This should use the fast path since radius is zero
    draw_linear_gradient_with_clipping(&rect, line, &gradient, &mut buffer, 0, Some(&rr_no_radius));

    // Verify that the gradient was applied (should be same as non-clipped version)
    assert!(buffer[50].alpha > 0, "Gradient should be applied even with zero radius");

    // Test that None for rounded_rect also uses fast path
    buffer.fill(PremultipliedRgbaColor::from(crate::Color::from_argb_u8(0, 0, 0, 0)));
    draw_linear_gradient_with_clipping(&rect, line, &gradient, &mut buffer, 0, None);
    assert!(buffer[50].alpha > 0, "Gradient should be applied with None rounded_rect");
}
