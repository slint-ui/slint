// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore flipv hypot texel

//! Procedural textures for the racing hall, so the demo needs no asset files apart from the
//! Slint logo.

use crate::renderer::TextureData;

/// Linear-blended sRGB color with alpha.
type Rgba = [f32; 4];

fn hash(x: u32, y: u32, seed: u32) -> f32 {
    let mut h =
        x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

/// Smooth value noise in 0..1 that tiles every `period` lattice cells.
fn noise(x: f32, y: f32, period: u32, seed: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let cell = |dx: u32, dy: u32| hash((x0 as u32 + dx) % period, (y0 as u32 + dy) % period, seed);
    let top = cell(0, 0) + (cell(1, 0) - cell(0, 0)) * sx;
    let bottom = cell(0, 1) + (cell(1, 1) - cell(0, 1)) * sx;
    top + (bottom - top) * sy
}

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn rgb(hex: u32) -> Rgba {
    [
        (hex >> 16) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
        1.0,
    ]
}

fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t.clamp(0.0, 1.0))
}

fn to_rgba8(color: Rgba) -> [u8; 4] {
    color.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// A `width` x `height` sRGB texture with its mipmaps.
fn mipmapped(
    width: u32,
    height: u32,
    repeat: bool,
    pixel: impl Fn(f32, f32) -> Rgba,
) -> TextureData {
    let texels = (0..width * height)
        .flat_map(|i| to_rgba8(pixel((i % width) as f32 + 0.5, (i / width) as f32 + 0.5)))
        .collect();
    with_mipmaps(width, height, texels, repeat)
}

/// An sRGB channel in linear light, from 0 to 1.
pub fn srgb_to_linear(value: u8) -> f32 {
    let c = value as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// Adds the mipmaps to `texels`, averaging each 2x2 block in linear light.
fn with_mipmaps(width: u32, height: u32, texels: Vec<u8>, repeat: bool) -> TextureData {
    // A table, since a pow per texel is slow on small CPUs.
    let linear: [f32; 256] = std::array::from_fn(|v| srgb_to_linear(v as u8));
    let to_linear = |v: u8| linear[v as usize];
    let to_srgb = |c: f32| {
        let c = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
        (c.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    let mut levels = vec![texels];
    let (mut w, mut h) = (width, height);
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let previous = levels.last().unwrap();
        let mut next = Vec::with_capacity((nw * nh * 4) as usize);
        for y in 0..nh {
            for x in 0..nw {
                let mut sum = [0.0; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((x * 2 + dx).min(w - 1), (y * 2 + dy).min(h - 1));
                    let texel = &previous[((sy * w + sx) * 4) as usize..][..4];
                    for c in 0..3 {
                        sum[c] += to_linear(texel[c]) * texel[3] as f32;
                    }
                    sum[3] += texel[3] as f32;
                }
                // Weighted by alpha, so transparent texels don't darken the edges.
                let alpha = sum[3] / 4.0;
                for c in 0..3 {
                    next.push(to_srgb(if sum[3] > 0.0 { sum[c] / sum[3] } else { 0.0 }));
                }
                next.push(alpha.round() as u8);
            }
        }
        levels.push(next);
        (w, h) = (nw, nh);
    }
    TextureData { width, height, levels, repeat }
}

/// The Slint logo for the screens on the walls, upside down like an image in three.js, so
/// that `v` runs up the picture as it runs up a plane.
pub fn logo() -> TextureData {
    let image =
        image::load_from_memory(include_bytes!("../../logo/slint-logo-simple-dark-large.png"))
            .expect("the logo is a valid PNG")
            .flipv()
            .into_rgba8();
    let (width, height) = image.dimensions();
    with_mipmaps(width, height, image.into_raw(), false)
}

/// The floor texture's color without its blotches, grain and seams.
pub const FLOOR_BASE: u32 = 0x24272c;

/// Polished concrete with soft blotches and a dark seam along two edges.
pub fn floor() -> TextureData {
    const SIZE: u32 = 256;
    let base = rgb(FLOOR_BASE);
    mipmapped(SIZE, SIZE, true, |x, y| {
        let blotch = noise(x / 40.0, y / 40.0, SIZE.div_ceil(40), 1) - 0.5;
        let grain = hash(x as u32, y as u32, 2) - 0.5;
        let shade = 1.0 + blotch * 0.35 + grain * 0.12;
        if x < 1.5 || y < 1.5 {
            rgb(0x0a0b0d)
        } else {
            [base[0] * shade, base[1] * shade, base[2] * shade + grain * 0.004, 1.0]
        }
    })
}

/// Vertical wall panels with dark joints and a faint highlight next to each joint.
pub fn wall() -> TextureData {
    const SIZE: u32 = 256;
    mipmapped(SIZE, SIZE, true, |x, y| {
        let column = x % 32.0;
        if y < 4.0 {
            rgb(0x262a31)
        } else if column < 3.0 {
            rgb(0x2d3239)
        } else if (4.0..6.0).contains(&column) {
            mix(rgb(0x3d434c), [1.0; 4], 0.06)
        } else {
            rgb(0x3d434c)
        }
    })
}

/// Half the side of the square outline that `halo` glows around, from the texture's center,
/// spanning 55% of the texture.
pub const HALO_OUTLINE: f32 = 0.273;
/// How far `halo` glows from its outline, until its alpha drops below 0.0004.
pub const HALO_REACH: f32 = 2.7 * HALO_GLOW;
const HALO_GLOW: f32 = 0.035;

/// The glow around a square gate frame, in white with alpha.
pub fn halo() -> TextureData {
    const SIZE: u32 = 128;
    mipmapped(SIZE, SIZE, false, |x, y| {
        // Distance to the outline.
        let (u, v) = ((x / SIZE as f32 - 0.5).abs(), (y / SIZE as f32 - 0.5).abs());
        let half = HALO_OUTLINE;
        let distance = if u < half && v < half {
            half - u.max(v)
        } else {
            (u - half).max(0.0).hypot((v - half).max(0.0))
        };
        let wide = 0.55 * (-(distance / HALO_GLOW).powi(2)).exp();
        let tight = 0.8 * (-(distance / 0.02).powi(2)).exp();
        [1.0, 1.0, 1.0, (wide + tight).min(1.0)]
    })
}

/// A soft round spot in white with alpha, for light pools and the blob shadow.
pub fn radial() -> TextureData {
    const SIZE: u32 = 128;
    mipmapped(SIZE, SIZE, false, |x, y| {
        let r = ((x / SIZE as f32 - 0.5).hypot(y / SIZE as f32 - 0.5) * 2.0).min(1.0);
        let alpha = if r < 0.3 { 1.0 - r / 0.3 * 0.55 } else { 0.45 * (1.0 - (r - 0.3) / 0.7) };
        [1.0, 1.0, 1.0, alpha]
    })
}

/// A light shaft, bright at the top (v = 0) and fading out at the bottom.
pub fn shaft() -> TextureData {
    mipmapped(16, 128, false, |_, y| {
        let v = y / 128.0;
        let alpha = if v < 0.25 { 0.9 - v / 0.25 * 0.55 } else { 0.35 * (1.0 - (v - 0.25) / 0.75) };
        [1.0, 1.0, 1.0, alpha]
    })
}

/// A spinning propeller: a faint disc with a brighter rim and two dark blades.
pub fn propeller() -> TextureData {
    const SIZE: u32 = 128;
    mipmapped(SIZE, SIZE, false, |x, y| {
        let (dx, dy) = (x / SIZE as f32 - 0.5, y / SIZE as f32 - 0.5);
        let r = dx.hypot(dy) * 2.0;
        let disc = if r > 1.0 {
            [0.0; 4]
        } else {
            let rim = smoothstep(0.6, 0.95, r) * (1.0 - smoothstep(0.95, 1.0, r));
            [0.85, 0.88, 0.93, 0.05 + rim * 0.16]
        };
        let blade = (dy.abs() < 0.04 * (1.0 - (dx.abs() * 2.0 - 0.5).abs())) && r < 0.95;
        if blade { [0.12, 0.13, 0.15, 0.55] } else { disc }
    })
}

/// One dash of the route line on the floor, in white with alpha.
pub fn dash() -> TextureData {
    mipmapped(16, 64, true, |_, y| [1.0, 1.0, 1.0, if y < 26.0 { 1.0 } else { 0.0 }])
}

/// White bands on dark, for the LED towers.
pub fn stripes() -> TextureData {
    mipmapped(16, 256, true, |_, y| {
        if (y - 8.0).rem_euclid(32.0) < 6.0 { [1.0; 4] } else { rgb(0x16181c) }
    })
}

/// The launch pad: a square outline, a circle, and a mark pointing along the course.
pub fn pad() -> TextureData {
    const SIZE: u32 = 128;
    mipmapped(SIZE, SIZE, false, |x, y| {
        let outline = (x.min(y).min(SIZE as f32 - x).min(SIZE as f32 - y) - 12.5).abs() < 2.5;
        let circle = ((x - 64.0).hypot(y - 64.0) - 26.0).abs() < 1.0;
        let mark = (62.0..66.0).contains(&x) && (14.0..34.0).contains(&y);
        if outline || circle || mark { [1.0; 4] } else { rgb(0x14161a) }
    })
}
