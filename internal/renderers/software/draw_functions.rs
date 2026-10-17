// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore premultiply
#![allow(clippy::identity_op)] // We use x + 0 a lot here for symmetry

//! This is the module for the functions that are drawing the pixels
//! on the line buffer

use super::{Fixed, PhysicalLength, PhysicalRect};
use crate::scene::PremultipliedGradientStop;
use derive_more::{Add, Mul, Sub};
use i_slint_core::Color;
use i_slint_core::graphics::{Rgb8Pixel, TexturePixelFormat};
use i_slint_core::lengths::{PointLengths, SizeLengths};
#[allow(unused_imports)]
use num_traits::Float;

/// Draw one line of the texture in the line buffer
///
#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), allow(unsafe_code))]
#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), unsafe(link_section = ".data.ram_func"))]
#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), inline(never))]
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

    let unit_sampling = match format {
        TexturePixelFormat::AlphaMap => true,
        #[cfg(feature = "image-pixel-format-rgb565")]
        TexturePixelFormat::Rgb565 => true,
        _ => false,
    };
    if unit_sampling
        && rotation == super::RenderingRotation::NoRotation
        && dx == Fixed::from_integer(1)
        && dy == Fixed::from_integer(1)
    {
        let source_y = off_y.truncate() + y;
        let source_x = off_x.truncate() + extra_clip_begin as i32;
        if source_y >= 0
            && source_y < source_size.height
            && source_x >= 0
            && source_x as usize + len <= source_size.width as usize
        {
            let bpp = format.bpp();
            let mut position =
                (source_y as usize * pixel_stride as usize + source_x as usize) * bpp;
            if format == TexturePixelFormat::AlphaMap {
                blend_alpha_map_line(line_buffer, &data[position..position + len], alpha, colorize);
                return;
            }
            #[cfg(feature = "image-pixel-format-rgb565")]
            if format == TexturePixelFormat::Rgb565 && alpha == u8::MAX {
                TargetPixel::copy_from_rgb565(line_buffer, &data[position..position + len * 2]);
                return;
            }
            fetch_blend_pixel(
                line_buffer,
                format,
                data,
                alpha,
                colorize,
                (pixel_stride as usize, dy),
                |_| {
                    let result = (position, 0, 0);
                    position += bpp;
                    result
                },
            );
            return;
        }
    }

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
                    let p = ((row_offset + pos.truncate() as usize) * bpp, pos.fract(), row_fract);
                    pos += delta;
                    p
                },
            );
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

    #[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), inline(always))]
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
                        let alpha = ((p[3] as u16 * alpha as u16) / 255) as u8;
                        if alpha == 0xff {
                            *pix = TargetPixel::from_rgb(p[0], p[1], p[2]);
                        } else {
                            pix.blend(PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                                alpha, p[0], p[1], p[2],
                            )));
                        }
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
                    let alpha = ((data[pos] as u16 * alpha as u16) / 255) as u8;
                    if alpha == 0 {
                        continue;
                    }
                    if alpha == u8::MAX {
                        *pix = TargetPixel::from_rgb(color.red(), color.green(), color.blue());
                        continue;
                    }
                    let c = PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                        alpha,
                        color.red(),
                        color.green(),
                        color.blue(),
                    ));
                    pix.blend(c);
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

#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), allow(unsafe_code))]
#[cfg_attr(
    all(feature = "embedded-ram-functions", target_arch = "arm"),
    unsafe(link_section = ".data.ram_func")
)]
#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), inline(never))]
fn blend_alpha_map_line<P: TargetPixel>(
    pixels: &mut [P],
    coverage: &[u8],
    alpha: u8,
    color: Color,
) {
    let opaque = P::from_rgb(color.red(), color.green(), color.blue());
    for (pixel, coverage) in pixels.iter_mut().zip(coverage) {
        let alpha = (*coverage as u16 * alpha as u16 / 255) as u8;
        if alpha == 0 {
            continue;
        }
        if alpha == u8::MAX {
            *pixel = opaque;
        } else {
            pixel.blend(PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                alpha,
                color.red(),
                color.green(),
                color.blue(),
            )));
        }
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
    let y1 = (line - span.origin.y_length()) + shape.top_clip;
    let y2 = (span.origin.y_length() + span.size.height_length() - line) + shape.bottom_clip
        - PhysicalLength::new(1);
    let y = y1.min(y2).get();
    debug_assert!(y >= 0);
    let r = &shape.radius;
    let left = if y1.get() < r.top_left {
        (r.top_left, y1.get())
    } else if y2.get() < r.bottom_left {
        (r.bottom_left, y2.get())
    } else {
        (0, 0)
    };
    let right = if y1.get() < r.top_right {
        (r.top_right, y1.get())
    } else if y2.get() < r.bottom_right {
        (r.bottom_right, y2.get())
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

#[cfg(all(test, feature = "image-pixel-format-rgb565"))]
#[test]
fn unit_rgb565_sampling_preserves_offsets_clipping_and_alpha() {
    fn check<P: TargetPixel + bytemuck::Pod>(initial: P) {
        let mut storage = [0u8; 49];
        for (i, bytes) in storage[1..].chunks_exact_mut(2).enumerate() {
            bytes.copy_from_slice(&((i as u16).wrapping_mul(2707)).to_ne_bytes());
        }
        let data = &storage[1..];
        let span =
            PhysicalRect::new(super::PhysicalPoint::new(3, 7), super::PhysicalSize::new(6, 4));
        for off_y in 0..3 {
            for off_x in 0..4 {
                for extra in 0..3 {
                    let source_x = off_x + extra;
                    let count = 6 - source_x;
                    for alpha in [0, 1, 63, 127, 254, 255] {
                        let texture = super::SceneTexture {
                            data,
                            pixel_stride: 6,
                            format: TexturePixelFormat::Rgb565,
                            extra: super::SceneTextureExtra {
                                dx: Fixed::from_integer(1),
                                dy: Fixed::from_integer(1),
                                off_x: Fixed::from_integer(off_x),
                                off_y: Fixed::from_integer(off_y),
                                alpha,
                                colorize: Color::default(),
                                rotation: super::RenderingRotation::NoRotation,
                            },
                        };
                        let mut actual = [initial; 6];
                        draw_texture_line(
                            &span,
                            PhysicalLength::new(8),
                            &texture,
                            &mut actual[..count as usize],
                            extra as i16,
                            0,
                        );
                        let mut expected = [initial; 6];
                        for (i, pixel) in expected[..count as usize].iter_mut().enumerate() {
                            let value =
                                (((off_y + 1) * 6 + source_x) as u16 + i as u16).wrapping_mul(2707);
                            if alpha == 255 {
                                let color = Rgb565Pixel(value);
                                *pixel = P::from_rgb(color.red(), color.green(), color.blue());
                            } else {
                                let color = Rgb565Pixel(value);
                                pixel.blend(PremultipliedRgbaColor::premultiply(
                                    Color::from_argb_u8(
                                        alpha,
                                        color.red(),
                                        color.green(),
                                        color.blue(),
                                    ),
                                ));
                            }
                        }
                        assert_eq!(
                            bytemuck::cast_slice::<P, u8>(&actual),
                            bytemuck::cast_slice::<P, u8>(&expected)
                        );
                    }
                }
            }
        }
    }
    check(Rgb565Pixel(0x52db));
    check(Rgb565BigEndianPixel(0x52dbu16.to_be()));
    check(Rgb8Pixel::new(83, 123, 217));
    check(PremultipliedRgbaColor { red: 17, green: 41, blue: 53, alpha: 100 });
}

/// draw one line of the rounded rectangle in the line buffer
#[allow(clippy::unnecessary_cast)] // Coord
#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), allow(unsafe_code))]
#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), unsafe(link_section = ".data.ram_func"))]
#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), inline(never))]
pub(super) fn draw_rounded_rectangle_line(
    span: &PhysicalRect,
    line: PhysicalLength,
    rr: &super::RoundedRectangle,
    line_buffer: &mut [impl TargetPixel],
    extra_left_clip: i16,
    extra_right_clip: i16,
) {
    let width = line_buffer.len();
    let top = (line - span.origin.y_length()) + rr.shape.top_clip;
    let bottom = span.origin.y_length() + span.size.height_length() - line + rr.shape.bottom_clip
        - PhysicalLength::new(1);
    if top.get() >= rr.shape.radius.top_left.max(rr.shape.radius.top_right)
        && bottom.get() >= rr.shape.radius.bottom_left.max(rr.shape.radius.bottom_right)
        && top.min(bottom) >= rr.width
    {
        let left = (rr.width.get() - rr.shape.left_clip.get() - extra_left_clip)
            .max(0) as usize;
        let right = (rr.width.get() - rr.shape.right_clip.get() - extra_right_clip)
            .max(0) as usize;
        let begin = left.min(width);
        let end = width.saturating_sub(right);
        if rr.width.get() > 0 {
            TargetPixel::blend_slice(&mut line_buffer[..begin], rr.border_color);
            TargetPixel::blend_slice(&mut line_buffer[end..], rr.border_color);
        }
        if begin < end && rr.inner_color.alpha > 0 {
            TargetPixel::blend_slice(&mut line_buffer[begin..end], rr.inner_color);
        }
        return;
    }
    draw_curved_rectangle_line(span, line, rr, line_buffer, extra_left_clip, extra_right_clip);
}

/// draw one line of the rounded rectangle in the line buffer
#[allow(clippy::unnecessary_cast)] // Coord
#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), allow(unsafe_code))]
#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), unsafe(link_section = ".data.ram_func"))]
#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), inline(never))]
fn draw_curved_rectangle_line(
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
    let anti_alias = |x1: Shifted, x2: Shifted, process_pixel: &mut dyn FnMut(usize, u32)| {
        // x1 and x2 are the coordinate on the top and bottom of the intersection of the pixel
        // line and the curve.
        // `process_pixel` be called for the coordinate in the array and a coverage between 0..255
        for x in x1.floor()..x2.ceil() {
            process_pixel(x as usize, edge_coverage(x, x1, x2));
        }
    };
    let rev = |x: Shifted| {
        (Shifted::new(width) + Shifted::new(shape.right_clip.get() + extra_right_clip))
            .saturating_sub(x)
    };
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
    // `rev` gives line buffer positions, while the left side's are relative to the rectangle.
    let left_clip = Shifted::new(shape.left_clip.get() + extra_left_clip);
    let x4 = x4.min(x6 + left_clip);
    let x3 = x3.min(x4);
    let x5 = x5.max(x3.saturating_sub(left_clip));
    let x6 = x6.max(x5);
    anti_alias(
        x1.saturating_sub(Shifted::new(shape.left_clip.get() + extra_left_clip)),
        x2.saturating_sub(Shifted::new(shape.left_clip.get() + extra_left_clip)),
        &mut |x, cov| {
            if x >= width {
                return;
            }
            let c = if border == Shifted::ZERO { rr.inner_color } else { rr.border_color };
            let col = PremultipliedRgbaColor {
                alpha: (((c.alpha as u32) * cov as u32) / 255) as u8,
                red: (((c.red as u32) * cov as u32) / 255) as u8,
                green: (((c.green as u32) * cov as u32) / 255) as u8,
                blue: (((c.blue as u32) * cov as u32) / 255) as u8,
            };
            line_buffer[x].blend(col);
        },
    );
    if y < rr.width.get() {
        // up or down border (x2 .. x7)
        let l = x2
            .ceil()
            .saturating_sub((shape.left_clip.get() + extra_left_clip) as u32)
            .min(width as u32) as usize;
        let r = x7.floor().min(width as u32) as usize;
        if l < r {
            TargetPixel::blend_slice(&mut line_buffer[l..r], rr.border_color)
        }
    } else {
        if border > Shifted::ZERO {
            // 3. draw the border (between x2 and x3)
            if Shifted::ONE + x2 <= x3 {
                TargetPixel::blend_slice(
                    &mut line_buffer[x2
                        .ceil()
                        .saturating_sub((shape.left_clip.get() + extra_left_clip) as u32)
                        .min(width as u32) as usize
                        ..x3.floor()
                            .saturating_sub((shape.left_clip.get() + extra_left_clip) as u32)
                            .min(width as u32) as usize],
                    rr.border_color,
                )
            }
            // 4. anti-aliasing for the contents (x3 .. x4)
            anti_alias(
                x3.saturating_sub(Shifted::new(shape.left_clip.get() + extra_left_clip)),
                x4.saturating_sub(Shifted::new(shape.left_clip.get() + extra_left_clip)),
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
                .saturating_sub((shape.left_clip.get() + extra_left_clip) as u32)
                .min(width as u32);
            let end = x5.floor().min(width as u32);
            if begin < end {
                TargetPixel::blend_slice(
                    &mut line_buffer[begin as usize..end as usize],
                    rr.inner_color,
                )
            }
        }
        if border > Shifted::ZERO {
            // 6. border anti-aliasing: x5..x6
            anti_alias(x5, x6, &mut |x, cov| {
                if x >= width {
                    return;
                }
                let col = interpolate_color(cov, rr.inner_color, rr.border_color);
                line_buffer[x].blend(col)
            });
            // 7. border x6 .. x7
            if Shifted::ONE + x6 <= x7 {
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
    let left_offset = (shape.left_clip.get() + extra_left_clip) as u32;
    let shape_width =
        Shifted::new(len as u32 + left_offset + (shape.right_clip.get() + extra_right_clip) as u32);
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
    // Bounds the stack scratch buffer. Most edges fit in one chunk;
    // the flat top of a large corner takes several.
    const CHUNK: usize = 16;
    let mut scratch = [PremultipliedRgbaColor::default(); CHUNK];
    for edge in [begin..inner_begin, inner_end..end] {
        for start in edge.clone().step_by(CHUNK) {
            let chunk = start..(start + CHUNK).min(edge.end);
            let scratch = &mut scratch[..chunk.len()];
            scratch.fill(PremultipliedRgbaColor::default());
            let (left_clip, right_clip) = clips(&chunk);
            g.draw_scratch_line(rect, line, scratch, left_clip, right_clip);
            for (x, color) in chunk.zip(scratch.iter()) {
                let color =
                    interpolate_color(coverage(x), PremultipliedRgbaColor::default(), *color);
                buffer[x].blend(color);
            }
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
            blend_gradient_slice(buffer, col, noise);
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

    let mut offset = 0;
    if x < 0 {
        let l = (-x as usize).min(buffer.len());
        if invert_slope {
            if fill_col1 {
                blend_gradient_slice(&mut buffer[..l], g.color1, noise);
            }
        } else if fill_col2 {
            blend_gradient_slice(&mut buffer[..l], g.color2, noise);
        }
        buffer = &mut buffer[l..];
        offset += l;
        x = 0;
    }

    if buffer.len() + x as usize > len {
        let l = len.saturating_sub(x as usize);
        if invert_slope {
            if fill_col2 {
                blend_gradient_slice(&mut buffer[l..], g.color2, noise.skip(offset + l));
            }
        } else if fill_col1 {
            blend_gradient_slice(&mut buffer[l..], g.color1, noise.skip(offset + l));
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
        pix.blend_dithered(color, noise.next());
        r = r.wrapping_add(dr as _);
        g = g.wrapping_add(dg as _);
        b = b.wrapping_add(db as _);
        a = a.wrapping_add(da as _);
    }
}

fn blend_stops(
    stops: &i_slint_core::SharedVector<PremultipliedGradientStop>,
    pixel: &mut impl TargetPixel,
    position: f32,
    noise: &mut GradientNoise,
) {
    let mut fallback = stops.first().map(|s| s.color).unwrap_or_default();

    for [stop1, stop2] in stops.array_windows() {
        if position >= stop1.position && position <= stop2.position {
            // Interpolate between the two stops
            let t = if stop2.position == stop1.position {
                0.0
            } else {
                (position - stop1.position) / (stop2.position - stop1.position)
            };
            let (c1, c2) = (stop1.color, stop2.color);
            let lerp = |a: u8, b: u8| ((1.0 - t) * a as f32 + t * b as f32) as u8;

            let color = PremultipliedRgbaColor {
                alpha: lerp(c1.alpha, c2.alpha),
                red: lerp(c1.red, c2.red),
                green: lerp(c1.green, c2.green),
                blue: lerp(c1.blue, c2.blue),
            };
            pixel.blend_dithered(color, noise.next());

            return;
        } else if position > stop2.position {
            fallback = stop2.color;
        }
    }
    pixel.blend_dithered(fallback, noise.next());
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
        g.radius >= 0.0,
        "radius must be resolved before constructing RadialGradientCommand"
    );
    let max_radius = g.radius.max(f32::EPSILON);

    let start_x = rect.min_x() + extra_left_clip;
    let dy = line.get() as f32 - center_y;
    let dy_squared = dy * dy;
    let mut noise = GradientNoise::new(start_x as i32, line.get() as i32);

    for (i, pixel) in buffer.iter_mut().enumerate() {
        let x = start_x + i as i16;
        let dx = x as f32 - center_x;
        let distance = (dx * dx + dy_squared).sqrt();
        let position = (distance / max_radius).clamp(0.0, 1.0);

        blend_stops(&g.stops, pixel, position, &mut noise);
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

    for (i, pixel) in buffer.iter_mut().enumerate() {
        let dx = (start_x + i as i16) as f32 - center_x;
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
        pix.blend_dithered(color, noise.next());
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
            Self::fill_slice(slice, Self::from_rgb(color.red, color.green, color.blue))
        } else {
            for x in slice {
                Self::blend(x, color);
            }
        }
    }
    /// Fill the slice with one pixel.
    fn fill_slice(slice: &mut [Self], pixel: Self) {
        slice.fill(pixel);
    }
    /// Copy native-endian RGB565 bytes; the source length must be twice the pixel count.
    fn copy_from_rgb565(slice: &mut [Self], source: &[u8]) {
        copy_rgb565_pixels(slice, source);
    }
    /// Create a pixel from the red, gree, blue component in the range 0..=255
    fn from_rgb(red: u8, green: u8, blue: u8) -> Self;

    /// Pixel which will be filled as the background in case the slint view has transparency
    fn background() -> Self {
        Self::from_rgb(0, 0, 0)
    }

    /// Like `blend`, with `noise` in 0..=255 (a fraction of one quantization step).
    /// Formats with less than 8 bits per channel can use it to dither.
    #[inline]
    fn blend_dithered(&mut self, color: PremultipliedRgbaColor, _noise: u8) {
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

fn copy_rgb565_pixels<P: TargetPixel>(pixels: &mut [P], source: &[u8]) {
    assert_eq!(source.len(), pixels.len() * 2);
    for (pixel, bytes) in pixels.iter_mut().zip(source.chunks_exact(2)) {
        let color = Rgb565Pixel(u16::from_ne_bytes([bytes[0], bytes[1]]));
        *pixel = P::from_rgb(color.red(), color.green(), color.blue());
    }
}

#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), allow(unsafe_code))]
#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), unsafe(link_section = ".data.ram_func"))]
#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), inline(never))]
fn copy_rgb565_big_endian(pixels: &mut [u16], source: &[u16]) {
    assert_eq!(source.len(), pixels.len());
    if cfg!(target_endian = "big") {
        pixels.copy_from_slice(source);
        return;
    }
    if pixels.is_empty() {
        return;
    }
    let prefix_len = usize::from(pixels.as_ptr() as usize & 3 != 0).min(pixels.len());
    let (prefix, remaining) = pixels.split_at_mut(prefix_len);
    if let Some(pixel) = prefix.first_mut() {
        *pixel = source[0].swap_bytes();
    }
    let (pairs, suffix) = remaining.split_at_mut(remaining.len() & !1);
    let words = bytemuck::cast_slice_mut::<u16, u32>(pairs);
    for (pixel_pair, pair) in words.iter_mut().zip(source[prefix_len..].chunks_exact(2)) {
        let value = u32::from(pair[0]) | (u32::from(pair[1]) << 16);
        *pixel_pair = ((value & 0x00ff00ff) << 8) | ((value >> 8) & 0x00ff00ff);
    }
    if let Some(pixel) = suffix.first_mut() {
        *pixel = source[source.len() - 1].swap_bytes();
    }
}

#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), allow(unsafe_code))]
#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), unsafe(link_section = ".data.ram_func"))]
#[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), inline(never))]
fn fill_rgb565(pixels: &mut [u16], value: u16) {
    if pixels.is_empty() {
        return;
    }
    let prefix_len = usize::from(pixels.as_ptr() as usize & 3 != 0).min(pixels.len());
    let (prefix, remaining) = pixels.split_at_mut(prefix_len);
    prefix.fill(value);
    let (pairs, suffix) = remaining.split_at_mut(remaining.len() & !1);
    bytemuck::cast_slice_mut::<u16, u32>(pairs).fill(u32::from(value) * 0x10001);
    suffix.fill(value);
}

#[test]
fn rgb565_fills_preserve_alignment_clipping_and_alpha() {
    fn check<P: TargetPixel + bytemuck::Pod>() {
        #[repr(align(4))]
        struct Storage<P>([P; 24]);
        for offset in 0..4 {
            for len in 0..=18 {
                for alpha in [0, 1, 127, 254, 255] {
                    for color in [0, 0x123456, 0xa2f378, 0xffffff] {
                        let color = PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                            alpha,
                            (color >> 16) as u8,
                            (color >> 8) as u8,
                            color as u8,
                        ));
                        let initial = P::from_rgb(19, 73, 157);
                        let mut actual = Storage([initial; 24]);
                        let mut expected = Storage([initial; 24]);
                        P::blend_slice(&mut actual.0[offset..offset + len], color);
                        for pixel in &mut expected.0[offset..offset + len] {
                            if alpha == 255 {
                                *pixel = P::from_rgb(color.red, color.green, color.blue);
                            } else {
                                pixel.blend(color);
                            }
                        }
                        assert_eq!(
                            bytemuck::cast_slice::<P, u8>(&actual.0),
                            bytemuck::cast_slice::<P, u8>(&expected.0),
                            "offset {offset}, length {len}, alpha {alpha}"
                        );
                    }
                }
            }
        }
    }
    check::<Rgb565Pixel>();
    check::<Rgb565BigEndianPixel>();
}

#[test]
fn rgb565_copies_preserve_every_color_alignment_and_tail() {
    fn check<P: TargetPixel + bytemuck::Pod>() {
        for source_offset in 0..4 {
            let mut source = alloc::vec![0; source_offset];
            for value in 0..=u16::MAX {
                source.extend_from_slice(&value.to_ne_bytes());
            }
            for target_offset in 0..4 {
                for count in (0..=33).chain([65536]) {
                    let initial = P::from_rgb(17, 73, 191);
                    let mut actual = alloc::vec![initial; target_offset + count + 2];
                    let mut expected = actual.clone();
                    P::copy_from_rgb565(
                        &mut actual[target_offset..target_offset + count],
                        &source[source_offset..source_offset + count * 2],
                    );
                    for (i, pixel) in expected[target_offset..target_offset + count].iter_mut().enumerate() {
                        let color = Rgb565Pixel(i as u16);
                        *pixel = P::from_rgb(color.red(), color.green(), color.blue());
                    }
                    assert_eq!(
                        bytemuck::cast_slice::<P, u8>(&actual),
                        bytemuck::cast_slice::<P, u8>(&expected),
                        "source offset {source_offset}, target offset {target_offset}, length {count}"
                    );
                }
            }
        }
    }
    check::<Rgb565Pixel>();
    check::<Rgb565BigEndianPixel>();
}

impl TargetPixel for Rgb565Pixel {
    fn copy_from_rgb565(slice: &mut [Self], source: &[u8]) {
        if let Ok(pixels) = bytemuck::try_cast_slice::<u8, Self>(source) {
            slice.copy_from_slice(pixels);
        } else {
            copy_rgb565_pixels(slice, source);
        }
    }

    fn fill_slice(slice: &mut [Self], pixel: Self) {
        fill_rgb565(bytemuck::cast_slice_mut(slice), pixel.0);
    }

    #[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), inline(always))]
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

    fn blend_dithered(&mut self, color: PremultipliedRgbaColor, noise: u8) {
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
    fn copy_from_rgb565(slice: &mut [Self], source: &[u8]) {
        if let Ok(pixels) = bytemuck::try_cast_slice::<u8, u16>(source) {
            copy_rgb565_big_endian(bytemuck::cast_slice_mut(slice), pixels);
        } else {
            copy_rgb565_pixels(slice, source);
        }
    }

    fn fill_slice(slice: &mut [Self], pixel: Self) {
        fill_rgb565(bytemuck::cast_slice_mut(slice), pixel.0);
    }

    #[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), inline(always))]
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

    fn blend_dithered(&mut self, color: PremultipliedRgbaColor, noise: u8) {
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
fn alpha_map_matches_scalar_blending() {
    fn check<Pixel: TargetPixel + bytemuck::Pod>(initial: Pixel) {
        let coverage: [u8; 256] = core::array::from_fn(|i| i as u8);
        let span =
            PhysicalRect::new(super::PhysicalPoint::new(0, 0), super::PhysicalSize::new(256, 1));
        for color in [
            Color::from_rgb_u8(0, 0, 0),
            Color::from_rgb_u8(255, 255, 255),
            Color::from_rgb_u8(17, 83, 219),
            Color::from_rgb_u8(248, 252, 248),
        ] {
            for alpha in 0..=255 {
                let texture = super::SceneTexture {
                    data: &coverage,
                    format: TexturePixelFormat::AlphaMap,
                    pixel_stride: 256,
                    extra: super::SceneTextureExtra {
                        dx: Fixed::from_integer(1),
                        dy: Fixed::from_integer(1),
                        off_x: Fixed::from_integer(0),
                        off_y: Fixed::from_integer(0),
                        colorize: color,
                        alpha,
                        rotation: super::RenderingRotation::NoRotation,
                    },
                };
                let mut actual = [initial; 256];
                draw_texture_line(&span, PhysicalLength::new(0), &texture, &mut actual, 0, 0);
                let mut expected = [initial; 256];
                for (pixel, coverage) in expected.iter_mut().zip(coverage) {
                    pixel.blend(PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                        (coverage as u16 * alpha as u16 / 255) as u8,
                        color.red(),
                        color.green(),
                        color.blue(),
                    )));
                }
                assert_eq!(
                    bytemuck::cast_slice::<Pixel, u8>(&actual),
                    bytemuck::cast_slice::<Pixel, u8>(&expected),
                    "global alpha {alpha}, color {color:?}",
                );
            }
        }
    }
    check(Rgb8Pixel::new(41, 119, 213));
    check(Rgb565Pixel::from_rgb(41, 119, 213));
    check(Rgb565BigEndianPixel::from_rgb(41, 119, 213));
    check(PremultipliedRgbaColor::premultiply(Color::from_argb_u8(81, 41, 119, 213)));
}

#[test]
fn contiguous_alpha_map_matches_scalar_blending() {
    fn check<P: TargetPixel + bytemuck::Pod>(initial: P) {
        let coverage: [u8; 256] = core::array::from_fn(|i| i as u8);
        for color in [
            Color::from_rgb_u8(255, 255, 255),
            Color::from_rgb_u8(0, 0, 0),
            Color::from_rgb_u8(237, 119, 53),
        ] {
            for alpha in 0..=255 {
                let mut actual = [initial; 256];
                let mut expected = actual;
                blend_alpha_map_line(&mut actual, &coverage, alpha, color);
                for (pixel, coverage) in expected.iter_mut().zip(coverage) {
                    let alpha = (coverage as u16 * alpha as u16 / 255) as u8;
                    if alpha == 255 {
                        *pixel = P::from_rgb(color.red(), color.green(), color.blue());
                    } else if alpha != 0 {
                        pixel.blend(PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                            alpha,
                            color.red(),
                            color.green(),
                            color.blue(),
                        )));
                    }
                }
                assert_eq!(
                    bytemuck::cast_slice::<P, u8>(&actual),
                    bytemuck::cast_slice::<P, u8>(&expected)
                );
            }
        }
    }
    check(Rgb8Pixel::new(41, 119, 213));
    check(Rgb565Pixel::from_rgb(41, 119, 213));
    check(Rgb565BigEndianPixel::from_rgb(41, 119, 213));
    check(PremultipliedRgbaColor::premultiply(Color::from_argb_u8(81, 41, 119, 213)));
}

#[test]
fn unit_alpha_map_matches_scalar_sampling_with_clipping_and_tiling() {
    fn check<Pixel: TargetPixel + bytemuck::Pod>(initial: Pixel) {
        let data: [u8; 64] = core::array::from_fn(|i| (i as u8).wrapping_mul(37));
        let color = Color::from_rgb_u8(237, 119, 53);
        let span = super::PhysicalRect::new(
            super::PhysicalPoint::new(2, 3),
            super::PhysicalSize::new(20, 12),
        );
        for alpha in [0, 91, 254, 255] {
            for source_x in [0, 1, 8, 10] {
                for source_y in [0, 1, 5, 7] {
                    for fraction in 0..16 {
                        for clip in [0, 1, 4] {
                            for line in [0, 1, 5] {
                                for len in [0, 1, 4, 9, 20] {
                                    let texture = super::SceneTexture {
                                        data: &data,
                                        format: TexturePixelFormat::AlphaMap,
                                        pixel_stride: 11,
                                        extra: super::SceneTextureExtra {
                                            dx: Fixed::from_integer(1),
                                            dy: Fixed::from_integer(1),
                                            off_x: Fixed(source_x * 16 + fraction),
                                            off_y: Fixed(source_y * 16 + fraction),
                                            colorize: color,
                                            alpha,
                                            rotation: super::RenderingRotation::NoRotation,
                                        },
                                    };
                                    let mut actual = alloc::vec![initial; len];
                                    draw_texture_line(
                                        &span,
                                        PhysicalLength::new(3 + line),
                                        &texture,
                                        &mut actual,
                                        clip,
                                        0,
                                    );
                                    let mut expected = alloc::vec![initial; len];
                                    for (column, pixel) in expected.iter_mut().enumerate() {
                                        let x = (source_x as usize + clip as usize + column) % 9;
                                        let y = (source_y as usize + line as usize) % 6;
                                        pixel.blend(PremultipliedRgbaColor::premultiply(
                                            Color::from_argb_u8(
                                                (data[y * 11 + x] as u16 * alpha as u16 / 255)
                                                    as u8,
                                                color.red(),
                                                color.green(),
                                                color.blue(),
                                            ),
                                        ));
                                    }
                                    assert_eq!(
                                        bytemuck::cast_slice::<Pixel, u8>(&actual),
                                        bytemuck::cast_slice::<Pixel, u8>(&expected),
                                        "offset ({source_x}, {source_y}), fraction {fraction}, clip {clip}, line {line}, len {len}, alpha {alpha}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    check(Rgb8Pixel::new(41, 119, 213));
    check(Rgb565Pixel::from_rgb(41, 119, 213));
    check(Rgb565BigEndianPixel::from_rgb(41, 119, 213));
    check(PremultipliedRgbaColor::premultiply(Color::from_argb_u8(81, 41, 119, 213)));
}

#[test]
fn rounded_straight_scanlines_match_curved_rasterizer() {
    fn check<P: TargetPixel + bytemuck::Pod>(initial: P) {
        for radius in [
            super::PhysicalBorderRadius::new(0, 0, 0, 0),
            super::PhysicalBorderRadius::new(14, 14, 14, 14),
            super::PhysicalBorderRadius::new(3, 12, 1, 8),
        ] {
            for border in [0, 1, 2, 4] {
                for alpha in [0, 73, 128, 255] {
                    for (left, right, top, bottom) in [(0, 0, 0, 0), (3, 5, 7, 4), (19, 19, 16, 18)]
                    {
                        let span = PhysicalRect::new(
                            super::PhysicalPoint::new(7, 11),
                            super::PhysicalSize::new(44 - left - right, 40 - top - bottom),
                        );
                        let rr = super::RoundedRectangle {
                            shape: super::RoundedShape {
                                radius,
                                left_clip: PhysicalLength::new(left),
                                right_clip: PhysicalLength::new(right),
                                top_clip: PhysicalLength::new(top),
                                bottom_clip: PhysicalLength::new(bottom),
                            },
                            width: PhysicalLength::new(border),
                            border_color: PremultipliedRgbaColor::from(Color::from_argb_u8(
                                alpha, 53, 137, 211,
                            )),
                            inner_color: PremultipliedRgbaColor::from(Color::from_argb_u8(
                                alpha, 193, 71, 29,
                            )),
                        };
                        for (extra_left, extra_right) in [(0, 0), (2, 3)] {
                            let width = (span.size.width - extra_left - extra_right) as usize;
                            for line in span.min_y()..span.max_y() {
                                let mut actual = alloc::vec![initial; width];
                                let mut expected = actual.clone();
                                draw_rounded_rectangle_line(
                                    &span,
                                    PhysicalLength::new(line),
                                    &rr,
                                    &mut actual,
                                    extra_left,
                                    extra_right,
                                );
                                draw_curved_rectangle_line(
                                    &span,
                                    PhysicalLength::new(line),
                                    &rr,
                                    &mut expected,
                                    extra_left,
                                    extra_right,
                                );
                                assert_eq!(
                                    bytemuck::cast_slice::<P, u8>(&actual),
                                    bytemuck::cast_slice::<P, u8>(&expected),
                                    "{rr:?}, line={line}, extra={extra_left}/{extra_right}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    check(Rgb565Pixel(0x52db));
    check(Rgb565BigEndianPixel(0x52dbu16.to_be()));
    check(Rgb8Pixel::new(83, 123, 217));
    check(PremultipliedRgbaColor {
        red: 17,
        green: 41,
        blue: 53,
        alpha: 100,
    });
}
