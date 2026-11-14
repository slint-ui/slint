// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Blending many pixels at once.
//!
//! The pixel types made of 8-bit channels blend each byte of the buffer alike,
//! `dst * (255 - alpha) / 255 + src`, 16 at a time: with SIMD instructions where the target has
//! them, and elsewhere in the bytes of a `usize`. `Rgb565Pixel` blends 8 pixels at a time where
//! the target has SIMD instructions.

use crate::target_pixel::{PremultipliedRgbaColor, TargetPixel, blend_opaque_per_pixel};
use i_slint_core::graphics::{Rgb565Pixel, Rgba8Pixel};
use wide::{u8x16, u16x8};

/// Whether `wide` uses SIMD instructions: the condition of its own code, which elsewhere works
/// on arrays.
const SIMD: bool = cfg!(any(
    target_feature = "sse2",
    target_feature = "simd128",
    all(target_feature = "neon", target_arch = "aarch64")
));

/// `t / 255`, rounded down, for `t` up to 255 * 255.
#[inline(always)]
fn div255(t: u16x8) -> u16x8 {
    (t + (t >> 8) + u16x8::splat(1)) >> 8
}

/// `bytes * factor / 255 + add` for each byte, rounded down and wrapping like `u8`.
#[inline(always)]
fn scale_add(bytes: &mut [u8; 16], factor: u8, add: &[u8; 16]) {
    if SIMD { scale_add_simd(bytes, factor, add) } else { scale_add_words(bytes, factor, add) }
}

#[inline(always)]
fn scale_add_simd(bytes: &mut [u8; 16], factor: u8, add: &[u8; 16]) {
    let factor = u16x8::splat(factor as u16);
    let x = u8x16::new(*bytes);
    let low = div255(u16x8::from_u8x16_low(x) * factor);
    let high = div255(u16x8::from_u8x16_high(x) * factor);
    let scaled = u8x16::narrow_i16x8(bytemuck::cast(low), bytemuck::cast(high));
    *bytes = (scaled + u8x16::new(*add)).to_array();
}

/// Every other byte, in the low half of 16-bit lanes where the products fit: one multiply for
/// several bytes.
#[inline(always)]
fn scale_add_words(bytes: &mut [u8; 16], factor: u8, add: &[u8; 16]) {
    const LOW: usize = usize::MAX / 0xffff * 0xff;
    const ONES: usize = usize::MAX / 0xffff;
    const N: usize = core::mem::size_of::<usize>();
    let factor = factor as usize;
    let scale_add = |x: usize, add: usize| {
        let t = (x & LOW) * factor;
        (((t + ((t >> 8) & LOW) + ONES) >> 8) + (add & LOW)) & LOW
    };
    for (word, add) in bytes.as_chunks_mut::<N>().0.iter_mut().zip(add.as_chunks::<N>().0) {
        let (x, add) = (usize::from_ne_bytes(*word), usize::from_ne_bytes(*add));
        *word = (scale_add(x, add) | (scale_add(x >> 8, add >> 8) << 8)).to_ne_bytes();
    }
}

/// [`scale_add`] on the bytes of 16 pixels.
#[inline(always)]
fn scale_add_pixels<P: bytemuck::Pod>(pixels: &mut [P; 16], factor: u8, add: &[P; 16]) {
    let bytes = bytemuck::cast_slice_mut::<P, u8>(pixels).as_chunks_mut::<16>().0;
    let add = bytemuck::cast_slice::<P, u8>(add).as_chunks::<16>().0;
    for (bytes, add) in bytes.iter_mut().zip(add) {
        scale_add(bytes, factor, add);
    }
}

/// [`TargetPixel::blend_slice`], `pixel` being `color` as a pixel, with its alpha if it has one.
#[inline(always)]
pub(crate) fn blend_slice<P: TargetPixel + bytemuck::Pod>(
    slice: &mut [P],
    color: PremultipliedRgbaColor,
    pixel: P,
) {
    if color.alpha == u8::MAX {
        return slice.fill(P::from_rgb(color.red, color.green, color.blue));
    }
    let (groups, rest) = slice.as_chunks_mut::<16>();
    for group in groups {
        scale_add_pixels(group, 255 - color.alpha, &[pixel; 16]);
    }
    for pix in rest {
        pix.blend(color);
    }
}

#[inline(always)]
pub(crate) fn blend_opaque<P: TargetPixel + bytemuck::Pod>(
    slice: &mut [P],
    src: &[Rgba8Pixel],
    alpha: u8,
) {
    let (groups, rest) = slice.as_chunks_mut::<16>();
    let (src_groups, src_rest) = src.as_chunks::<16>();
    if alpha == 0xff {
        for (group, src) in groups.iter_mut().zip(src_groups) {
            *group = src.map(|p| P::from_rgb(p.r, p.g, p.b));
        }
    } else {
        for (group, src) in groups.iter_mut().zip(src_groups) {
            let mut colors = src.map(|p| P::from_rgb(p.r, p.g, p.b));
            scale_add_pixels(&mut colors, alpha, &[P::zeroed(); 16]);
            scale_add_pixels(group, 255 - alpha, &colors);
        }
    }
    blend_opaque_per_pixel(rest, src_rest, alpha);
}

/// [`TargetPixel::blend_slice`] for `Rgb565Pixel`.
#[inline]
pub(crate) fn rgb565_blend_slice(slice: &mut [Rgb565Pixel], color: PremultipliedRgbaColor) {
    if color.alpha == u8::MAX {
        return slice.fill(Rgb565Pixel::from_rgb(color.red, color.green, color.blue));
    }
    let channel = |c: u8, bits: u8| u16x8::splat((c >> (8 - bits)) as u16);
    let src = [channel(color.red, 5), channel(color.green, 6), channel(color.blue, 5)];
    let (groups, rest) = rgb565_groups(slice);
    for pixels in groups {
        blend_rgb565(pixels, color.alpha, src);
    }
    for pix in rest {
        pix.blend(color);
    }
}

/// [`blend_opaque`] for `Rgb565Pixel`.
pub(crate) fn rgb565_blend_opaque(slice: &mut [Rgb565Pixel], src: &[Rgba8Pixel], alpha: u8) {
    let (groups, rest) = if alpha == 0xff { (&mut [][..], slice) } else { rgb565_groups(slice) };
    let (src_groups, _) = src.as_chunks::<8>();
    let src_rest = &src[groups.len() * 8..];
    let opacity = u16x8::splat(alpha as u16);
    for (pixels, src) in groups.iter_mut().zip(src_groups) {
        let channel =
            |c: [u8; 8], bits: u16| div255(u16x8::new(c.map(u16::from)) * opacity) >> (8 - bits);
        let [r, g, b] = [src.map(|p| p.r), src.map(|p| p.g), src.map(|p| p.b)];
        blend_rgb565(pixels, alpha, [channel(r, 5), channel(g, 6), channel(b, 5)]);
    }
    blend_opaque_per_pixel(rest, src_rest, alpha);
}

/// The groups of 8 pixels to blend with SIMD instructions, and the rest.
fn rgb565_groups(slice: &mut [Rgb565Pixel]) -> (&mut [[Rgb565Pixel; 8]], &mut [Rgb565Pixel]) {
    if SIMD { slice.as_chunks_mut() } else { (&mut [], slice) }
}

/// `Rgb565Pixel::blend` with a channel per 16-bit lane, `src` being the premultiplied red, green
/// and blue, reduced to 5, 6 and 5 bits.
#[inline(always)]
fn blend_rgb565(pixels: &mut [Rgb565Pixel; 8], alpha: u8, [r, g, b]: [u16x8; 3]) {
    let factor = u16x8::splat((255 - alpha as u16 + 4) >> 3);
    let v = u16x8::new(pixels.map(|p| p.0));
    let channel = |x: u16x8, mask: u16, src: u16x8| {
        let mask = u16x8::splat(mask);
        ((((x & mask) * factor) >> 5) + src) & mask
    };
    let out: u16x8 =
        (channel(v >> 11, 31, r) << 11) | (channel(v >> 5, 63, g) << 5) | channel(v, 31, b);
    *pixels = out.to_array().map(Rgb565Pixel);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Argb8888Pixel;
    use crate::target_pixel::RgbaTexel;
    use alloc::vec::Vec;
    use i_slint_core::InternalToken;
    use i_slint_core::graphics::Rgb8Pixel;

    fn random(len: usize, seed: &mut u32) -> Vec<u32> {
        (0..len)
            .map(|_| {
                *seed ^= *seed << 13;
                *seed ^= *seed >> 17;
                *seed ^= *seed << 5;
                *seed
            })
            .collect()
    }

    /// Compares the trait's `blend_slice` and `blend_rgba_slice` with blending pixel by pixel.
    fn matches_per_pixel<P: TargetPixel + PartialEq + core::fmt::Debug>(pixel: impl Fn(u32) -> P) {
        let mut seed = 1;
        for len in [0, 1, 7, 8, 9, 15, 16, 17, 33, 50] {
            let dst: Vec<P> = random(len, &mut seed).into_iter().map(&pixel).collect();
            let src: Vec<Rgba8Pixel> = random(len, &mut seed)
                .iter()
                .map(|p| bytemuck::cast::<[u8; 4], Rgba8Pixel>(p.to_le_bytes()))
                .collect();
            for alpha in [0, 1, 0x7f, 0x80, 0xfe, 0xff] {
                let [r, g, b, _] = random(1, &mut seed)[0].to_le_bytes();
                let premultiply = |c: u8| (c as u16 * alpha as u16 / 255) as u8;
                // A random color, and the extremes: channels at 0 or at the alpha.
                for (red, green, blue) in [
                    (premultiply(r), premultiply(g), premultiply(b)),
                    (0, alpha, alpha),
                    (alpha, 0, alpha),
                    (alpha, alpha, 0),
                    (alpha, alpha, alpha),
                ] {
                    let color = PremultipliedRgbaColor { alpha, red, green, blue };
                    let (mut a, mut b) = (dst.clone(), dst.clone());
                    TargetPixel::blend_slice(&mut a, color);
                    b.iter_mut().for_each(|p| p.blend(color));
                    assert_eq!(a, b, "{color:?}");
                }

                // Opaque, transparent and mixed chunks.
                for src_alpha in [Some(0xff), Some(0), None] {
                    let src: Vec<_> = src
                        .iter()
                        .map(|p| Rgba8Pixel { a: src_alpha.unwrap_or(p.a), ..*p })
                        .collect();
                    let (mut a, mut b) = (dst.clone(), dst.clone());
                    TargetPixel::blend_rgba_slice(&mut a, &src, alpha, InternalToken);
                    b.iter_mut().zip(&src).for_each(|(pix, p)| p.blend_into(pix, alpha));
                    assert_eq!(a, b);
                }
            }
        }
    }

    #[test]
    fn argb8888() {
        matches_per_pixel(Argb8888Pixel);
    }

    #[test]
    fn rgb8() {
        matches_per_pixel(|v| {
            let [r, g, b, _] = v.to_le_bytes();
            Rgb8Pixel::new(r, g, b)
        });
    }

    #[test]
    fn rgb565() {
        matches_per_pixel(|v| Rgb565Pixel(v as u16));
    }

    #[test]
    fn scale_add_is_exact() {
        for x in 0..=255u8 {
            for f in 0..=255u8 {
                let expected = [((x as u16 * f as u16 / 255) as u8).wrapping_add(3); 16];
                for scale_add in [scale_add_simd, scale_add_words] {
                    let mut bytes = [x; 16];
                    scale_add(&mut bytes, f, &[3; 16]);
                    assert_eq!(bytes, expected, "{x} * {f}");
                }
            }
        }
    }
}
