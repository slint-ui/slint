// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Meshes for the track and the cars.

use std::f32::consts::TAU;

use glam::{Quat, Vec2, Vec3};

use crate::renderer::Geometry;

/// A `width` x `height` rectangle in the XY plane, facing +Z, with `v` running up.
pub fn plane(width: f32, height: f32) -> Geometry {
    let (x, y) = (width / 2.0, height / 2.0);
    Geometry {
        positions: vec![[-x, y, 0.0], [x, y, 0.0], [-x, -y, 0.0], [x, -y, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        uvs: vec![[0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
        colors: Vec::new(),
        indices: vec![0, 2, 1, 2, 3, 1],
    }
}

/// A closed cylinder along Y, centered on the origin, with `u` running around it.
pub fn cylinder(top_radius: f32, bottom_radius: f32, height: f32, segments: u32) -> Geometry {
    let mut geometry = Geometry::default();
    let half = height / 2.0;
    let slope = (bottom_radius - top_radius) / height;
    for (v, radius) in [(0.0, top_radius), (1.0, bottom_radius)] {
        for i in 0..=segments {
            let u = i as f32 / segments as f32;
            let (sin, cos) = (u * TAU).sin_cos();
            geometry.positions.push([radius * sin, half - v * height, radius * cos]);
            geometry.normals.push(Vec3::new(sin, slope, cos).normalize().to_array());
            geometry.uvs.push([u, 1.0 - v]);
        }
    }
    let row = segments + 1;
    for i in 0..segments {
        let (a, b, c, d) = (i, i + row, i + row + 1, i + 1);
        geometry.indices.extend([a, b, d, b, c, d]);
    }
    for (sign, radius) in [(1.0, top_radius), (-1.0, bottom_radius)] {
        if radius == 0.0 {
            continue;
        }
        let center = geometry.positions.len() as u32;
        geometry.positions.push([0.0, half * sign, 0.0]);
        geometry.normals.push([0.0, sign, 0.0]);
        geometry.uvs.push([0.5, 0.5]);
        let ring = geometry.positions.len() as u32;
        for i in 0..=segments {
            let (sin, cos) = (i as f32 / segments as f32 * TAU).sin_cos();
            geometry.positions.push([radius * sin, half * sign, radius * cos]);
            geometry.normals.push([0.0, sign, 0.0]);
            geometry.uvs.push([cos * 0.5 + 0.5, sin * 0.5 * sign + 0.5]);
        }
        for i in 0..segments {
            let r = ring + i;
            if sign > 0.0 {
                geometry.indices.extend([r, r + 1, center]);
            } else {
                geometry.indices.extend([r + 1, r, center]);
            }
        }
    }
    geometry
}

/// A unit cube, centered on the origin.
pub fn cube() -> Geometry {
    // Each face's normal and two edge directions, with `u × v = normal`, so the
    // corners below wind counter-clockwise when seen from outside.
    const FACES: [(Vec3, Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::NEG_Z, Vec3::Y),
        (Vec3::NEG_X, Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::X, Vec3::NEG_Z),
        (Vec3::NEG_Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y),
    ];
    let mut geometry = Geometry::default();
    for (normal, u, v) in FACES {
        let first = geometry.positions.len() as u32;
        for (du, dv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            geometry.positions.push(((normal + u * du + v * dv) * 0.5).to_array());
            geometry.normals.push(normal.to_array());
            geometry.uvs.push([(du + 1.0) / 2.0, (dv + 1.0) / 2.0]);
        }
        geometry.indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    geometry
}

/// `parts`, each moved, rotated, and scaled, and with its vertices colored, as one geometry.
pub fn merge<'a>(
    parts: impl IntoIterator<Item = (&'a Geometry, (Vec3, Quat, Vec3), Vec3)>,
) -> Geometry {
    let mut merged = Geometry::default();
    for (part, (translation, rotation, scale), color) in parts {
        append(&mut merged, part, (translation, rotation, scale), color);
    }
    merged
}

/// Adds `part` to `merged`, moved, rotated, scaled, and colored.
pub fn append(merged: &mut Geometry, part: &Geometry, place: (Vec3, Quat, Vec3), color: Vec3) {
    let (translation, rotation, scale) = place;
    let first = merged.positions.len() as u32;
    for (i, &position) in part.positions.iter().enumerate() {
        let position = translation + rotation * (Vec3::from(position) * scale);
        merged.positions.push(position.to_array());
        let normal = part.normals.get(i).map_or(Vec3::Y, |&n| Vec3::from(n));
        merged.normals.push((rotation * (normal / scale)).normalize().to_array());
        merged.uvs.push(part.uvs.get(i).copied().unwrap_or_default());
        let base = part.colors.get(i).map_or(Vec3::ONE, |&c| Vec3::from(c));
        merged.colors.push((base * color).to_array());
    }
    merged.indices.extend(part.indices.iter().map(|index| first + index));
}

/// A horizontal square of size 1, facing up.
pub fn floor_spot() -> Geometry {
    Geometry {
        positions: vec![[-0.5, 0.0, -0.5], [0.5, 0.0, -0.5], [-0.5, 0.0, 0.5], [0.5, 0.0, 0.5]],
        normals: vec![[0.0, 1.0, 0.0]; 4],
        uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]],
        colors: Vec::new(),
        indices: vec![0, 2, 1, 1, 2, 3],
    }
}

/// Where a swept profile sits: a point on the track, the track's right side there, how far
/// along the track it is for the texture's `v`, and the color of its vertices.
#[derive(Clone, Copy)]
pub struct Section {
    pub point: Vec3,
    pub right: Vec3,
    pub v: f32,
    pub color: Vec3,
}

/// The surface that `profile` sweeps along `sections`. The profile's points are across
/// (along the section's right side) and up, in meters, and `u` runs along the profile,
/// `u_length` meters per repeat.
///
/// Each face looks to the left of the profile's direction, seen along the sweep: a profile
/// running from left to right faces up. With `smooth`, neighboring faces share their
/// normals, otherwise each face is flat.
pub fn sweep(sections: &[Section], profile: &[Vec2], smooth: bool, u_length: f32) -> Geometry {
    let mut geometry = Geometry::default();
    if sections.len() < 2 || profile.len() < 2 {
        return geometry;
    }
    let face_normal = |a: Vec2, b: Vec2| {
        let d = (b - a).normalize_or_zero();
        Vec2::new(-d.y, d.x)
    };
    // The profile's corners, each with the normal and `u` of its vertices.
    let mut corners: Vec<(Vec2, Vec2, f32)> = Vec::new();
    let mut u = 0.0;
    for k in 0..profile.len() - 1 {
        let (a, b) = (profile[k], profile[k + 1]);
        let length = a.distance(b) / u_length;
        if smooth {
            let before = (k > 0).then(|| face_normal(profile[k - 1], a));
            let normal = (face_normal(a, b) + before.unwrap_or_default()).normalize_or_zero();
            corners.push((a, normal, u));
            if k + 2 == profile.len() {
                corners.push((b, face_normal(a, b), u + length));
            }
        } else {
            let normal = face_normal(a, b);
            corners.push((a, normal, u));
            corners.push((b, normal, u + length));
        }
        u += length;
    }
    for section in sections {
        for &(corner, normal, u) in &corners {
            let position = section.point + section.right * corner.x + Vec3::Y * corner.y;
            geometry.positions.push(position.to_array());
            geometry.normals.push((section.right * normal.x + Vec3::Y * normal.y).to_array());
            geometry.uvs.push([u, section.v]);
            geometry.colors.push(section.color.to_array());
        }
    }
    let row = corners.len() as u32;
    let step = if smooth { 1 } else { 2 };
    for s in 0..sections.len() as u32 - 1 {
        for k in (0..row - 1).step_by(step) {
            let (a, a1) = (s * row + k, s * row + k + 1);
            let (b, b1) = (a + row, a1 + row);
            geometry.indices.extend([a, a1, b, a1, b1, b]);
        }
    }
    geometry
}

/// Flips the triangles that face against their vertices' normals.
pub fn orient_to_normals(geometry: &mut Geometry) {
    let position = |i: u32| Vec3::from(geometry.positions[i as usize]);
    let normal = |i: u32| Vec3::from(geometry.normals[i as usize]);
    for triangle in geometry.indices.chunks_mut(3) {
        let (a, b, c) = (position(triangle[0]), position(triangle[1]), position(triangle[2]));
        let normal = normal(triangle[0]) + normal(triangle[1]) + normal(triangle[2]);
        if (b - a).cross(c - a).dot(normal) < 0.0 {
            triangle.swap(1, 2);
        }
    }
}
