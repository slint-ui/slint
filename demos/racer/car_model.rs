// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! The race cars' meshes: an open-wheel car whose body and sidepods are lofted through
//! rounded cross-sections, with airfoil wings, a halo, and lathed wheels.

use std::f32::consts::{FRAC_PI_2, TAU};

use glam::{Quat, Vec2, Vec3};

use crate::cars::{self, Car};
use crate::geometry::{self, orient_to_normals};
use crate::renderer::Geometry;
use crate::world::hex;

/// The cars' parts, with their noses towards -Z and their wheels on the ground at y = 0.
pub struct CarMeshes {
    /// In the team color, which multiplies the vertex colors: 1 for the team color itself,
    /// less for its darker accents.
    pub paint: Geometry,
    /// The parts in their own colors.
    pub trim: Geometry,
    /// A wheel of radius 1 and width 1 around the Y axis.
    pub wheel: Geometry,
}

/// The wheels' places on the car, whether they steer, and their width.
pub const WHEELS: [(f32, f32, bool, f32); 4] = [
    (-0.9, -1.45, true, 0.34),
    (0.9, -1.45, true, 0.34),
    (-0.88, 1.35, false, 0.42),
    (0.88, 1.35, false, 0.42),
];

const TEAM: f32 = 1.0;
/// The darker accents in the team color.
const ACCENT: f32 = 0.3;
const CARBON: u32 = 0x17191c;

/// A cross-section of a loft, at `z`, with its vertices' color.
struct Ring {
    z: f32,
    points: Vec<Vec2>,
    color: Vec3,
}

/// A point on a superellipse of the given exponent: 2 is an ellipse, higher is boxier.
fn superellipse(angle: f32, exponent: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    let power = 2.0 / exponent;
    Vec2::new(cos.signum() * cos.abs().powf(power), sin.signum() * sin.abs().powf(power))
}

/// A rounded cross-section around `center_x`, `half_width` wide, from `bottom` to `top`.
fn section(
    z: f32,
    center_x: f32,
    half_width: f32,
    bottom: f32,
    top: f32,
    exponent: f32,
    color: Vec3,
) -> Ring {
    const SIDES: usize = 20;
    let (middle, half_height) = ((bottom + top) / 2.0, (top - bottom) / 2.0);
    let points = (0..SIDES)
        .map(|k| {
            let p = superellipse(k as f32 / SIDES as f32 * TAU, exponent);
            Vec2::new(center_x + p.x * half_width, middle + p.y * half_height)
        })
        .collect();
    Ring { z, points, color }
}

/// Flips the triangles that face towards `inside` of their center.
fn orient_outward(geometry: &mut Geometry, inside: impl Fn(Vec3) -> Vec3) {
    let position = |i: u32| Vec3::from(geometry.positions[i as usize]);
    for triangle in geometry.indices.chunks_mut(3) {
        let (a, b, c) = (position(triangle[0]), position(triangle[1]), position(triangle[2]));
        let center = (a + b + c) / 3.0;
        if (b - a).cross(c - a).dot(center - inside(center)) < 0.0 {
            triangle.swap(1, 2);
        }
    }
}

/// Normals averaged over the faces around each vertex.
fn smooth_normals(geometry: &mut Geometry) {
    let mut normals = vec![Vec3::ZERO; geometry.positions.len()];
    for triangle in geometry.indices.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(geometry.positions[triangle[k] as usize]));
        let face = (b - a).cross(c - a);
        for &index in triangle {
            normals[index as usize] += face;
        }
    }
    geometry.normals = normals.iter().map(|n| n.normalize_or(Vec3::Y).to_array()).collect();
}

/// A smooth surface through `rings`, ordered by `z`, closed at both ends.
fn loft(rings: &[Ring]) -> Geometry {
    let mut geometry = Geometry::default();
    let sides = rings[0].points.len() as u32;
    for ring in rings {
        for point in &ring.points {
            geometry.positions.push([point.x, point.y, ring.z]);
            geometry.uvs.push([0.0, 0.0]);
            geometry.colors.push(ring.color.to_array());
        }
    }
    for r in 0..rings.len() as u32 - 1 {
        for k in 0..sides {
            let (a, a1) = (r * sides + k, r * sides + (k + 1) % sides);
            geometry.indices.extend([a, a1, a + sides, a1, a1 + sides, a + sides]);
        }
    }
    // The axis through the rings' centers, which the surface faces away from.
    let centers: Vec<(f32, Vec2)> = rings
        .iter()
        .map(|ring| (ring.z, ring.points.iter().sum::<Vec2>() / ring.points.len() as f32))
        .collect();
    let axis = |z: f32| {
        let i = centers.partition_point(|c| c.0 < z).clamp(1, centers.len() - 1);
        let ((z0, c0), (z1, c1)) = (centers[i - 1], centers[i]);
        let t = if z1 > z0 { ((z - z0) / (z1 - z0)).clamp(0.0, 1.0) } else { 0.0 };
        c0.lerp(c1, t)
    };
    orient_outward(&mut geometry, |p| axis(p.z).extend(p.z));
    smooth_normals(&mut geometry);

    // Flat caps at both ends, with their own vertices so their edges stay sharp.
    for (ring, facing) in [(&rings[0], -1.0), (&rings[rings.len() - 1], 1.0)] {
        let first = geometry.positions.len() as u32;
        let center = ring.points.iter().sum::<Vec2>() / ring.points.len() as f32;
        for point in std::iter::once(&center).chain(&ring.points) {
            geometry.positions.push([point.x, point.y, ring.z]);
            geometry.normals.push([0.0, 0.0, facing]);
            geometry.uvs.push([0.0, 0.0]);
            geometry.colors.push(ring.color.to_array());
        }
        let position = |i: u32| Vec3::from(geometry.positions[i as usize]);
        for k in 0..sides {
            let (a, b, c) = (first, first + 1 + k, first + 1 + (k + 1) % sides);
            let normal = (position(b) - position(a)).cross(position(c) - position(a));
            if normal.z * facing >= 0.0 {
                geometry.indices.extend([a, b, c]);
            } else {
                geometry.indices.extend([a, c, b]);
            }
        }
    }
    geometry
}

/// A wing element across X, centered on the origin, with its leading edge towards -Z and
/// thickest just behind it.
fn airfoil(span: f32, chord: f32, thickness: f32, color: Vec3) -> Geometry {
    let points: Vec<Vec2> = (0..16)
        .map(|k| {
            let (sin, cos) = (k as f32 / 16.0 * TAU).sin_cos();
            // Positive `cos` is the leading edge; the underside is flatter.
            let depth = thickness / 2.0 * (0.35 + 0.65 * (cos + 1.0) / 2.0);
            Vec2::new(cos * chord / 2.0, sin * depth * if sin < 0.0 { 0.5 } else { 1.0 })
        })
        .collect();
    let rings: Vec<Ring> =
        [-span / 2.0, span / 2.0].map(|z| Ring { z, points: points.clone(), color }).into();
    let wing = loft(&rings);
    // The span runs along Z in the loft, and the leading edge along +X.
    geometry::merge([(&wing, (Vec3::ZERO, Quat::from_rotation_y(FRAC_PI_2), Vec3::ONE), Vec3::ONE)])
}

/// A round tube through `points`.
fn tube(points: &[Vec3], radius: f32, color: Vec3) -> Geometry {
    const SIDES: u32 = 6;
    let mut geometry = Geometry::default();
    let count = points.len();
    for i in 0..count {
        let along = (points[(i + 1).min(count - 1)] - points[i.saturating_sub(1)]).normalize();
        let side = if along.y.abs() > 0.9 { Vec3::X } else { Vec3::Y.cross(along).normalize() };
        let up = along.cross(side);
        for k in 0..SIDES {
            let (sin, cos) = (k as f32 / SIDES as f32 * TAU).sin_cos();
            let normal = side * cos + up * sin;
            geometry.positions.push((points[i] + normal * radius).to_array());
            geometry.normals.push(normal.to_array());
            geometry.uvs.push([0.0, 0.0]);
            geometry.colors.push(color.to_array());
        }
    }
    for i in 0..count as u32 - 1 {
        for k in 0..SIDES {
            let (a, a1) = (i * SIDES + k, i * SIDES + (k + 1) % SIDES);
            geometry.indices.extend([a, a1, a + SIDES, a1, a1 + SIDES, a + SIDES]);
        }
    }
    orient_to_normals(&mut geometry);
    geometry
}

/// An ellipsoid of `size` (its half axes) around `center`.
fn ellipsoid(center: Vec3, size: Vec3, color: Vec3) -> Geometry {
    const RINGS: u32 = 8;
    const SEGMENTS: u32 = 14;
    let mut geometry = Geometry::default();
    for r in 0..=RINGS {
        let (sin_lat, cos_lat) = (r as f32 / RINGS as f32 * std::f32::consts::PI).sin_cos();
        for s in 0..=SEGMENTS {
            let (sin, cos) = (s as f32 / SEGMENTS as f32 * TAU).sin_cos();
            let direction = Vec3::new(sin_lat * cos, cos_lat, sin_lat * sin);
            geometry.positions.push((center + direction * size).to_array());
            geometry.normals.push((direction / size).normalize().to_array());
            geometry.uvs.push([0.0, 0.0]);
            geometry.colors.push(color.to_array());
        }
    }
    let row = SEGMENTS + 1;
    for r in 0..RINGS {
        for s in 0..SEGMENTS {
            let (a, b) = (r * row + s, (r + 1) * row + s);
            geometry.indices.extend([a, a + 1, b, a + 1, b + 1, b]);
        }
    }
    orient_to_normals(&mut geometry);
    geometry
}

/// A surface turned around the Y axis through `profile`: radius, height, and the normal's
/// radial and vertical parts at each point.
fn lathe(profile: &[(f32, f32, f32, f32)], color: Vec3) -> Geometry {
    const SEGMENTS: u32 = 28;
    let mut geometry = Geometry::default();
    for &(radius, y, normal_radial, normal_y) in profile {
        for s in 0..SEGMENTS {
            let (sin, cos) = (s as f32 / SEGMENTS as f32 * TAU).sin_cos();
            geometry.positions.push([radius * cos, y, radius * sin]);
            let normal = Vec3::new(normal_radial * cos, normal_y, normal_radial * sin);
            geometry.normals.push(normal.normalize().to_array());
            geometry.uvs.push([0.0, 0.0]);
            geometry.colors.push(color.to_array());
        }
    }
    for p in 0..profile.len() as u32 - 1 {
        for s in 0..SEGMENTS {
            let (a, a1) = (p * SEGMENTS + s, p * SEGMENTS + (s + 1) % SEGMENTS);
            geometry.indices.extend([a, a1, a + SEGMENTS, a1, a1 + SEGMENTS, a + SEGMENTS]);
        }
    }
    orient_to_normals(&mut geometry);
    geometry
}

fn add(target: &mut Geometry, part: &Geometry) {
    geometry::append(target, part, (Vec3::ZERO, Quat::IDENTITY, Vec3::ONE), Vec3::ONE);
}

fn add_box(target: &mut Geometry, center: Vec3, rotation: Quat, size: Vec3, color: Vec3) {
    geometry::append(target, &geometry::cube(), (center, rotation, size), color);
}

fn add_placed(target: &mut Geometry, part: &Geometry, center: Vec3, rotation: Quat) {
    geometry::append(target, part, (center, rotation, Vec3::ONE), Vec3::ONE);
}

pub fn meshes() -> CarMeshes {
    let team = Vec3::splat(TEAM);
    let accent = Vec3::splat(ACCENT);
    let carbon = hex(CARBON, 1.0);
    let flat = Quat::IDENTITY;

    let mut paint = Geometry::default();
    // The monocoque, from the nose tip, in the dark accent, to the airbox behind the driver
    // and down to the tail. Rings at the same `z` make a sharp edge between the colors.
    let body = [
        (-2.32, 0.05, 0.27, 0.31, accent),
        (-2.2, 0.12, 0.24, 0.36, accent),
        (-2.02, 0.15, 0.23, 0.4, accent),
        (-2.02, 0.15, 0.23, 0.4, team),
        (-1.7, 0.22, 0.21, 0.47, team),
        (-1.2, 0.29, 0.19, 0.55, team),
        (-0.75, 0.36, 0.18, 0.63, team),
        (-0.4, 0.41, 0.17, 0.68, team),
        (0.3, 0.42, 0.17, 0.7, team),
        (0.65, 0.36, 0.17, 0.92, team),
        (0.95, 0.3, 0.17, 0.86, team),
        (1.4, 0.22, 0.18, 0.66, team),
        (1.8, 0.14, 0.22, 0.48, team),
        (1.8, 0.14, 0.22, 0.48, accent),
        (2.02, 0.09, 0.25, 0.38, accent),
    ];
    let rings: Vec<Ring> = body
        .iter()
        .map(|&(z, w, bottom, top, color)| section(z, 0.0, w, bottom, top, 3.0, color))
        .collect();
    add(&mut paint, &loft(&rings));
    // The sidepods, with dark inlets at the front.
    for side in [-1.0, 1.0] {
        let pod = [
            (-0.6, 0.2, 0.2, 0.5, Vec3::splat(0.05)),
            (-0.6, 0.2, 0.2, 0.5, team),
            (-0.35, 0.25, 0.18, 0.55, team),
            (0.3, 0.24, 0.18, 0.52, team),
            (0.75, 0.16, 0.19, 0.42, team),
            (0.75, 0.16, 0.19, 0.42, accent),
            (1.0, 0.06, 0.22, 0.3, accent),
        ];
        let rings: Vec<Ring> = pod
            .iter()
            .map(|&(z, w, bottom, top, color)| section(z, side * 0.6, w, bottom, top, 2.6, color))
            .collect();
        add(&mut paint, &loft(&rings));
        add_box(
            &mut paint,
            Vec3::new(side * 0.5, 0.75, -0.55),
            flat,
            Vec3::new(0.12, 0.06, 0.05),
            team,
        );
    }
    // The helmet, the engine cover's fin, the front wing's flap, and the rear wing's main plane.
    add(&mut paint, &ellipsoid(Vec3::new(0.0, 0.8, -0.05), Vec3::new(0.14, 0.14, 0.16), team));
    add_box(
        &mut paint,
        Vec3::new(0.0, 0.98, 1.25),
        Quat::from_rotation_x(-0.15),
        Vec3::new(0.02, 0.2, 0.75),
        team,
    );
    add_placed(
        &mut paint,
        &airfoil(1.6, 0.22, 0.035, team),
        Vec3::new(0.0, 0.2, -2.06),
        Quat::from_rotation_x(-0.35),
    );
    add_placed(&mut paint, &airfoil(1.5, 0.42, 0.06, team), Vec3::new(0.0, 0.98, 2.0), flat);

    let mut trim = Geometry::default();
    let black = hex(0x08090b, 1.0);
    // The cockpit opening, the visor, and the airbox's inlet.
    add(&mut trim, &ellipsoid(Vec3::new(0.0, 0.69, -0.12), Vec3::new(0.24, 0.03, 0.42), black));
    add(&mut trim, &ellipsoid(Vec3::new(0.0, 0.82, -0.17), Vec3::new(0.12, 0.05, 0.05), black));
    add(&mut trim, &ellipsoid(Vec3::new(0.0, 0.85, 0.5), Vec3::new(0.1, 0.07, 0.03), black));
    // The halo over the cockpit.
    let titanium = hex(0x2a2d33, 1.0);
    add(
        &mut trim,
        &tube(&[Vec3::new(0.0, 0.66, -0.62), Vec3::new(0.0, 0.92, -0.44)], 0.03, titanium),
    );
    for side in [-1.0, 1.0] {
        let halo = [
            Vec3::new(0.0, 0.92, -0.44),
            Vec3::new(side * 0.18, 0.93, -0.38),
            Vec3::new(side * 0.26, 0.92, -0.15),
            Vec3::new(side * 0.25, 0.89, 0.1),
            Vec3::new(side * 0.24, 0.78, 0.26),
            Vec3::new(side * 0.24, 0.68, 0.3),
        ];
        add(&mut trim, &tube(&halo, 0.025, titanium));
        // The mirror's stalk.
        let stalk = [Vec3::new(side * 0.36, 0.64, -0.5), Vec3::new(side * 0.47, 0.74, -0.55)];
        add(&mut trim, &tube(&stalk, 0.012, carbon));
    }
    // The floor, flat and wide between the wheels.
    let floor = [(-1.25, 0.32), (-0.7, 0.88), (1.15, 0.88), (1.6, 0.55), (2.05, 0.5)];
    let rings: Vec<Ring> =
        floor.iter().map(|&(z, w)| section(z, 0.0, w, 0.11, 0.15, 8.0, carbon)).collect();
    add(&mut trim, &loft(&rings));
    // The front wing's main plane and endplates, and the pylons that hold it.
    add_placed(&mut trim, &airfoil(1.85, 0.36, 0.05, carbon), Vec3::new(0.0, 0.12, -2.22), flat);
    for side in [-1.0, 1.0] {
        add_box(
            &mut trim,
            Vec3::new(side * 0.93, 0.17, -2.18),
            flat,
            Vec3::new(0.03, 0.16, 0.5),
            carbon,
        );
        add_box(
            &mut trim,
            Vec3::new(side * 0.08, 0.2, -2.1),
            flat,
            Vec3::new(0.03, 0.12, 0.2),
            carbon,
        );
    }
    // The rear wing's flap, endplates, and pylon, the beam wing below it, and the exhaust.
    add_placed(
        &mut trim,
        &airfoil(1.5, 0.22, 0.035, carbon),
        Vec3::new(0.0, 1.1, 2.1),
        Quat::from_rotation_x(-0.45),
    );
    add_placed(&mut trim, &airfoil(1.0, 0.2, 0.03, carbon), Vec3::new(0.0, 0.48, 2.05), flat);
    for side in [-1.0, 1.0] {
        add_box(
            &mut trim,
            Vec3::new(side * 0.77, 0.95, 2.02),
            flat,
            Vec3::new(0.03, 0.46, 0.62),
            carbon,
        );
    }
    add_box(&mut trim, Vec3::new(0.0, 0.74, 1.9), flat, Vec3::new(0.04, 0.48, 0.12), carbon);
    add(
        &mut trim,
        &tube(&[Vec3::new(0.0, 0.52, 1.7), Vec3::new(0.0, 0.52, 1.98)], 0.05, hex(0x3a3d42, 1.0)),
    );
    // The wishbones from the body to the wheels.
    let arm = hex(0x202328, 1.0);
    for side in [-1.0, 1.0] {
        for (z, wheel) in [(-1.45, 0.78), (1.35, 0.75)] {
            let hub = Vec3::new(side * wheel, 0.32, z);
            for (from, to) in [
                (Vec3::new(side * 0.25, 0.24, z + 0.18), hub),
                (Vec3::new(side * 0.25, 0.24, z - 0.18), hub),
                (Vec3::new(side * 0.28, 0.46, z), hub + Vec3::Y * 0.12),
            ] {
                add(&mut trim, &tube(&[from, to], 0.014, arm));
            }
        }
    }

    CarMeshes { paint, trim, wheel: wheel() }
}

/// A slick tire with rounded shoulders and a lettered stripe, a dark rim with spokes that
/// show it turning, a hub nut, and a brake disc behind.
fn wheel() -> Geometry {
    let mut wheel = Geometry::default();
    let rubber = hex(0x1a1b1d, 1.0);
    let tire = [
        (0.66, -0.5, 0.0, -1.0),
        (0.9, -0.5, 0.3, -1.0),
        (0.98, -0.42, 0.7, -0.7),
        (1.0, -0.3, 1.0, 0.0),
        (1.0, 0.3, 1.0, 0.0),
        (0.98, 0.42, 0.7, 0.7),
        (0.9, 0.5, 0.3, 1.0),
        (0.66, 0.5, 0.0, 1.0),
    ];
    add(&mut wheel, &lathe(&tire, rubber));
    for side in [-1.0, 1.0] {
        let y = side * 0.503;
        let stripe = [(0.8, y, 0.0, side), (0.85, y, 0.0, side)];
        add(&mut wheel, &lathe(&stripe, hex(0xd9c25a, 1.0)));
        let lip = [(0.66, side * 0.5, 0.0, side), (0.62, side * 0.46, 0.3, side)];
        add(&mut wheel, &lathe(&lip, hex(0xb8bec6, 1.0)));
        let dish = [(0.62, side * 0.46, 0.0, side), (0.2, side * 0.3, -0.2, side)];
        add(&mut wheel, &lathe(&dish, hex(0x2b2e34, 1.0)));
        for k in 0..6 {
            let angle = k as f32 / 6.0 * TAU;
            let direction = Vec3::new(angle.cos(), 0.0, angle.sin());
            add_box(
                &mut wheel,
                direction * 0.4 + Vec3::Y * side * 0.36,
                Quat::from_rotation_y(-angle),
                Vec3::new(0.42, 0.04, 0.09),
                hex(0x8d949c, 1.0),
            );
        }
    }
    let nut = geometry::cylinder(0.13, 0.13, 0.8, 10);
    geometry::append(&mut wheel, &nut, (Vec3::ZERO, Quat::IDENTITY, Vec3::ONE), hex(0xc0c6cf, 1.0));
    let disc = geometry::cylinder(0.5, 0.5, 0.5, 20);
    geometry::append(
        &mut wheel,
        &disc,
        (Vec3::ZERO, Quat::IDENTITY, Vec3::ONE),
        hex(0x4a4d52, 1.0),
    );
    wheel
}

/// The lights on a car: where, how big, and their color.
pub fn lights(car: &Car) -> Vec<(Vec3, Vec3, Vec3)> {
    let head = Vec3::new(3.0, 2.9, 2.6);
    let tail = if car.braking { Vec3::new(6.0, 0.25, 0.15) } else { Vec3::new(1.8, 0.08, 0.05) };
    let mut lights = vec![(Vec3::new(0.0, 0.33, 2.04), Vec3::new(0.14, 0.06, 0.03), tail)];
    for side in [-1.0, 1.0] {
        lights.push((Vec3::new(side * 0.93, 0.17, -2.44), Vec3::new(0.035, 0.1, 0.03), head));
        lights.push((Vec3::new(side * 0.77, 0.95, 2.34), Vec3::new(0.035, 0.3, 0.02), tail));
    }
    if car.boost > 0.0 {
        // The exhaust flame flickers.
        let flicker = (car.wheel_angle * 1.7).sin() * 0.5 + 0.5;
        let length = 0.3 + 0.5 * flicker * (car.boost / cars::BOOST_TIME).min(1.0);
        // A bright core in a wider, shorter, dimmer glow, so the flame tapers.
        for (width, share, brightness) in [(0.05, 1.0, 4.0), (0.1, 0.55, 1.6)] {
            let flame = length * share;
            lights.push((
                Vec3::new(0.0, 0.52, 1.98 + flame / 2.0),
                Vec3::new(width, width, flame),
                hex(crate::world::BOOST_CYAN, brightness),
            ));
        }
    }
    lights
}
