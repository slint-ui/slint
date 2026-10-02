// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore ilog prefiltered softbox softboxes texel

//! Small procedural textures with full mipmap chains, so the demo needs no asset files.

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::math::Vec3;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};

type Rgb = [f32; 3];

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

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t.clamp(0.0, 1.0))
}

fn srgb(hex: u32) -> Rgb {
    [(hex >> 16) as f32 / 255.0, ((hex >> 8) & 0xff) as f32 / 255.0, (hex & 0xff) as f32 / 255.0]
}

/// Renders a square sRGB texture of `size` pixels and box-filters it down to 1x1.
fn mipmapped(size: u32, repeat: bool, pixel: impl Fn(f32, f32) -> Rgb) -> Image {
    let mut level: Vec<Rgb> =
        (0..size * size).map(|i| pixel((i % size) as f32 + 0.5, (i / size) as f32 + 0.5)).collect();
    let mut data = Vec::new();
    let mut width = size;
    loop {
        data.extend(level.iter().flat_map(|c| {
            let [r, g, b] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
            [r, g, b, 255]
        }));
        if width == 1 {
            break;
        }
        let half = width / 2;
        level = (0..half * half)
            .map(|i| {
                let (x, y) = ((i % half) * 2, (i / half) * 2);
                let at = |dx, dy| level[((y + dy) * width + x + dx) as usize];
                std::array::from_fn(|c| {
                    (at(0, 0)[c] + at(1, 0)[c] + at(0, 1)[c] + at(1, 1)[c]) / 4.0
                })
            })
            .collect();
        width = half;
    }

    let mut image = Image::new_uninit(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.mip_level_count = size.ilog2() + 1;
    image.data = Some(data);
    let address_mode =
        if repeat { ImageAddressMode::Repeat } else { ImageAddressMode::ClampToEdge };
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: address_mode,
        address_mode_v: address_mode,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..Default::default()
    });
    image
}

/// One 0.5 m concrete floor tile with a seam along its edges.
pub fn concrete() -> Image {
    const SIZE: u32 = 256;
    let (dark, light, seam) = (srgb(0x3b4047), srgb(0x50565d), srgb(0x25292e));
    mipmapped(SIZE, true, |x, y| {
        let grain = noise(x / 32.0, y / 32.0, SIZE / 32, 1) * 0.6
            + noise(x / 8.0, y / 8.0, SIZE / 8, 2) * 0.3
            + hash(x as u32, y as u32, 3) * 0.1;
        let edge = x.min(y).min(SIZE as f32 - x).min(SIZE as f32 - y);
        mix(mix(dark, light, grain), seam, 1.0 - (edge - 1.0) / 2.0)
    })
}

/// Diagonal hazard stripes, one stripe pair per texture width.
pub fn hazard() -> Image {
    const SIZE: u32 = 64;
    let (yellow, black) = (srgb(0xc29a2a), srgb(0x16181c));
    mipmapped(SIZE, true, |x, y| {
        let t = ((x + y) / SIZE as f32).fract();
        let wear = noise(x / 8.0, y / 8.0, SIZE / 8, 4) * 0.25;
        mix(if t < 0.5 { yellow } else { black }, srgb(0x3a3d42), wear)
    })
}

/// A fixture's machined steel top: brushed grain, an engraved border, and corner marks.
pub fn fixture_top() -> Image {
    const SIZE: u32 = 128;
    let (steel, groove, mark) = (srgb(0xa9b0b9), srgb(0x50565d), srgb(0xe4e7eb));
    mipmapped(SIZE, false, |x, y| {
        let (u, v) = (x / SIZE as f32, y / SIZE as f32);
        let brushed = noise(x / 64.0, y / 1.5, 2, 6) * 0.12 + hash(x as u32, y as u32, 7) * 0.04;
        let inset = (u - 0.5).abs().max((v - 0.5).abs());
        let corner = |a: f32, b: f32| a < 0.16 && b < 0.035;
        let (cu, cv) = (0.5 - (u - 0.5).abs() - 0.06, 0.5 - (v - 0.5).abs() - 0.06);
        if (cu >= 0.0 && cv >= 0.0) && (corner(cu, cv) || corner(cv, cu)) {
            mark
        } else if (inset - 0.47).abs() < 0.006 {
            groove
        } else {
            steel.map(|c| c - 0.06 + brushed)
        }
    })
}

/// A cardboard box face: a darker frame and a white label.
pub fn crate_face() -> Image {
    const SIZE: u32 = 128;
    let (body, frame, label) = (srgb(0xc4914f), srgb(0x9c6c35), srgb(0xeef2f7));
    mipmapped(SIZE, false, |x, y| {
        let (u, v) = (x / SIZE as f32, y / SIZE as f32);
        let edge = u.min(v).min(1.0 - u).min(1.0 - v);
        let grain = noise(x / 4.0, y / 32.0, 33, 5) * 0.12;
        if edge < 0.09 {
            frame.map(|c| c - grain)
        } else if (0.3..0.7).contains(&u) && (0.6..0.78).contains(&v) {
            if (0.34..0.66).contains(&u) && (v - 0.69).abs() < 0.012 {
                srgb(0x2379f4)
            } else {
                label
            }
        } else {
            body.map(|c| c - grain)
        }
    })
}

/// A light in the studio environment, as a soft disc on the sphere around the scene.
struct Softbox {
    direction: Vec3,
    radius: f32,
    brightness: f32,
}

/// The radiance of a dark studio with a few softboxes, as seen from a surface of the given
/// roughness. Blurring the softboxes with roughness and keeping their energy approximates a
/// prefiltered environment map.
fn studio(direction: Vec3, roughness: f32) -> Rgb {
    let softboxes = [
        Softbox { direction: Vec3::new(0.25, 1.0, 0.35), radius: 0.5, brightness: 1.0 },
        Softbox { direction: Vec3::new(-1.0, 0.35, 0.4), radius: 0.28, brightness: 0.7 },
        Softbox { direction: Vec3::new(0.4, 0.45, -1.0), radius: 0.3, brightness: 0.55 },
        // Fill from behind the default camera position, for highlights on the near sides.
        Softbox { direction: Vec3::new(0.6, 0.25, 0.8), radius: 0.45, brightness: 0.5 },
    ];
    let up = direction.y;
    let (floor, horizon, ceiling) = (srgb(0x2a3036), srgb(0x6f7984), srgb(0x4a525b));
    let mut color = if up >= 0.0 {
        mix(horizon, ceiling, up.sqrt())
    } else {
        mix(horizon, floor, (-up).sqrt())
    };
    let blur = 0.02 + roughness * 1.1;
    for softbox in &softboxes {
        let angle = direction.angle_between(softbox.direction.normalize());
        let edge = ((softbox.radius + blur - angle) / (2.0 * blur)).clamp(0.0, 1.0);
        let falloff = edge * edge * (3.0 - 2.0 * edge);
        let energy = (softbox.radius / (softbox.radius + blur)).powi(2);
        let light = softbox.brightness * energy * falloff;
        color = color.map(|c| c + light);
    }
    color
}

/// The direction through texel (`x`, `y`) of cube face `face`, in wgpu's face order.
fn cube_direction(face: u32, x: f32, y: f32, size: u32) -> Vec3 {
    let (u, v) = (2.0 * x / size as f32 - 1.0, 2.0 * y / size as f32 - 1.0);
    match face {
        0 => Vec3::new(1.0, -v, -u),
        1 => Vec3::new(-1.0, -v, u),
        2 => Vec3::new(u, 1.0, v),
        3 => Vec3::new(u, -1.0, -v),
        4 => Vec3::new(u, -v, 1.0),
        _ => Vec3::new(-u, -v, -1.0),
    }
    .normalize()
}

fn studio_cube(size: u32, mip_levels: u32, roughness_of_level: impl Fn(u32) -> f32) -> Image {
    let mut data = Vec::new();
    for face in 0..6 {
        for level in 0..mip_levels {
            let level_size = (size >> level).max(1);
            let roughness = roughness_of_level(level);
            for i in 0..level_size * level_size {
                let (x, y) = ((i % level_size) as f32 + 0.5, (i / level_size) as f32 + 0.5);
                let [r, g, b] = studio(cube_direction(face, x, y, level_size), roughness)
                    .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
                data.extend([r, g, b, 255]);
            }
        }
    }
    let mut image = Image::new_uninit(
        Extent3d { width: size, height: size, depth_or_array_layers: 6 },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.mip_level_count = mip_levels;
    image.data = Some(data);
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..Default::default()
    });
    image
}

/// The studio's specular map, where Bevy picks the mip level from the surface roughness.
pub fn studio_specular() -> Image {
    const SIZE: u32 = 64;
    let levels = SIZE.ilog2() + 1;
    studio_cube(SIZE, levels, |level| level as f32 / (levels - 1) as f32)
}

/// The studio's diffuse map, blurred further than the roughest specular level.
pub fn studio_diffuse() -> Image {
    studio_cube(16, 1, |_| 1.6)
}
