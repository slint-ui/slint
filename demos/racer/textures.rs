// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore flipv hypot texel

//! Procedural textures for the track, so the demo needs no asset files apart from the
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

fn shade(color: Rgba, factor: f32) -> Rgba {
    [color[0] * factor, color[1] * factor, color[2] * factor, color[3]]
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

/// The Slint logo, upside down like an image in three.js, so that `v` runs up the picture
/// as it runs up a plane.
pub fn logo() -> TextureData {
    let image =
        image::load_from_memory(include_bytes!("../../logo/slint-logo-simple-dark-large.png"))
            .expect("the logo is a valid PNG")
            .flipv()
            .into_rgba8();
    let (width, height) = image.dimensions();
    with_mipmaps(width, height, image.into_raw(), false)
}

/// A soft round spot in white with alpha, for light pools and the blob shadows.
pub fn radial() -> TextureData {
    const SIZE: u32 = 128;
    mipmapped(SIZE, SIZE, false, |x, y| {
        let r = ((x / SIZE as f32 - 0.5).hypot(y / SIZE as f32 - 0.5) * 2.0).min(1.0);
        let alpha = if r < 0.3 { 1.0 - r / 0.3 * 0.55 } else { 0.45 * (1.0 - (r - 0.3) / 0.7) };
        [1.0, 1.0, 1.0, alpha]
    })
}

/// The asphalt's color without its grain and markings.
pub const ASPHALT_BASE: u32 = 0x3a3d42;

/// Asphalt across the track (`u`) for 12 m along it (`v`), with white lines along the edges
/// and a dashed one down the middle.
pub fn asphalt() -> TextureData {
    const SIZE: u32 = 256;
    let base = rgb(ASPHALT_BASE);
    mipmapped(SIZE, SIZE, true, |x, y| {
        let (u, v) = (x / SIZE as f32, y / SIZE as f32);
        let grain = hash(x as u32, y as u32, 5) - 0.5;
        let patches = noise(x / 32.0, y / 32.0, SIZE / 32, 6) - 0.5;
        // The racing line, darkened by rubber.
        let rubber = 1.0 - 0.18 * (1.0 - ((u - 0.5) / 0.22).powi(2)).max(0.0);
        let asphalt = shade(base, (1.0 + grain * 0.22 + patches * 0.25) * rubber);
        let edge = (0.035..0.055).contains(&u) || (0.945..0.965).contains(&u);
        let dash = (u - 0.5).abs() < 0.008 && v < 0.5;
        if edge || dash { mix(asphalt, rgb(0xe8e8e2), 0.9 + grain * 0.2) } else { asphalt }
    })
}

/// The grass's color without its noise.
pub const GRASS_BASE: u32 = 0x3d5a2c;

/// Grass with mown stripes, for 16 m.
pub fn grass() -> TextureData {
    const SIZE: u32 = 256;
    let base = rgb(GRASS_BASE);
    mipmapped(SIZE, SIZE, true, |x, y| {
        let clumps = noise(x / 24.0, y / 24.0, SIZE.div_ceil(24), 8) - 0.5;
        let grain = hash(x as u32, y as u32, 9) - 0.5;
        let stripe = if ((y / 64.0).floor() as u32).is_multiple_of(2) { 1.06 } else { 0.94 };
        let mut color = shade(base, stripe * (1.0 + clumps * 0.35 + grain * 0.15));
        color[0] *= 1.0 + clumps * 0.3;
        color
    })
}

/// Red and white curb stripes, 1.5 m each along `v`.
pub fn curb() -> TextureData {
    mipmapped(16, 64, true, |x, y| {
        let grain = hash(x as u32, y as u32, 11) * 0.08;
        let color = if y < 32.0 { rgb(0xc8261e) } else { rgb(0xeeeeea) };
        shade(color, 0.92 + grain)
    })
}

/// Concrete with a Slint blue band on both faces of the barriers, across `u`, for 4 m
/// along `v`.
pub fn barrier() -> TextureData {
    const SIZE: u32 = 128;
    mipmapped(SIZE, SIZE, true, |x, y| {
        let u = x / SIZE as f32;
        let grain = hash(x as u32, y as u32, 12) - 0.5;
        let concrete = shade(rgb(0x9a9a96), 1.0 + grain * 0.12);
        // The faces are 0 to 0.39 and 0.61 to 1 of the profile, see `world::barrier_profile`.
        let band = (0.14..0.27).contains(&u) || (0.73..0.86).contains(&u);
        let gap = y < 6.0;
        if band && !gap { shade(rgb(0x2379f4), 0.85 + grain * 0.1) } else { concrete }
    })
}

/// Light concrete for the tunnels and stands.
pub fn concrete() -> TextureData {
    const SIZE: u32 = 128;
    mipmapped(SIZE, SIZE, true, |x, y| {
        let blotch = noise(x / 20.0, y / 20.0, SIZE.div_ceil(20), 13) - 0.5;
        let grain = hash(x as u32, y as u32, 14) - 0.5;
        let joint = x < 1.5 || y < 1.5;
        if joint { rgb(0x5a5a58) } else { shade(rgb(0xb0afa9), 1.0 + blotch * 0.2 + grain * 0.1) }
    })
}

/// White chevrons pointing along `v`, with alpha, and a glowing edge along both sides.
pub fn chevrons() -> TextureData {
    const SIZE: u32 = 64;
    mipmapped(SIZE, SIZE, true, |x, y| {
        let (u, v) = (x / SIZE as f32, y / SIZE as f32);
        let t = (v * 2.0 + (u - 0.5).abs() * 1.4).fract();
        let chevron = smoothstep(0.0, 0.04, t) * (1.0 - smoothstep(0.3, 0.34, t));
        let edge = 1.0 - smoothstep(0.04, 0.1, u.min(1.0 - u));
        [1.0, 1.0, 1.0, (chevron * 0.95 + edge * 0.8 + 0.12).min(1.0)]
    })
}

/// Black and white checks, two by two.
pub fn checker() -> TextureData {
    mipmapped(64, 64, true, |x, y| {
        if ((x / 32.0) as u32 + (y / 32.0) as u32).is_multiple_of(2) {
            rgb(0xf2f2f2)
        } else {
            rgb(0x111111)
        }
    })
}
