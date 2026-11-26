// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Blending many pixels at once.
//!
//! The pixel types made of 8-bit channels blend each byte of the buffer alike,
//! `dst * (255 - alpha) / 255 + src`, 32 pixels at a time: with SIMD instructions where the CPU
//! has them, and elsewhere in the bytes of a `usize`.
//! `Rgb565Pixel` blends 16 pixels at a time where the CPU has SIMD instructions.

use crate::target_pixel::{PremultipliedRgbaColor, TargetPixel, blend_opaque_per_pixel};
use fearless_simd::{Level, Simd, SimdBase, u8x32, u16x16};
use i_slint_core::graphics::{Rgb565Pixel, Rgba8Pixel};

const GROUP: usize = 32;

#[inline(always)]
fn level() -> Level {
    Level::try_detect().unwrap_or(Level::baseline())
}

/// `dispatch!`, except on x86 targets without SSE2, such as UEFI, where LLVM can't build its
/// SIMD arms.
macro_rules! simd_dispatch {
    ($level:expr, $simd:ident => $op:expr) => {{
        #[cfg(not(all(any(target_arch = "x86", target_arch = "x86_64"), not(target_feature = "sse2"))))]
        {
            fearless_simd::dispatch!($level, $simd => $op)
        }
        #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), not(target_feature = "sse2")))]
        {
            let _ = $level;
            let $simd = fearless_simd::Fallback::new();
            $op
        }
    }};
}

/// `t / 255`, rounded down, for `t` up to 255 * 255.
#[inline(always)]
fn div255<S: Simd>(t: u16x16<S>) -> u16x16<S> {
    (t + (t >> 8) + 1) >> 8
}

/// `bytes * factor / 255 + add` for each byte, rounded down and wrapping like `u8`.
#[inline(always)]
fn scale_add<S: Simd>(simd: S, bytes: &mut [u8; 32], factor: u8, add: &[u8; 32]) {
    if simd.level().is_fallback() {
        scale_add_words(bytes, factor, add)
    } else {
        scale_add_simd(simd, bytes, factor, add)
    }
}

#[inline(always)]
fn scale_add_simd<S: Simd>(simd: S, bytes: &mut [u8; 32], factor: u8, add: &[u8; 32]) {
    let x = u8x32::from_slice(simd, bytes);
    let (low, high) = simd.widening_mul_u8x32(x, simd.splat_u8x32(factor));
    let scaled = simd.narrow_u16x16(div255(low), div255(high));
    (scaled + u8x32::from_slice(simd, add)).store_slice(bytes);
}

/// Every other byte, in the low half of 16-bit lanes where the products fit: one multiply for
/// several bytes.
#[inline(always)]
fn scale_add_words(bytes: &mut [u8; 32], factor: u8, add: &[u8; 32]) {
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

/// [`scale_add`] on the bytes of a group of pixels.
#[inline(always)]
fn scale_add_pixels<S: Simd, P: bytemuck::Pod>(
    simd: S,
    pixels: &mut [P; GROUP],
    factor: u8,
    add: &[P; GROUP],
) {
    let bytes = bytemuck::cast_slice_mut::<P, u8>(pixels).as_chunks_mut::<32>().0;
    let add = bytemuck::cast_slice::<P, u8>(add).as_chunks::<32>().0;
    for (bytes, add) in bytes.iter_mut().zip(add) {
        scale_add(simd, bytes, factor, add);
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
    let (groups, rest) = slice.as_chunks_mut::<GROUP>();
    if !groups.is_empty() {
        simd_dispatch!(level(), simd => fill(simd, groups, 255 - color.alpha, &[pixel; GROUP]));
    }
    for pix in rest {
        pix.blend(color);
    }
}

#[inline(always)]
fn fill<S: Simd, P: bytemuck::Pod>(
    simd: S,
    groups: &mut [[P; GROUP]],
    factor: u8,
    add: &[P; GROUP],
) {
    for group in groups {
        scale_add_pixels(simd, group, factor, add);
    }
}

#[inline(always)]
pub(crate) fn blend_opaque<P: TargetPixel + bytemuck::Pod>(
    slice: &mut [P],
    src: &[Rgba8Pixel],
    alpha: u8,
) {
    let (groups, rest) = slice.as_chunks_mut::<GROUP>();
    let (src_groups, src_rest) = src.as_chunks::<GROUP>();
    if alpha == 0xff {
        for (group, src) in groups.iter_mut().zip(src_groups) {
            *group = src.map(|p| P::from_rgb(p.r, p.g, p.b));
        }
    } else if !groups.is_empty() {
        simd_dispatch!(level(), simd => fade(simd, groups, src_groups, alpha));
    }
    blend_opaque_per_pixel(rest, src_rest, alpha);
}

#[inline(always)]
fn fade<S: Simd, P: TargetPixel + bytemuck::Pod>(
    simd: S,
    groups: &mut [[P; GROUP]],
    src_groups: &[[Rgba8Pixel; GROUP]],
    alpha: u8,
) {
    for (group, src) in groups.iter_mut().zip(src_groups) {
        let mut colors = src.map(|p| P::from_rgb(p.r, p.g, p.b));
        scale_add_pixels(simd, &mut colors, alpha, &[P::zeroed(); GROUP]);
        scale_add_pixels(simd, group, 255 - alpha, &colors);
    }
}

/// [`TargetPixel::blend_slice`] for `Rgb565Pixel`.
#[inline]
pub(crate) fn rgb565_blend_slice(slice: &mut [Rgb565Pixel], color: PremultipliedRgbaColor) {
    if color.alpha == u8::MAX {
        return slice.fill(Rgb565Pixel::from_rgb(color.red, color.green, color.blue));
    }
    let level = level();
    let (groups, rest) = rgb565_groups(level, slice);
    if !groups.is_empty() {
        simd_dispatch!(level, simd => rgb565_fill(simd, groups, color));
    }
    for pix in rest {
        pix.blend(color);
    }
}

/// [`blend_opaque`] for `Rgb565Pixel`.
pub(crate) fn rgb565_blend_opaque(slice: &mut [Rgb565Pixel], src: &[Rgba8Pixel], alpha: u8) {
    let level = level();
    let (groups, rest) =
        if alpha == 0xff { (&mut [][..], slice) } else { rgb565_groups(level, slice) };
    let (src_groups, _) = src.as_chunks::<16>();
    let src_rest = &src[groups.len() * 16..];
    if !groups.is_empty() {
        simd_dispatch!(level, simd => rgb565_fade(simd, groups, src_groups, alpha));
    }
    blend_opaque_per_pixel(rest, src_rest, alpha);
}

#[inline(always)]
fn rgb565_fill<S: Simd>(simd: S, groups: &mut [[Rgb565Pixel; 16]], color: PremultipliedRgbaColor) {
    let src = [
        simd.splat_u16x16((color.red >> 3) as u16),
        simd.splat_u16x16((color.green >> 2) as u16),
        simd.splat_u16x16((color.blue >> 3) as u16),
    ];
    for pixels in groups {
        blend_rgb565(simd, pixels, color.alpha, src);
    }
}

#[inline(always)]
fn rgb565_fade<S: Simd>(
    simd: S,
    groups: &mut [[Rgb565Pixel; 16]],
    src_groups: &[[Rgba8Pixel; 16]],
    alpha: u8,
) {
    for (pixels, src) in groups.iter_mut().zip(src_groups) {
        let src = [
            rgb565_channel(simd, src.map(|p| p.r), alpha, 5),
            rgb565_channel(simd, src.map(|p| p.g), alpha, 6),
            rgb565_channel(simd, src.map(|p| p.b), alpha, 5),
        ];
        blend_rgb565(simd, pixels, alpha, src);
    }
}

/// `c * alpha / 255`, reduced to `bits` bits.
#[inline(always)]
fn rgb565_channel<S: Simd>(simd: S, c: [u8; 16], alpha: u8, bits: u32) -> u16x16<S> {
    div255(u16x16::from_slice(simd, &c.map(u16::from)) * alpha as u16) >> (8 - bits)
}

/// The groups of 16 pixels to blend with SIMD instructions, and the rest.
fn rgb565_groups(
    level: Level,
    slice: &mut [Rgb565Pixel],
) -> (&mut [[Rgb565Pixel; 16]], &mut [Rgb565Pixel]) {
    if level.is_fallback() { (&mut [], slice) } else { slice.as_chunks_mut() }
}

/// `Rgb565Pixel::blend` with a channel per 16-bit lane, `src` being the premultiplied red, green
/// and blue, reduced to 5, 6 and 5 bits.
#[inline(always)]
fn blend_rgb565<S: Simd>(
    simd: S,
    pixels: &mut [Rgb565Pixel; 16],
    alpha: u8,
    [r, g, b]: [u16x16<S>; 3],
) {
    let factor = (255 - alpha as u16 + 4) >> 3;
    let v = u16x16::from_slice(simd, bytemuck::cast_slice(pixels));
    #[inline(always)]
    fn channel<S: Simd>(x: u16x16<S>, mask: u16, factor: u16, src: u16x16<S>) -> u16x16<S> {
        ((((x & mask) * factor) >> 5) + src) & mask
    }
    let out = (channel(v >> 11, 31, factor, r) << 11)
        | (channel(v >> 5, 63, factor, g) << 5)
        | channel(v, 31, factor, b);
    out.store_slice(bytemuck::cast_slice_mut(pixels));
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
                let expected = [((x as u16 * f as u16 / 255) as u8).wrapping_add(3); 32];
                let mut bytes = [x; 32];
                scale_add_words(&mut bytes, f, &[3; 32]);
                assert_eq!(bytes, expected, "{x} * {f}");
                let mut bytes = [x; 32];
                simd_dispatch!(level(), simd => scale_add_simd(simd, &mut bytes, f, &[3; 32]));
                assert_eq!(bytes, expected, "{x} * {f}");
            }
        }
    }
}
