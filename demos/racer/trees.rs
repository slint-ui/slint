// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Medium and high quality's trees: spruces with drooping, ragged tiers of branches, and a broadleaf
//! tree with a lumpy crown. Their foliage is shaded as a soft volume, with normals that point
//! out from the tree rather than across each facet.

use std::f32::consts::{PI, TAU};

use glam::{Quat, Vec3};

use crate::geometry::{self, orient_to_normals};
use crate::renderer::Geometry;
use crate::world::hex;

/// Repeatable noise from -1 to 1 for `seed` and `index`.
fn jitter(seed: u32, index: u32) -> f32 {
    let mut h = seed.wrapping_mul(0x9e37_79b9) ^ index.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 32767.5 - 1.0
}

fn push(geometry: &mut Geometry, position: Vec3, normal: Vec3, color: Vec3) -> u32 {
    geometry.positions.push(position.to_array());
    geometry.normals.push(normal.normalize_or(Vec3::Y).to_array());
    geometry.uvs.push([0.0, 0.0]);
    geometry.colors.push(color.to_array());
    geometry.positions.len() as u32 - 1
}

fn bark(geometry: &mut Geometry, height: f32, bottom_radius: f32) {
    let trunk = geometry::cylinder(bottom_radius * 0.35, bottom_radius, height, 8);
    let place = (Vec3::Y * height / 2.0, Quat::IDENTITY, Vec3::ONE);
    geometry::append(geometry, &trunk, place, hex(0x3d2b20, 1.0));
}

/// A spruce `height` meters tall and `width` meters wide at its lowest tier, with `tiers`
/// tiers of branches. Each tier's branch tips droop and stick out by different lengths, its
/// upper side lightens towards the tips, and its underside is in shade.
pub fn spruce(seed: u32, tiers: u32, height: f32, width: f32) -> Geometry {
    const SIDES: u32 = 14;
    let mut geometry = Geometry::default();
    bark(&mut geometry, height * 0.5, 0.26);
    let (inner, tips, shade) = (hex(0x1f3626, 1.0), hex(0x3f5f36, 1.0), hex(0x111c13, 1.0));
    let (crown_bottom, crown) = (height * 0.17, height * 0.83);
    let mut noise = 0;
    let mut next = || {
        noise += 1;
        jitter(seed, noise)
    };
    for tier in 0..tiers {
        let f = tier as f32 / (tiers - 1) as f32;
        let bottom = crown_bottom + crown * f * 0.8;
        let tier_height = crown * (0.32 - 0.12 * f);
        let top = if tier == tiers - 1 { height } else { bottom + tier_height };
        let radius = width / 2.0 * (1.0 - 0.82 * f) * (0.92 + 0.08 * next());
        let tone = 1.0 + 0.1 * next();
        let apex = push(&mut geometry, Vec3::Y * top, Vec3::Y, inner * tone);
        // The branch tips around the tier, every other one shorter, all drooping a little.
        let twist = next() * PI;
        let ring: Vec<(Vec3, Vec3)> = (0..SIDES)
            .map(|k| {
                let angle = (k as f32 + 0.3 * next()) / SIDES as f32 * TAU + twist;
                let out = Vec3::new(angle.cos(), 0.0, angle.sin());
                let reach = radius * (if k % 2 == 0 { 1.0 } else { 0.78 }) * (0.9 + 0.15 * next());
                let droop = radius * (0.1 + 0.08 * next());
                (out * reach + Vec3::Y * (bottom - droop), out)
            })
            .collect();
        let upper: Vec<u32> = ring
            .iter()
            .map(|&(tip, out)| {
                let color = tips * tone * (1.0 + 0.12 * next());
                push(&mut geometry, tip, out + Vec3::Y * 0.6, color)
            })
            .collect();
        let lower: Vec<u32> = ring
            .iter()
            .map(|&(tip, out)| push(&mut geometry, tip, out * 0.4 - Vec3::Y, shade * tone))
            .collect();
        // The underside rises back towards the trunk.
        let core: Vec<u32> = ring
            .iter()
            .map(|&(tip, out)| {
                let point = (tip * Vec3::new(0.25, 0.0, 0.25)).with_y(bottom + tier_height * 0.3);
                push(&mut geometry, point, out * 0.2 - Vec3::Y, shade * 0.8)
            })
            .collect();
        for k in 0..SIDES as usize {
            let n = (k + 1) % SIDES as usize;
            geometry.indices.extend([apex, upper[k], upper[n]]);
            geometry.indices.extend([lower[k], lower[n], core[k], core[k], lower[n], core[n]]);
        }
    }
    orient_to_normals(&mut geometry);
    geometry
}

/// A broadleaf tree about `height` meters tall: a trunk, and a crown of deformed lumps
/// around a bigger one, lighter on top and darker below.
pub fn broadleaf(seed: u32, height: f32) -> Geometry {
    const RINGS: u32 = 7;
    const SEGMENTS: u32 = 12;
    let mut geometry = Geometry::default();
    bark(&mut geometry, height * 0.55, 0.3);
    let center = Vec3::Y * height * 0.64;
    let crown = height * 0.3;
    let (dark, light) = (hex(0x22381c, 1.0), hex(0x4d6d2d, 1.0));
    let mut lumps = vec![(center, crown)];
    for k in 0..6 {
        let angle = (k as f32 + 0.4 * jitter(seed, 100 + k)) / 6.0 * TAU;
        let rise = crown * 0.35 * jitter(seed, 200 + k);
        let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * crown * 0.7 + Vec3::Y * rise;
        lumps.push((center + offset, crown * (0.55 + 0.12 * jitter(seed, 300 + k))));
    }
    for (index, &(lump, radius)) in lumps.iter().enumerate() {
        let first = geometry.positions.len() as u32;
        for r in 0..=RINGS {
            let (sin_lat, cos_lat) = (r as f32 / RINGS as f32 * PI).sin_cos();
            for s in 0..=SEGMENTS {
                let (sin, cos) = ((s % SEGMENTS) as f32 / SEGMENTS as f32 * TAU).sin_cos();
                let direction = Vec3::new(sin_lat * cos, cos_lat, sin_lat * sin);
                // The poles' vertices share one point, so they share its noise too.
                let key = if r == 0 || r == RINGS { r * 31 } else { r * 31 + s % SEGMENTS };
                let bump = 1.0 + 0.14 * jitter(seed + index as u32, key);
                let point = lump + direction * radius * bump;
                // Lit as part of the whole crown, with a little of the lump's own roundness.
                let normal = (point - center).normalize_or_zero() * 0.7 + direction * 0.3;
                let height = ((point.y - center.y) / crown * 0.5 + 0.5).clamp(0.0, 1.0);
                let color = dark.lerp(light, height) * (1.0 + 0.08 * jitter(seed, 400 + key));
                push(&mut geometry, point, normal, color);
            }
        }
        let row = SEGMENTS + 1;
        for r in 0..RINGS {
            for s in 0..SEGMENTS {
                let (a, b) = (first + r * row + s, first + (r + 1) * row + s);
                geometry.indices.extend([a, a + 1, b, a + 1, b + 1, b]);
            }
        }
    }
    orient_to_normals(&mut geometry);
    geometry
}

/// The medium and high quality trees: two spruces, a slender and a full one, and a broadleaf tree, each
/// about as big as low quality's pine.
pub fn kinds() -> [Geometry; 3] {
    [spruce(11, 7, 10.5, 4.6), spruce(23, 6, 9.0, 5.4), broadleaf(37, 9.0)]
}
