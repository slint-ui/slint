// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore premultiply
#![allow(clippy::identity_op)] // We use x + 0 a lot here for symmetry

//! This is the module for the functions that are drawing the pixels
//! on the line buffer

use super::{Fixed, PhysicalLength, PhysicalRect};
use crate::scene::{GradientFactor, GradientPosition, PremultipliedGradientStop};
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
        extra: super::SceneTextureExtra { colorize, alpha, rotation, dx, dy, off_x, off_y, smooth },
    } = *texture;

    let source_size = texture.source_size().cast::<i32>();
    let len = line_buffer.len();
    let y = line - span.origin.y_length();
    let y = if rotation.mirror_width() { span.size.height - y.get() - 1 } else { y.get() } as i32;

    let off_y = Fixed::<i32, 8>::from_fixed(off_y);
    let dx = Fixed::<i32, 8>::from_fixed(dx);
    let dy = Fixed::<i32, 8>::from_fixed(dy);
    let off_x = Fixed::<i32, 8>::from_fixed(off_x);

    let smooth = smooth
        && (dx != Fixed::from_integer(1) || dy != Fixed::from_integer(1))
        && format != TexturePixelFormat::SignedDistanceField;
    // Bilinear sampling reads at the target pixel's center: `(i + 0.5) * d - 0.5`
    let half_x = Fixed::<i32, 8>((dx.0 - (1 << 8)) / 2);
    let half_y = Fixed::<i32, 8>((dy.0 - (1 << 8)) / 2);

    if !rotation.is_transpose() {
        let mut delta = dx;
        let row = off_y + dy * y;
        // The position where to start in the image array for a this row
        let row_offset = (row.truncate() % source_size.height) as usize * pixel_stride as usize;
        let mut tile_start = 0;

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
            pos = o;
            tile_start = source_size.width;
            end = (o / delta) as usize + 1;
            acc_err = -delta + o % delta;
            delta = -delta;
            remainder = -remainder;
        } else {
            let o =
                (off_x + delta * extra_clip_begin as i32) % Fixed::from_integer(source_size.width);
            pos = o;
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
        let stride = pixel_stride as usize * format.bpp();
        let (smooth_row, fy, below) = if smooth {
            bilinear_axis(
                (row + half_y) % Fixed::from_integer(source_size.height),
                source_size.height,
                stride,
            )
        } else {
            Default::default()
        };
        while begin < len {
            if smooth {
                fetch_blend_pixel_smooth(
                    &mut line_buffer[begin..end],
                    format,
                    data,
                    alpha,
                    colorize,
                    #[inline(always)]
                    |bpp| {
                        let (x, fx, right) = bilinear_axis(pos + half_x, source_size.width, bpp);
                        pos += delta;
                        BilinearPos { pos: smooth_row * stride + x * bpp, right, below, fx, fy }
                    },
                );
            } else {
                fetch_blend_pixel(
                    &mut line_buffer[begin..end],
                    format,
                    data,
                    alpha,
                    colorize,
                    (pixel_stride as usize, dy),
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
        let bpp = format.bpp();
        let col = off_x + dx * y;
        let col_fract = col.fract();
        let (smooth_col, fx, right) = if smooth {
            bilinear_axis(
                (col + half_x) % Fixed::from_integer(source_size.width),
                source_size.width,
                bpp,
            )
        } else {
            Default::default()
        };
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
            if smooth {
                fetch_blend_pixel_smooth(
                    &mut line_buffer[begin..end],
                    format,
                    data,
                    alpha,
                    colorize,
                    #[inline(always)]
                    |_| {
                        let (y, fy, below) =
                            bilinear_axis(row + half_y, source_size.height, stride);
                        row += row_delta;
                        BilinearPos { pos: y * stride + smooth_col * bpp, right, below, fx, fy }
                    },
                );
            } else {
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
            }
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
                        match ((p[3] as u16 * alpha as u16) / 255) as u8 {
                            0 => {}
                            0xff => *pix = TargetPixel::from_rgb(p[0], p[1], p[2]),
                            alpha => {
                                pix.blend(PremultipliedRgbaColor::premultiply(
                                    Color::from_argb_u8(alpha, p[0], p[1], p[2]),
                                ));
                            }
                        }
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
                        let p: &[u8] = &data[pos..pos + 4];
                        match p[3] {
                            0 => {}
                            0xff => *pix = TargetPixel::from_rgb(p[0], p[1], p[2]),
                            alpha => pix.blend(PremultipliedRgbaColor {
                                alpha,
                                red: p[0],
                                green: p[1],
                                blue: p[2],
                            }),
                        }
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

/// The four texels around a sample point, for bilinear filtering.
#[derive(Clone, Copy)]
struct BilinearPos {
    /// Byte offset of the top-left texel
    pos: usize,
    /// Byte offsets from `pos` to the texel on the right and below, 0 at the texture's edge
    right: usize,
    below: usize,
    fx: u8,
    fy: u8,
}

impl BilinearPos {
    #[inline(always)]
    fn sample<const N: usize>(self, data: &[u8], texel: impl Fn(&[u8]) -> [u32; N]) -> [u32; N] {
        let (fx, fy) = (self.fx as u32, self.fy as u32);
        // Derived from `w11` so that the weights sum to exactly 256.
        let w11 = (fx * fy) >> 8;
        let w10 = fx - w11;
        let w01 = fy - w11;
        let w00 = 256 + w11 - fx - fy;
        let t00 = texel(&data[self.pos..]);
        let t10 = texel(&data[self.pos + self.right..]);
        let t01 = texel(&data[self.pos + self.below..]);
        let t11 = texel(&data[self.pos + self.below + self.right..]);
        core::array::from_fn(|i| (t00[i] * w00 + t10[i] * w10 + t01[i] * w01 + t11[i] * w11) >> 8)
    }
}

/// Returns the texel index, the fraction towards the next texel, and `step` to reach it,
/// clamping to the edge instead of reading past it.
#[inline(always)]
fn bilinear_axis(v: Fixed<i32, 8>, size: i32, step: usize) -> (usize, u8, usize) {
    let v = v.max(Fixed::default());
    let i = v.truncate();
    if i + 1 < size { (i as usize, v.fract(), step) } else { ((size - 1) as usize, 0, 0) }
}

fn fetch_blend_pixel_smooth(
    line_buffer: &mut [impl TargetPixel],
    format: TexturePixelFormat,
    data: &[u8],
    alpha: u8,
    color: Color,
    mut pos: impl FnMut(usize) -> BilinearPos,
) {
    let opaque = TargetPixel::from_rgb(color.red(), color.green(), color.blue());
    match format {
        TexturePixelFormat::Rgb => {
            for pix in line_buffer {
                let [r, g, b] = pos(3).sample(data, |p| [p[0] as u32, p[1] as u32, p[2] as u32]);
                blend_premultiplied(pix, [r, g, b, 0xff], alpha);
            }
        }
        #[cfg(feature = "image-pixel-format-rgb565")]
        TexturePixelFormat::Rgb565 => {
            for pix in line_buffer {
                let [r, g, b] = pos(2).sample(data, |p| {
                    let p = Rgb565Pixel(u16::from_ne_bytes([p[0], p[1]]));
                    [p.red() as u32, p.green() as u32, p.blue() as u32]
                });
                blend_premultiplied(pix, [r, g, b, 0xff], alpha);
            }
        }
        TexturePixelFormat::Rgba if color.alpha() == 0 => {
            // Premultiplies each texel, so that transparent texels don't bleed their color.
            for pix in line_buffer {
                let [r, g, b, a] = pos(4).sample(data, |p| {
                    let a = p[3] as u32;
                    [p[0] as u32 * a, p[1] as u32 * a, p[2] as u32 * a, a]
                });
                blend_premultiplied(pix, [r / 255, g / 255, b / 255, a], alpha);
            }
        }
        TexturePixelFormat::RgbaPremultiplied if color.alpha() == 0 => {
            for pix in line_buffer {
                let c =
                    pos(4).sample(data, |p| [p[0] as u32, p[1] as u32, p[2] as u32, p[3] as u32]);
                blend_premultiplied(pix, c, alpha);
            }
        }
        TexturePixelFormat::Rgba | TexturePixelFormat::RgbaPremultiplied => {
            for pix in line_buffer {
                let [a] = pos(4).sample(data, |p| [p[3] as u32]);
                blend_coverage(pix, opaque, color, (a * alpha as u32 / 255) as u8);
            }
        }
        TexturePixelFormat::AlphaMap => {
            for pix in line_buffer {
                let [a] = pos(1).sample(data, |p| [p[0] as u32]);
                blend_coverage(pix, opaque, color, (a * alpha as u32 / 255) as u8);
            }
        }
        #[cfg(feature = "image-pixel-format-gray8")]
        TexturePixelFormat::Gray8 => {
            for pix in line_buffer {
                let [v] = pos(1).sample(data, |p| [p[0] as u32]);
                blend_premultiplied(pix, [v, v, v, 0xff], alpha);
            }
        }
        TexturePixelFormat::SignedDistanceField => {
            unreachable!("signed distance fields always interpolate")
        }
    }
}

/// Blends a premultiplied `[r, g, b, a]`, scaled by `alpha`.
#[inline(always)]
fn blend_premultiplied(pix: &mut impl TargetPixel, mut c: [u32; 4], alpha: u8) {
    if alpha != 0xff {
        c = c.map(|v| v * alpha as u32 / 255);
    }
    let [red, green, blue, alpha] = c.map(|v| v as u8);
    match alpha {
        0 => {}
        0xff => *pix = TargetPixel::from_rgb(red, green, blue),
        _ => pix.blend(PremultipliedRgbaColor { red, green, blue, alpha }),
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
#[derive(Clone, Copy, PartialEq, Ord, PartialOrd, Eq, Add, Sub, Mul)]
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
    fn floor(self) -> u32 {
        self.0 >> 4
    }
    #[inline(always)]
    fn ceil(self) -> u32 {
        (self.0 + Self::ONE.0 - 1) >> 4
    }
    #[inline(always)]
    fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
    #[inline(always)]
    fn sqrt(self) -> Self {
        Self(self.0.isqrt())
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

/// Where a pixel line crosses a circle of radius `circle`,
/// centered `r` in from both edges of a corner.
/// Returns `(x1, x2)` measured from the vertical edge,
/// at the line's sides nearer to and farther from the center.
/// `y` is `r` minus the line's distance from the horizontal edge.
/// Each crossing is `r - √(circle² - y²)`, from the circle equation.
fn arc_crossing(r: Shifted, circle: Shifted, y: Shifted) -> (Shifted, Shifted) {
    let x1 = r - (circle * circle).saturating_sub((y - Shifted::ONE) * (y - Shifted::ONE)).sqrt();
    let x2 = r - (circle * circle).saturating_sub(y * y).sqrt();
    (x1, x2)
}

/// Coverage, between 0 and 255, of pixel `x` by a shape whose edge goes from `x1` to `x2`
/// across the pixel line, with the shape to the right of the edge.
/// This interpolates linearly, which isn't exact, but good enough.
fn edge_coverage(x: u32, x1: Shifted, x2: Shifted) -> u32 {
    ((Shifted::ONE + Shifted::new(x) - x1).0 << 8) / (Shifted::ONE + x2 - x1).0
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
    // doesn't change the ramp; only `to_buffer` and `anti_alias` map to buffer indices.
    let left_clip = shape.left_clip.get().saturating_add(extra_left_clip) as u32;
    let to_buffer = |x: u32| x.saturating_sub(left_clip).min(width as u32) as usize;
    let right_edge = Shifted::new(
        left_clip + width as u32 + shape.right_clip.get().saturating_add(extra_right_clip) as u32,
    );
    let anti_alias = |x1: Shifted, x2: Shifted, process_pixel: &mut dyn FnMut(usize, u32)| {
        // x1 and x2 are the coordinate on the top and bottom of the intersection of the pixel
        // line and the curve.
        // `process_pixel` be called for the coordinate in the array and a coverage between 0..255
        for x in x1.floor().max(left_clip)..x2.ceil().min(left_clip + width as u32) {
            process_pixel((x - left_clip) as usize, edge_coverage(x, x1, x2));
        }
    };
    let rev = |x: Shifted| right_edge.saturating_sub(x);
    let calculate_xxxx = |r: i16, y: i16| {
        if r == 0 {
            return (Shifted::ZERO, Shifted::ZERO, border, border);
        }
        let r = Shifted::new(r);
        let y = r - Shifted::new(y);
        let (x1, x2) = arc_crossing(r, r, y);
        if border == Shifted::ZERO {
            return (x1, x2, x1, x2);
        }
        let (x3, x4) = arc_crossing(r, r.saturating_sub(border), y);
        (x1, x2, x3, x4)
    };

    let (x1, x2, x3, x4) = calculate_xxxx(left_radius, left_y);
    let (x8, x7, x6, x5) = if (right_radius, right_y) == (left_radius, left_y) {
        (x1, x2, x3, x4)
    } else {
        calculate_xxxx(right_radius, right_y)
    };
    let (x5, x6, x7, x8) = (rev(x5), rev(x6), rev(x7), rev(x8));
    // A corner wider than half the rectangle reaches past the opposite side's border.
    let x4 = x4.min(x6);
    let x3 = x3.min(x4);
    let x5 = x5.max(x3);
    let x6 = x6.max(x5);
    anti_alias(x1, x2, &mut |x, cov| {
        let c = if border == Shifted::ZERO { rr.inner_color } else { rr.border_color };
        let col = PremultipliedRgbaColor {
            alpha: (((c.alpha as u32) * cov as u32) / 255) as u8,
            red: (((c.red as u32) * cov as u32) / 255) as u8,
            green: (((c.green as u32) * cov as u32) / 255) as u8,
            blue: (((c.blue as u32) * cov as u32) / 255) as u8,
        };
        line_buffer[x].blend(col);
    });
    if y < rr.width.get() {
        // up or down border (x2 .. x7)
        let l = to_buffer(x2.ceil());
        let r = to_buffer(x7.floor());
        if l < r {
            TargetPixel::blend_slice(&mut line_buffer[l..r], rr.border_color)
        }
    } else {
        if border > Shifted::ZERO {
            // 3. draw the border (between x2 and x3)
            if Shifted::ONE + x2 <= x3 {
                TargetPixel::blend_slice(
                    &mut line_buffer[to_buffer(x2.ceil())..to_buffer(x3.floor())],
                    rr.border_color,
                )
            }
            // 4. anti-aliasing for the contents (x3 .. x4)
            anti_alias(x3, x4, &mut |x, cov| {
                let col = interpolate_color(cov, rr.border_color, rr.inner_color);
                line_buffer[x].blend(col);
            });
        }
        if rr.inner_color.alpha > 0 {
            // 5. inside (x4 .. x5)
            let begin = to_buffer(x4.ceil());
            let end = to_buffer(x5.floor());
            if begin < end {
                TargetPixel::blend_slice(&mut line_buffer[begin..end], rr.inner_color)
            }
        }
        if border > Shifted::ZERO {
            // 6. border anti-aliasing: x5..x6
            anti_alias(x5, x6, &mut |x, cov| {
                let col = interpolate_color(cov, rr.inner_color, rr.border_color);
                line_buffer[x].blend(col)
            });
            // 7. border x6 .. x7
            if Shifted::ONE + x6 <= x7 {
                TargetPixel::blend_slice(
                    &mut line_buffer[to_buffer(x6.ceil())..to_buffer(x7.floor())],
                    rr.border_color,
                )
            }
        }
    }
    anti_alias(x7, x8, &mut |x, cov| {
        let c = if border == Shifted::ZERO { rr.inner_color } else { rr.border_color };
        let col = PremultipliedRgbaColor {
            alpha: (((c.alpha as u32) * (255 - cov) as u32) / 255) as u8,
            red: (((c.red as u32) * (255 - cov) as u32) / 255) as u8,
            green: (((c.green as u32) * (255 - cov) as u32) / 255) as u8,
            blue: (((c.blue as u32) * (255 - cov) as u32) / 255) as u8,
        };
        line_buffer[x].blend(col);
    });
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

/// Taken as `dyn` by [`draw_gradient_line`] so it's only instantiated once per pixel type.
pub(super) trait GradientCommand<T: TargetPixel> {
    fn clip(&self) -> &super::GradientClip;
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

impl<T: TargetPixel> GradientCommand<T> for super::LinearGradientCommand {
    fn clip(&self) -> &super::GradientClip {
        &self.clip
    }
    fn draw_line(
        &self,
        rect: &PhysicalRect,
        line: PhysicalLength,
        buffer: &mut [T],
        extra_left_clip: i16,
        _extra_right_clip: i16,
    ) {
        draw_linear_gradient(rect, line, self, buffer, extra_left_clip)
    }
    fn draw_scratch_line(
        &self,
        rect: &PhysicalRect,
        line: PhysicalLength,
        buffer: &mut [PremultipliedRgbaColor],
        extra_left_clip: i16,
        _extra_right_clip: i16,
    ) {
        draw_linear_gradient(rect, line, self, buffer, extra_left_clip)
    }
}

impl<T: TargetPixel> GradientCommand<T> for super::RadialGradientCommand {
    fn clip(&self) -> &super::GradientClip {
        &self.clip
    }
    fn draw_line(
        &self,
        rect: &PhysicalRect,
        line: PhysicalLength,
        buffer: &mut [T],
        extra_left_clip: i16,
        extra_right_clip: i16,
    ) {
        draw_radial_gradient(rect, line, self, buffer, extra_left_clip, extra_right_clip)
    }
    fn draw_scratch_line(
        &self,
        rect: &PhysicalRect,
        line: PhysicalLength,
        buffer: &mut [PremultipliedRgbaColor],
        extra_left_clip: i16,
        extra_right_clip: i16,
    ) {
        draw_radial_gradient(rect, line, self, buffer, extra_left_clip, extra_right_clip)
    }
}

impl<T: TargetPixel> GradientCommand<T> for super::ConicGradientCommand {
    fn clip(&self) -> &super::GradientClip {
        &self.clip
    }
    fn draw_line(
        &self,
        rect: &PhysicalRect,
        line: PhysicalLength,
        buffer: &mut [T],
        extra_left_clip: i16,
        extra_right_clip: i16,
    ) {
        draw_conic_gradient(rect, line, self, buffer, extra_left_clip, extra_right_clip)
    }
    fn draw_scratch_line(
        &self,
        rect: &PhysicalRect,
        line: PhysicalLength,
        buffer: &mut [PremultipliedRgbaColor],
        extra_left_clip: i16,
        extra_right_clip: i16,
    ) {
        draw_conic_gradient(rect, line, self, buffer, extra_left_clip, extra_right_clip)
    }
}

/// Draw one line of a gradient, clipped to its rounded shape with anti-aliased corners.
pub(super) fn draw_gradient_line<T: TargetPixel>(
    rect: &PhysicalRect,
    line: PhysicalLength,
    g: &dyn GradientCommand<T>,
    buffer: &mut [T],
    extra_left_clip: i16,
    extra_right_clip: i16,
) {
    let clip = g.clip();
    let shape = &clip.shape;
    let ((left_radius, left_y), (right_radius, right_y), _) = if shape.radius.is_zero() {
        ((0, 0), (0, 0), 0)
    } else {
        corner_radii_on_line(rect, line, shape)
    };
    if left_radius == 0 && right_radius == 0 {
        g.draw_line(rect, line, buffer, extra_left_clip, extra_right_clip);
        return;
    }

    let len = buffer.len();
    // The edges are computed relative to the shape's left edge rather than the buffer,
    // so the coverage doesn't depend on where the dirty region splits the line.
    let left_offset = shape.left_clip.get().saturating_add(extra_left_clip) as u32;
    let right_offset = shape.right_clip.get().saturating_add(extra_right_clip) as u32;
    let shape_width = Shifted::new(len as u32 + left_offset + right_offset);
    let border = Shifted::new(clip.opaque_border.get());
    let arc = |r: i16, y: i16| {
        if r == 0 {
            return (Shifted::ZERO, Shifted::ZERO);
        }
        let r = Shifted::new(r);
        arc_crossing(r, r.saturating_sub(border), r - Shifted::new(y))
    };
    let (l1, l2) = arc(left_radius, left_y);
    let (r2, r1) = if (right_radius, right_y) == (left_radius, left_y) {
        (l1, l2)
    } else {
        arc(right_radius, right_y)
    };
    let (r1, r2) = (shape_width.saturating_sub(r1), shape_width.saturating_sub(r2));

    let to_buffer = |x: u32| (x.saturating_sub(left_offset) as usize).min(len);
    let begin = to_buffer(l1.floor());
    let end = to_buffer(r2.ceil());
    let clips = |range: &core::ops::Range<usize>| {
        (extra_left_clip + range.start as i16, extra_right_clip + (len - range.end) as i16)
    };

    if border > Shifted::ZERO {
        // `draw_rounded_rectangle_line` anti-aliases the border's inner edge over this same
        // span, so the gradient must have full coverage in it.
        let inner = begin..end.max(begin);
        let (left_clip, right_clip) = clips(&inner);
        g.draw_line(rect, line, &mut buffer[inner], left_clip, right_clip);
        return;
    }

    let inner_begin = to_buffer(l2.ceil()).clamp(begin, end);
    let inner_end = to_buffer(r1.floor()).clamp(inner_begin, end);

    let inner = inner_begin..inner_end;
    let (left_clip, right_clip) = clips(&inner);
    g.draw_line(rect, line, &mut buffer[inner], left_clip, right_clip);

    // On a narrow shape both arcs can cross the same pixel, so take the smaller coverage.
    let coverage = |x: usize| {
        let x = x as u32 + left_offset;
        let left = if x >= l2.ceil() { 255 } else { edge_coverage(x, l1, l2) };
        let right = if x < r1.floor() { 255 } else { 255 - edge_coverage(x, r1, r2) };
        left.min(right)
    };
    for edge in [begin..inner_begin, inner_end..end] {
        blend_with_coverage(
            buffer,
            edge,
            |scratch, chunk| {
                let (left_clip, right_clip) = clips(&chunk);
                g.draw_scratch_line(rect, line, scratch, left_clip, right_clip);
            },
            coverage,
        );
    }
}

/// `coverage(x)` is 0 to 255.
/// `draw` draws the gradient for a sub-range of `range` into a cleared scratch buffer.
pub(super) fn blend_with_coverage<T: TargetPixel>(
    buffer: &mut [T],
    range: core::ops::Range<usize>,
    mut draw: impl FnMut(&mut [PremultipliedRgbaColor], core::ops::Range<usize>),
    coverage: impl Fn(usize) -> u32,
) {
    // Bounds the stack scratch buffer.
    const CHUNK: usize = 16;
    let mut scratch = [PremultipliedRgbaColor::default(); CHUNK];
    for start in range.clone().step_by(CHUNK) {
        let chunk = start..(start + CHUNK).min(range.end);
        let scratch = &mut scratch[..chunk.len()];
        scratch.fill(PremultipliedRgbaColor::default());
        draw(scratch, chunk.clone());
        for (x, color) in chunk.zip(scratch.iter()) {
            let color = interpolate_color(coverage(x), PremultipliedRgbaColor::default(), *color);
            buffer[x].blend(color);
        }
    }
}

#[cfg(feature = "path")]
impl super::AnyGradientCommand {
    pub(super) fn as_command<T: TargetPixel>(&self) -> &dyn GradientCommand<T> {
        match self {
            Self::Linear(g) => g,
            Self::Radial(g) => g,
            Self::Conic(g) => g,
        }
    }
}

impl super::LinearGradientCommand {
    fn invert_slope(&self) -> bool {
        self.flags & 0b1 != 0
    }

    /// Whether the band fills past its stop with (color1, color2).
    fn fills_beyond_stops(&self) -> (bool, bool) {
        (self.flags & 0b010 != 0, self.flags & 0b100 != 0)
    }

    /// Whether the band fills past its stop on its (left, right) side.
    fn fills_beyond_edges(&self) -> (bool, bool) {
        let (fill_col1, fill_col2) = self.fills_beyond_stops();
        if self.invert_slope() { (fill_col1, fill_col2) } else { (fill_col2, fill_col1) }
    }

    /// The band's (y, height) on `line`, measured from the band's top.
    fn band_y(&self, rect: &PhysicalRect, line: PhysicalLength) -> (i32, i32) {
        let top_clip = self.top_clip.get() as i32;
        let y = line.get() as i32 - rect.min_y() as i32 + top_clip;
        let size_y = rect.height() as i32 + top_clip + self.bottom_clip.get() as i32;
        (y, size_y)
    }

    /// The position (0 to 255) in a vertical band on `line`, or `None` where the band draws nothing.
    fn vertical_position(&self, rect: &PhysicalRect, line: PhysicalLength) -> Option<i32> {
        debug_assert_eq!(self.start, 0);
        let (y, size_y) = self.band_y(rect, line);
        let p = 255 * y / size_y;
        let (fill_col1, fill_col2) = self.fills_beyond_stops();
        ((fill_col1 || p >= 0) && (fill_col2 || p < 255)).then_some(p)
    }

    /// Whether [`draw_linear_gradient`] overwrites every pixel of `rect` on `line`.
    pub(super) fn covers_line(&self, rect: &PhysicalRect, line: PhysicalLength) -> bool {
        if self.color1.alpha != 255 || self.color2.alpha != 255 || !self.clip.shape.radius.is_zero()
        {
            return false;
        }
        match self.start {
            0 => self.vertical_position(rect, line).is_some(),
            255 => {
                let (fill_left, fill_right) = self.fills_beyond_edges();
                (fill_left || self.left_clip.get() >= 0)
                    && (fill_right || self.right_clip.get() >= 0)
            }
            _ => false,
        }
    }
}

fn draw_linear_gradient(
    rect: &PhysicalRect,
    line: PhysicalLength,
    g: &super::LinearGradientCommand,
    mut buffer: &mut [impl TargetPixel],
    extra_left_clip: i16,
) {
    let noise = GradientNoise::new(rect.min_x() as i32 + extra_left_clip as i32, line.get() as i32);
    let invert_slope = g.invert_slope();
    let (fill_left, fill_right) = g.fills_beyond_edges();

    let (y, size_y) = g.band_y(rect, line);
    let start = g.start as i32;

    let (mut color1, mut color2) = (g.color1, g.color2);

    if g.start == 0 {
        if let Some(p) = g.vertical_position(rect, line) {
            let col = interpolate_color(p.clamp(0, 255) as u32, color1, color2);
            blend_gradient_slice(buffer, col, noise);
        }
        return;
    }

    let size_x = rect.width() as i32 + g.left_clip.get() as i32 + g.right_clip.get() as i32;

    let k = size_x * (255 - start);
    let slope_x = |y: i32| match y.checked_mul(k) {
        Some(n) => n / (size_y * start),
        // A band taller than a few thousand pixels.
        None => (y as i64 * k as i64 / (size_y * start) as i64) as i32,
    };
    let mut x = slope_x(if invert_slope { y } else { size_y - y })
        + g.left_clip.get() as i32
        + extra_left_clip as i32;

    let len = ((255 * size_x) / start) as usize;

    let (left_color, right_color) =
        if invert_slope { (g.color1, g.color2) } else { (g.color2, g.color1) };

    let mut offset = 0;
    if x < 0 {
        let l = (-x as usize).min(buffer.len());
        if fill_left {
            blend_gradient_slice(&mut buffer[..l], left_color, noise);
        }
        buffer = &mut buffer[l..];
        offset += l;
        x = 0;
    }

    if buffer.len() + x as usize > len {
        let l = len.saturating_sub(x as usize);
        if fill_right {
            blend_gradient_slice(&mut buffer[l..], right_color, noise.skip(offset + l));
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

    let mut noise = noise.skip(offset);
    for pix in buffer {
        let color = PremultipliedRgbaColor {
            red: (r >> 15) as u8,
            green: (g >> 15) as u8,
            blue: (b >> 15) as u8,
            alpha: (a >> 15) as u8,
        };
        pix.blend_dithered(color, noise.next(), InternalToken);
        r = r.wrapping_add(dr as _);
        g = g.wrapping_add(dg as _);
        b = b.wrapping_add(db as _);
        a = a.wrapping_add(da as _);
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

#[test]
fn rgb565_gradient_is_dithered() {
    let rect = PhysicalRect::from_size(super::PhysicalSize::new(256, 64));
    let black = PremultipliedRgbaColor { red: 0, green: 0, blue: 0, alpha: 255 };
    let gray = PremultipliedRgbaColor { red: 8, green: 8, blue: 8, alpha: 255 };
    let g = super::LinearGradientCommand {
        color1: black,
        color2: gray,
        start: 0,
        flags: 0b110,
        left_clip: PhysicalLength::new(0),
        right_clip: PhysicalLength::new(0),
        top_clip: PhysicalLength::new(0),
        bottom_clip: PhysicalLength::new(0),
        clip: Default::default(),
    };
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
        for invert_slope in [0, 1] {
            let g = super::LinearGradientCommand {
                color1,
                color2,
                start: 200,
                flags: 0b110 | invert_slope,
                left_clip: PhysicalLength::new(-16),
                right_clip: PhysicalLength::new(-16),
                top_clip: PhysicalLength::new(0),
                bottom_clip: PhysicalLength::new(0),
                clip: Default::default(),
            };
            let (left_fill, right_fill) =
                if invert_slope == 1 { (color1, color2) } else { (color2, color1) };
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
                    assert_eq!(parts, whole, "line {y}, split at {split}, invert {invert_slope}");
                }
            }
        }
    }
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
