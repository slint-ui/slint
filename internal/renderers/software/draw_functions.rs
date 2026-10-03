// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore premultiply
#![allow(clippy::identity_op)] // We use x + 0 a lot here for symmetry

//! This is the module for the functions that are drawing the pixels
//! on the line buffer

use super::{Fixed, PhysicalLength, PhysicalRect, PhysicalSize};
use alloc::vec::Vec;
use derive_more::{Add, Mul, Sub};
use i_slint_core::Color;
use i_slint_core::graphics::{BorderRadius, Rgb8Pixel, TexturePixelFormat};
use i_slint_core::lengths::{PhysicalPx, PointLengths, SizeLengths};
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
                        let p: &[u8] = &data[pos(2).0..][..2];
                        *pix = TargetPixel::from_rgb565(u16::from_ne_bytes([p[0], p[1]]));
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
                    let c = PremultipliedRgbaColor::premultiply(Color::from_argb_u8(
                        ((data[pos] as u16 * alpha as u16) / 255) as u8,
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
    /// This is an integer shifted by 4 bits.
    /// Note: this is not a "fixed point" because multiplication and sqrt operation operate to
    /// the shifted integer
    #[derive(Clone, Copy, PartialEq, Ord, PartialOrd, Eq, Add, Sub, Mul)]
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
    let width = line_buffer.len();
    let y1 = (line - span.origin.y_length()) + rr.top_clip;
    let y2 = (span.origin.y_length() + span.size.height_length() - line) + rr.bottom_clip
        - PhysicalLength::new(1);
    let y = y1.min(y2);
    debug_assert!(y.get() >= 0,);
    let border = Shifted::new(rr.width.get());
    const ONE: Shifted = Shifted::ONE;
    const ZERO: Shifted = Shifted(0);
    let left_clip = (rr.left_clip.get() + extra_left_clip) as u32;
    let to_buffer = |x: u32| x.saturating_sub(left_clip).min(width as u32) as usize;
    let anti_alias = |x1: Shifted, x2: Shifted, process_pixel: &mut dyn FnMut(usize, u32)| {
        // x1 and x2 are the coordinate on the top and bottom of the intersection of the pixel
        // line and the curve.
        // `process_pixel` be called for the coordinate in the array and a coverage between 0..255
        // This algorithm just go linearly which is not perfect, but good enough.
        for x in x1.floor().max(left_clip)..x2.ceil().min(left_clip + width as u32) {
            // the coverage is basically how much of the pixel should be used
            let cov = ((ONE + Shifted::new(x) - x1).0 << 8) / (ONE + x2 - x1).0;
            process_pixel((x - left_clip) as usize, cov);
        }
    };
    let rev = |x: Shifted| {
        (Shifted::new(left_clip)
            + Shifted::new(width)
            + Shifted::new(rr.right_clip.get() + extra_right_clip))
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
    anti_alias(x1, x2, &mut |x, cov| {
        let c = if border == ZERO { rr.inner_color } else { rr.border_color };
        let col = PremultipliedRgbaColor {
            alpha: (((c.alpha as u32) * cov as u32) / 255) as u8,
            red: (((c.red as u32) * cov as u32) / 255) as u8,
            green: (((c.green as u32) * cov as u32) / 255) as u8,
            blue: (((c.blue as u32) * cov as u32) / 255) as u8,
        };
        line_buffer[x].blend(col);
    });
    if y < rr.width {
        // up or down border (x2 .. x7)
        let l = to_buffer(x2.ceil());
        let r = to_buffer(x7.floor());
        if l < r {
            TargetPixel::blend_slice(&mut line_buffer[l..r], rr.border_color)
        }
    } else {
        if border > ZERO {
            // 3. draw the border (between x2 and x3)
            if ONE + x2 <= x3 {
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
        if border > ZERO {
            // 6. border anti-aliasing: x5..x6
            anti_alias(x5, x6, &mut |x, cov| {
                let col = interpolate_color(cov, rr.inner_color, rr.border_color);
                line_buffer[x].blend(col)
            });
            // 7. border x6 .. x7
            if ONE + x6 <= x7 {
                TargetPixel::blend_slice(
                    &mut line_buffer[to_buffer(x6.ceil())..to_buffer(x7.floor())],
                    rr.border_color,
                )
            }
        }
    }
    anti_alias(x7, x8, &mut |x, cov| {
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

    let center_x = rect.min_x() as f32 + g.center_x;
    let center_y = rect.min_y() as f32 + g.center_y;

    debug_assert!(
        g.radius >= 0.0,
        "radius must be resolved before constructing RadialGradientCommand"
    );
    let max_radius = g.radius.max(f32::EPSILON);

    let start_x = rect.min_x() + extra_left_clip;
    let dy = line.get() as f32 - center_y;
    let dy_squared = dy * dy;

    for (i, pixel) in buffer.iter_mut().enumerate() {
        let x = start_x + i as i16;
        let dx = x as f32 - center_x;
        let distance = (dx * dx + dy_squared).sqrt();
        let position = (distance / max_radius).clamp(0.0, 1.0);

        // Find the two gradient stops to interpolate between
        let mut color = g.stops.first().map(|s| s.color).unwrap_or_default();

        for [stop1, stop2] in g.stops.array_windows() {
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

    let center_x = rect.min_x() as f32 + g.center_x;
    let center_y = rect.min_y() as f32 + g.center_y;

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

        for [stop1, stop2] in g.stops.array_windows() {
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

const GAUSSIAN_TAIL_STEPS: usize = 64;
const GAUSSIAN_CUTOFF: i32 = 3;
/// `Φ(+∞)` in the Q15 representation of [`Gaussian::cdf`].
const GAUSSIAN_ONE: u32 = 1 << 15;

/// The upper tail of the standard normal distribution, `Q(t) = 1 − Φ(t)`,
/// for `t = i / GAUSSIAN_TAIL_STEPS` in `[0, GAUSSIAN_CUTOFF]`, in Q15.
static GAUSSIAN_TAIL: [u16; GAUSSIAN_CUTOFF as usize * GAUSSIAN_TAIL_STEPS + 1] =
    gaussian_tail_table();

const fn gaussian_tail_table() -> [u16; GAUSSIAN_CUTOFF as usize * GAUSSIAN_TAIL_STEPS + 1] {
    let mut table = [0; GAUSSIAN_CUTOFF as usize * GAUSSIAN_TAIL_STEPS + 1];
    let mut i = 0;
    while i < table.len() {
        let q = gaussian_q(i as f64 / GAUSSIAN_TAIL_STEPS as f64);
        table[i] = (q * GAUSSIAN_ONE as f64 + 0.5) as u16;
        i += 1;
    }
    table
}

/// `Q(t) = 1 − Φ(t)` for `0 ≤ t ≤ 4`, in a const context.
const fn gaussian_q(t: f64) -> f64 {
    // cSpell: ignore Abramowitz Stegun erfc
    // Abramowitz and Stegun 7.1.26: erfc(x) ≈ poly(1 / (1 + p·x)) · e^(−x²),
    // with an absolute error below 1.5e-7. Q(t) = erfc(t / √2) / 2.
    let x = t / core::f64::consts::SQRT_2;
    let k = 1. / (1. + 0.3275911 * x);
    let poly = k
        * (0.254829592
            + k * (-0.284496736 + k * (1.421413741 + k * (-1.453152027 + k * 1.061405429))));
    0.5 * poly * exp_neg(x * x)
}

/// `e^(−y)` for `0 ≤ y ≤ 8`, in a const context.
const fn exp_neg(y: f64) -> f64 {
    // The Taylor series converges fast for e^(−y/16); squaring four times restores e^(−y).
    let z = -y / 16.;
    let mut sum = 1.;
    let mut term = 1.;
    let mut k = 1;
    while k < 16 {
        term *= z / k as f64;
        sum += term;
        k += 1;
    }
    let mut i = 0;
    while i < 4 {
        sum *= sum;
        i += 1;
    }
    sum
}

#[derive(Clone, Copy)]
struct Gaussian {
    sigma: Fixed<i32, 8>,
    /// `2^29 / sigma`, so `|d| · inv_sigma` stays below `2^31` wherever the CDF doesn't saturate.
    inv_sigma: u32,
}

impl Gaussian {
    fn new(sigma: Fixed<i32, 8>) -> Self {
        Self { sigma, inv_sigma: (1 << 29) / sigma.0 as u32 }
    }

    /// The distance beyond which [`Self::cdf`] saturates.
    fn cutoff(self) -> Fixed<i32, 8> {
        self.sigma * GAUSSIAN_CUTOFF
    }

    /// The CDF `Φ(d / σ)` in Q15.
    #[inline]
    fn cdf(self, d: Fixed<i32, 8>) -> u32 {
        let cutoff = self.cutoff();
        if d >= cutoff {
            return GAUSSIAN_ONE;
        }
        if d <= -cutoff {
            return 0;
        }
        // |d| / σ in Q29: the table index is its top bits, the interpolation fraction the next 10.
        let t = d.0.unsigned_abs() * self.inv_sigma;
        let index = (t >> 23) as usize;
        let fract = (t >> 13) & 0x3ff;
        let q0 = GAUSSIAN_TAIL[index] as u32;
        let q1 = GAUSSIAN_TAIL[index + 1] as u32;
        let q = q0 - (((q0 - q1) * fract) >> 10);
        if d >= Fixed(0) { GAUSSIAN_ONE - q } else { q }
    }
}

const HALF_PIXEL: Fixed<i32, 8> = Fixed(128);

/// The index of the first pixel whose center is at or after `v`.
#[inline]
fn first_pixel_at(v: Fixed<i32, 8>) -> i32 {
    ((v - HALF_PIXEL).0 + 255).div_euclid(256)
}

/// The horizontal coverage of the rows whose blur only reaches the shape's straight sides.
#[derive(Debug)]
struct StraightRowProfile {
    /// The coverage of the columns outside `inner`, in 1/255:
    /// first the columns before `inner`, then those after it.
    ramps: Vec<u8>,
    /// The columns whose coverage saturates.
    inner: core::ops::Range<i32>,
}

impl StraightRowProfile {
    fn new(width: i32, left: Fixed<i32, 8>, right: Fixed<i32, 8>, gaussian: Gaussian) -> Self {
        let cutoff = gaussian.cutoff();
        let start = first_pixel_at(left + cutoff).clamp(0, width);
        let end = first_pixel_at(right - cutoff + Fixed(1)).clamp(start, width);
        let ramps = (0..start)
            .chain(end..width)
            .map(|x| {
                let x_center = Fixed::from_integer(x) + HALF_PIXEL;
                let coverage =
                    gaussian.cdf(x_center - left).saturating_sub(gaussian.cdf(x_center - right));
                ((coverage * 255 + GAUSSIAN_ONE / 2) >> 15) as u8
            })
            .collect();
        Self { ramps, inner: start..end }
    }
}

/// A blurred drop shadow: a rounded rectangle convolved with a Gaussian.
#[derive(Debug)]
pub struct BoxShadowCommand {
    left: Fixed<i32, 8>,
    top: Fixed<i32, 8>,
    right: Fixed<i32, 8>,
    bottom: Fixed<i32, 8>,
    radius: BorderRadius<Fixed<i32, 8>, PhysicalPx>,
    sigma: Fixed<i32, 8>,
    color: PremultipliedRgbaColor,
    profile: Option<StraightRowProfile>,
}

impl BoxShadowCommand {
    // Together with an i16 geometry, these keep every edge within 2^29 / 256 px,
    // so the arithmetic of `draw_box_shadow_line` stays within i32.
    const MAX_SIGMA: Fixed<i32, 8> = Fixed(1 << 23);
    const MAX_RADIUS: Fixed<i32, 8> = Fixed(1 << 28);

    /// A shadow of standard deviation `sigma` for `shape`,
    /// relative to the origin of the geometry of `size` it's drawn in.
    pub(super) fn new(
        size: PhysicalSize,
        shape: euclid::Rect<f32, PhysicalPx>,
        radius: BorderRadius<f32, PhysicalPx>,
        sigma: f32,
        color: PremultipliedRgbaColor,
    ) -> Self {
        let fixed = |v: f32| Fixed::<i32, 8>((v * 256.).round() as i32);
        let sigma = fixed(sigma).clamp(Fixed(1), Self::MAX_SIGMA);
        let radius = BorderRadius::new(
            fixed(radius.top_left),
            fixed(radius.top_right),
            fixed(radius.bottom_right),
            fixed(radius.bottom_left),
        )
        .min(BorderRadius::new_uniform(Self::MAX_RADIUS));
        // An edge further out than the Gaussian's reach plus its corners' radii can't affect
        // any pixel of the geometry.
        let reach = Gaussian::new(sigma).cutoff();
        let width = Fixed::from_integer(size.width as i32);
        let height = Fixed::from_integer(size.height as i32);
        let mut shadow = Self {
            left: fixed(shape.min_x()).max(-(reach + radius.top_left.max(radius.bottom_left))),
            top: fixed(shape.min_y()).max(-(reach + radius.top_left.max(radius.top_right))),
            right: fixed(shape.max_x())
                .min(width + reach + radius.top_right.max(radius.bottom_right)),
            bottom: fixed(shape.max_y())
                .min(height + reach + radius.bottom_left.max(radius.bottom_right)),
            radius,
            sigma,
            color,
            profile: None,
        };
        shadow.profile = shadow.has_straight_rows(size.height as i32).then(|| {
            StraightRowProfile::new(
                size.width as i32,
                shadow.left,
                shadow.right,
                Gaussian::new(sigma),
            )
        });
        shadow
    }

    /// Where the top corner curves end and the bottom ones start.
    fn curve_bounds(&self) -> (Fixed<i32, 8>, Fixed<i32, 8>) {
        let radius = &self.radius;
        let top_curve_end = (self.top + radius.top_left.max(radius.top_right)).min(self.bottom);
        let bottom_curve_start =
            (self.bottom - radius.bottom_left.max(radius.bottom_right)).max(top_curve_end);
        (top_curve_end, bottom_curve_start)
    }

    /// Whether any of the `height` rows can be drawn from a [`StraightRowProfile`].
    fn has_straight_rows(&self, height: i32) -> bool {
        let cutoff = Gaussian::new(self.sigma).cutoff();
        let (top_curve_end, bottom_curve_start) = self.curve_bounds();
        let first = if top_curve_end > self.top {
            first_pixel_at(top_curve_end + cutoff).max(0)
        } else {
            0
        };
        let end = if bottom_curve_start < self.bottom {
            first_pixel_at(bottom_curve_start - cutoff + Fixed(1)).min(height)
        } else {
            height
        };
        first < end
    }
}

/// `color` scaled by a Q16 `coverage`, or `None` when it rounds to transparent.
#[inline(always)]
fn scale_color(color: PremultipliedRgbaColor, coverage: u32) -> Option<PremultipliedRgbaColor> {
    // Q16, so that full coverage keeps the color and 255 · 2^16 fits in u32.
    let scale = |c: u8| ((c as u32 * coverage + (1 << 15)) >> 16) as u8;
    // Premultiplied components never exceed alpha, so a zero alpha is a zero color.
    let alpha = scale(color.alpha);
    (alpha > 0).then(|| PremultipliedRgbaColor {
        alpha,
        red: scale(color.red),
        green: scale(color.green),
        blue: scale(color.blue),
    })
}

/// Draws one line of a shadow row that only reaches the shape's straight sides,
/// with `weight` its vertical Gaussian weight in Q15.
// Kept out of line, to leave `draw_box_shadow_line` within LLVM's inlining budget.
#[inline(never)]
fn draw_straight_box_shadow_line(
    profile: &StraightRowProfile,
    weight: u32,
    color: PremultipliedRgbaColor,
    line_buffer: &mut [impl TargetPixel],
    extra_left_clip: i16,
) {
    let first_x = extra_left_clip as i32;
    let len = line_buffer.len() as i32;
    let inner = &profile.inner;
    let index = |x: i32| (x - first_x).clamp(0, len) as usize;
    let (inner_start, inner_end) = (index(inner.start), index(inner.end));
    if let Some(c) = scale_color(color, 2 * weight) {
        TargetPixel::blend_slice(&mut line_buffer[inner_start..inner_end], c);
    }
    let (before, after) = profile.ramps.split_at(inner.start as usize);
    let before = &before[(first_x as usize).min(before.len())..];
    let after = &after[(first_x - inner.end).max(0) as usize..];
    let (head, tail) = line_buffer.split_at_mut(inner_end);
    let ramps = head[..inner_start].iter_mut().zip(before).chain(tail.iter_mut().zip(after));
    for (pixel, &coverage) in ramps {
        // 257 widens the 1/255 coverage to Q16; `weight` is Q15.
        if let Some(c) = scale_color(color, (weight * (coverage as u32 * 257) + (1 << 14)) >> 15) {
            pixel.blend(c);
        }
    }
}

/// Draws one line of a blurred drop shadow.
///
/// The rows within reach of the blur are cut into horizontal slices,
/// each approximated as a rectangle spanning the shape's width at the slice's middle.
/// Neighboring slices of nearly the same width merge into a band,
/// whose edges are the weighted means of its slices' edges.
/// A band's contribution is its vertical Gaussian weight times its horizontal coverage,
/// both exact.
/// Rows out of reach of the corner curves are a single band, drawn from the precomputed
/// [`StraightRowProfile`].
pub(super) fn draw_box_shadow_line(
    span: &PhysicalRect,
    line: PhysicalLength,
    shadow: &BoxShadowCommand,
    line_buffer: &mut [impl TargetPixel],
    extra_left_clip: i16,
) {
    const MAX_BANDS: usize = 16;
    const MAX_SLICES: i32 = 32;
    /// The cost of interpolating a pixel, in quarters of a band loop.
    const INTERPOLATION_COST: i32 = 3;

    let BoxShadowCommand { left, top, right, bottom, radius, sigma, color, ref profile } = *shadow;
    let gaussian = Gaussian::new(sigma);
    let cutoff = gaussian.cutoff();

    let y_center = Fixed::from_integer((line - span.origin.y_length()).get() as i32) + HALF_PIXEL;
    let window_top = top.max(y_center - cutoff);
    let window_bottom = bottom.min(y_center + cutoff);
    if window_top >= window_bottom {
        return;
    }

    let (top_curve_end, bottom_curve_start) = shadow.curve_bounds();
    if let Some(profile) = profile
        && window_top >= top_curve_end
        && window_bottom <= bottom_curve_start
    {
        let weight = gaussian.cdf(y_center - window_top) - gaussian.cdf(y_center - window_bottom);
        draw_straight_box_shadow_line(profile, weight, color, line_buffer, extra_left_clip);
        return;
    }
    let in_window = |a: Fixed<i32, 8>, b: Fixed<i32, 8>| (a.max(window_top), b.min(window_bottom));
    let intervals = [
        (in_window(top, top_curve_end), true),
        (in_window(top_curve_end, bottom_curve_start), false),
        (in_window(bottom_curve_start, bottom), true),
    ];
    let length = |(a, b): (Fixed<i32, 8>, Fixed<i32, 8>)| (b - a).max(Fixed(0));
    let curved_budget = MAX_SLICES - (length(intervals[1].0) > Fixed(0)) as i32;
    let curved_length = length(intervals[0].0) + length(intervals[2].0);
    // For why σ/4 slices merged within σ, see `box_shadow_bands_stay_close_to_the_exact_blur`.
    // Rounding up the slice count of each of the two curved intervals adds at most one slice
    // each, so reserve two.
    let slice_height =
        (sigma / 4).max(Fixed((curved_length.0 + curved_budget - 3) / (curved_budget - 2)));

    // How far a corner of radius `r` indents the shape's edge at `distance` from the corner's
    // horizontal side.
    let corner_inset = |r: Fixed<i32, 8>, distance: Fixed<i32, 8>| {
        let dy = r - distance;
        if dy <= Fixed(0) {
            return Fixed(0);
        }
        // Cortex-M0+ has no 64-bit multiply, and radii below 256 px keep r² within u32.
        if r.0 < 1 << 16 {
            let (r, dy) = (r.0 as u32, dy.0 as u32);
            return Fixed((r - (r * r - dy * dy).isqrt()) as i32);
        }
        let (r, dy) = (r.0 as i64, dy.0 as i64);
        Fixed((r - ((r * r - dy * dy) as u64).isqrt() as i64) as i32)
    };

    #[derive(Clone, Copy, Default)]
    struct Band {
        x0: Fixed<i32, 8>,
        x1: Fixed<i32, 8>,
        weight: u32,
        /// The first slice's edges, which later slices must stay close to.
        first: (Fixed<i32, 8>, Fixed<i32, 8>),
        /// The slices' edges, weighted.
        sum: (i64, i64),
        slices: u32,
    }
    let mut bands = [Band::default(); MAX_BANDS];
    let mut band_count: usize = 0;
    let mut edge = window_top;
    let mut edge_cdf = gaussian.cdf(y_center - edge);
    for ((a, b), curved) in intervals {
        if a >= b {
            continue;
        }
        debug_assert_eq!(a, edge);
        let n = if curved { (b - a + slice_height - Fixed(1)) / slice_height } else { 1 };
        for k in 1..=n {
            let next_edge = a + (b - a) * k / n;
            let next_cdf = gaussian.cdf(y_center - next_edge);
            let weight = edge_cdf - next_cdf;
            let middle = (edge + next_edge) / 2;
            edge = next_edge;
            edge_cdf = next_cdf;
            if weight == 0 {
                continue;
            }
            let x0 = left
                + corner_inset(radius.top_left, middle - top)
                    .max(corner_inset(radius.bottom_left, bottom - middle));
            let x1 = right
                - corner_inset(radius.top_right, middle - top)
                    .max(corner_inset(radius.bottom_right, bottom - middle));
            let close = |a: Fixed<i32, 8>, b: Fixed<i32, 8>| (a - b).0.abs() <= sigma.0;
            match band_count.checked_sub(1).map(|last| &mut bands[last]) {
                Some(last)
                    if band_count == MAX_BANDS
                        || (close(last.first.0, x0) && close(last.first.1, x1)) =>
                {
                    last.weight += weight;
                    last.sum.0 += weight as i64 * x0.0 as i64;
                    last.sum.1 += weight as i64 * x1.0 as i64;
                    last.slices += 1;
                }
                _ => {
                    bands[band_count] = Band {
                        x0,
                        x1,
                        weight,
                        first: (x0, x1),
                        sum: (weight as i64 * x0.0 as i64, weight as i64 * x1.0 as i64),
                        slices: 1,
                    };
                    band_count += 1;
                }
            }
        }
    }
    let bands = &mut bands[..band_count];
    for band in bands.iter_mut().filter(|b| b.slices > 1) {
        band.x0 = Fixed((band.sum.0 / band.weight as i64) as i32);
        band.x1 = Fixed((band.sum.1 / band.weight as i64) as i32);
    }
    let bands = &*bands;
    // Pixels this far inside every band see the CDFs saturate, so their coverage is constant.
    let (Some(inner_begin), Some(inner_end)) =
        (bands.iter().map(|b| b.x0).max(), bands.iter().map(|b| b.x1).min())
    else {
        return;
    };
    let (inner_begin, inner_end) = (inner_begin + cutoff, inner_end - cutoff);

    // Coverage is in Q30: weights and horizontal coverages are both Q15, and the weights sum to
    // at most 1.
    let scaled_color = |coverage: u32| scale_color(color, (coverage + (1 << 13)) >> 14);

    let first_x = extra_left_clip as i32;
    let len = line_buffer.len() as i32;
    let index = |v: Fixed<i32, 8>| (first_pixel_at(v) - first_x).clamp(0, len) as usize;
    let inner = if inner_begin <= inner_end {
        index(inner_begin)..index(inner_end + Fixed(1))
    } else {
        0..0
    };

    if let Some(c) = scaled_color(bands.iter().map(|b| b.weight).sum::<u32>() * GAUSSIAN_ONE) {
        TargetPixel::blend_slice(&mut line_buffer[inner.clone()], c);
    }
    // Forced inline, and a loop rather than `Iterator::sum`:
    // LLVM outlines either on thumbv6m, which costs a call per pixel.
    #[inline(always)]
    fn band_coverage(bands: &[Band], gaussian: Gaussian, x: i32) -> u32 {
        let x_center = Fixed::from_integer(x) + HALF_PIXEL;
        let mut coverage = 0;
        for b in bands {
            coverage += b.weight
                * gaussian.cdf(x_center - b.x0).saturating_sub(gaussian.cdf(x_center - b.x1));
        }
        coverage
    }
    // The coverage is smooth over σ, so evaluate it every σ/4 pixels and interpolate.
    // Interpolating skips the band loop for all but one pixel in `step`,
    // but costs more per pixel than a single band's loop.
    // The samples are anchored to the target buffer's origin, so they don't depend on the clip.
    let step = sigma.truncate() / 4;
    let step = if step >= 2 && 4 * band_count as i32 * (step - 1) >= INTERPOLATION_COST * step {
        step
    } else {
        1
    };
    let anchor = -(span.origin.x as i32);
    // `2^16 / step`, so the slope needs no division.
    let inv_step = (1 << 16) / step as i64;
    for range in [0..inner.start, inner.end..len as usize] {
        if step == 1 {
            for i in range {
                if let Some(c) = scaled_color(band_coverage(bands, gaussian, first_x + i as i32)) {
                    line_buffer[i].blend(c);
                }
            }
            continue;
        }
        // Interpolated in Q24, so the steps fit in i32.
        let q24 = |x: i32| (band_coverage(bands, gaussian, x) >> 6) as i32;
        let mut i = range.start;
        let mut sample_x = anchor + (first_x + i as i32 - anchor).div_euclid(step) * step;
        let mut c1 = q24(sample_x);
        while i < range.end {
            let c0 = c1;
            c1 = q24(sample_x + step);
            let slope = (((c1 - c0) as i64 * inv_step) >> 16) as i32;
            let mut value = c0 + slope * (first_x + i as i32 - sample_x);
            let segment_end = ((sample_x + step - first_x) as usize).min(range.end);
            for pixel in &mut line_buffer[i..segment_end] {
                // The slope is rounded down, so a falling segment can undershoot zero.
                if let Some(c) = scaled_color((value.max(0) as u32) << 6) {
                    pixel.blend(c);
                }
                value += slope;
            }
            i = segment_end;
            sample_x += step;
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

    /// Create a pixel from a 16-bit RGB565 value in native byte order
    /// (5 red bits, 6 green bits, 5 blue bits).
    ///
    /// The default implementation expands the components. RGB565 pixel
    /// types override this and use the value as-is.
    fn from_rgb565(value: u16) -> Self {
        let pixel = Rgb565Pixel(value);
        Self::from_rgb(pixel.red(), pixel.green(), pixel.blue())
    }

    /// Pixel which will be filled as the background in case the slint view has transparency
    fn background() -> Self {
        Self::from_rgb(0, 0, 0)
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

    fn from_rgb565(value: u16) -> Self {
        Self(value)
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

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self(Rgb565Pixel::from_rgb(r, g, b).0.to_be())
    }

    fn from_rgb565(value: u16) -> Self {
        // Same 565 bit layout, only the byte order differs.
        Self(value.to_be())
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

    assert_eq!(Rgb8Pixel::from_rgb565(white.0), Rgb8Pixel { r: 0xff, g: 0xff, b: 0xff });

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
fn target_pixel_from_rgb565() {
    let value = Rgb565Pixel::from_rgb(0x56, 0x42, 0xe3).0;

    // Native-endian 565 targets take the value as-is.
    assert_eq!(Rgb565Pixel::from_rgb565(value), Rgb565Pixel(value));

    // Big-endian 565 targets only swap the bytes.
    let be = Rgb565BigEndianPixel::from_rgb565(value);
    assert_eq!(be.0, value.to_be());
    assert_eq!(be, Rgb565BigEndianPixel::from_rgb(0x56, 0x42, 0xe3));

    // The default implementation expands like the accessors.
    let p = Rgb565Pixel(value);
    let rgb8 = Rgb8Pixel::from_rgb565(value);
    assert_eq!(rgb8, Rgb8Pixel { r: p.red(), g: p.green(), b: p.blue() });
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
fn gaussian_cdf_is_symmetric_monotonic_and_saturating() {
    for sigma in [1, 3 * 256 + 17, 40 * 256] {
        let gaussian = Gaussian::new(Fixed(sigma));
        let cdf = |d: i32| gaussian.cdf(Fixed(d));
        let cutoff = gaussian.cutoff().0;
        assert_eq!(cdf(0), GAUSSIAN_ONE / 2);
        assert_eq!(cdf(cutoff), GAUSSIAN_ONE);
        assert_eq!(cdf(cutoff + 1000), GAUSSIAN_ONE);
        assert_eq!(cdf(-cutoff), 0);
        assert_eq!(cdf(-cutoff - 1000), 0);
        let step = (sigma / 97).max(1);
        let mut previous = 0;
        for d in (-cutoff - 5 * step..=cutoff + 5 * step).step_by(step as usize) {
            assert_eq!(cdf(d) + cdf(-d), GAUSSIAN_ONE, "sigma {sigma}, d {d}");
            assert!(cdf(d) >= previous, "sigma {sigma}, d {d}");
            previous = cdf(d);
        }
    }
}

#[test]
fn gaussian_tail_matches_reference_values() {
    // Q(t) · 2^15 for t = 0.5, 1, 2, 3
    for (t, expected) in [(0.5, 10110.2), (1., 5198.8), (2., 745.5), (3., 44.2)] {
        let entry = GAUSSIAN_TAIL[(t * GAUSSIAN_TAIL_STEPS as f64) as usize] as f64;
        assert!((entry - expected).abs() <= 1., "Q({t}) = {entry}, expected {expected}");
    }
    assert!(GAUSSIAN_TAIL.array_windows().all(|[a, b]| a >= b));
}

#[test]
fn box_shadow_straight_row_is_separable() {
    use super::PhysicalPoint;
    let sigma = 4 * 256 + 37;
    let white = PremultipliedRgbaColor { red: 255, green: 255, blue: 255, alpha: 255 };
    let (left, right) = (Fixed(20 * 256 + 77), Fixed(60 * 256 + 3));
    let shadow = BoxShadowCommand {
        left,
        top: Fixed(20 * 256 + 5),
        right,
        bottom: Fixed(300 * 256),
        radius: BorderRadius::new_uniform(Fixed(10 * 256)),
        sigma: Fixed(sigma),
        color: white,
        profile: Some(StraightRowProfile::new(80, left, right, Gaussian::new(Fixed(sigma)))),
    };
    let span = PhysicalRect::new(PhysicalPoint::new(0, 0), PhysicalSize::new(80, 320));
    // More than 3σ away from both corner curves.
    let line = 150;
    let cdf = |d: i32| Gaussian::new(Fixed(sigma)).cdf(Fixed(d));
    let y_center = line * 256 + 128;
    let coverage_y = cdf(y_center - shadow.top.0) - cdf(y_center - shadow.bottom.0);

    for extra_left_clip in [0, 13] {
        let mut buffer = [PremultipliedRgbaColor::default(); 80];
        let buffer = &mut buffer[extra_left_clip as usize..];
        draw_box_shadow_line(
            &span,
            PhysicalLength::new(line as i16),
            &shadow,
            buffer,
            extra_left_clip,
        );
        for (x, pixel) in (extra_left_clip as i32..).zip(buffer.iter()) {
            let x_center = x * 256 + 128;
            let coverage_x = cdf(x_center - shadow.left.0) - cdf(x_center - shadow.right.0);
            let expected =
                ((((coverage_x * coverage_y) + (1 << 13)) >> 14) * 255 + (1 << 15)) >> 16;
            // The profile's 8-bit coverage rounds once more.
            assert!(
                pixel.alpha.abs_diff(expected as u8) <= 1,
                "x {x}: {} vs {expected}",
                pixel.alpha
            );
        }
    }
}

#[test]
fn box_shadow_bands_stay_close_to_the_exact_blur() {
    use super::PhysicalPoint;
    let phi = |t: f64| {
        let q = gaussian_q(t.abs().min(4.));
        if t >= 0. { 1. - q } else { q }
    };
    let corner_inset = |r: f64, distance: f64| {
        if distance >= r { 0. } else { r - (r * r - (r - distance).powi(2)).sqrt() }
    };
    let white = PremultipliedRgbaColor { red: 255, green: 255, blue: 255, alpha: 255 };
    for (sigma, radius) in [
        (1., 40.),
        (0.6, 16.),
        (1., 16.),
        (2.3, 30.),
        (4.1, 25.),
        (7.7, 40.),
        (5.2, 6.),
        (3.3, 3.),
        (8.3, 30.),
        (12.4, 40.),
        (16.7, 20.),
        (24.2, 40.),
        (12.1, 8.),
    ] {
        let px = |v: f64| Fixed((v * 256.) as i32);
        let (left, top, right, bottom) = (40.3, 30.6, 200.9, 180.2);
        let radius = [radius, radius * 0.7, radius * 0.4, radius * 0.9];
        let shadow = BoxShadowCommand {
            left: px(left),
            top: px(top),
            right: px(right),
            bottom: px(bottom),
            radius: BorderRadius::new(px(radius[0]), px(radius[1]), px(radius[2]), px(radius[3])),
            sigma: px(sigma),
            color: white,
            profile: Some(StraightRowProfile::new(
                240,
                px(left),
                px(right),
                Gaussian::new(px(sigma)),
            )),
        };
        let f = |v: Fixed<i32, 8>| v.0 as f64 / 256.;
        let (left, top, right, bottom, sigma) =
            (f(shadow.left), f(shadow.top), f(shadow.right), f(shadow.bottom), f(shadow.sigma));
        let r = [
            f(shadow.radius.top_left),
            f(shadow.radius.top_right),
            f(shadow.radius.bottom_right),
            f(shadow.radius.bottom_left),
        ];
        let span = PhysicalRect::new(PhysicalPoint::new(0, 0), PhysicalSize::new(240, 220));
        // The rows within reach of the corners.
        for line in (20..80).chain(130..190) {
            let mut buffer = [PremultipliedRgbaColor::default(); 240];
            draw_box_shadow_line(&span, PhysicalLength::new(line), &shadow, &mut buffer, 0);
            let y_center = line as f64 + 0.5;
            // Rows of 1/16 px, each close to a rectangle.
            let rows = ((top.max(y_center - 4. * sigma) * 16.).floor() as i32)
                ..((bottom.min(y_center + 4. * sigma) * 16.).ceil() as i32);
            let rows: Vec<_> = rows
                .map(|row| {
                    let (y0, y1) =
                        ((row as f64 / 16.).max(top), ((row + 1) as f64 / 16.).min(bottom));
                    let weight = phi((y_center - y0) / sigma) - phi((y_center - y1) / sigma);
                    let middle = (y0 + y1) / 2.;
                    let x0 = left
                        + corner_inset(r[0], middle - top).max(corner_inset(r[3], bottom - middle));
                    let x1 = right
                        - corner_inset(r[1], middle - top).max(corner_inset(r[2], bottom - middle));
                    (weight, x0, x1)
                })
                .collect();
            for (x, pixel) in buffer.iter().enumerate() {
                let x_center = x as f64 + 0.5;
                let coverage: f64 = rows
                    .iter()
                    .map(|(w, x0, x1)| {
                        w * (phi((x_center - x0) / sigma) - phi((x_center - x1) / sigma))
                    })
                    .sum();
                // Slices of σ/4 merged within σ stay within 3.1/255.
                let error = (pixel.alpha as f64 - coverage * 255.).abs();
                assert!(error <= 3.5, "sigma {sigma}, line {line}, x {x}: error {error}");
            }
        }
    }
}

#[test]
fn box_shadow_has_straight_rows_matches_a_row_scan() {
    let white = PremultipliedRgbaColor { red: 255, green: 255, blue: 255, alpha: 255 };
    let height = 60;
    // (top, bottom, top radius, bottom radius, sigma) in 1/256 px
    for (top, bottom, top_radius, bottom_radius, sigma) in [
        (10 * 256, 50 * 256, 0, 0, 4 * 256),
        (10 * 256 + 77, 50 * 256 + 3, 8 * 256, 5 * 256, 3 * 256 + 50),
        (10 * 256, 50 * 256, 16 * 256, 16 * 256, 2 * 256),
        (10 * 256, 50 * 256, 16 * 256, 16 * 256, 4 * 256),
        (-900 * 256, 50 * 256, 16 * 256, 0, 5 * 256),
        (10 * 256, 900 * 256, 0, 16 * 256, 5 * 256),
    ] {
        let shadow = BoxShadowCommand {
            left: Fixed(0),
            top: Fixed(top),
            right: Fixed(50 * 256),
            bottom: Fixed(bottom),
            radius: BorderRadius::new(
                Fixed(top_radius),
                Fixed(top_radius),
                Fixed(bottom_radius),
                Fixed(bottom_radius),
            ),
            sigma: Fixed(sigma),
            color: white,
            profile: None,
        };
        let cutoff = Gaussian::new(shadow.sigma).cutoff();
        let (top_curve_end, bottom_curve_start) = shadow.curve_bounds();
        let scanned = (0..height).any(|line| {
            let y_center = Fixed::from_integer(line) + HALF_PIXEL;
            shadow.top.max(y_center - cutoff) >= top_curve_end
                && shadow.bottom.min(y_center + cutoff) <= bottom_curve_start
        });
        assert_eq!(shadow.has_straight_rows(height), scanned, "top {top}, sigma {sigma}");
    }
}
