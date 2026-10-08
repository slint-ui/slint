// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Meshes for the hall, with the vertex order and texture coordinates of three.js's
//! geometries of the same names.

use std::f32::consts::TAU;

use glam::{Quat, Vec3};

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

/// A square ring in the XY plane, facing +Z, between squares of half-widths `inner` and
/// `outer`, textured like the part of `plane(1.0, 1.0)` it covers.
pub fn square_ring(inner: f32, outer: f32) -> Geometry {
    let mut geometry = Geometry::default();
    for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        for half in [outer, inner] {
            geometry.positions.push([x * half, y * half, 0.0]);
            geometry.uvs.push([x * half + 0.5, y * half + 0.5]);
        }
    }
    geometry.normals = vec![[0.0, 0.0, 1.0]; geometry.positions.len()];
    geometry.indices = (0..4)
        .flat_map(|side| {
            let (outer, inner) = (side * 2, side * 2 + 1);
            let (next_outer, next_inner) = ((side * 2 + 2) % 8, (side * 2 + 3) % 8);
            [outer, next_outer, inner, inner, next_outer, next_inner]
        })
        .collect();
    geometry
}

/// A disc in the XY plane, facing +Z.
pub fn circle(radius: f32, segments: u32) -> Geometry {
    let mut geometry = Geometry {
        positions: vec![[0.0; 3]],
        normals: vec![[0.0, 0.0, 1.0]; segments as usize + 2],
        uvs: vec![[0.5, 0.5]],
        ..Default::default()
    };
    for i in 0..=segments {
        let (sin, cos) = (i as f32 / segments as f32 * TAU).sin_cos();
        geometry.positions.push([radius * cos, radius * sin, 0.0]);
        geometry.uvs.push([(cos + 1.0) / 2.0, (sin + 1.0) / 2.0]);
    }
    geometry.indices = (1..=segments).flat_map(|i| [i, i + 1, 0]).collect();
    geometry
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
        let center = geometry.positions.len() as u32;
        for _ in 0..segments {
            geometry.positions.push([0.0, half * sign, 0.0]);
            geometry.normals.push([0.0, sign, 0.0]);
            geometry.uvs.push([0.5, 0.5]);
        }
        let ring = geometry.positions.len() as u32;
        for i in 0..=segments {
            let (sin, cos) = (i as f32 / segments as f32 * TAU).sin_cos();
            geometry.positions.push([radius * sin, half * sign, radius * cos]);
            geometry.normals.push([0.0, sign, 0.0]);
            geometry.uvs.push([cos * 0.5 + 0.5, sin * 0.5 * sign + 0.5]);
        }
        for i in 0..segments {
            let (c, r) = (center + i, ring + i);
            if sign > 0.0 {
                geometry.indices.extend([r, r + 1, c]);
            } else {
                geometry.indices.extend([r + 1, r, c]);
            }
        }
    }
    geometry
}

/// Unit cubes moved, rotated, and scaled by each of `boxes`, merged into one geometry
/// around their mean position, which is returned with it.
///
/// Drawing static boxes that share a material as one mesh saves a draw call per box,
/// which costs more than the box itself on small embedded GPUs. Keeping the vertices
/// near the mesh's origin matters on NXP's Vivante GPUs: they transform vertices at
/// reduced precision, which collapses thin boxes far from the origin.
pub fn boxes(boxes: &[(Vec3, Quat, Vec3)]) -> (Geometry, Vec3) {
    let center = boxes.iter().map(|b| b.0).sum::<Vec3>() / boxes.len().max(1) as f32;
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
    for &(translation, rotation, scale) in boxes {
        for (normal, u, v) in FACES {
            let first = geometry.positions.len() as u32;
            for (du, dv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let corner = (normal + u * du + v * dv) * 0.5;
                let position = translation - center + rotation * (corner * scale);
                geometry.positions.push(position.to_array());
                geometry.normals.push((rotation * normal).to_array());
                geometry.uvs.push([(du + 1.0) / 2.0, (dv + 1.0) / 2.0]);
            }
            geometry.indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
        }
    }
    (geometry, center)
}

/// `parts`, each moved, rotated, and scaled, and with its vertices colored, as one geometry.
pub fn merge<'a>(
    parts: impl IntoIterator<Item = (&'a Geometry, (Vec3, Quat, Vec3), Vec3)>,
) -> Geometry {
    let mut merged = Geometry::default();
    for (part, (translation, rotation, scale), color) in parts {
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
    merged
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

/// A cone open at both ends, with `v` running from the top (0) to the bottom (1).
pub fn open_cone(top_radius: f32, bottom_radius: f32, height: f32, segments: u32) -> Geometry {
    let mut geometry = Geometry::default();
    for i in 0..=segments {
        let u = i as f32 / segments as f32;
        let (sin, cos) = (u * TAU).sin_cos();
        geometry.positions.push([top_radius * cos, 0.0, top_radius * sin]);
        geometry.positions.push([bottom_radius * cos, -height, bottom_radius * sin]);
        geometry.uvs.push([u, 0.0]);
        geometry.uvs.push([u, 1.0]);
    }
    geometry.normals = vec![[0.0, 1.0, 0.0]; geometry.positions.len()];
    geometry.indices = (0..segments)
        .flat_map(|i| {
            let k = i * 2;
            [k, k + 2, k + 1, k + 1, k + 2, k + 3]
        })
        .collect();
    geometry
}
