// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore metalness

//! The 3D scene: the sky and the land around the track, the current track, and the cars.

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::rc::Rc;

use glam::{Mat4, Quat, Vec2, Vec3, Vec3Swizzles, Vec4};

use crate::Quality;
use crate::camera::Director;
use crate::car_model;
use crate::cars::{self, Car, HALF_LENGTH, WALL, WHEEL_RADIUS};
use crate::geometry::{self, Section};
use crate::renderer::{
    Blend, Camera, FloorSpot, Geometry, Instance, LightMap, Material, Node, Renderer, Scene,
    SpotLight, SpotShape, Texture,
};
use crate::textures;
use crate::track::{
    Crossing, EMBANKMENT_SLOPE, EMBANKMENT_TOP, HALF_WIDTH, HILL_REACH, PAD_HALF, SAMPLES,
    TUNNEL_HEIGHT, TUNNEL_LIGHT_SPACING, Track,
};
use crate::trees;
use crate::yaw_towards;

const GROUND_SIZE: f32 = 1100.0;
/// How far the camera sees. The sky and the mountains move with the camera, just inside.
const FAR: f32 = 520.0;
const SKY_RADIUS: f32 = 480.0;
const GROUND_LIGHT_TEXELS_PER_METER: f32 = 0.5;
/// Towards the setting sun.
const SUN_DIRECTION: Vec3 = Vec3::new(-0.62, 0.34, -0.76);
const SKY_TOP: u32 = 0x1a2244;
/// The heights of the road and of what lies on it. The road and the curbs draw in order
/// without depth, see `FlatLayer`, since 16-bit depth can't tell them apart from afar.
const ROAD_Y: f32 = 0.06;
const CURB_Y: f32 = 0.09;
const MARK_Y: f32 = 0.1;
const POOL_Y: f32 = 0.12;
const LAMP_WARM: u32 = 0xffd49a;
pub const BOOST_CYAN: u32 = 0x35d6ff;

/// An sRGB hex color in linear light, scaled.
pub fn hex(color: u32, scale: f32) -> Vec3 {
    let channel = |shift: u32| textures::srgb_to_linear((color >> shift) as u8);
    Vec3::new(channel(16), channel(8), channel(0)) * scale
}

/// The rotation that turns a plane, which faces +Z, to face horizontally along `direction`.
fn facing(direction: Vec3) -> Quat {
    Quat::from_rotation_y(direction.x.atan2(direction.z))
}

/// The rotation that turns -Z along the track's `tangent`, and X to its right.
fn along(tangent: Vec3) -> Quat {
    Quat::from_rotation_y(yaw_towards(tangent))
}

fn oriented(center: Vec3, rotation: Quat, scale: Vec3) -> Mat4 {
    Mat4::from_scale_rotation_translation(scale, rotation, center)
}

fn instance(transform: Mat4, color: Vec3) -> Instance {
    Instance { transform, color: color.extend(1.0) }
}

/// A glow that adds to what's behind it: `texture`'s alpha, tinted with `tint`.
fn additive(texture: &Rc<Texture>, tint: Vec3) -> Material {
    Material {
        texture: Some(texture.clone()),
        blend: Blend::Additive,
        depth_write: false,
        double_sided: true,
        ..Material::unlit(tint)
    }
}

fn standard(color: Vec3, roughness: f32, metalness: f32, texture: Option<Rc<Texture>>) -> Material {
    Material { texture, ..Material::lit(color, roughness, metalness) }
}

/// The stretches of samples where `keep` holds, as their first sample and their count.
fn runs(keep: impl Fn(usize) -> bool) -> Vec<(usize, usize)> {
    let Some(gap) = (0..SAMPLES).find(|&i| !keep(i)) else { return vec![(0, SAMPLES)] };
    let mut runs = Vec::new();
    let mut current: Option<(usize, usize)> = None;
    for k in 1..=SAMPLES {
        let i = (gap + k) % SAMPLES;
        if keep(i) {
            match &mut current {
                Some((_, count)) => *count += 1,
                None => current = Some((i, 1)),
            }
        } else if let Some(run) = current.take() {
            runs.push(run);
        }
    }
    runs
}

/// The track's sections from `start`, `count` samples long and one more to join the next,
/// with `v` repeating every `v_length` meters.
fn sections(
    track: &Track,
    start: usize,
    count: usize,
    v_length: f32,
    color: impl Fn(usize) -> Vec3,
) -> Vec<Section> {
    (start..=start + count)
        .map(|k| {
            let i = k % SAMPLES;
            Section {
                point: track.points[i],
                right: track.rights[i],
                v: k as f32 * track.spacing() / v_length,
                color: color(i),
            }
        })
        .collect()
}

/// Points along `line`, evenly spaced by length.
fn resample(line: &[Vec2], count: usize) -> Vec<Vec2> {
    let lengths: Vec<f32> = std::iter::once(0.0)
        .chain(line.windows(2).scan(0.0, |total, pair| {
            *total += pair[0].distance(pair[1]);
            Some(*total)
        }))
        .collect();
    let total = *lengths.last().unwrap();
    (0..count)
        .map(|k| {
            let at = k as f32 / (count - 1) as f32 * total;
            let i = lengths.partition_point(|&length| length < at).clamp(1, line.len() - 1);
            let span = (lengths[i] - lengths[i - 1]).max(1e-6);
            line[i - 1].lerp(line[i], ((at - lengths[i - 1]) / span).clamp(0.0, 1.0))
        })
        .collect()
}

/// The flat layers on the ground, which draw first, in this order, without depth: the
/// 16-bit depth buffer of some GPUs, such as NXP's Vivante ones, can't tell layers a few
/// centimeters apart once they're far away. Nothing is below them, so everything drawn
/// later may cover them.
#[derive(Clone, Copy)]
enum FlatLayer {
    Sky = -10,
    Ground,
    GroundLight,
    Road,
    Curbs,
}

/// How a pool of light brightens the road, from 1 at its center to 0 at its edge, as
/// `textures::radial`.
fn pool_falloff(distance: f32) -> f32 {
    let r = distance.min(1.0);
    if r < 0.3 { 1.0 - r / 0.3 * 0.55 } else { 0.45 * (1.0 - (r - 0.3) / 0.7) }
}

/// The point on the road at sample `i`, from its left (`u` = 0) to its right edge.
fn road_point(track: &Track, i: usize, u: f32) -> Vec3 {
    track.points[i] + track.rights[i] * (u * 2.0 - 1.0) * WALL + Vec3::Y * ROAD_Y
}

/// The road's light at `position` near sample `i`: the tunnels' darkness, and the light of
/// `pools`. Each pool is a center, a radius, and the light it adds.
fn road_light(track: &Track, pools: &[(Vec2, f32, Vec3)], i: usize, position: Vec3) -> Vec3 {
    // How much brighter the asphalt gets for each unit of light, about one over its own
    // brightness, so the pools look like added light.
    const GAIN: f32 = 16.0;
    let light = pools
        .iter()
        .map(|&(center, radius, light)| {
            light * pool_falloff(position.xz().distance(center) / radius)
        })
        .sum::<Vec3>();
    Vec3::splat(1.0 - 0.6 * track.cover[i]) + light * GAIN
}

/// The road, with `road_light` in its vertex colors, which costs nothing per pixel.
fn road(track: &Track, pools: &[(Vec2, f32, Vec3)]) -> Geometry {
    // The vertices across the road, enough for the pools' soft edges.
    const ACROSS: u32 = 9;
    let mut geometry = Geometry::default();
    for k in 0..=SAMPLES {
        let i = k % SAMPLES;
        for j in 0..ACROSS {
            let u = j as f32 / (ACROSS - 1) as f32;
            let position = road_point(track, i, u);
            geometry.positions.push(position.to_array());
            geometry.normals.push([0.0, 1.0, 0.0]);
            geometry.uvs.push([u, k as f32 * track.spacing() / 12.0]);
            geometry.colors.push(road_light(track, pools, i, position).to_array());
        }
    }
    for k in 0..SAMPLES as u32 {
        for j in 0..ACROSS - 1 {
            let a = k * ACROSS + j;
            let b = a + ACROSS;
            geometry.indices.extend([a, a + 1, b, a + 1, b + 1, b]);
        }
    }
    geometry
}

/// The road for low quality, with the asphalt texture's lines and racing line in its vertex
/// colors as well as the light of `road`'s.
fn road_low(track: &Track, pools: &[(Vec2, f32, Vec3)]) -> Geometry {
    // The texture shades in sRGB, so its factors become powers here.
    let to_linear = |factor: f32| factor.powf(2.2);
    let asphalt = |u: f32| {
        let rubber = 1.0 - 0.18 * (1.0 - ((u - 0.5) / 0.22).powi(2)).max(0.0);
        hex(textures::ASPHALT_BASE, 1.0) * to_linear(rubber)
    };
    let line = |u: f32| asphalt(u).lerp(hex(0xe8e8e2, 1.0), 0.9);
    let mut geometry = Geometry::default();
    // Bands across the road, each with its own vertices so that their colors change sharply.
    let half = [0.0, 0.035, 0.055, 0.17, 0.28, 0.34, 0.4, 0.46, 0.492];
    let mut bands: Vec<(Vec<f32>, bool)> = Vec::new();
    for mirrored in [false, true] {
        let side = |u: f32| if mirrored { 1.0 - u } else { u };
        let mut columns: Vec<Vec<f32>> =
            vec![half[0..2].to_vec(), half[1..3].to_vec(), half[2..].to_vec()];
        for columns in &mut columns {
            for u in columns.iter_mut() {
                *u = side(*u);
            }
            if mirrored {
                columns.reverse();
            }
        }
        for (k, columns) in columns.into_iter().enumerate() {
            bands.push((columns, k == 1));
        }
    }
    for (columns, is_line) in bands {
        let first = geometry.positions.len() as u32;
        let across = columns.len() as u32;
        for k in 0..=SAMPLES {
            let i = k % SAMPLES;
            for &u in &columns {
                let position = road_point(track, i, u);
                let color = if is_line { line(u) } else { asphalt(u) };
                geometry.positions.push(position.to_array());
                geometry.colors.push((color * road_light(track, pools, i, position)).to_array());
            }
        }
        for k in 0..SAMPLES as u32 {
            for j in 0..across - 1 {
                let a = first + k * across + j;
                let b = a + across;
                geometry.indices.extend([a, a + 1, b, a + 1, b + 1, b]);
            }
        }
    }
    // The dashed line down the middle, 6 m on and 6 m off, one quad per sample.
    for k in 0..SAMPLES {
        let dash = ((k as f32 + 0.5) * track.spacing() / 12.0).fract() < 0.5;
        let first = geometry.positions.len() as u32;
        for i in [k, (k + 1) % SAMPLES] {
            for u in [0.492, 0.508] {
                let position = road_point(track, i, u);
                let color = if dash { line(u) } else { asphalt(u) };
                geometry.positions.push(position.to_array());
                geometry.colors.push((color * road_light(track, pools, i, position)).to_array());
            }
        }
        geometry.indices.extend([first, first + 1, first + 2, first + 1, first + 3, first + 2]);
    }
    geometry
}

/// The ground for low quality: the grass texture's mown stripes, as strips of the ground's
/// color, lighter and darker.
fn ground_low() -> Geometry {
    const STRIPE: f32 = 4.0;
    let half = GROUND_SIZE / 2.0;
    let mut geometry = Geometry::default();
    let count = (GROUND_SIZE / STRIPE) as u32;
    for n in 0..count {
        let (z0, z1) = (-half + n as f32 * STRIPE, -half + (n + 1) as f32 * STRIPE);
        let shade = if n % 2 == 0 { 1.06f32 } else { 0.94 }.powf(2.2);
        let first = geometry.positions.len() as u32;
        for z in [z0, z1] {
            for x in [-half, half] {
                geometry.positions.push([x, 0.0, z]);
                geometry.colors.push([shade; 3]);
            }
        }
        geometry.indices.extend([first, first + 2, first + 1, first + 1, first + 2, first + 3]);
    }
    geometry
}

/// The ground's round pools of light for low quality, fading out like the light map's,
/// see `light_map.wgsl`, in vertex colors to add to the ground's light.
fn ground_pools_low(spots: &[FloorSpot]) -> Geometry {
    const SEGMENTS: u32 = 24;
    const Y: f32 = 0.02;
    let mut geometry = Geometry::default();
    for spot in spots.iter().filter(|spot| spot.shape == SpotShape::Round) {
        let across = Vec2::new(-spot.along.y, spot.along.x);
        let first = geometry.positions.len() as u32;
        // The fall-off is linear from the center to 0.3 and from there to the edge.
        for (r, strength, count) in [(0.0, 1.0, 1), (0.3, 0.45, SEGMENTS), (1.0, 0.0, SEGMENTS)] {
            for s in 0..count {
                let (sin, cos) = (s as f32 / SEGMENTS as f32 * TAU).sin_cos();
                let point = spot.center
                    + spot.along * spot.half.x * r * cos
                    + across * spot.half.y * r * sin;
                geometry.positions.push([point.x, Y, point.y]);
                geometry.colors.push((spot.light * strength).to_array());
            }
        }
        let (inner, outer) = (first + 1, first + 1 + SEGMENTS);
        for s in 0..SEGMENTS {
            let next = (s + 1) % SEGMENTS;
            geometry.indices.extend([first, inner + s, inner + next]);
            geometry.indices.extend([inner + s, outer + s, inner + next]);
            geometry.indices.extend([inner + next, outer + s, outer + next]);
        }
    }
    geometry
}

/// `geometry`'s triangles on the ground, and those raised above it, such as on the ramps up
/// to a bridge. The raised ones need the depth buffer, see `FlatLayer`.
fn split_raised(geometry: Geometry) -> (Geometry, Geometry) {
    let raised = |triangle: &[u32]| {
        triangle.iter().any(|&i| geometry.positions[i as usize][1] > MARK_Y + 0.05)
    };
    let (mut flat, mut high) = (geometry.clone(), geometry.clone());
    flat.indices = geometry.indices.chunks(3).filter(|t| !raised(t)).flatten().copied().collect();
    high.indices = geometry.indices.chunks(3).filter(|t| raised(t)).flatten().copied().collect();
    (flat, high)
}

/// The embankment under a figure eight's raised stretch, cut away where the lower stretch
/// passes, and the abutments' walls facing the cut. Slopes reach out `EMBANKMENT_SLOPE`
/// meters per meter of height from the barriers' outside.
fn embankment(track: &Track, crossing: &Crossing) -> (Geometry, Geometry) {
    let gap = (crossing.gap / track.spacing()).ceil() as isize;
    let in_gap = |i: usize| Track::steps(crossing.upper, i).abs() < gap;
    let grass = hex(textures::GRASS_BASE, 1.0);
    let mut slopes = Geometry::default();
    for (start, count) in runs(|i| track.points[i].y > 0.0 && !in_gap(i)) {
        let first = slopes.positions.len() as u32;
        for k in start..=start + count {
            let i = k % SAMPLES;
            let (point, right) = (track.points[i], track.rights[i]);
            let reach = EMBANKMENT_TOP + EMBANKMENT_SLOPE * point.y;
            let at = |across: f32, height: f32| (point + right * across).with_y(height);
            for (position, normal) in [
                (at(-reach, -0.2), Vec2::new(-1.0, EMBANKMENT_SLOPE)),
                (at(-EMBANKMENT_TOP, point.y), Vec2::new(-1.0, EMBANKMENT_SLOPE)),
                (at(EMBANKMENT_TOP, point.y), Vec2::new(1.0, EMBANKMENT_SLOPE)),
                (at(reach, -0.2), Vec2::new(1.0, EMBANKMENT_SLOPE)),
            ] {
                let normal = (right * normal.x + Vec3::Y * normal.y).normalize();
                slopes.positions.push(position.to_array());
                slopes.normals.push(normal.to_array());
                // The grass texture repeats every 16 m, like on the hills.
                let across = position.distance(point.with_y(position.y));
                slopes.uvs.push([across / 16.0, k as f32 * track.spacing() / 16.0]);
                slopes.colors.push(grass.to_array());
            }
        }
        for k in 0..count as u32 {
            let a = first + k * 4;
            let b = a + 4;
            // Each side's two corners, from left to right, so both slopes face out.
            for (left, right) in [(a, a + 1), (a + 2, a + 3)] {
                let (left_next, right_next) = (left - a + b, right - a + b);
                slopes.indices.extend([left, right, left_next, right, right_next, left_next]);
            }
        }
    }
    let mut walls = Geometry::default();
    for side in [-1, 1] {
        let i = Track::wrap(crossing.upper as isize + side * gap);
        let (point, right) = (track.points[i], track.rights[i]);
        let reach = EMBANKMENT_TOP + EMBANKMENT_SLOPE * point.y;
        let at = |across: f32, height: f32| (point + right * across).with_y(height);
        let outline = [
            at(-reach, -0.2),
            at(-EMBANKMENT_TOP, point.y),
            at(EMBANKMENT_TOP, point.y),
            at(reach, -0.2),
        ];
        let normal = track.tangents[i].with_y(0.0).normalize() * side as f32;
        let first = walls.positions.len() as u32;
        for p in outline {
            walls.positions.push(p.to_array());
            walls.normals.push(normal.to_array());
            walls.uvs.push([0.0, 0.0]);
            walls.colors.push(hex(0xb0afa9, 0.35).to_array());
        }
        walls.indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    (slopes, walls)
}

/// The hill over a tunnel, across the track.
fn hill_profile() -> [Vec2; 7] {
    let (w, r, h) = (HALF_WIDTH, HILL_REACH, TUNNEL_HEIGHT);
    [
        Vec2::new(-r, -0.2),
        Vec2::new(-r + 6.0, 3.5),
        Vec2::new(-w - 3.5, h + 1.6),
        Vec2::new(0.0, h + 2.4),
        Vec2::new(w + 3.5, h + 1.6),
        Vec2::new(r - 6.0, 3.5),
        Vec2::new(r, -0.2),
    ]
}

/// The inside of a tunnel, facing in: up the right wall, across the roof, and down the left.
fn tunnel_profile() -> [Vec2; 4] {
    [
        Vec2::new(WALL, 0.0),
        Vec2::new(WALL, TUNNEL_HEIGHT),
        Vec2::new(-WALL, TUNNEL_HEIGHT),
        Vec2::new(-WALL, 0.0),
    ]
}

/// The barriers' outside face, top, and inside face, on the left and on the right. The
/// faces span 0 to 0.39 and 0.61 to 1 of `u`, see `textures::barrier`.
fn barrier_profiles() -> [[Vec2; 4]; 2] {
    let (inner, outer, height) = (WALL, WALL + 0.5, 0.9);
    [
        [
            Vec2::new(-outer, 0.0),
            Vec2::new(-outer, height),
            Vec2::new(-inner, height),
            Vec2::new(-inner, 0.0),
        ],
        [
            Vec2::new(inner, 0.0),
            Vec2::new(inner, height),
            Vec2::new(outer, height),
            Vec2::new(outer, 0.0),
        ],
    ]
}

/// The wall at a tunnel's end, from the outline of the hill down to the opening.
fn portal(track: &Track, index: usize) -> Geometry {
    let opening = [
        Vec2::new(-WALL, 0.0),
        Vec2::new(-WALL, TUNNEL_HEIGHT),
        Vec2::new(WALL, TUNNEL_HEIGHT),
        Vec2::new(WALL, 0.0),
    ];
    let outer = resample(&hill_profile(), 24);
    let inner = resample(&opening, 24);
    let (point, right) = (track.points[index], track.rights[index]);
    let at = |p: Vec2| (point + right * p.x + Vec3::Y * p.y).to_array();
    let mut geometry = Geometry::default();
    for (o, i) in outer.iter().zip(&inner) {
        geometry.positions.push(at(*o));
        geometry.positions.push(at(*i));
    }
    let count = geometry.positions.len();
    geometry.normals = vec![track.tangents[index].to_array(); count];
    geometry.uvs = geometry.positions.iter().map(|p| [p[0] / 4.0, p[1] / 4.0]).collect();
    geometry.colors = vec![[0.45; 3]; count];
    geometry.indices = (0..outer.len() as u32 - 1)
        .flat_map(|k| {
            let (o, i) = (k * 2, k * 2 + 1);
            [o, o + 2, i, o + 2, i + 2, i]
        })
        .collect();
    geometry
}

/// The sky around the scene: a band from the warm horizon, brightest towards the sun, up
/// to the dark blue of the background.
fn sky() -> Geometry {
    const SEGMENTS: u32 = 64;
    // Heights as a share of the radius.
    let rows: [(f32, u32); 5] =
        [(-0.06, 0xf2a86a), (0.02, 0xe39a7a), (0.11, 0x9a7aa0), (0.26, 0x4c5590), (0.7, SKY_TOP)];
    let sun = SUN_DIRECTION.xz().normalize();
    let mut geometry = Geometry::default();
    for &(height, color) in &rows {
        for i in 0..=SEGMENTS {
            let angle = i as f32 / SEGMENTS as f32 * TAU;
            let direction = Vec2::new(angle.cos(), angle.sin());
            let glow = direction.dot(sun).max(0.0).powi(3) * (1.0 - height / 0.7).max(0.0);
            let position = direction.extend(height) * SKY_RADIUS;
            geometry.positions.push([position.x, position.z, position.y]);
            geometry.colors.push((hex(color, 1.0) * (1.0 + glow * 0.9)).to_array());
        }
    }
    let row = SEGMENTS + 1;
    for r in 0..rows.len() as u32 - 1 {
        for i in 0..SEGMENTS {
            let (a, b) = (r * row + i, (r + 1) * row + i);
            geometry.indices.extend([a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    geometry
}

/// Two rings of mountains on the horizon, the farther one hazier.
fn mountains() -> Geometry {
    let mut geometry = Geometry::default();
    for (radius, base, peak, color, seed) in
        [(470.0, 30.0, 38.0, 0x535678, 1.7), (440.0, 13.0, 37.0, 0x262a40, 0.0)]
    {
        const SEGMENTS: u32 = 120;
        let first = geometry.positions.len() as u32;
        for i in 0..=SEGMENTS {
            let angle = i as f32 / SEGMENTS as f32 * TAU;
            let ridge = (angle * 3.0 + seed).sin() * 0.5
                + (angle * 7.0 + seed * 2.0).sin() * 0.3
                + (angle * 17.0 + seed * 3.0).sin() * 0.2;
            let height = base + peak * (ridge * 0.5 + 0.5);
            let (x, z) = (angle.cos() * radius, angle.sin() * radius);
            // Down below the ground's edge, which draws over it.
            geometry.positions.push([x, -30.0, z]);
            geometry.positions.push([x, height, z]);
            geometry.colors.push(hex(color, 0.7).to_array());
            geometry.colors.push(hex(color, 1.0).to_array());
        }
        for i in 0..SEGMENTS {
            let k = first + i * 2;
            geometry.indices.extend([k, k + 1, k + 2, k + 1, k + 3, k + 2]);
        }
    }
    geometry
}

/// A pine: a trunk and two cones of needles.
fn tree() -> Geometry {
    let trunk = geometry::cylinder(0.22, 0.32, 2.4, 6);
    let lower = geometry::cylinder(0.0, 2.6, 5.0, 8);
    let upper = geometry::cylinder(0.0, 1.8, 4.0, 8);
    let place = |y: f32| (Vec3::Y * y, Quat::IDENTITY, Vec3::ONE);
    geometry::merge([
        (&trunk, place(1.2), hex(0x4a3426, 1.0)),
        (&lower, place(4.6), hex(0x2c4a2a, 1.0)),
        (&upper, place(7.4), hex(0x36572f, 1.0)),
    ])
}

pub enum StartLights {
    Off,
    /// How many of the five are lit red.
    Red(usize),
    Green,
}

#[derive(Clone, Copy, PartialEq)]
pub enum CameraMode {
    Chase,
    Bumper,
}

/// What a frame shows.
pub struct FrameInput<'a> {
    pub track: &'a Track,
    /// The player's car comes first.
    pub cars: &'a [Car],
    pub colors: &'a [Vec3],
    pub start_lights: StartLights,
    pub camera: CameraMode,
    /// Whether the broadcast director cuts between cameras.
    pub cutting: bool,
    pub quality: Quality,
}

pub struct World {
    pub scene: Scene,
    pub camera: Camera,
    director: Director,
    asphalt: Material,
    curb: Material,
    barrier: Material,
    concrete: Material,
    grass: Material,
    checker: Material,
    /// The track's own meshes, replaced with each track.
    track_group: Node,
    /// The glowing boxes: lamps, tunnel lights, markings, and the cars' lights.
    glows: Node,
    track_glows: Vec<Instance>,
    start_lights: Vec<Mat4>,
    concrete_boxes: Node,
    steel_boxes: Node,
    /// The sky, the mountains, and the sun, with their places relative to the camera.
    sky: Vec<(Node, Vec3)>,
    pads: Node,
    /// Low quality's pine, and the other qualities' kinds of trees, see `trees::kinds`.
    trees: Node,
    trees_high: [Node; 3],
    crowd: Node,
    logos: Node,
    /// The lamps over the track, for the spot lights.
    lamps: Vec<Vec3>,
    /// Medium and high quality's ground, textured and with the light map.
    ground: Node,
    ground_light: LightMap,
    /// Low quality's ground, see `ground_low`.
    ground_low: Node,
    /// The track's objects for only medium and high (`true`) or only low quality.
    track_variants: Vec<(Node, bool)>,
    paint: Node,
    trim: Node,
    wheels: Node,
    shadows: Node,
    beams: Node,
    time: f32,
    /// Whether the scene shows medium and high quality's objects rather than low quality's.
    detailed: Option<bool>,
}

impl World {
    pub fn new(renderer: &Renderer) -> Self {
        let mut scene = Scene::default();
        let unit_cube = renderer.create_mesh(&geometry::cube());
        let floor_spot = renderer.create_mesh(&geometry::floor_spot());
        let radial = renderer.create_texture(&textures::radial());

        scene.background = hex(SKY_TOP, 1.0);
        let flat = |material: Material| Material { depth_write: false, ..material };
        let backdrop = flat(Material { double_sided: true, ..Material::unlit(Vec3::ONE) });
        let mut sky_nodes = Vec::new();
        // The sky is the haze's color already; the mountains are partly hazy in their colors.
        for (geometry, haze) in [(sky(), 0.0), (mountains(), 0.6)] {
            let mesh = renderer.create_mesh(&geometry);
            let material = Material { haze, ..backdrop.clone() };
            let node =
                scene.add_mesh(None, &mesh, &material, Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
            scene.object(node).render_order = FlatLayer::Sky as i32;
            sky_nodes.push((node, Vec3::ZERO));
        }
        let sun = renderer.create_mesh(&geometry::plane(1.0, 1.0));
        let sun_offset = SUN_DIRECTION.normalize() * (SKY_RADIUS - 10.0);
        let sun = scene.add_mesh(
            None,
            &sun,
            &Material { haze: 0.0, ..additive(&radial, hex(0xffc080, 1.6)) },
            sun_offset,
            Quat::from_rotation_arc(Vec3::Z, -SUN_DIRECTION.normalize()),
            Vec3::splat(140.0),
        );
        sky_nodes.push((sun, sun_offset));

        let lights = &mut scene.lights;
        lights.sky = hex(0x8fa0c8, 1.0);
        lights.ground = hex(0x3a4a2a, 1.0);
        lights.hemisphere_intensity = 0.65 * PI;
        lights.sun_direction = SUN_DIRECTION;
        lights.sun_color = hex(0xffc89a, 1.0);
        lights.sun_intensity = 2.3 * PI;
        lights.horizon = hex(0xe39a7a, 0.55);
        lights.zenith = hex(0x4c5590, 0.8);
        // Half of the view's light is haze at about 350 m.
        lights.haze_density = 2.0f32.ln() / 350.0;

        let grass_texture = renderer.create_texture(&textures::grass());
        let ground_light =
            renderer.create_light_map(Vec2::splat(GROUND_SIZE), GROUND_LIGHT_TEXELS_PER_METER);
        let ground_material = flat(Material {
            light_map: Some(ground_light.texture.clone()),
            uv_scale: Vec2::splat(GROUND_SIZE / 16.0),
            ..standard(Vec3::ONE, 0.95, 0.0, Some(grass_texture.clone()))
        });
        let ground = scene.add_mesh(
            None,
            &renderer.create_mesh(&geometry::plane(GROUND_SIZE, GROUND_SIZE)),
            &ground_material,
            Vec3::ZERO,
            Quat::from_rotation_x(-FRAC_PI_2),
            Vec3::ONE,
        );
        scene.object(ground).render_order = FlatLayer::Ground as i32;
        let ground_low = scene.add_mesh(
            None,
            &renderer.create_mesh(&ground_low()),
            &flat(standard(hex(textures::GRASS_BASE, 1.0), 0.95, 0.0, None)),
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ONE,
        );
        scene.object(ground_low).render_order = FlatLayer::Ground as i32;

        let texture = |data| Some(renderer.create_texture(&data));
        let concrete = standard(Vec3::ONE, 0.85, 0.0, texture(textures::concrete()));
        let steel = standard(hex(0x4a525c, 1.0), 0.4, 0.7, None);
        let track_group = scene.group(None);
        let glows = scene.add_instanced(&unit_cube, &Material::unlit(Vec3::ONE));
        let concrete_boxes = scene.add_instanced(&unit_cube, &concrete);
        let steel_boxes = scene.add_instanced(&unit_cube, &steel);
        let pad_mesh = renderer.create_mesh(&geometry::plane(PAD_HALF.y * 2.0, PAD_HALF.x * 2.0));
        let pad_material = Material {
            uv_scale: Vec2::new(1.0, 2.0),
            ..additive(&renderer.create_texture(&textures::chevrons()), hex(BOOST_CYAN, 1.6))
        };
        let pads = scene.add_instanced(&pad_mesh, &pad_material);
        let foliage = standard(Vec3::ONE, 0.9, 0.0, None);
        let trees = scene.add_instanced(&renderer.create_mesh(&tree()), &foliage);
        let trees_high =
            trees::kinds().map(|kind| scene.add_instanced(&renderer.create_mesh(&kind), &foliage));
        let crowd = scene.add_instanced(&unit_cube, &standard(Vec3::ONE, 0.8, 0.0, None));
        let logo = Material {
            texture: texture(textures::logo()),
            blend: Blend::Alpha,
            ..Material::unlit(Vec3::ONE)
        };
        // The logo's aspect ratio, 423 x 126.
        let logos =
            scene.add_instanced(&renderer.create_mesh(&geometry::plane(1.0, 126.0 / 423.0)), &logo);

        let meshes = car_model::meshes();
        let paint = scene.add_instanced(
            &renderer.create_mesh(&meshes.paint),
            &Material { clear_coat: 1.0, ..standard(Vec3::ONE, 0.35, 0.45, None) },
        );
        let trim = scene.add_instanced(
            &renderer.create_mesh(&meshes.trim),
            &standard(Vec3::ONE, 0.5, 0.2, None),
        );
        let wheels = scene.add_instanced(
            &renderer.create_mesh(&meshes.wheel),
            &standard(Vec3::ONE, 0.8, 0.1, None),
        );
        let shadow = Material {
            texture: Some(radial.clone()),
            blend: Blend::Alpha,
            depth_write: false,
            ..Material::unlit(Vec3::ZERO)
        };
        let shadows = scene.add_instanced(&floor_spot, &shadow);
        let beams = scene.add_instanced(&floor_spot, &additive(&radial, Vec3::ONE));
        scene.object(pads).render_order = 1;
        scene.object(beams).render_order = 2;

        let camera = Camera {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            fov_degrees: 60.0,
            aspect: 1.6,
            near: 0.5,
            far: FAR,
        };

        Self {
            scene,
            camera,
            director: Director::default(),
            asphalt: flat(standard(Vec3::ONE, 0.75, 0.0, texture(textures::asphalt()))),
            curb: flat(standard(Vec3::ONE, 0.6, 0.0, texture(textures::curb()))),
            barrier: standard(Vec3::ONE, 0.8, 0.0, texture(textures::barrier())),
            concrete,
            grass: standard(Vec3::ONE, 0.95, 0.0, Some(grass_texture)),
            checker: flat(Material {
                uv_scale: Vec2::new(7.75, 1.0),
                ..standard(Vec3::ONE, 0.6, 0.0, texture(textures::checker()))
            }),
            track_group,
            glows,
            track_glows: Vec::new(),
            start_lights: Vec::new(),
            concrete_boxes,
            steel_boxes,
            sky: sky_nodes,
            pads,
            trees,
            trees_high,
            crowd,
            logos,
            lamps: Vec::new(),
            ground,
            ground_light,
            ground_low,
            track_variants: Vec::new(),
            paint,
            trim,
            wheels,
            shadows,
            beams,
            time: 0.0,
            detailed: None,
        }
    }

    fn add_mesh(&mut self, renderer: &Renderer, geometry: &Geometry, material: &Material) -> Node {
        let mesh = renderer.create_mesh(geometry);
        let group = Some(self.track_group);
        self.scene.add_mesh(group, &mesh, material, Vec3::ZERO, Quat::IDENTITY, Vec3::ONE)
    }

    fn add_flat(
        &mut self,
        renderer: &Renderer,
        geometry: &Geometry,
        material: &Material,
        layer: FlatLayer,
    ) -> Node {
        let node = self.add_mesh(renderer, geometry, material);
        self.scene.object(node).render_order = layer as i32;
        node
    }

    /// Replaces the track and everything along it.
    pub fn show_track(&mut self, renderer: &Renderer, track: &Track) {
        self.scene.clear(self.track_group);
        self.track_variants.clear();
        // Shows the new track's variants for the current quality.
        self.detailed = None;
        let mut glows = Vec::new();
        let mut concrete = Vec::new();
        let mut steel = Vec::new();
        let mut pools = Vec::new();
        let mut logos = Vec::new();
        let mut ground_spots = Vec::new();
        let warm = hex(LAMP_WARM, 1.0);
        let cyan = hex(BOOST_CYAN, 1.0);

        // Curbs in the turns, and barriers along the open stretches.
        let mut curbs = Geometry::default();
        let open = |i: usize| track.cover[i] == 0.0;
        let turning = |i: usize| open(i) && track.curvature[i].abs() > 1.0 / 60.0;
        for (start, count) in runs(turning) {
            let sections = sections(track, start, count, 3.0, |_| Vec3::ONE);
            for side in [-1.0, 1.0] {
                let (a, b) = (side * WALL, side * (HALF_WIDTH - 0.9));
                let (left, right) = (a.min(b), a.max(b));
                let profile = [Vec2::new(left, CURB_Y), Vec2::new(right, CURB_Y)];
                let curb = geometry::sweep(&sections, &profile, false, 1.1);
                geometry::append(
                    &mut curbs,
                    &curb,
                    (Vec3::ZERO, Quat::IDENTITY, Vec3::ONE),
                    Vec3::ONE,
                );
            }
        }
        let curb = self.curb.clone();
        let (curbs, raised_curbs) = split_raised(curbs);
        self.add_flat(renderer, &curbs, &curb, FlatLayer::Curbs);
        self.add_mesh(renderer, &raised_curbs, &Material { depth_write: true, ..curb });
        let mut barriers = Geometry::default();
        for (start, count) in runs(open) {
            let sections = sections(track, start, count, 4.0, |_| Vec3::ONE);
            for profile in barrier_profiles() {
                let barrier = geometry::sweep(&sections, &profile, false, 2.3);
                geometry::append(
                    &mut barriers,
                    &barrier,
                    (Vec3::ZERO, Quat::IDENTITY, Vec3::ONE),
                    Vec3::ONE,
                );
            }
        }
        let barrier = self.barrier.clone();
        self.add_mesh(renderer, &barriers, &barrier);

        // The tunnels: the inside, the hill over it, the walls at both ends, and the lights.
        let (mut insides, mut hills, mut portals) =
            (Geometry::default(), Geometry::default(), Geometry::default());
        let identity = (Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
        for tunnel in track.tunnels.iter().filter(|tunnel| !tunnel.bridge) {
            let count = tunnel.samples - 1;
            let inside = sections(track, tunnel.start, count, 4.0, |_| Vec3::splat(0.2));
            let inside = geometry::sweep(&inside, &tunnel_profile(), false, 4.0);
            geometry::append(&mut insides, &inside, identity, Vec3::ONE);
            let hill = sections(track, tunnel.start, count, 16.0, |_| Vec3::ONE);
            let hill = geometry::sweep(&hill, &hill_profile(), true, 16.0);
            geometry::append(&mut hills, &hill, identity, Vec3::ONE);
            let end = (tunnel.start + count) % SAMPLES;
            for index in [tunnel.start, end] {
                geometry::append(&mut portals, &portal(track, index), identity, Vec3::ONE);
                let (point, tangent, right) =
                    (track.points[index], track.tangents[index], track.rights[index]);
                let rotation = along(tangent);
                let lintel = point + Vec3::Y * (TUNNEL_HEIGHT + 1.2);
                concrete.push(instance(
                    oriented(lintel, rotation, Vec3::new(2.0 * WALL + 4.0, 2.4, 1.0)),
                    Vec3::ONE,
                ));
                for side in [-1.0, 1.0] {
                    let pillar =
                        point + right * side * (WALL + 1.0) + Vec3::Y * TUNNEL_HEIGHT / 2.0;
                    concrete.push(instance(
                        oriented(pillar, rotation, Vec3::new(2.0, TUNNEL_HEIGHT, 1.0)),
                        Vec3::ONE,
                    ));
                }
                if index == tunnel.start {
                    let logo = lintel - tangent * 0.52;
                    logos.push(instance(
                        oriented(logo, facing(-tangent), Vec3::splat(6.5)),
                        Vec3::ONE,
                    ));
                }
            }
            let length = tunnel.samples as f32 * track.spacing();
            let mut at = TUNNEL_LIGHT_SPACING / 2.0;
            while at < length {
                let (point, tangent, right) =
                    track.frame_at(tunnel.start as f32 * track.spacing() + at);
                let rotation = along(tangent);
                glows.push(instance(
                    oriented(
                        point + Vec3::Y * (TUNNEL_HEIGHT - 0.06),
                        rotation,
                        Vec3::new(2.6, 0.1, 0.7),
                    ),
                    warm * 2.4,
                ));
                for side in [-1.0, 1.0] {
                    let strip = point + right * side * (WALL - 0.03) + Vec3::Y * 1.0;
                    let size = Vec3::new(0.06, 0.12, TUNNEL_LIGHT_SPACING * 0.8);
                    glows.push(instance(oriented(strip, rotation, size), cyan * 1.6));
                }
                pools.push((point.xz(), 5.0, warm * 0.32));
                at += TUNNEL_LIGHT_SPACING;
            }
        }
        let concrete_material = self.concrete.clone();
        // Without shadows, the sun would light the inside, so its light is in its color.
        let inside = Material { lit: None, haze: 0.0, ..concrete_material.clone() };
        self.add_mesh(renderer, &insides, &inside);
        self.add_mesh(renderer, &portals, &Material { double_sided: true, ..concrete_material });
        let grass = self.grass.clone();
        self.add_mesh(renderer, &hills, &grass);

        // A figure eight's bridge: the embankment with the abutments' walls, and the deck over
        // the lower stretch, with its sides closing the gap below the barriers, a logo on each
        // side, and a light underneath.
        if let Some(crossing) = &track.crossing {
            let (slopes, walls) = embankment(track, crossing);
            // Textured like the hills in medium and high quality, in the grass's color in low.
            let white = Geometry { colors: vec![[1.0; 3]; slopes.colors.len()], ..slopes.clone() };
            let textured = self.add_mesh(renderer, &white, &grass);
            let plain = self.add_mesh(renderer, &slopes, &standard(Vec3::ONE, 0.95, 0.0, None));
            self.track_variants.extend([(textured, true), (plain, false)]);
            let wall = Material { double_sided: true, ..standard(Vec3::ONE, 0.85, 0.0, None) };
            self.add_mesh(renderer, &walls, &wall);
            let (point, tangent, right) = (
                track.points[crossing.upper],
                track.tangents[crossing.upper],
                track.rights[crossing.upper],
            );
            let rotation = along(tangent);
            let length = 2.0 * crossing.gap + 2.0;
            concrete.push(instance(
                oriented(
                    point - Vec3::Y * 1.1,
                    rotation,
                    Vec3::new(2.0 * EMBANKMENT_TOP, 1.0, length),
                ),
                Vec3::ONE,
            ));
            for side in [-1.0, 1.0] {
                let fascia = point + right * side * (EMBANKMENT_TOP - 0.06) - Vec3::Y * 0.8;
                concrete.push(instance(
                    oriented(fascia, rotation, Vec3::new(0.12, 1.6, length)),
                    Vec3::ONE,
                ));
                let logo = point + right * side * (EMBANKMENT_TOP + 0.02) - Vec3::Y * 0.8;
                logos.push(instance(
                    oriented(logo, facing(right * side), Vec3::splat(4.8)),
                    Vec3::ONE,
                ));
            }
            let (below, lower_tangent) =
                (track.points[crossing.lower], track.tangents[crossing.lower]);
            glows.push(instance(
                oriented(
                    // Under the deck, see `track::BRIDGE_HEIGHT`.
                    below + Vec3::Y * (track.points[crossing.upper].y - 1.65),
                    along(lower_tangent),
                    Vec3::new(2.6, 0.1, 0.7),
                ),
                warm * 2.4,
            ));
            pools.push((below.xz(), 6.0, warm * 0.32));
        }

        // The boost pads, glowing on the road.
        let pads = track.pads.iter().map(|pad| {
            let (point, tangent, right) =
                (track.points[pad.index], track.tangents[pad.index], track.rights[pad.index]);
            let center = point + right * pad.offset + Vec3::Y * MARK_Y;
            let rotation = along(tangent) * Quat::from_rotation_x(-FRAC_PI_2);
            pools.push((center.xz(), 5.0, cyan * 0.3));
            instance(Mat4::from_rotation_translation(rotation, center), Vec3::ONE)
        });
        self.scene.object(self.pads).instances = Some(pads.collect());

        // Lamps on poles over the open stretches, on the outside of the turns.
        self.lamps.clear();
        let mut side = 1.0;
        let mut at = 30.0;
        while at < track.length - 25.0 {
            let index = track.index_at(at);
            let near_tunnel = (-60..=60)
                .step_by(5)
                .any(|step| track.cover[Track::wrap(index as isize + step)] > 0.0);
            let raised = (-30..=30)
                .step_by(5)
                .any(|step| track.points[Track::wrap(index as isize + step)].y > 0.0);
            if !near_tunnel && !raised {
                let curvature = track.curvature[index];
                side = if curvature > 0.004 {
                    1.0
                } else if curvature < -0.004 {
                    -1.0
                } else {
                    -side
                };
                let (point, tangent, right) =
                    (track.points[index], track.tangents[index], track.rights[index]);
                let rotation = along(tangent);
                let base = point + right * side * (WALL + 1.4);
                steel.push(instance(
                    oriented(base + Vec3::Y * 4.5, rotation, Vec3::new(0.3, 9.0, 0.3)),
                    Vec3::ONE,
                ));
                let arm = point + right * side * (WALL - 0.6) + Vec3::Y * 9.0;
                steel.push(instance(oriented(arm, rotation, Vec3::new(4.2, 0.2, 0.25)), Vec3::ONE));
                let lamp = point + right * side * (WALL - 2.2) + Vec3::Y * 8.85;
                glows.push(instance(
                    oriented(lamp, rotation, Vec3::new(1.4, 0.12, 0.6)),
                    warm * 3.0,
                ));
                pools.push((lamp.xz(), 7.0, warm * 0.2));
                ground_spots.push(FloorSpot {
                    center: lamp.xz(),
                    along: Vec2::X,
                    half: Vec2::splat(11.0),
                    light: warm * 0.3,
                    shade: 0.0,
                    shape: SpotShape::Round,
                });
                self.lamps.push(lamp);
            }
            at += 46.0;
        }

        self.start_line(renderer, track, &mut glows, &mut concrete, &mut steel, &mut logos);
        let stand = self.grandstand(track, &mut concrete, &mut steel);
        ground_spots.push(FloorSpot {
            center: stand.0.xz(),
            along: stand.1.xz().normalize(),
            half: Vec2::new(30.0, 9.0),
            light: Vec3::ZERO,
            shade: 0.5,
            shape: SpotShape::Soft(Vec2::new(0.3, 0.5)),
        });
        self.place_trees(track, stand.0);

        self.track_glows = glows;
        self.scene.object(self.concrete_boxes).instances = Some(concrete);
        self.scene.object(self.steel_boxes).instances = Some(steel);
        let asphalt = self.asphalt.clone();
        let road_low_material =
            Material { texture: None, color: Vec3::ONE, ..self.asphalt.clone() };
        let (road_high, raised_high) = split_raised(road(track, &pools));
        let (road_low, raised_low) = split_raised(road_low(track, &pools));
        let raised_high = self.add_mesh(
            renderer,
            &raised_high,
            &Material { depth_write: true, ..asphalt.clone() },
        );
        let raised_low = self.add_mesh(
            renderer,
            &raised_low,
            &Material { depth_write: true, ..road_low_material.clone() },
        );
        self.track_variants.extend([(raised_high, true), (raised_low, false)]);
        let road_high = self.add_flat(renderer, &road_high, &asphalt, FlatLayer::Road);
        let road_low = self.add_flat(renderer, &road_low, &road_low_material, FlatLayer::Road);
        let pools_low = self.add_flat(
            renderer,
            &ground_pools_low(&ground_spots),
            &Material {
                blend: Blend::Additive,
                depth_write: false,
                double_sided: true,
                ..Material::unlit(Vec3::ONE)
            },
            FlatLayer::GroundLight,
        );
        self.track_variants.extend([(road_high, true), (road_low, false), (pools_low, false)]);
        self.scene.object(self.logos).instances = Some(logos);
        renderer.draw_light_map(&self.ground_light, &ground_spots);
    }

    /// The checkered line, the gantry over it with the start lights and a logo, and the
    /// marks on the grid.
    fn start_line(
        &mut self,
        renderer: &Renderer,
        track: &Track,
        glows: &mut Vec<Instance>,
        concrete: &mut Vec<Instance>,
        steel: &mut Vec<Instance>,
        logos: &mut Vec<Instance>,
    ) {
        let (point, tangent, right) = (track.points[0], track.tangents[0], track.rights[0]);
        let rotation = along(tangent);
        let line = geometry::merge([(
            &geometry::plane(2.0 * WALL, 1.6),
            (point + Vec3::Y * MARK_Y, rotation * Quat::from_rotation_x(-FRAC_PI_2), Vec3::ONE),
            Vec3::ONE,
        )]);
        let checker = self.checker.clone();
        self.add_flat(renderer, &line, &checker, FlatLayer::Curbs);

        for side in [-1.0, 1.0] {
            let pillar = point + right * side * (WALL + 1.5) + Vec3::Y * 4.2;
            concrete
                .push(instance(oriented(pillar, rotation, Vec3::new(1.0, 8.4, 1.0)), Vec3::ONE));
        }
        let beam = point + Vec3::Y * 7.6;
        concrete.push(instance(
            oriented(beam, rotation, Vec3::new(2.0 * WALL + 4.0, 1.8, 1.0)),
            Vec3::ONE,
        ));
        logos.push(instance(
            oriented(beam - tangent * 0.52, facing(-tangent), Vec3::splat(5.2)),
            Vec3::ONE,
        ));
        let housing = point + Vec3::Y * 6.2 - tangent * 0.1;
        steel.push(instance(oriented(housing, rotation, Vec3::new(5.6, 0.9, 0.4)), Vec3::ONE));
        self.start_lights = (0..5)
            .map(|k| {
                let lamp = housing + right * (k as f32 - 2.0) * 1.05 - tangent * 0.22;
                oriented(lamp, rotation, Vec3::new(0.6, 0.6, 0.1))
            })
            .collect();

        for slot in 0..5 {
            let place = cars::grid_slot(slot);
            let (point, tangent, right) = track.frame_at(place.x + HALF_LENGTH + 0.6);
            let mark = (point + right * place.y).with_y(MARK_Y);
            glows.push(instance(
                oriented(mark, along(tangent), Vec3::new(2.2, 0.01, 0.18)),
                Vec3::splat(0.7),
            ));
        }
    }

    /// A grandstand beside the grid, on the outside of the turn that follows, with a crowd.
    /// Returns its center and the direction of its length.
    fn grandstand(
        &mut self,
        track: &Track,
        concrete: &mut Vec<Instance>,
        steel: &mut Vec<Instance>,
    ) -> (Vec3, Vec3) {
        let index = track.index_at(-25.0);
        let turn: f32 = (0..200).map(|k| track.curvature[(index + k) % SAMPLES]).sum();
        let side = if turn > 0.0 { 1.0 } else { -1.0 };
        let (point, tangent, right) =
            (track.points[index], track.tangents[index], track.rights[index]);
        let rotation = along(tangent);
        let outward = right * side;
        let front = WALL + 6.0;
        let length = 48.0;
        let mut random = crate::track::scenery_random(track.seed);
        let mut crowd = Vec::new();
        let shirts = [0xd8d8d8, 0x2379f4, 0xc8261e, 0xf2c94c, 0x2b2f36, 0x3fd0a8, 0xff8a3d];
        for row in 0..7 {
            let height = (row + 1) as f32 * 0.75;
            let center =
                point + outward * (front + 0.7 + row as f32 * 1.4) + Vec3::Y * height / 2.0;
            concrete.push(instance(
                oriented(center, rotation, Vec3::new(1.4, height, length)),
                Vec3::ONE,
            ));
            for seat in 0..40 {
                if random() < 0.22 {
                    continue;
                }
                let along_stand = (seat as f32 + 0.5) / 40.0 - 0.5;
                let person = point
                    + outward * (front + 0.9 + row as f32 * 1.4)
                    + tangent * along_stand * (length - 1.0)
                    + Vec3::Y * (height + 0.45);
                let shirt = shirts[(random() * shirts.len() as f32) as usize % shirts.len()];
                crowd.push(instance(
                    oriented(person, rotation, Vec3::new(0.45, 0.9, 0.5)),
                    hex(shirt, 0.8 + random() * 0.3),
                ));
            }
        }
        let back = point + outward * (front + 10.4) + Vec3::Y * 4.5;
        concrete.push(instance(oriented(back, rotation, Vec3::new(0.6, 9.0, length)), Vec3::ONE));
        let roof = point + outward * (front + 5.0) + Vec3::Y * 9.2;
        steel.push(instance(
            oriented(roof, rotation, Vec3::new(11.5, 0.25, length + 2.0)),
            Vec3::ONE,
        ));
        self.scene.object(self.crowd).instances = Some(crowd);
        (point + outward * (front + 5.0), tangent)
    }

    /// Pines in loose clusters around the track, clear of the track, the hills, and the stand.
    fn place_trees(&mut self, track: &Track, stand: Vec3) {
        let mut random = crate::track::scenery_random(track.seed.wrapping_add(1));
        let tunnel_samples: Vec<Vec3> = (track.tunnels.iter())
            .flat_map(|tunnel| {
                (0..tunnel.samples).step_by(6).map(move |k| (tunnel.start + k) % SAMPLES)
            })
            .map(|i| track.points[i])
            .collect();
        let mut trees = Vec::new();
        let mut high: [Vec<Instance>; 3] = Default::default();
        for _ in 0..1600 {
            if trees.len() >= 280 {
                break;
            }
            let angle = random() * TAU;
            // Within the far plane from anywhere on the track.
            let radius = 40.0 + random().sqrt() * 290.0;
            let position = Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius);
            let cluster = (position.x * 0.021).sin() * (position.z * 0.027 + 1.3).cos();
            if cluster < 0.05 && random() > 0.15 {
                continue;
            }
            let near_track = (0..SAMPLES)
                .step_by(6)
                .any(|i| track.points[i].distance_squared(position) < (HALF_WIDTH + 11.0).powi(2));
            let on_hill = tunnel_samples
                .iter()
                .any(|p| p.distance_squared(position) < (HILL_REACH + 3.0).powi(2));
            let on_embankment = (0..SAMPLES).step_by(6).any(|i| {
                let point = track.points[i];
                let reach = EMBANKMENT_TOP + EMBANKMENT_SLOPE * point.y + 4.0;
                point.y > 0.0 && point.xz().distance(position.xz()) < reach
            });
            if near_track || on_hill || on_embankment || position.distance(stand) < 40.0 {
                continue;
            }
            let scale = 0.8 + random() * 0.8;
            let tint =
                Vec3::new(0.85 + random() * 0.3, 0.9 + random() * 0.2, 0.85 + random() * 0.2);
            let tree = instance(
                oriented(position, Quat::from_rotation_y(random() * TAU), Vec3::splat(scale)),
                tint,
            );
            // Mostly spruces, and some broadleaf trees.
            let kind = random();
            let kind = if kind < 0.4 {
                0
            } else if kind < 0.78 {
                1
            } else {
                2
            };
            high[kind].push(tree);
            trees.push(tree);
        }
        self.scene.object(self.trees).instances = Some(trees);
        for (node, list) in self.trees_high.into_iter().zip(high) {
            self.scene.object(node).instances = Some(list);
        }
    }

    fn apply_quality(&mut self, detailed: bool) {
        if self.detailed == Some(detailed) {
            return;
        }
        self.detailed = Some(detailed);
        // Textures cost small GPUs much per pixel, so low quality draws the ground and the
        // road with their textures' looks in vertex colors.
        self.scene.object(self.ground).visible = detailed;
        self.scene.object(self.ground_low).visible = !detailed;
        self.scene.object(self.trees).visible = !detailed;
        for node in self.trees_high {
            self.scene.object(node).visible = detailed;
        }
        for &(node, for_detailed) in &self.track_variants {
            self.scene.object(node).visible = for_detailed == detailed;
        }
    }

    /// Moves the cars and the camera, and lights the start lights.
    pub fn update(&mut self, frame: &FrameInput, dt: f32, aspect: f32) {
        self.time += dt;
        let detailed = frame.quality != Quality::Low;
        self.apply_quality(detailed);
        // The chevrons run along the pads.
        self.scene.object(self.pads).material.uv_offset =
            Vec2::new(0.0, -(self.time * 1.2).fract());

        let mut glows = self.track_glows.clone();
        for (k, &transform) in self.start_lights.iter().enumerate() {
            let color = match frame.start_lights {
                StartLights::Red(lit) if k < lit => Vec3::new(4.0, 0.15, 0.1),
                StartLights::Green => Vec3::new(0.2, 3.5, 0.6),
                _ => Vec3::splat(0.06),
            };
            glows.push(instance(transform, color));
        }
        self.place_cars(frame, &mut glows);
        self.scene.object(self.glows).instances = Some(glows);

        let player = &frame.cars[0];
        let view = match frame.camera {
            CameraMode::Bumper if !frame.cutting => self.director.bumper(player),
            _ => self.director.view(player, frame.track, frame.cutting, dt),
        };
        self.camera.translation = view.eye;
        self.camera.rotation = view.rotation;
        // The field of view is vertical, so tall windows widen it to see as much to the sides.
        self.camera.fov_degrees = if aspect < 1.0 {
            (2.0 * ((view.fov_degrees.to_radians() / 2.0).tan() / aspect.max(0.5)).atan())
                .to_degrees()
                .min(100.0)
        } else {
            view.fov_degrees
        };
        self.camera.aspect = aspect;
        for &(node, offset) in &self.sky {
            self.scene.object(node).translation = view.eye.with_y(0.0) + offset;
        }

        // The lamps nearest the camera light the scene per pixel in medium and high quality.
        let mut lamps = self.lamps.clone();
        lamps.sort_by(|a, b| a.distance_squared(view.eye).total_cmp(&b.distance_squared(view.eye)));
        self.scene.lights.spots = lamps
            .iter()
            .take(5)
            .map(|&lamp| SpotLight {
                position: lamp - Vec3::Y * 0.2,
                target: lamp.with_y(0.0),
                color: hex(LAMP_WARM, 1.0),
                intensity: 5.0 * PI,
                distance: 32.0,
                angle: 0.95,
                penumbra: 0.6,
            })
            .collect();
    }

    /// Draws each car: its body, wheels, lights, shadow, and the light of its headlights
    /// on the road. Cars in a tunnel get darker.
    fn place_cars(&mut self, frame: &FrameInput, glows: &mut Vec<Instance>) {
        let detailed = frame.quality != Quality::Low;
        let (mut paint, mut trim, mut wheels, mut shadows, mut beams) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let speed_share = |car: &Car| (car.speed / cars::TOP_SPEED).clamp(-1.0, 1.0);
        for (car, &color) in frame.cars.iter().zip(frame.colors) {
            let cover = frame.track.cover[car.index];
            let light = 1.0 - 0.55 * cover;
            // Leans out of turns, and dips its nose under braking.
            let roll = car.steer * speed_share(car) * 0.035;
            let pitch = if car.braking {
                -0.02
            } else if car.boost > 0.0 {
                0.015
            } else {
                0.0
            };
            // Up or down the ramps to a bridge, whichever way the car faces.
            let tangent = frame.track.tangents[car.index];
            let slope = tangent.y.asin() * car.forward().dot(tangent).signum();
            let rotation = Quat::from_rotation_y(car.yaw)
                * Quat::from_rotation_z(roll)
                * Quat::from_rotation_x(pitch + slope);
            let body = Mat4::from_rotation_translation(rotation, car.position);
            paint.push(instance(body, color * light));
            trim.push(instance(body, Vec3::splat(light)));
            for (x, z, steers, width) in car_model::WHEELS {
                let steer = if steers { -car.steer * 0.4 } else { 0.0 };
                let wheel = body
                    * Mat4::from_translation(Vec3::new(x, WHEEL_RADIUS, z))
                    * Mat4::from_rotation_y(steer)
                    * Mat4::from_rotation_x(-car.wheel_angle)
                    * Mat4::from_rotation_z(FRAC_PI_2)
                    * Mat4::from_scale(Vec3::new(WHEEL_RADIUS, width, WHEEL_RADIUS));
                wheels.push(instance(wheel, Vec3::splat(light)));
            }
            for (place, size, color) in car_model::lights(car) {
                glows.push(instance(
                    body * Mat4::from_scale_rotation_translation(size, Quat::IDENTITY, place),
                    color,
                ));
            }
            let flat = Quat::from_rotation_y(car.yaw);
            shadows.push(Instance {
                transform: oriented(
                    car.position + Vec3::Y * (POOL_Y + 0.01),
                    flat,
                    Vec3::new(2.8, 1.0, 5.6),
                ),
                // Medium and high quality's sun casts the shadows; the blob only darkens below the car.
                color: Vec4::new(1.0, 1.0, 1.0, if detailed { 0.4 } else { 0.8 }),
            });
            // Big blended surfaces cost small GPUs much per pixel, so only medium and
            // high quality draw the light of the headlights.
            if detailed {
                let road = frame.track.frame_at(car.progress() + 9.0).0.y;
                let ahead = (car.position + car.forward() * 9.0).with_y(road + POOL_Y + 0.02);
                beams.push(instance(
                    oriented(ahead, flat, Vec3::new(6.0, 1.0, 13.0)),
                    hex(LAMP_WARM, 0.12 + 0.3 * cover),
                ));
            }
        }
        for (node, list) in [
            (self.paint, paint),
            (self.trim, trim),
            (self.wheels, wheels),
            (self.shadows, shadows),
            (self.beams, beams),
        ] {
            self.scene.object(node).instances = Some(list);
        }
    }
}
