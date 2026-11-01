// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore premultiply
#![allow(clippy::identity_op)] // We use x + 0 a lot here for symmetry

//! This is the module for the functions that are drawing the pixels
//! on the line buffer

use super::{Fixed, PhysicalLength, PhysicalRect};
use crate::scene::{
    GradientFactor, GradientPosition, LinearGradientPosition, PremultipliedGradientStop,
};
use derive_more::{Add, Mul, Sub};
use i_slint_core::graphics::{Rgb8Pixel, TexturePixelFormat};
use i_slint_core::lengths::{PointLengths, SizeLengths};
use i_slint_core::{Color, InternalToken};
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
    let source_width = source_size.width as i64;
    let source_height = source_size.height as i64;
    let len = line_buffer.len();
    let y = line - span.origin.y_length();
    let y = (if rotation.mirror_width() { span.size.height - y.get() - 1 } else { y.get() }) as i64;

    let off_y = Fixed::<i64, 32>::from_fixed(off_y);
    let off_x = Fixed::<i64, 32>::from_fixed(off_x);
    let dy_q8 = Fixed::<i32, 8>::try_from_fixed(dy).expect("texture step fits in Q8");
    let bpp = format.bpp();

    if !rotation.is_transpose() {
        let mut delta = dx;
        let row = off_y + dy * y;
        // The position where to start in the image array for a this row
        let row_offset = (row.truncate() % source_height) as usize * pixel_stride as usize;
        let mut tile_start = 0;

        // the size of the tile in physical pixels in the target
        let tile_len = (Fixed::from_integer(source_width) / delta) as usize;
        // the amount of missing image pixel on one tile
        let mut remainder = Fixed::from_integer(source_width) % delta;
        // The position in image pixel where to get the image
        let mut pos;
        // the end index in the target buffer
        let mut end;
        // the accumulated error in image pixels
        let mut acc_err;
        if rotation.mirror_height() {
            let o = (off_x + (delta * (extra_clip_end as i64 + len as i64 - 1)))
                % Fixed::from_integer(source_width);
            pos = o;
            tile_start = source_width;
            end = (o / delta) as usize + 1;
            acc_err = -delta + o % delta;
            delta = -delta;
            remainder = -remainder;
        } else {
            let o = (off_x + delta * extra_clip_begin as i64) % Fixed::from_integer(source_width);
            pos = o;
            end = ((Fixed::from_integer(source_width) - o) / delta) as usize;
            acc_err = (Fixed::from_integer(source_width) - o) % delta;
            if acc_err != Fixed::default() {
                acc_err = delta - acc_err;
                end += 1;
            }
        }
        end = end.min(len);
        let mut begin = 0;
        let row_fract = row.fract();
        let contiguous = delta == Fixed::from_integer(1)
            && alpha == 0xff
            && has_contiguous_path(format, colorize);
        while begin < len {
            if contiguous {
                draw_contiguous_pixels(
                    &mut line_buffer[begin..end],
                    format,
                    &data[(row_offset + pos.truncate() as usize) * bpp..][..(end - begin) * bpp],
                );
            } else {
                fetch_blend_pixel(
                    &mut line_buffer[begin..end],
                    format,
                    data,
                    alpha,
                    colorize,
                    (pixel_stride as usize, dy_q8),
                    #[inline(always)]
                    |bpp| {
                        let p =
                            ((row_offset + pos.truncate() as usize) * bpp, pos.fract(), row_fract);
                        pos += delta;
                        p
                    },
                );
            }
            begin = end;
            end += tile_len;
            pos = acc_err + Fixed::from_integer(tile_start);
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
        let col = off_x + dx * y;
        let col_fract = col.fract();
        let col = (col.truncate() % source_width) as usize * bpp;
        let stride = pixel_stride as usize * bpp;
        let mut row_delta = dy;
        let tile_len = (Fixed::from_integer(source_height) / row_delta) as usize;
        let mut remainder = Fixed::from_integer(source_height) % row_delta;
        let mut end;
        let mut row_init = Fixed::default();
        let mut row;
        let mut acc_err;
        if rotation.mirror_height() {
            row_init = Fixed::from_integer(source_height);
            row = (off_y + (row_delta * (extra_clip_end as i64 + len as i64 - 1)))
                % Fixed::from_integer(source_height);
            end = (row / row_delta) as usize + 1;
            acc_err = -row_delta + row % row_delta;
            row_delta = -row_delta;
            remainder = -remainder;
        } else {
            row =
                (off_y + row_delta * extra_clip_begin as i64) % Fixed::from_integer(source_height);
            end = ((Fixed::from_integer(source_height) - row) / row_delta) as usize;
            acc_err = (Fixed::from_integer(source_height) - row) % row_delta;
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
                (stride, dy_q8),
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

    fn has_contiguous_path(format: TexturePixelFormat, colorize: Color) -> bool {
        match format {
            TexturePixelFormat::Rgb => true,
            #[cfg(feature = "image-pixel-format-rgb565")]
            TexturePixelFormat::Rgb565 => true,
            #[cfg(feature = "image-pixel-format-gray8")]
            TexturePixelFormat::Gray8 => true,
            TexturePixelFormat::Rgba | TexturePixelFormat::RgbaPremultiplied => {
                colorize.alpha() == 0
            }
            TexturePixelFormat::AlphaMap | TexturePixelFormat::SignedDistanceField => false,
        }
    }

    fn draw_contiguous_pixels(
        line_buffer: &mut [impl TargetPixel],
        format: TexturePixelFormat,
        src: &[u8],
    ) {
        match format {
            TexturePixelFormat::Rgb => {
                for (pix, p) in line_buffer.iter_mut().zip(src.as_chunks::<3>().0) {
                    *pix = TargetPixel::from_rgb(p[0], p[1], p[2]);
                }
            }
            #[cfg(feature = "image-pixel-format-rgb565")]
            TexturePixelFormat::Rgb565 => {
                for (pix, p) in line_buffer.iter_mut().zip(src.as_chunks::<2>().0) {
                    let p = Rgb565Pixel(u16::from_ne_bytes(*p));
                    *pix = TargetPixel::from_rgb(p.red(), p.green(), p.blue());
                }
            }
            #[cfg(feature = "image-pixel-format-gray8")]
            TexturePixelFormat::Gray8 => {
                for (pix, &v) in line_buffer.iter_mut().zip(src) {
                    *pix = TargetPixel::from_rgb(v, v, v);
                }
            }
            TexturePixelFormat::Rgba => {
                for (pix, p) in line_buffer.iter_mut().zip(src.as_chunks::<4>().0) {
                    blend_rgba(pix, p[3], p);
                }
            }
            TexturePixelFormat::RgbaPremultiplied => {
                for (pix, p) in line_buffer.iter_mut().zip(src.as_chunks::<4>().0) {
                    blend_premultiplied(pix, p);
                }
            }
            TexturePixelFormat::AlphaMap | TexturePixelFormat::SignedDistanceField => {
                unreachable!("rejected by has_contiguous_path")
            }
        }
    }

    #[inline(always)]
    fn blend_rgba(pix: &mut impl TargetPixel, alpha: u8, p: &[u8]) {
        match alpha {
            0 => {}
            0xff => *pix = TargetPixel::from_rgb(p[0], p[1], p[2]),
            a => pix.blend(PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                a, p[0], p[1], p[2],
            ))),
        }
    }

    #[inline(always)]
    fn blend_premultiplied(pix: &mut impl TargetPixel, p: &[u8]) {
        match p[3] {
            0 => {}
            0xff => *pix = TargetPixel::from_rgb(p[0], p[1], p[2]),
            alpha => {
                pix.blend(PremultipliedRgbaColor { alpha, red: p[0], green: p[1], blue: p[2] })
            }
        }
    }

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
            #[cfg(feature = "image-pixel-format-rgb565")]
            TexturePixelFormat::Rgb565 => {
                if alpha == 0xff {
                    for pix in line_buffer {
                        let b: &[u8] = &data[pos(2).0..][..2];
                        let p = Rgb565Pixel(u16::from_ne_bytes([b[0], b[1]]));
                        *pix = TargetPixel::from_rgb(p.red(), p.green(), p.blue());
                    }
                } else {
                    for pix in line_buffer {
                        let b: &[u8] = &data[pos(2).0..][..2];
                        let p = Rgb565Pixel(u16::from_ne_bytes([b[0], b[1]]));
                        pix.blend(PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                            alpha,
                            p.red(),
                            p.green(),
                            p.blue(),
                        )))
                    }
                }
            }
            TexturePixelFormat::Rgba => {
                if color.alpha() == 0 {
                    for pix in line_buffer {
                        let pos = pos(4).0;
                        let p: &[u8] = &data[pos..pos + 4];
                        blend_rgba(pix, ((p[3] as u16 * alpha as u16) / 255) as u8, p);
                    }
                } else {
                    let opaque = TargetPixel::from_rgb(color.red(), color.green(), color.blue());
                    for pix in line_buffer {
                        let pos = pos(4).0;
                        let coverage = ((data[pos + 3] as u16 * alpha as u16) / 255) as u8;
                        blend_coverage(pix, opaque, color, coverage);
                    }
                }
            }
            TexturePixelFormat::RgbaPremultiplied => {
                if color.alpha() > 0 {
                    let opaque = TargetPixel::from_rgb(color.red(), color.green(), color.blue());
                    for pix in line_buffer {
                        let pos = pos(4).0;
                        let coverage = ((data[pos + 3] as u16 * alpha as u16) / 255) as u8;
                        blend_coverage(pix, opaque, color, coverage);
                    }
                } else if alpha == 0xff {
                    for pix in line_buffer {
                        let pos = pos(4).0;
                        blend_premultiplied(pix, &data[pos..pos + 4]);
                    }
                } else {
                    for pix in line_buffer {
                        let pos = pos(4).0;
                        if data[pos + 3] == 0 {
                            continue;
                        }
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
                let opaque = TargetPixel::from_rgb(color.red(), color.green(), color.blue());
                for pix in line_buffer {
                    let pos = pos(1).0;
                    let coverage = ((data[pos] as u16 * alpha as u16) / 255) as u8;
                    blend_coverage(pix, opaque, color, coverage);
                }
            }
            #[cfg(feature = "image-pixel-format-gray8")]
            TexturePixelFormat::Gray8 => {
                if alpha == 0xff {
                    for pix in line_buffer {
                        let pos = pos(1).0;
                        let v = data[pos];
                        *pix = TargetPixel::from_rgb(v, v, v);
                    }
                } else {
                    for pix in line_buffer {
                        let pos = pos(1).0;
                        let v = data[pos];
                        pix.blend(PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                            alpha, v, v, v,
                        )));
                    }
                }
            }
            TexturePixelFormat::SignedDistanceField => {
                const RANGE: i32 = 6;
                let factor = (362 * 256 / delta.0) * RANGE; // 362 ≃ 255 * sqrt(2)
                let opaque = TargetPixel::from_rgb(color.red(), color.green(), color.blue());
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
                    blend_coverage(pix, opaque, color, (a / 255) as u8);
                }
            }
        };
    }
}

#[inline(always)]
fn blend_coverage<T: TargetPixel>(pix: &mut T, opaque: T, color: Color, coverage: u8) {
    match coverage {
        0 => {}
        0xff => *pix = opaque,
        _ => pix.blend(PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
            coverage,
            color.red(),
            color.green(),
            color.blue(),
        ))),
    }
}

/// This is an integer shifted by 4 bits.
/// Note: this is not a "fixed point" because multiplication and sqrt operation operate to
/// the shifted integer
#[derive(Clone, Copy, Default, PartialEq, Ord, PartialOrd, Eq, Add, Sub, Mul)]
struct Shifted(u32);
impl Shifted {
    const ONE: Self = Shifted(1 << 4);
    const ZERO: Self = Shifted(0);
    #[track_caller]
    #[inline]
    fn new(value: impl TryInto<u32> + core::fmt::Debug + Copy) -> Self {
        Self(value.try_into().unwrap_or_else(|_| panic!("Overflow {value:?}")) << 4)
    }
    #[inline(always)]
    fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
    #[inline(always)]
    fn sqrt(self) -> Self {
        let root = self.0.isqrt();
        Self(if self.0 - root * root > root { root + 1 } else { root })
    }
}
impl core::ops::Mul for Shifted {
    type Output = Shifted;
    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

/// The radius of the left and right corner that `line` crosses (0 if none),
/// each with the distance of `line` from that corner's horizontal edge of `shape`,
/// and the distance of `line` from the nearest horizontal edge.
fn corner_radii_on_line(
    span: &PhysicalRect,
    line: PhysicalLength,
    shape: &super::RoundedShape,
) -> ((i16, i16), (i16, i16), i16) {
    let y1 = (line - span.origin.y_length()).get().saturating_add(shape.top_clip.get());
    let y2 = (span.origin.y_length() + span.size.height_length() - line)
        .get()
        .saturating_add(shape.bottom_clip.get())
        - 1;
    let y = y1.min(y2);
    debug_assert!(y >= 0);
    let r = &shape.radius;
    let left = if y1 < r.top_left {
        (r.top_left, y1)
    } else if y2 < r.bottom_left {
        (r.bottom_left, y2)
    } else {
        (0, 0)
    };
    let right = if y1 < r.top_right {
        (r.top_right, y1)
    } else if y2 < r.bottom_right {
        (r.bottom_right, y2)
    } else {
        (0, 0)
    };
    (left, right, y)
}

/// Where a pixel line crosses a corner arc, approximated by the arc's tangent
/// at the line's vertical center, and measured from the corner's vertical edge.
/// A pixel's coverage is 0.5 plus the distance of its center inside the tangent, clamped.
#[derive(Clone, Copy, Default)]
struct ArcEdge {
    /// Where the tangent crosses the line's vertical center, in 1/16 pixel.
    center: i32,
    /// The x component of the tangent's unit normal, in 1/8192.
    normal_x: i32,
    /// The pixels the tangent covers partially.
    first: u32,
    end: u32,
}

/// The side of the shape a corner is on.
#[derive(Clone, Copy)]
enum Side {
    Left,
    /// The shape's right edge is at this pixel.
    Right(u32),
}

impl ArcEdge {
    const NORMAL_ONE: i32 = 1 << 13;

    fn vertical(x: Shifted) -> Self {
        Self::from_tangent(x.0 as i32, Self::NORMAL_ONE, Shifted::ONE.0 as i32 / 2)
    }

    /// The edge of a circle of radius `circle`, centered `r` in from both edges of a corner.
    /// `y` is `r` minus the line's distance from the horizontal edge.
    fn new(r: Shifted, circle: Shifted, y: Shifted) -> Self {
        let center_y = y - Shifted(Shifted::ONE.0 / 2);
        if center_y >= circle {
            return Self::vertical(r);
        }
        // From the circle equation; nonzero, since `center_y` is at least half a pixel below `circle`.
        let x = (circle * circle - center_y * center_y).sqrt().0;
        Self::from_tangent(
            (r.0 - x) as i32,
            ((x * Self::NORMAL_ONE as u32) / circle.0) as i32,
            ((Shifted::ONE.0 / 2 * circle.0) / x) as i32,
        )
    }

    /// `half_width` is `0.5 / normal_x`, in 1/16 pixel:
    /// how far from `center` the coverage reaches 0 and 255 along the line.
    fn from_tangent(center: i32, normal_x: i32, half_width: i32) -> Self {
        let half_pixel = Shifted::ONE.0 as i32 / 2;
        let first = ((center - half_pixel - half_width) >> 4) + 1;
        let end = (center - half_pixel + half_width + Shifted::ONE.0 as i32 - 1) >> 4;
        Self { center, normal_x, first: first.max(0) as u32, end: end.max(0) as u32 }
    }

    /// Coverage, between 0 and 255, of pixel `x` from the corner's vertical edge.
    #[inline(always)]
    fn coverage(&self, x: u32) -> u32 {
        let distance = x as i32 * Shifted::ONE.0 as i32 + Shifted::ONE.0 as i32 / 2 - self.center;
        (128 + ((distance * self.normal_x) >> 9)).clamp(0, 255) as u32
    }

    /// The pixels of the shape the edge covers partially.
    #[inline(always)]
    fn pixels(&self, side: Side) -> core::ops::Range<u32> {
        match side {
            Side::Left => self.first..self.end,
            Side::Right(right) => right.saturating_sub(self.end)..right.saturating_sub(self.first),
        }
    }

    /// Coverage, between 0 and 255, of pixel `x` of the shape.
    #[inline(always)]
    fn shape_coverage(&self, side: Side, x: u32) -> u32 {
        match side {
            Side::Left => self.coverage(x),
            Side::Right(right) => self.coverage(right - 1 - x),
        }
    }

    /// Calls `process_pixel` with the buffer index and coverage of each pixel the edge covers partially,
    /// for a buffer of `len` pixels starting at pixel `left_clip` of the shape.
    #[inline(always)]
    fn for_each_pixel(
        &self,
        side: Side,
        left_clip: u32,
        len: usize,
        mut process_pixel: impl FnMut(usize, u32),
    ) {
        let pixels = self.pixels(side);
        for x in pixels.start.max(left_clip)..pixels.end.min(left_clip + len as u32) {
            process_pixel((x - left_clip) as usize, self.shape_coverage(side, x));
        }
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
    let shape = &rr.shape;
    let ((left_radius, left_y), (right_radius, right_y), y) =
        corner_radii_on_line(span, line, shape);
    let border = Shifted::new(rr.width.get());
    // Everything below works in unclipped coordinates, so a clip through an anti-aliasing ramp
    // doesn't change the ramp; only `to_buffer` and `for_each_pixel` map to buffer indices.
    let left_clip = shape.left_clip.get().saturating_add(extra_left_clip) as u32;
    let to_buffer = |x: u32| x.saturating_sub(left_clip).min(width as u32) as usize;
    let right = Side::Right(
        left_clip + width as u32 + shape.right_clip.get().saturating_add(extra_right_clip) as u32,
    );
    let corner = |r: i16, y: i16| {
        if r == 0 {
            return (ArcEdge::vertical(Shifted::ZERO), ArcEdge::vertical(border));
        }
        let r = Shifted::new(r);
        let y = r - Shifted::new(y);
        let outer = ArcEdge::new(r, r, y);
        if border == Shifted::ZERO {
            return (outer, outer);
        }
        (outer, ArcEdge::new(r, r.saturating_sub(border), y))
    };

    let (outer_left, inner_left) = corner(left_radius, left_y);
    let (outer_right, inner_right) = if (right_radius, right_y) == (left_radius, left_y) {
        (outer_left, inner_left)
    } else {
        corner(right_radius, right_y)
    };
    let c = if border == Shifted::ZERO { rr.inner_color } else { rr.border_color };
    let blend_outer = |line_buffer: &mut [_], x: usize, cov: u32| {
        let col = PremultipliedRgbaColor {
            alpha: (((c.alpha as u32) * cov as u32) / 255) as u8,
            red: (((c.red as u32) * cov as u32) / 255) as u8,
            green: (((c.green as u32) * cov as u32) / 255) as u8,
            blue: (((c.blue as u32) * cov as u32) / 255) as u8,
        };
        TargetPixel::blend(&mut line_buffer[x], col);
    };
    let blend_range = |line_buffer: &mut [_], begin: u32, end: u32, color| {
        let (begin, end) = (to_buffer(begin), to_buffer(end));
        if begin < end {
            TargetPixel::blend_slice(&mut line_buffer[begin..end], color)
        }
    };
    let blend_inner = |line_buffer: &mut [_], x: usize, cov: u32| {
        TargetPixel::blend(
            &mut line_buffer[x],
            interpolate_color(cov, rr.border_color, rr.inner_color),
        )
    };

    outer_left
        .for_each_pixel(Side::Left, left_clip, width, |x, cov| blend_outer(line_buffer, x, cov));
    let outer_end = outer_left.pixels(Side::Left).end;
    let outer_start = outer_right.pixels(right).start;
    if y < rr.width.get() {
        // up or down border
        blend_range(line_buffer, outer_end, outer_start, rr.border_color);
    } else {
        let (left_pixels, right_pixels) =
            (inner_left.pixels(Side::Left), inner_right.pixels(right));
        if left_pixels.end <= right_pixels.start {
            if border > Shifted::ZERO {
                blend_range(line_buffer, outer_end, left_pixels.start, rr.border_color);
                inner_left.for_each_pixel(Side::Left, left_clip, width, |x, cov| {
                    blend_inner(line_buffer, x, cov)
                });
            }
            if rr.inner_color.alpha > 0 {
                blend_range(line_buffer, left_pixels.end, right_pixels.start, rr.inner_color);
            }
            if border > Shifted::ZERO {
                inner_right.for_each_pixel(right, left_clip, width, |x, cov| {
                    blend_inner(line_buffer, x, cov)
                });
                blend_range(line_buffer, right_pixels.end, outer_start, rr.border_color);
            }
        } else {
            // A corner wider than half the rectangle reaches past the opposite side's border,
            // so both inner edges cross the same pixels.
            let begin = left_pixels.start.min(right_pixels.start);
            let end = left_pixels.end.max(right_pixels.end);
            blend_range(line_buffer, outer_end, begin, rr.border_color);
            for x in begin.max(left_clip)..end.min(left_clip + width as u32) {
                let coverage = |edge: &ArcEdge, side, pixels: &core::ops::Range<u32>| {
                    if x < pixels.start {
                        if matches!(side, Side::Left) { 0 } else { 255 }
                    } else if x >= pixels.end {
                        if matches!(side, Side::Left) { 255 } else { 0 }
                    } else {
                        edge.shape_coverage(side, x)
                    }
                };
                let cov = coverage(&inner_left, Side::Left, &left_pixels).min(coverage(
                    &inner_right,
                    right,
                    &right_pixels,
                ));
                blend_inner(line_buffer, (x - left_clip) as usize, cov);
            }
            blend_range(line_buffer, end, outer_start, rr.border_color);
        }
    }
    outer_right.for_each_pixel(right, left_clip, width, |x, cov| blend_outer(line_buffer, x, cov));
}

// a is between 0 and 255. When 0, we get color1, when 255 we get color2
pub(super) fn interpolate_color(
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

/// Taken as `dyn` by [`draw_line_with_clips`] so it's only instantiated once per pixel type.
pub(super) trait LineCommand<T: TargetPixel> {
    fn draw_line(
        &self,
        rect: &PhysicalRect,
        line: PhysicalLength,
        buffer: &mut [T],
        extra_left_clip: i16,
        extra_right_clip: i16,
    );
    fn draw_scratch_line(
        &self,
        rect: &PhysicalRect,
        line: PhysicalLength,
        buffer: &mut [PremultipliedRgbaColor],
        extra_left_clip: i16,
        extra_right_clip: i16,
    );
}

macro_rules! impl_line_command {
    ($ty:ty, |$cmd:ident, $rect:ident, $line:ident, $buffer:ident, $left:ident, $right:ident| $body:expr) => {
        impl<T: TargetPixel> LineCommand<T> for $ty {
            fn draw_line(
                &self,
                $rect: &PhysicalRect,
                $line: PhysicalLength,
                $buffer: &mut [T],
                $left: i16,
                $right: i16,
            ) {
                let $cmd = self;
                $body
            }
            fn draw_scratch_line(
                &self,
                $rect: &PhysicalRect,
                $line: PhysicalLength,
                $buffer: &mut [PremultipliedRgbaColor],
                $left: i16,
                $right: i16,
            ) {
                let $cmd = self;
                $body
            }
        }
    };
}

pub(super) struct SolidColor(pub PremultipliedRgbaColor);

impl_line_command!(SolidColor, |c, _rect, _line, buffer, _left, _right| {
    TargetPixel::blend_slice(buffer, c.0)
});
impl_line_command!(super::RoundedRectangle, |rr, rect, line, buffer, left, right| {
    draw_rounded_rectangle_line(rect, line, rr, buffer, left, right)
});
impl_line_command!(super::SceneTexture<'_>, |texture, rect, line, buffer, left, right| {
    draw_texture_line(rect, line, texture, buffer, left, right)
});
impl_line_command!(super::LinearGradientCommand, |g, rect, line, buffer, left, _right| {
    draw_linear_gradient(rect, line, g, buffer, left)
});
impl_line_command!(super::RadialGradientCommand, |g, rect, line, buffer, left, right| {
    draw_radial_gradient(rect, line, g, buffer, left, right)
});
impl_line_command!(super::ConicGradientCommand, |g, rect, line, buffer, left, right| {
    draw_conic_gradient(rect, line, g, buffer, left, right)
});
impl_line_command!(super::BoxShadowCommand, |shadow, rect, line, buffer, left, _right| {
    super::box_shadow::draw_box_shadow_line(rect, line, shadow, buffer, left)
});

/// Where the arcs of one [`super::ShapeClip`] cross a line.
/// The arcs are relative to the shape's left edge rather than the buffer,
/// so the coverage doesn't depend on where the dirty region splits the line.
#[derive(Clone, Copy, Default)]
struct ClipSpan {
    /// The shape's left edge, in pixels left of the buffer.
    left_offset: u32,
    len: usize,
    shape_width: u32,
    left_edge: ArcEdge,
    right_edge: ArcEdge,
    binary: bool,
}

impl ClipSpan {
    fn new(
        rect: &PhysicalRect,
        line: PhysicalLength,
        clip: &super::ShapeClip,
        len: usize,
        extra_left_clip: i16,
        extra_right_clip: i16,
    ) -> Option<Self> {
        let shape = &clip.shape;
        let ((left_radius, left_y), (right_radius, right_y), _) =
            corner_radii_on_line(rect, line, shape);
        if left_radius == 0 && right_radius == 0 {
            return None;
        }
        let left_offset = shape.left_clip.get().saturating_add(extra_left_clip) as u32;
        let right_offset = shape.right_clip.get().saturating_add(extra_right_clip) as u32;
        let shape_width = len as u32 + left_offset + right_offset;
        let border = Shifted::new(clip.opaque_border.get());
        let arc = |r: i16, y: i16| {
            if r == 0 {
                return ArcEdge::vertical(Shifted::ZERO);
            }
            let r = Shifted::new(r);
            ArcEdge::new(r, r.saturating_sub(border), r - Shifted::new(y))
        };
        let left_edge = arc(left_radius, left_y);
        let right_edge = if (right_radius, right_y) == (left_radius, left_y) {
            left_edge
        } else {
            arc(right_radius, right_y)
        };
        Some(Self {
            left_offset,
            len,
            shape_width,
            left_edge,
            right_edge,
            binary: border > Shifted::ZERO,
        })
    }

    fn right(&self) -> Side {
        Side::Right(self.shape_width)
    }

    fn buffer_index(&self, x: u32) -> usize {
        (x.saturating_sub(self.left_offset) as usize).min(self.len)
    }

    fn outer(&self) -> core::ops::Range<usize> {
        let left = self.left_edge.pixels(Side::Left);
        let right = self.right_edge.pixels(self.right());
        self.buffer_index(left.start)..self.buffer_index(right.end)
    }

    fn inner(&self) -> core::ops::Range<usize> {
        if self.binary {
            // `draw_rounded_rectangle_line` anti-aliases the border's inner edge over this same
            // span, so the command must have full coverage in it.
            self.outer()
        } else {
            let left = self.left_edge.pixels(Side::Left);
            let right = self.right_edge.pixels(self.right());
            self.buffer_index(left.end)..self.buffer_index(right.start)
        }
    }

    /// The coverage of buffer pixel `x`, which must be in `outer()`.
    fn coverage(&self, x: usize) -> u32 {
        if self.binary {
            return 255;
        }
        let x = x as u32 + self.left_offset;
        let right = self.right();
        let left = if x >= self.left_edge.pixels(Side::Left).end {
            255
        } else {
            self.left_edge.coverage(x)
        };
        let right = if x < self.right_edge.pixels(right).start {
            255
        } else {
            self.right_edge.shape_coverage(right, x)
        };
        // On a narrow shape both arcs can cross the same pixel.
        left.min(right)
    }
}

/// Bounds the stack buffers of the edge pixels.
/// Most edges fit in one chunk, but the flat top of a large corner takes several.
const EDGE_CHUNK: usize = 16;

/// Most runs have a few entries; a longer one recomputes its spans for each edge chunk.
const CACHED_SPANS: usize = 4;

/// A command's clip run on one line of its buffer.
pub(super) struct LineClip<'a> {
    rect: &'a PhysicalRect,
    line: PhysicalLength,
    clips: &'a [super::ShapeClip],
    len: usize,
    extra_left_clip: i16,
    extra_right_clip: i16,
    spans: [ClipSpan; CACHED_SPANS],
    span_count: usize,
    /// The pixels the run covers at all.
    pub outer: core::ops::Range<usize>,
    /// The pixels it covers fully.
    pub inner: core::ops::Range<usize>,
}

impl<'a> LineClip<'a> {
    /// `None` if the run doesn't clip this line of `len` pixels.
    #[inline]
    pub fn new(
        rect: &'a PhysicalRect,
        line: PhysicalLength,
        clips: &'a [super::ShapeClip],
        len: usize,
        extra_left_clip: i16,
        extra_right_clip: i16,
    ) -> Option<Self> {
        // The setup costs more than an MCU's fill of an unclipped line.
        if clips.is_empty() {
            return None;
        }
        let mut this = Self {
            rect,
            line,
            clips,
            len,
            extra_left_clip,
            extra_right_clip,
            spans: Default::default(),
            span_count: 0,
            outer: 0..len,
            inner: 0..len,
        };
        for span in this.compute_spans() {
            if let Some(cached) = this.spans.get_mut(this.span_count) {
                *cached = span;
            }
            this.span_count += 1;
            let (outer, inner) = (span.outer(), span.inner());
            this.outer = this.outer.start.max(outer.start)..this.outer.end.min(outer.end);
            this.inner = this.inner.start.max(inner.start)..this.inner.end.min(inner.end);
        }
        if this.span_count == 0 {
            return None;
        }
        this.outer.end = this.outer.end.max(this.outer.start);
        let inner_start = this.inner.start.clamp(this.outer.start, this.outer.end);
        this.inner = inner_start..this.inner.end.clamp(inner_start, this.outer.end);
        Some(this)
    }

    /// The spans of the clips that have a corner on this line; the others don't clip it.
    fn compute_spans(&self) -> impl Iterator<Item = ClipSpan> + 'a {
        let (rect, line, len) = (self.rect, self.line, self.len);
        let (extra_left_clip, extra_right_clip) = (self.extra_left_clip, self.extra_right_clip);
        self.clips.iter().filter_map(move |clip| {
            ClipSpan::new(rect, line, clip, len, extra_left_clip, extra_right_clip)
        })
    }

    /// Calls `f` with each chunk of the pixels between `outer` and `inner`, and their coverage.
    /// Clips combine by their minimum, which is exact for coincident and concentric edges.
    pub fn for_each_edge_chunk(&self, mut f: impl FnMut(core::ops::Range<usize>, &[u8])) {
        let mut coverage = [0u8; EDGE_CHUNK];
        for edge in [self.outer.start..self.inner.start, self.inner.end..self.outer.end] {
            for start in edge.clone().step_by(EDGE_CHUNK) {
                let chunk = start..(start + EDGE_CHUNK).min(edge.end);
                let coverage = &mut coverage[..chunk.len()];
                coverage.fill(255);
                let mut apply = |span: &ClipSpan| {
                    for (x, c) in chunk.clone().zip(coverage.iter_mut()) {
                        *c = (*c as u32).min(span.coverage(x)) as u8;
                    }
                };
                if self.span_count <= CACHED_SPANS {
                    self.spans[..self.span_count].iter().for_each(&mut apply);
                } else {
                    self.compute_spans().for_each(|span| apply(&span));
                }
                f(chunk, coverage);
            }
        }
    }
}

/// Draw one line of a command, clipped to its clip run with anti-aliased corners.
#[inline]
pub(super) fn draw_clipped_line<T: TargetPixel>(
    rect: &PhysicalRect,
    line: PhysicalLength,
    clips: &[super::ShapeClip],
    cmd: &impl LineCommand<T>,
    buffer: &mut [T],
    extra_left_clip: i16,
    extra_right_clip: i16,
) {
    if clips.is_empty() {
        cmd.draw_line(rect, line, buffer, extra_left_clip, extra_right_clip);
    } else {
        draw_line_with_clips(rect, line, clips, cmd, buffer, extra_left_clip, extra_right_clip);
    }
}

fn draw_line_with_clips<T: TargetPixel>(
    rect: &PhysicalRect,
    line: PhysicalLength,
    clips: &[super::ShapeClip],
    cmd: &dyn LineCommand<T>,
    buffer: &mut [T],
    extra_left_clip: i16,
    extra_right_clip: i16,
) {
    let len = buffer.len();
    let Some(line_clip) = LineClip::new(rect, line, clips, len, extra_left_clip, extra_right_clip)
    else {
        cmd.draw_line(rect, line, buffer, extra_left_clip, extra_right_clip);
        return;
    };
    let extra_clips = |range: &core::ops::Range<usize>| {
        (extra_left_clip + range.start as i16, extra_right_clip + (len - range.end) as i16)
    };

    let inner = line_clip.inner.clone();
    // `draw_texture_line` doesn't support an empty buffer.
    if !inner.is_empty() {
        let (left_clip, right_clip) = extra_clips(&inner);
        cmd.draw_line(rect, line, &mut buffer[inner], left_clip, right_clip);
    }

    let mut scratch = [PremultipliedRgbaColor::default(); EDGE_CHUNK];
    line_clip.for_each_edge_chunk(|chunk, coverage| {
        let scratch = &mut scratch[..chunk.len()];
        scratch.fill(PremultipliedRgbaColor::default());
        let (left_clip, right_clip) = extra_clips(&chunk);
        cmd.draw_scratch_line(rect, line, scratch, left_clip, right_clip);
        for ((x, color), cov) in chunk.zip(scratch.iter()).zip(coverage.iter()) {
            let color = interpolate_color(*cov as u32, PremultipliedRgbaColor::default(), *color);
            buffer[x].blend(color);
        }
    });
}

#[cfg(feature = "path")]
impl super::AnyGradientCommand {
    pub(super) fn as_command<T: TargetPixel>(&self) -> &dyn LineCommand<T> {
        match self {
            Self::Linear(g) => g,
            Self::Radial(g) => g,
            Self::Conic(g) => g,
        }
    }
}

impl super::LinearGradientCommand {
    /// Whether [`draw_linear_gradient`] overwrites every pixel of every line.
    pub(super) fn covers_line(&self) -> bool {
        self.stops.iter().all(|s| s.color.alpha == 255)
    }
}

/// The Q15 channels at `position`, which is between `stop` and the next stop.
#[inline]
fn linear_gradient_channels(
    stop: &super::LinearGradientStop,
    position: LinearGradientPosition,
) -> [u32; 4] {
    let offset = GradientPosition::from_fixed(position - stop.position);
    // Q12; the product stays below 2^31 since the offset is less than the distance.
    let factor = ((offset.0 as u32 * stop.factor_step) >> 19) as i32;
    let color = stop.channels();
    core::array::from_fn(|i| (((color[i] as i32) << 15) + ((stop.delta[i] * factor) << 3)) as u32)
}

#[inline(always)]
fn color_from_q15([r, g, b, a]: [u32; 4]) -> PremultipliedRgbaColor {
    PremultipliedRgbaColor {
        red: (r >> 15) as u8,
        green: (g >> 15) as u8,
        blue: (b >> 15) as u8,
        alpha: (a >> 15) as u8,
    }
}

// The per-pixel loops of `draw_linear_gradient` aren't inlined into it: with its setup around
// them, Cortex-M0 runs out of registers and spills their state on every pixel.
#[inline(never)]
fn draw_linear_fill(
    pixels: &mut [impl TargetPixel],
    color: PremultipliedRgbaColor,
    noise: GradientNoise,
) {
    blend_gradient_slice(pixels, color, noise)
}

/// Draws the pixels of a ramp from `stop` towards the next stop, `skipped` pixels into a
/// segment of `len` pixels whose first pixel is at `start_position`.
///
/// Not inlined, see `draw_linear_fill`.
#[inline(never)]
fn draw_linear_ramp(
    pixels: &mut [impl TargetPixel],
    stop: &super::LinearGradientStop,
    start_position: LinearGradientPosition,
    len: usize,
    skipped: usize,
    mut noise: GradientNoise,
) {
    let last = len as i32 - 1;
    // Keeps both ends of the ramp between the two stops' colors, so no channel wraps,
    // then moves to the first pixel to draw.
    let colors = stop.channels();
    let fit = |value: u32, i: usize| {
        let (color, delta, step) = (colors[i] as i32, stop.delta[i], stop.step[i]);
        let (low, high) =
            (color.min(color + delta) << 15, (color.max(color + delta) << 15) | 0x7fff);
        let value = (value as i32).clamp(low, high);
        let end = value + last * step;
        ((value + end.clamp(low, high) - end) as u32)
            .wrapping_add((skipped as u32).wrapping_mul(step as u32))
    };
    let [r, g, b, a] = linear_gradient_channels(stop, start_position);
    let (mut r, mut g, mut b, mut a) = (fit(r, 0), fit(g, 1), fit(b, 2), fit(a, 3));
    let [dr, dg, db, da] = stop.step.map(|step| step as u32);
    for pix in pixels {
        pix.blend_dithered(color_from_q15([r, g, b, a]), noise.next(), InternalToken);
        r = r.wrapping_add(dr);
        g = g.wrapping_add(dg);
        b = b.wrapping_add(db);
        a = a.wrapping_add(da);
    }
}

/// Whether a pixel at `position` is past a stop at `stop_position`,
/// going right on a line whose position changes by `dx` per pixel.
fn crossed(
    position: LinearGradientPosition,
    stop_position: LinearGradientPosition,
    dx: LinearGradientPosition,
) -> bool {
    if dx.0 > 0 { position >= stop_position } else { position < stop_position }
}

fn draw_linear_gradient(
    rect: &PhysicalRect,
    line: PhysicalLength,
    g: &super::LinearGradientCommand,
    buffer: &mut [impl TargetPixel],
    extra_left_clip: i16,
) {
    let stops = g.stops.as_slice();
    let (Some(first), Some(last)) = (stops.first(), stops.last()) else { return };
    let noise = GradientNoise::new(rect.min_x() as i32 + extra_left_clip as i32, line.get() as i32);
    // Everything below is relative to the rect's left edge rather than the buffer, so drawing a
    // line in pieces gives the same pixels as drawing it whole.
    let position = g.position + g.dy * (line.get() as i32 - rect.min_y() as i32);
    let width = rect.width() as usize;
    let offset = extra_left_clip.max(0) as usize;
    let visible = offset..offset + buffer.len();

    if g.dx.0 == 0 {
        let segment = stops
            .windows(2)
            .find(|w| position >= w[0].position && position < w[1].position)
            .map(|w| &w[0]);
        let color = match segment {
            Some(stop) => color_from_q15(linear_gradient_channels(stop, position)),
            None if position < first.position => first.color,
            None => last.color,
        };
        draw_linear_fill(buffer, color, noise);
        return;
    }

    // The index of the first pixel past `stop_position`, going right; see `crossed`.
    // Every segment's ends come from here, so neighboring segments meet exactly.
    let crossing = |stop_position: LinearGradientPosition| -> usize {
        // Fits an i32 since stop positions are within ±63 and pixel positions near [0, 1];
        // an i64 division is a library call on Cortex-M0.
        let distance = stop_position - position;
        let n = if g.dx.0 > 0 {
            if distance.0 <= 0 { 0 } else { (distance - Fixed(1)) / g.dx + 1 }
        } else if distance.0 > 0 {
            0
        } else {
            -distance / -g.dx + 1
        };
        (n as usize).min(width)
    };

    enum Region<'a> {
        Fill(PremultipliedRgbaColor),
        Ramp(&'a super::LinearGradientStop),
    }
    let mut draw = |segment: core::ops::Range<usize>, region| {
        let range = segment.start.max(visible.start)..segment.end.min(visible.end);
        if range.is_empty() {
            return;
        }
        let noise = noise.skip(range.start - offset);
        let pixels = &mut buffer[range.start - offset..range.end - offset];
        match region {
            Region::Fill(color) => draw_linear_fill(pixels, color, noise),
            Region::Ramp(stop) => {
                let start_position = position + g.dx * segment.start as i32;
                let skipped = range.start - segment.start;
                draw_linear_ramp(pixels, stop, start_position, segment.len(), skipped, noise);
            }
        }
    };

    // The stops in the order they're crossed going right. Region 0 is before the first of them,
    // region `n` after the last, and the regions in between interpolate between two stops.
    let n = stops.len();
    let boundary = |j: usize| if g.dx.0 > 0 { &stops[j] } else { &stops[n - 1 - j] };
    let region = |j: usize| match j {
        0 => Region::Fill(boundary(0).color),
        _ if j == n => Region::Fill(boundary(n - 1).color),
        _ => Region::Ramp(if g.dx.0 > 0 { boundary(j - 1) } else { boundary(j) }),
    };
    let visible_position = position + g.dx * visible.start as i32;
    let mut j = 0;
    while j < n && crossed(visible_position, boundary(j).position, g.dx) {
        j += 1;
    }
    let mut start = if j == 0 { 0 } else { crossing(boundary(j - 1).position) };
    loop {
        let end = if j == n { width } else { crossing(boundary(j).position) };
        draw(start..end, region(j));
        if j == n || end >= visible.end {
            break;
        }
        start = end;
        j += 1;
    }
}

/// Blends the color at `position`, which is clamped to [0, 1].
fn blend_stops(
    stops: &i_slint_core::SharedVector<PremultipliedGradientStop>,
    pixel: &mut impl TargetPixel,
    position: f32,
    noise: &mut GradientNoise,
) {
    let position = fixed_position(position);
    let mut fallback = stops.first().map(|s| s.color).unwrap_or_default();

    for [stop1, stop2] in stops.array_windows() {
        if position >= stop1.position && position <= stop2.position {
            let t = stop1.factor(position).0 as i32;
            let (c1, c2) = (stop1.color, stop2.color);
            let lerp = |a: u8, b: u8| {
                (a as i32 + (((b as i32 - a as i32) * t) >> GradientFactor::SHIFT)) as u8
            };

            let color = PremultipliedRgbaColor {
                alpha: lerp(c1.alpha, c2.alpha),
                red: lerp(c1.red, c2.red),
                green: lerp(c1.green, c2.green),
                blue: lerp(c1.blue, c2.blue),
            };
            pixel.blend_dithered(color, noise.next(), InternalToken);

            return;
        } else if position > stop2.position {
            fallback = stop2.color;
        }
    }
    pixel.blend_dithered(fallback, noise.next(), InternalToken);
}

/// `position` clamped to [0, 1].
///
/// Converts through the bits, since a float conversion and each float compare are library calls
/// without an FPU.
fn fixed_position(position: f32) -> GradientPosition {
    let bits = position.to_bits();
    // position.min(0)
    if bits >> 31 != 0 {
        return Fixed(0);
    }
    // position.max(1)
    if bits >= 1f32.to_bits() {
        return Fixed::from_integer(1);
    }
    let exponent = (bits >> 23) as i32 - 127;
    let mantissa = (bits & 0x7f_ffff) | 0x80_0000;
    // The value is `mantissa * 2^(exponent - 23)`, and the exponent is negative here.
    let shift = (23 - GradientPosition::SHIFT as i32 - exponent) as u32;
    Fixed(mantissa.checked_shr(shift).unwrap_or(0) as i32)
}

/// Draw a radial gradient on a line
fn draw_radial_gradient(
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

    // Pixels are sampled at their centers, half a pixel past their integer coordinates.
    let center_x = rect.min_x() as f32 + g.center_x - 0.5;
    let center_y = rect.min_y() as f32 + g.center_y - 0.5;

    debug_assert!(
        g.radius_x > 0.0 && g.radius_y > 0.0,
        "radii must be resolved before constructing RadialGradientCommand"
    );
    let inv_radius_x = 1. / g.radius_x;
    let inv_radius_y = 1. / g.radius_y;

    let start_x = rect.min_x() + extra_left_clip;
    let dy_norm = (line.get() as f32 - center_y) * inv_radius_y;
    let dy_norm_squared = dy_norm * dy_norm;
    let mut noise = GradientNoise::new(start_x as i32, line.get() as i32);

    let mut dx = start_x as f32 - center_x;
    for pixel in buffer {
        let dx_norm = dx * inv_radius_x;
        let distance = (dx_norm * dx_norm + dy_norm_squared).sqrt();
        blend_stops(&g.stops, pixel, distance, &mut noise);
        dx += 1.;
    }
}

/// Draw a conic gradient on a line
fn draw_conic_gradient(
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

    // Sampled at pixel centers, see `draw_radial_gradient`.
    let center_x = rect.min_x() as f32 + g.center_x - 0.5;
    let center_y = rect.min_y() as f32 + g.center_y - 0.5;

    let start_x = rect.min_x() + extra_left_clip;
    let dy = line.get() as f32 - center_y;
    let mut noise = GradientNoise::new(start_x as i32, line.get() as i32);

    // In turns, so that 0 is north, then normalized to [0, 1].
    let mut offset = 0.25 - g.rotation / core::f32::consts::TAU;
    offset -= offset as i32 as f32;
    if offset < 0. {
        offset += 1.;
    }

    let mut dx = start_x as f32 - center_x;
    for pixel in buffer {
        // position is in the range [-0.5, 1.5]
        let position = atan2_turns(dy, dx) + offset;
        // bits for integer compares
        let bits = position.to_bits();
        let position = if bits >> 31 != 0 {
            // move the range in [-0.5, 0.0] to [0.5, 1.0]
            position + 1.
        } else if bits >= 1f32.to_bits() {
            // move the range in [1.0, 1.5] to [0.0, 0.5]
            position - 1.
        } else {
            position
        };
        // position is in [0, 1]
        blend_stops(&g.stops, pixel, position, &mut noise);
        dx += 1.;
    }
}

/// `atan2(y, x)` in turns, between -0.5 and 0.5, accurate to 6.1e-4 rad.
///
/// Comparisons are done with bits as floating point ops are slow without an FPU.
fn atan2_turns(y: f32, x: f32) -> f32 {
    // A minimax fit of atan(t) for t in [0, 1], divided by 2π.
    const C1: f32 = 0.9953579 / core::f32::consts::TAU;
    const C3: f32 = -0.28868982 / core::f32::consts::TAU;
    const C5: f32 = 0.07933861 / core::f32::consts::TAU;

    let (x_bits, y_bits) = (x.to_bits(), y.to_bits());
    // mask out sign bit as positive floats compare like integers
    let (abs_x, abs_y) = (x_bits & 0x7fff_ffff, y_bits & 0x7fff_ffff);
    let (min, max) = if abs_x < abs_y { (abs_x, abs_y) } else { (abs_y, abs_x) };
    if max == 0 {
        return 0.;
    }
    let t = f32::from_bits(min) / f32::from_bits(max);
    let t2 = t * t;
    let mut turns = t * (C1 + t2 * (C3 + t2 * C5));
    // if it is atan(x/y)
    if abs_x < abs_y {
        turns = 0.25 - turns;
    }
    if x_bits >> 31 != 0 {
        // mirror across the y axis
        turns = 0.5 - turns;
    }
    // check if y is negative and negate turns if so
    if y_bits >> 31 != 0 { -turns } else { turns }
}

/// Interleaved gradient noise (Jimenez 2014) in fixed point, with the same formula as femtovg's `ditherNoise`.
#[derive(Clone, Copy)]
struct GradientNoise(u32);

impl GradientNoise {
    const X: u32 = (0.06711056 * 4294967296.0) as u32;
    const Y: u32 = (0.00583715 * 4294967296.0) as u32;
    // Q6.10, so its product with the top 16 bits of the phase fits in a u32
    const SCALE: u32 = (52.9829189 * 1024.0) as u32;

    /// Noise for the pixels going right from `(x, y)` in screen coordinates.
    fn new(x: i32, y: i32) -> Self {
        Self((x as u32).wrapping_mul(Self::X).wrapping_add((y as u32).wrapping_mul(Self::Y)))
    }

    fn skip(self, n: usize) -> Self {
        Self(self.0.wrapping_add(Self::X.wrapping_mul(n as u32)))
    }

    fn next(&mut self) -> u8 {
        let noise = (((self.0 >> 16) * Self::SCALE) >> 18) as u8;
        self.0 = self.0.wrapping_add(Self::X);
        noise
    }
}

/// See [`TargetPixel::blend_dithered`].
fn dither_rgb565(color: PremultipliedRgbaColor, noise: u8) -> PremultipliedRgbaColor {
    let dither = |v: u8, bits: u8| v.saturating_add(noise >> bits).min(color.alpha);
    PremultipliedRgbaColor {
        red: dither(color.red, 5),
        green: dither(color.green, 6),
        blue: dither(color.blue, 5),
        alpha: color.alpha,
    }
}

fn blend_gradient_slice(
    slice: &mut [impl TargetPixel],
    color: PremultipliedRgbaColor,
    mut noise: GradientNoise,
) {
    for pix in slice {
        pix.blend_dithered(color, noise.next(), InternalToken);
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

    /// Like `blend`, with `noise` in 0..=255 (a fraction of one quantization step).
    /// Formats with less than 8 bits per channel can use it to dither.
    #[doc(hidden)]
    #[inline]
    fn blend_dithered(&mut self, color: PremultipliedRgbaColor, _noise: u8, _: InternalToken) {
        self.blend(color)
    }
}

impl TargetPixel for Rgb8Pixel {
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
        self.red = (self.red as u16 * a / 255) as u8 + color.red;
        self.green = (self.green as u16 * a / 255) as u8 + color.green;
        self.blue = (self.blue as u16 * a / 255) as u8 + color.blue;
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

pub use i_slint_core::graphics::Rgb565Pixel;

const R_MASK: u16 = 0b1111_1000_0000_0000;
const G_MASK: u16 = 0b0000_0111_1110_0000;
const B_MASK: u16 = 0b0000_0000_0001_1111;

impl TargetPixel for Rgb565Pixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let a = (u8::MAX - color.alpha) as u32;
        // convert to 5 bits
        let a = (a + 4) >> 3;

        // 00000ggg_ggg00000_rrrrr000_000bbbbb
        let expanded = (self.0 & (R_MASK | B_MASK)) as u32 | (((self.0 & G_MASK) as u32) << 16);

        // gggggggg_000rrrrr_rrr000bb_bbbbbb00
        let c =
            ((color.red as u32) << 13) | ((color.green as u32) << 24) | ((color.blue as u32) << 2);
        // gggggg00_000rrrrr_000000bb_bbb00000
        let c = c & 0b11111100_00011111_00000011_11100000;

        let res = expanded * a + c;

        self.0 = ((res >> 21) as u16 & G_MASK) | ((res >> 5) as u16 & (R_MASK | B_MASK));
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        // This calls the inherent from_rgb, not this trait method.
        Rgb565Pixel::from_rgb(r, g, b)
    }

    fn blend_dithered(&mut self, color: PremultipliedRgbaColor, noise: u8, _: InternalToken) {
        self.blend(dither_rgb565(color, noise))
    }
}

// cSpell: ignore RRRRRGGG GGGBBBBB bswap

/// A 16bit RGB565 pixel stored in big-endian byte order.
///
/// The in-memory byte layout is `[RRRRRGGG, GGGBBBBB]` regardless of
/// host endianness — the format expected by most SPI display
/// controllers (ILI9341, ILI9342C, ST7789, etc.) without any
/// post-render byte swapping.
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Rgb565BigEndianPixel(pub u16);

impl Rgb565BigEndianPixel {
    /// Return the red component as a u8.
    ///
    /// The bits are shifted so that the result is between 0 and 255
    pub fn red(self) -> u8 {
        Rgb565Pixel(u16::from_be(self.0)).red()
    }
    /// Return the green component as a u8.
    ///
    /// The bits are shifted so that the result is between 0 and 255
    pub fn green(self) -> u8 {
        Rgb565Pixel(u16::from_be(self.0)).green()
    }
    /// Return the blue component as a u8.
    ///
    /// The bits are shifted so that the result is between 0 and 255
    pub fn blue(self) -> u8 {
        Rgb565Pixel(u16::from_be(self.0)).blue()
    }
}

impl TargetPixel for Rgb565BigEndianPixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        // Reuse the canonical native-endian Rgb565Pixel::blend by decoding
        // from BE byte order, blending, and re-encoding. On targets with a
        // byte-swap instruction (ARM REV16, RISC-V Zbb rev8, x86 bswap) each
        // `to_be`/`from_be` is one cycle on a little-endian host and a no-op
        // on a big-endian host. Benchmarking this against a direct-BE
        // bit-reassembly variant on Cortex-M33 showed the swap-around-native
        // form generates tighter code.
        let mut native = Rgb565Pixel(u16::from_be(self.0));
        native.blend(color);
        self.0 = native.0.to_be();
    }

    fn blend_dithered(&mut self, color: PremultipliedRgbaColor, noise: u8, _: InternalToken) {
        self.blend(dither_rgb565(color, noise))
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self(Rgb565Pixel::from_rgb(r, g, b).0.to_be())
    }
}

impl From<Rgb8Pixel> for Rgb565BigEndianPixel {
    fn from(p: Rgb8Pixel) -> Self {
        Self(Rgb565Pixel::from(p).0.to_be())
    }
}

impl From<Rgb565BigEndianPixel> for Rgb8Pixel {
    fn from(p: Rgb565BigEndianPixel) -> Self {
        Rgb565Pixel(u16::from_be(p.0)).into()
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
fn rgb565_full_component_expands_to_255() {
    // The C++ Rgb565Pixel accessors replicate the high bits the same way, so an RGB565 image
    // has to reach pure white on every renderer, not #f8fcf8.
    let white = Rgb565Pixel::from_rgb(0xff, 0xff, 0xff);
    assert_eq!(white.red(), 0xff);
    assert_eq!(white.green(), 0xff);
    assert_eq!(white.blue(), 0xff);

    let black = Rgb565Pixel::from_rgb(0, 0, 0);
    assert_eq!(black.red(), 0);
    assert_eq!(black.green(), 0);
    assert_eq!(black.blue(), 0);

    let white_be = Rgb565BigEndianPixel::from_rgb(0xff, 0xff, 0xff);
    assert_eq!(white_be.red(), 0xff);
    assert_eq!(white_be.green(), 0xff);
    assert_eq!(white_be.blue(), 0xff);
}

#[test]
fn rgb565_be() {
    // BE should be byte-swapped LE for any color
    for &(r, g, b) in &[(0xff, 0x25, 0u8), (0x56, 0x42, 0xe3), (0, 0xff, 0), (0, 0, 0xff)] {
        let le = Rgb565Pixel::from_rgb(r, g, b);
        let be = Rgb565BigEndianPixel::from_rgb(r, g, b);
        assert_eq!(le.0.swap_bytes(), be.0, "mismatch for ({r}, {g}, {b})");
    }

    // Round-trip through Rgb8Pixel
    let pix_be = Rgb565BigEndianPixel::from_rgb(0xff, 0x25, 0);
    let pix888: Rgb8Pixel = pix_be.into();
    assert_eq!(pix_be, pix888.into());

    let pix_be = Rgb565BigEndianPixel::from_rgb(0x56, 0x42, 0xe3);
    let pix888: Rgb8Pixel = pix_be.into();
    assert_eq!(pix_be, pix888.into());
}

#[test]
fn rgb565_round_trips_through_from_rgb() {
    // Drawing an RGB565 image relies on this to copy pixels unchanged to an RGB565 target.
    for value in 0..=u16::MAX {
        let p = Rgb565Pixel(value);
        assert_eq!(<Rgb565Pixel as TargetPixel>::from_rgb(p.red(), p.green(), p.blue()), p);
        let be = Rgb565BigEndianPixel::from_rgb(p.red(), p.green(), p.blue());
        assert_eq!(be.0, value.to_be());
    }
}

#[test]
fn rgb565_be_blend() {
    // Blending a BE pixel should produce the same visual result as LE
    let color = PremultipliedRgbaColor { red: 127, green: 0, blue: 0, alpha: 127 };

    let mut le = Rgb565Pixel::from_rgb(0, 0, 255);
    le.blend(color);
    let mut be = Rgb565BigEndianPixel::from_rgb(0, 0, 255);
    be.blend(color);
    assert_eq!(le.0.swap_bytes(), be.0);

    // Blend with green over a white background
    let color = PremultipliedRgbaColor { red: 0, green: 200, blue: 0, alpha: 200 };

    let mut le = Rgb565Pixel::from_rgb(255, 255, 255);
    le.blend(color);
    let mut be = Rgb565BigEndianPixel::from_rgb(255, 255, 255);
    be.blend(color);
    assert_eq!(le.0.swap_bytes(), be.0);
}

#[test]
fn atan2_turns_accuracy() {
    let check = |dy: f32, dx: f32| {
        let exact = (dy as f64).atan2(dx as f64) / core::f64::consts::TAU;
        let mut error = atan2_turns(dy, dx) as f64 - exact;
        error -= error.round(); // -0.5 and 0.5 turns are the same direction
        assert!(error.abs() * core::f64::consts::TAU < 6.2e-4, "dy {dy}, dx {dx}: error {error}");
    };
    for i in 0..100_000 {
        let a = i as f64 / 100_000. * core::f64::consts::TAU;
        check((a.sin() * 1000.) as f32, (a.cos() * 1000.) as f32);
    }
    for dy in -100..=100 {
        for dx in -100..=100 {
            check(dy as f32 - 0.25, dx as f32 + 0.5);
        }
    }
    assert_eq!(atan2_turns(0., 0.), 0.);
    assert_eq!(atan2_turns(-0., -0.), 0.);
}

#[test]
fn arc_edge_coverage_matches_area() {
    // (radius, circle) in pixels; a smaller circle is a border's inner arc.
    for (r, circle) in [(1, 1), (2, 2), (3, 3), (4, 4), (16, 16), (64, 64), (16, 13), (20, 4)] {
        let center = r as f64;
        for dist in r - circle..r {
            let line = dist as f64;
            let edge = ArcEdge::new(Shifted::new(r), Shifted::new(circle), Shifted::new(r - dist));
            let pixels = edge.pixels(Side::Left);
            for x in pixels.start.saturating_sub(1)..pixels.end + 1 {
                const SAMPLES: u32 = 64;
                let inside = (0..SAMPLES * SAMPLES)
                    .filter(|i| {
                        let px = x as f64 + ((i % SAMPLES) as f64 + 0.5) / SAMPLES as f64;
                        let py = line + ((i / SAMPLES) as f64 + 0.5) / SAMPLES as f64;
                        px >= center
                            || (px - center).powi(2) + (py - center).powi(2)
                                <= (circle as f64).powi(2)
                    })
                    .count();
                let exact = inside as f64 * 255. / (SAMPLES * SAMPLES) as f64;
                let coverage = if x < pixels.start {
                    0
                } else if x >= pixels.end {
                    255
                } else {
                    edge.coverage(x)
                };
                assert!(
                    (coverage as f64 - exact).abs() <= 26.,
                    "r {r}, circle {circle}, line {dist}, x {x}: {coverage} instead of {exact}"
                );
            }
        }
    }
}

#[test]
fn gradient_noise_matches_formula() {
    let fract = |v: f64| v - v.floor();
    for y in -40..40 {
        let mut noise = GradientNoise::new(-40, y);
        for x in -40..40 {
            let expected =
                fract(52.9829189 * fract(0.06711056 * x as f64 + 0.00583715 * y as f64)) * 256.;
            let diff = (noise.next() as f64 - expected).abs();
            assert!(diff.min(256. - diff) <= 2., "({x}, {y}): diff {diff}");
        }
    }
}

#[test]
fn rgb565_dither_keeps_representable_colors() {
    for noise in 0..=255 {
        for (r, g, b) in [(0, 0, 0), (0xf8, 0xfc, 0xf8), (0xff, 0xff, 0xff), (0x40, 0x84, 0x10)] {
            let c = PremultipliedRgbaColor { red: r, green: g, blue: b, alpha: 255 };
            let d = dither_rgb565(c, noise);
            assert_eq!(
                Rgb565Pixel::from_rgb(d.red, d.green, d.blue),
                Rgb565Pixel::from_rgb(r, g, b)
            );
        }
    }
}

#[test]
fn rgb565_dither_stays_premultiplied() {
    let c = PremultipliedRgbaColor { red: 20, green: 18, blue: 19, alpha: 20 };
    let d = dither_rgb565(c, 255);
    assert!(d.red <= d.alpha && d.green <= d.alpha && d.blue <= d.alpha);
}

#[cfg(test)]
fn linear_gradient<const N: usize>(
    stops: [(PremultipliedRgbaColor, f32); N],
    position: LinearGradientPosition,
    dx: LinearGradientPosition,
    dy: LinearGradientPosition,
) -> super::LinearGradientCommand {
    let stops = super::LinearGradientStop::collect(stops.into_iter(), dx);
    super::LinearGradientCommand { stops, position, dx, dy }
}

#[test]
fn rgb565_gradient_is_dithered() {
    let rect = PhysicalRect::from_size(super::PhysicalSize::new(256, 64));
    let black = PremultipliedRgbaColor { red: 0, green: 0, blue: 0, alpha: 255 };
    let gray = PremultipliedRgbaColor { red: 8, green: 8, blue: 8, alpha: 255 };
    let one = LinearGradientPosition::from_integer(1);
    // Line 32's center is at 32/64.
    let g = linear_gradient([(black, 0.), (gray, 1.)], -one / 128, Fixed(0), one / 64);
    // Halfway down, the color is about 3/8 of the way to the first non-black 565 red level.
    let line = PhysicalLength::new(32);

    let mut rgb565 = [Rgb565Pixel(0); 256];
    draw_linear_gradient(&rect, line, &g, &mut rgb565, 0);
    let lit = rgb565.iter().filter(|p| p.red() != 0).count();
    assert!((64..=128).contains(&lit), "{lit} of 256 pixels above black");

    let mut big_endian = [Rgb565BigEndianPixel(0); 256];
    draw_linear_gradient(&rect, line, &g, &mut big_endian, 0);
    assert!(big_endian.iter().zip(&rgb565).all(|(be, p)| u16::from_be(be.0) == p.0));

    let mut rgb8 = [Rgb8Pixel::new(0, 0, 0); 256];
    draw_linear_gradient(&rect, line, &g, &mut rgb8, 0);
    assert!(rgb8.iter().all(|p| *p == rgb8[0]));
}

#[test]
fn rgb565_linear_gradient_noise_follows_screen_position() {
    // Negative side clips make the ramp narrower than the rect,
    // so every line has a flat fill on both ends around the ramp.
    let rect = PhysicalRect::new(super::PhysicalPoint::new(5, 0), super::PhysicalSize::new(64, 64));
    let background = Rgb565Pixel(0x5aeb);
    let opaque = (
        PremultipliedRgbaColor { red: 0x13, green: 0x27, blue: 0x4a, alpha: 0xff },
        PremultipliedRgbaColor { red: 0xc5, green: 0x31, blue: 0x8e, alpha: 0xff },
    );
    let translucent = (
        PremultipliedRgbaColor { red: 0x13, green: 0x27, blue: 0x3a, alpha: 0x80 },
        PremultipliedRgbaColor { red: 0x45, green: 0x31, blue: 0x0e, alpha: 0x60 },
    );
    let expected_fill = |color: PremultipliedRgbaColor, x: i32, y: i32| {
        let c = dither_rgb565(color, GradientNoise::new(x, y).next());
        let mut pix = background;
        pix.blend(c);
        pix
    };

    for (color1, color2) in [opaque, translucent] {
        for reversed in [false, true] {
            // The ramp covers the middle half of the first line, and moves right on later lines.
            let one = LinearGradientPosition::from_integer(1);
            let stops = [(color1, 0.25), (color2, 0.75)];
            let g = if reversed {
                linear_gradient(stops, one - one / 128, -one / 64, -one / 512)
            } else {
                linear_gradient(stops, one / 128, one / 64, one / 512)
            };
            let (left_fill, right_fill) =
                if reversed { (color2, color1) } else { (color1, color2) };
            for y in 0..64 {
                let line = PhysicalLength::new(y as i16);
                let mut whole = [background; 64];
                draw_linear_gradient(&rect, line, &g, &mut whole, 0);
                assert_eq!(whole[0], expected_fill(left_fill, 5, y));
                assert_eq!(whole[63], expected_fill(right_fill, 5 + 63, y));

                for split in 1..64 {
                    let mut parts = [background; 64];
                    let (left, right) = parts.split_at_mut(split);
                    draw_linear_gradient(&rect, line, &g, left, 0);
                    draw_linear_gradient(&rect, line, &g, right, split as i16);
                    assert_eq!(parts, whole, "line {y}, split at {split}, reversed {reversed}");
                }
            }
        }
    }
}

#[test]
fn linear_gradient_segments_meet() {
    // #6247: neighboring segments used to leave a pixel between them not drawn.
    let rect = PhysicalRect::from_size(super::PhysicalSize::new(97, 1));
    let line = PhysicalLength::new(0);
    let color = |c| PremultipliedRgbaColor { red: c, green: c, blue: c, alpha: 255 };
    let unset = PremultipliedRgbaColor { red: 255, green: 0, blue: 255, alpha: 255 };
    let stops = [(color(255), 0.), (color(0x66), 0.5), (color(0), 1.)];
    let one = LinearGradientPosition::from_integer(1);

    for pixels_per_unit in [3, 7, 40, 97, 300, 2000] {
        for direction in [1, -1] {
            let dx = one * direction / pixels_per_unit;
            for start in -6..=26 {
                let g = linear_gradient(stops, one * start / 20, dx, Fixed(0));
                let mut whole = [unset; 97];
                draw_linear_gradient(&rect, line, &g, &mut whole, 0);
                let context =
                    alloc::format!("{pixels_per_unit} px per unit, dx {}, start {start}", dx.0);
                assert!(whole.iter().all(|p| p.green == p.red), "unset pixel, {context}");
                // The gradient darkens along its line, and never wraps around at 0 or 255.
                assert!(
                    whole.windows(2).all(|w| if dx.0 > 0 {
                        w[1].red <= w[0].red
                    } else {
                        w[1].red >= w[0].red
                    }),
                    "not monotonic, {context}: {:?}",
                    whole.map(|p| p.red)
                );

                for split in [1, 30, 48, 96] {
                    let mut parts = [unset; 97];
                    let (left, right) = parts.split_at_mut(split);
                    draw_linear_gradient(&rect, line, &g, left, 0);
                    draw_linear_gradient(&rect, line, &g, right, split as i16);
                    assert!(
                        parts.iter().zip(&whole).all(|(a, b)| a.red == b.red && a.green == b.green),
                        "split at {split}, {context}"
                    );
                }
            }
        }
    }
}

#[test]
fn linear_gradient_stop_before_previous_moves_to_it() {
    let color = |c| PremultipliedRgbaColor { red: c, green: c, blue: c, alpha: 255 };
    let one = LinearGradientPosition::from_integer(1);
    let g = linear_gradient(
        [(color(0), 0.5), (color(100), 0.25), (color(200), 1.)],
        Fixed(0),
        one / 64,
        Fixed(0),
    );
    let positions: alloc::vec::Vec<_> = g.stops.iter().map(|s| s.position).collect();
    assert_eq!(positions, [one / 2, one / 2, one]);
    // The first segment is empty, so it has nothing to step.
    assert_eq!((g.stops[0].factor_step, g.stops[0].step), (0, [0; 4]));
}

#[test]
fn blend_coverage_matches_blend() {
    fn check<T: TargetPixel, K: PartialEq + core::fmt::Debug>(dest: T, key: impl Fn(T) -> K) {
        for c in (0..=255u8).step_by(3) {
            let color = Color::from_rgb_u8(c, c.wrapping_mul(7), 255 - c);
            let opaque = T::from_rgb(color.red(), color.green(), color.blue());
            for coverage in [0, 0xff] {
                let mut expected = dest;
                expected.blend(PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                    coverage,
                    color.red(),
                    color.green(),
                    color.blue(),
                )));
                let mut actual = dest;
                blend_coverage(&mut actual, opaque, color, coverage);
                assert_eq!(key(actual), key(expected), "color {color:?}, coverage {coverage}");
            }
        }
    }
    for d in (0..=255u8).step_by(5) {
        check(Rgb8Pixel::new(d, 255 - d, d.wrapping_mul(3)), |p| p);
        check(PremultipliedRgbaColor { red: d / 2, green: d / 3, blue: d / 4, alpha: d }, |p| {
            (p.red, p.green, p.blue, p.alpha)
        });
        check(Rgb565Pixel::from_rgb(d, 255 - d, d.wrapping_mul(3)), |p| p);
        check(Rgb565BigEndianPixel::from_rgb(d, 255 - d, d.wrapping_mul(3)), |p| p);
    }
}

#[test]
fn fixed_position_matches_float() {
    let one = GradientPosition::from_integer(1);
    for i in 0..=100_000 {
        let position = i as f32 / 100_000.;
        let expected = Fixed((position as f64 * one.0 as f64).floor() as i32);
        assert_eq!(fixed_position(position), expected, "position {position}");
    }
    assert_eq!(fixed_position(-0.), Fixed(0));
    assert_eq!(fixed_position(-0.5), Fixed(0));
    assert_eq!(fixed_position(1.5), one);
    assert_eq!(fixed_position(f32::INFINITY), one);
    assert_eq!(fixed_position(f32::MIN_POSITIVE / 2.), Fixed(0));
}

#[test]
fn blend_stops_interpolates_like_float() {
    let color = |red, green, blue, alpha| PremultipliedRgbaColor { red, green, blue, alpha };
    let stops = [
        (color(0, 0, 0, 0), -0.25),
        (color(200, 10, 80, 255), 0.3),
        (color(20, 250, 0, 255), 0.3),
        (color(120, 60, 255, 255), 0.9),
        (color(30, 30, 30, 128), 2.),
    ];
    let fixed = PremultipliedGradientStop::collect(stops.iter().copied());
    for i in 0..=1000 {
        let position = i as f32 / 1000.;
        let mut actual = PremultipliedRgbaColor::default();
        blend_stops(&fixed, &mut actual, position, &mut GradientNoise::new(0, 0));

        let [(c1, p1), (c2, p2)] = *stops
            .array_windows()
            .find(|[(_, p1), (_, p2)]| position >= *p1 && position <= *p2)
            .unwrap();
        let t = if p1 == p2 { 0. } else { (position - p1) as f64 / (p2 - p1) as f64 };
        let lerp = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t).floor();
        let near = |actual: u8, a: u8, b: u8| (actual as f64 - lerp(a, b)).abs() <= 1.;
        assert!(
            near(actual.red, c1.red, c2.red)
                && near(actual.green, c1.green, c2.green)
                && near(actual.blue, c1.blue, c2.blue)
                && near(actual.alpha, c1.alpha, c2.alpha),
            "position {position}: {actual:?}"
        );
    }
}

#[test]
fn unscaled_texture_line_wraps_into_next_tile() {
    use super::{SceneTexture, SceneTextureExtra};

    // A 4×1 source drawn at 1:1 from x = 1.5 into 7 pixels samples these source pixels.
    const SAMPLED: [usize; 7] = [1, 2, 3, 0, 1, 2, 3];

    fn draw(format: TexturePixelFormat, data: &[u8]) -> [Rgb8Pixel; 7] {
        let texture = SceneTexture {
            data,
            format,
            pixel_stride: 4,
            extra: SceneTextureExtra {
                dx: Fixed::from_integer(1),
                dy: Fixed::from_integer(1),
                off_x: Fixed(24),
                off_y: Fixed(0),
                colorize: Color::default(),
                alpha: 0xff,
                rotation: Default::default(),
            },
        };
        let mut line = [Rgb8Pixel::default(); 7];
        draw_texture_line(
            &PhysicalRect::from_size(euclid::size2(7, 1)),
            PhysicalLength::new(0),
            &texture,
            &mut line,
            0,
            0,
        );
        line
    }

    let rgb = [0, 0, 0, 10, 20, 30, 40, 50, 60, 70, 80, 90];
    assert_eq!(
        draw(TexturePixelFormat::Rgb, &rgb),
        SAMPLED.map(|i| Rgb8Pixel::new(rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]))
    );

    #[cfg(feature = "image-pixel-format-rgb565")]
    {
        let rgb565: [u16; 4] = [0x0000, 0xf800, 0x07e0, 0x001f];
        let data: alloc::vec::Vec<u8> = rgb565.iter().flat_map(|p| p.to_ne_bytes()).collect();
        assert_eq!(
            draw(TexturePixelFormat::Rgb565, &data),
            SAMPLED.map(|i| Rgb565Pixel(rgb565[i]).into())
        );
    }

    #[cfg(feature = "image-pixel-format-gray8")]
    {
        let gray = [0, 85, 170, 255];
        assert_eq!(
            draw(TexturePixelFormat::Gray8, &gray),
            SAMPLED.map(|i| Rgb8Pixel::new(gray[i], gray[i], gray[i]))
        );
    }

    // Blended onto the black line buffer, so a pixel ends up as its premultiplied color.
    let rgba = [0, 0, 0, 0, 200, 100, 50, 255, 200, 100, 50, 128, 10, 20, 30, 255];
    assert_eq!(
        draw(TexturePixelFormat::Rgba, &rgba),
        SAMPLED.map(|i| {
            let p = &rgba[i * 4..][..4];
            let c =
                PremultipliedRgbaColor::premultiply(Color::from_argb_u8(p[3], p[0], p[1], p[2]));
            Rgb8Pixel::new(c.red, c.green, c.blue)
        })
    );

    let premultiplied = [0, 0, 0, 0, 200, 100, 50, 255, 100, 50, 25, 128, 10, 20, 30, 255];
    assert_eq!(
        draw(TexturePixelFormat::RgbaPremultiplied, &premultiplied),
        SAMPLED.map(|i| {
            let p = &premultiplied[i * 4..][..4];
            Rgb8Pixel::new(p[0], p[1], p[2])
        })
    );
}

#[test]
fn identical_clips_combine_to_one() {
    let clip = super::ShapeClip {
        shape: super::RoundedShape {
            radius: super::PhysicalBorderRadius::new_uniform(16),
            ..Default::default()
        },
        opaque_border: PhysicalLength::new(0),
    };
    const SIZE: i16 = 40;
    let rect = PhysicalRect::new(Default::default(), euclid::size2(SIZE, SIZE));
    let color = SolidColor(PremultipliedRgbaColor { red: 0, green: 0, blue: 255, alpha: 255 });
    // More clips than are cached, so the spans are recomputed for each edge chunk.
    let clips = [clip; CACHED_SPANS + 1];
    for line in 0..SIZE {
        let line = PhysicalLength::new(line);
        let mut one = [PremultipliedRgbaColor::default(); SIZE as usize];
        draw_clipped_line(&rect, line, &clips[..1], &color, &mut one, 0, 0);
        for count in 2..=clips.len() {
            let mut many = [PremultipliedRgbaColor::default(); SIZE as usize];
            draw_clipped_line(&rect, line, &clips[..count], &color, &mut many, 0, 0);
            assert_eq!(
                bytemuck::cast_slice::<_, u8>(&one),
                bytemuck::cast_slice::<_, u8>(&many),
                "line {line:?} with {count} clips"
            );
        }
    }
}

#[test]
fn clipped_line_does_not_depend_on_split() {
    let clip = |radius, left, top, opaque_border| super::ShapeClip {
        shape: super::RoundedShape {
            radius,
            left_clip: PhysicalLength::new(left),
            right_clip: PhysicalLength::new(3),
            top_clip: PhysicalLength::new(top),
            bottom_clip: PhysicalLength::new(0),
        },
        opaque_border: PhysicalLength::new(opaque_border),
    };
    let asymmetric = super::PhysicalBorderRadius::new(13, 6, 17, 9);
    let uniform = super::PhysicalBorderRadius::new_uniform(11);
    let many: [_; CACHED_SPANS + 1] =
        core::array::from_fn(|i| clip(uniform, i as i16, i as i16, 0));
    let runs: [&[_]; 3] =
        [&[clip(asymmetric, 2, 1, 0)], &[clip(uniform, 0, 0, 0), clip(asymmetric, 2, 1, 3)], &many];
    const SIZE: i16 = 48;
    let rect = PhysicalRect::new(Default::default(), euclid::size2(SIZE, SIZE));
    let color = SolidColor(PremultipliedRgbaColor { red: 0, green: 0, blue: 255, alpha: 255 });
    for (run, clips) in runs.iter().enumerate() {
        for line in 0..SIZE {
            let line = PhysicalLength::new(line);
            let mut whole = [PremultipliedRgbaColor::default(); SIZE as usize];
            draw_clipped_line(&rect, line, clips, &color, &mut whole, 0, 0);
            for split in 1..SIZE {
                let mut parts = [PremultipliedRgbaColor::default(); SIZE as usize];
                let (left, right) = parts.split_at_mut(split as usize);
                draw_clipped_line(&rect, line, clips, &color, left, 0, SIZE - split);
                draw_clipped_line(&rect, line, clips, &color, right, split, 0);
                assert_eq!(
                    bytemuck::cast_slice::<_, u8>(&whole),
                    bytemuck::cast_slice::<_, u8>(&parts),
                    "run {run}, line {line:?} split at {split}"
                );
            }
        }
    }
}
