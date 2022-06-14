// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! This is the module for the functions that are drawing the pixels
//! on the line buffer

use crate::graphics::PixelFormat;
use crate::lengths::{PhysicalLength, PhysicalRect, PointLengths, SizeLengths};
use crate::Color;
use derive_more::{Add, Mul, Sub};
#[cfg(feature = "embedded-graphics")]
use embedded_graphics::prelude::RgbColor as _;
use integer_sqrt::IntegerSquareRoot;

/// Draw one line of the texture in the line buffer
pub(super) fn draw_texture_line(
    span: &PhysicalRect,
    line: PhysicalLength,
    texture: &super::SceneTexture,
    line_buffer: &mut [impl TargetPixel],
) {
    let super::SceneTexture { data, format, stride, source_size, color } = *texture;
    let source_size = source_size.cast::<usize>();
    let span_size = span.size.cast::<usize>();
    let bpp = super::bpp(format) as usize;
    let y = (line - span.origin.y_length()).cast::<usize>();
    let y_pos = (y.get() * source_size.height / span_size.height) * stride as usize;
    for (x, pix) in line_buffer
        [span.origin.x as usize..(span.origin.x_length() + span.size.width_length()).get() as usize]
        .iter_mut()
        .enumerate()
    {
        let pos = y_pos + (x * source_size.width / span_size.width) * bpp;
        let c = match format {
            PixelFormat::Rgb => {
                let p = &data[pos..pos + 3];
                *pix = TargetPixel::from_rgb(p[0], p[1], p[2]);
                continue;
            }
            PixelFormat::Rgba => {
                let alpha = data[pos + 3];
                Premultiplied::premultiply(if color.alpha() == 0 {
                    Color::from_argb_u8(alpha, data[pos + 0], data[pos + 1], data[pos + 2])
                } else {
                    Color::from_argb_u8(alpha, color.red(), color.green(), color.blue())
                })
            }
            PixelFormat::RgbaPremultiplied => {
                let alpha = data[pos + 3];
                if color.alpha() == 0 {
                    Premultiplied(Color::from_argb_u8(
                        alpha,
                        data[pos + 0],
                        data[pos + 1],
                        data[pos + 2],
                    ))
                } else {
                    Premultiplied::premultiply(Color::from_argb_u8(
                        alpha,
                        color.red(),
                        color.green(),
                        color.blue(),
                    ))
                }
            }
            PixelFormat::AlphaMap => Premultiplied::premultiply(Color::from_argb_u8(
                data[pos],
                color.red(),
                color.green(),
                color.blue(),
            )),
        };
        TargetPixel::blend_pixel(pix, c);
    }
}

/// draw one line of the rounded rectangle in the line buffer
pub(super) fn draw_rounded_rectangle_line(
    span: &PhysicalRect,
    line: PhysicalLength,
    rr: &super::RoundedRectangle,
    line_buffer: &mut [impl TargetPixel],
) {
    /// This is an integer shifted by 4 bits.
    /// Note: this is not a "fixed point" because multiplication and sqrt operation operate to
    /// the shifted integer
    #[derive(Clone, Copy, PartialEq, Ord, PartialOrd, Eq, Add, Sub, Mul)]
    struct Shifted(u32);
    impl Shifted {
        const ONE: Self = Shifted(1 << 4);
        pub fn new(value: impl TryInto<u32>) -> Self {
            Self(value.try_into().map_err(|_| ()).unwrap() << 4)
        }
        pub fn floor(self) -> u32 {
            self.0 >> 4
        }
        pub fn ceil(self) -> u32 {
            (self.0 + Self::ONE.0 - 1) >> 4
        }
        pub fn saturating_sub(self, other: Self) -> Self {
            Self(self.0.saturating_sub(other.0))
        }
        pub fn sqrt(self) -> Self {
            Self(self.0.integer_sqrt())
        }
    }
    impl core::ops::Mul for Shifted {
        type Output = Shifted;
        fn mul(self, rhs: Self) -> Self::Output {
            Self(self.0 * rhs.0)
        }
    }
    let pos_x = span.origin.x as usize;
    let y1 = (line - span.origin.y_length()) + rr.top_clip;
    let y2 = (span.origin.y_length() + span.size.height_length() - line) + rr.bottom_clip
        - PhysicalLength::new(1);
    let y = y1.min(y2);
    debug_assert!(y.get() >= 0,);
    let border = Shifted::new(rr.width.get());
    const ONE: Shifted = Shifted::ONE;
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
        (Shifted::new(span.size.width) + Shifted::new(rr.right_clip.get())).saturating_sub(x)
    };
    let (x1, x2, x3, x4) = if y < rr.radius {
        let r = Shifted::new(rr.radius.get());
        // `y` is how far away from the center of the circle the current line is.
        let y = r - Shifted::new(y.get());
        // Circle equation: x = √(r² - y²)
        // Coordinate from the left edge: x' = r - x
        let x2 = r - (r * r).saturating_sub(y * y).sqrt();
        let x1 = r - (r * r).saturating_sub((y - ONE) * (y - ONE)).sqrt();
        let r2 = r.saturating_sub(border);
        let x4 = r - (r2 * r2).saturating_sub(y * y).sqrt();
        let x3 = r - (r2 * r2).saturating_sub((y - ONE) * (y - ONE)).sqrt();
        (x1, x2, x3, x4)
    } else {
        (Shifted(0), Shifted(0), border, border)
    };
    anti_alias(
        x1.saturating_sub(Shifted::new(rr.left_clip.get())),
        x2.saturating_sub(Shifted::new(rr.left_clip.get())),
        &mut |x, cov| {
            if x >= span.size.width as usize {
                return;
            }
            let c = if border == Shifted(0) { rr.inner_color } else { rr.border_color };
            let alpha = ((c.alpha() as u32) * cov as u32) / 255;
            let col = Color::from_argb_u8(alpha as u8, c.red(), c.green(), c.blue());
            TargetPixel::blend_pixel(&mut line_buffer[pos_x + x], Premultiplied::premultiply(col))
        },
    );
    if y < rr.width {
        // up or down border (x2 .. x2)
        let l = x2.ceil().saturating_sub(rr.left_clip.get() as u32).min(span.size.width as u32)
            as usize;
        let r = rev(x2).floor().min(span.size.width as u32) as usize;
        if l < r {
            TargetPixel::blend_buffer(&mut line_buffer[pos_x + l..pos_x + r], rr.border_color)
        }
    } else {
        if border > Shifted(0) {
            // 3. draw the border (between x2 and x3)
            if ONE + x2 <= x3 {
                TargetPixel::blend_buffer(
                    &mut line_buffer[pos_x
                        + x2.ceil()
                            .saturating_sub(rr.left_clip.get() as u32)
                            .min(span.size.width as u32) as usize
                        ..pos_x
                            + x3.floor()
                                .saturating_sub(rr.left_clip.get() as u32)
                                .min(span.size.width as u32)
                                as usize],
                    rr.border_color,
                )
            }
            // 4. anti-aliasing for the contents (x3 .. x4)
            anti_alias(
                x3.saturating_sub(Shifted::new(rr.left_clip.get())),
                x4.saturating_sub(Shifted::new(rr.left_clip.get())),
                &mut |x, cov| {
                    if x >= span.size.width as usize {
                        return;
                    }
                    let col = interpolate_color(cov, rr.border_color, rr.inner_color);
                    TargetPixel::blend_pixel(&mut line_buffer[pos_x + x], col)
                },
            );
        }
        // 5. inside (x4 .. x4)
        let begin = x4.ceil().saturating_sub(rr.left_clip.get() as u32).min(span.size.width as u32);
        let end = rev(x4).floor().min(span.size.width as u32);
        if begin < end {
            TargetPixel::blend_buffer(
                &mut line_buffer[pos_x + begin as usize..pos_x + end as usize],
                rr.inner_color,
            )
        }
        if border > Shifted(0) {
            // 6. border anti-aliasing: x4..x3
            anti_alias(rev(x4), rev(x3), &mut |x, cov| {
                if x >= span.size.width as usize {
                    return;
                }
                let col = interpolate_color(cov, rr.inner_color, rr.border_color);
                TargetPixel::blend_pixel(&mut line_buffer[pos_x + x], col)
            });
            // 7. border x3 .. x2
            if ONE + x2 <= x3 {
                TargetPixel::blend_buffer(
                    &mut line_buffer[pos_x + rev(x3).ceil().min(span.size.width as u32) as usize
                        ..pos_x + rev(x2).floor().min(span.size.width as u32) as usize as usize],
                    rr.border_color,
                )
            }
        }
    }
    anti_alias(rev(x2), rev(x1), &mut |x, cov| {
        if x >= span.size.width as usize {
            return;
        }
        let c = if border == Shifted(0) { rr.inner_color } else { rr.border_color };
        let alpha = ((c.alpha() as u32) * (255 - cov) as u32) / 255;
        let col = Color::from_argb_u8(alpha as u8, c.red(), c.green(), c.blue());
        TargetPixel::blend_pixel(&mut line_buffer[pos_x + x], Premultiplied::premultiply(col))
    });
}

// a is between 0 and 255. When 0, we get color1, when 2 we get color2
fn interpolate_color(a: u32, color1: Color, color2: Color) -> Premultiplied {
    let b = 255 - a;

    let al1 = color1.alpha() as u32;
    let al2 = color2.alpha() as u32;

    let a_ = a * al2;
    let b_ = b * al1;
    let m = a_ + b_;

    if m == 0 {
        return Premultiplied(Color::default());
    }

    let div = 255 * 255;
    Premultiplied(Color::from_argb_u8(
        (m / 255) as u8,
        ((b_ * color1.red() as u32 + a_ * color2.red() as u32) / div) as u8,
        ((b_ * color1.green() as u32 + a_ * color2.green() as u32) / div) as u8,
        ((b_ * color1.blue() as u32 + a_ * color2.blue() as u32) / div) as u8,
    ))
}

/// Wrap a color whose component have been pre-multiplied by alpha
#[derive(Clone, Copy)]
pub struct Premultiplied(pub Color);

impl Premultiplied {
    /// Convert a non premultiplied color to a premultiplied one
    pub fn premultiply(col: Color) -> Self {
        let a = col.alpha() as u16;
        Self(Color::from_argb_u8(
            col.alpha(),
            (col.red() as u16 * a / 255) as u8,
            (col.green() as u16 * a / 255) as u8,
            (col.blue() as u16 * a / 255) as u8,
        ))
    }
}

/// Trait for the pixels in the buffer
pub trait TargetPixel: Sized + Copy {
    /// blend a single pixel
    fn blend_pixel(pix: &mut Self, color: Premultiplied);
    /// Fill (or blend) the color in the buffer
    fn blend_buffer(to_fill: &mut [Self], color: Color);
    /// Create a pixel from the red, gree, blue component in the range 0..=255
    fn from_rgb(r: u8, g: u8, b: u8) -> Self;
}

#[cfg(feature = "embedded-graphics")]
impl TargetPixel for embedded_graphics::pixelcolor::Rgb888 {
    fn blend_buffer(to_fill: &mut [Self], color: Color) {
        if color.alpha() == u8::MAX {
            to_fill.fill(Self::new(color.red(), color.green(), color.blue()))
        } else {
            for pix in to_fill {
                Self::blend_pixel(pix, Premultiplied::premultiply(color));
            }
        }
    }

    fn blend_pixel(pix: &mut Self, color: Premultiplied) {
        let a = (u8::MAX - color.0.alpha()) as u16;
        *pix = Self::new(
            (pix.r() as u16 * a / 255) as u8 + color.0.red(),
            (pix.g() as u16 * a / 255) as u8 + color.0.green(),
            (pix.b() as u16 * a / 255) as u8 + color.0.blue(),
        );
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b)
    }
}

#[cfg(feature = "embedded-graphics")]
impl TargetPixel for embedded_graphics::pixelcolor::Rgb565 {
    fn blend_buffer(to_fill: &mut [Self], color: Color) {
        if color.alpha() == u8::MAX {
            to_fill.fill(Self::from_rgb(color.red(), color.green(), color.blue()))
        } else {
            for pix in to_fill {
                Self::blend_pixel(pix, Premultiplied::premultiply(color));
            }
        }
    }

    fn blend_pixel(pix: &mut Self, color: Premultiplied) {
        let a = (u8::MAX - color.0.alpha()) as u16;
        *pix = Self::new(
            (((pix.r() as u16) * a) / 255) as u8 + (color.0.red() >> 3),
            (((pix.g() as u16) * a) / 255) as u8 + (color.0.green() >> 2),
            (((pix.b() as u16) * a) / 255) as u8 + (color.0.blue() >> 3),
        )
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r >> 3, g >> 2, b >> 3)
    }
}
