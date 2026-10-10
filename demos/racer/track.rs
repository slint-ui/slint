// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Seeded race tracks: circuits of straights and rounded corners around the origin, with
//! bays and hairpins, sometimes figure eights whose upper stretch crosses the lower one on a
//! bridge. The start is on the straightest stretch, tunnels go through hills on gentle
//! stretches, and boost pads lie on the road. The same seed always produces the same track.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3, Vec3Swizzles};

/// How many points the track is sampled at, evenly spaced along its length.
pub const SAMPLES: usize = 1200;
/// Half the width of the asphalt between the barriers.
pub const HALF_WIDTH: f32 = 6.0;
pub const TUNNEL_HEIGHT: f32 = 6.5;
/// How far the hill over a tunnel reaches to each side of the track's center line.
pub const HILL_REACH: f32 = HALF_WIDTH + 17.0;
/// Half the length and width of a boost pad.
pub const PAD_HALF: Vec2 = Vec2::new(3.5, 1.6);
/// How far the lights in a tunnel are apart.
pub const TUNNEL_LIGHT_SPACING: f32 = 7.0;
/// The tightest corner's radius on the center line.
const CORNER_RADIUS: f32 = 28.0;
/// How many of the tracks are figure eights.
const FIGURE_EIGHT_SHARE: f32 = 0.4;
/// The road's height on a bridge, over a tunnel's clearance and the bridge's deck.
pub const BRIDGE_HEIGHT: f32 = TUNNEL_HEIGHT + 1.2;
/// How long the ramps up to a bridge are.
const RAMP: f32 = 110.0;
/// How far the embankment under a raised road reaches out, per meter of its height.
pub const EMBANKMENT_SLOPE: f32 = 1.5;
/// How far the embankment's top reaches out from the center line: to the barriers' outside.
pub const EMBANKMENT_TOP: f32 = HALF_WIDTH + 0.7;
/// The flattest crossing a figure eight may have, between its two stretches.
const MIN_CROSSING_ANGLE: f32 = 70.0;

pub struct Tunnel {
    /// The first sample inside.
    pub start: usize,
    /// How many samples it covers, from `start` on, wrapping around the track's end.
    pub samples: usize,
    /// Under a figure eight's bridge, rather than through a hill.
    pub bridge: bool,
}

/// Where a figure eight crosses itself.
pub struct Crossing {
    /// The samples of the upper and the lower stretch at the crossing.
    pub upper: usize,
    pub lower: usize,
    /// How far the embankment under the upper stretch is cut away to each side of the
    /// crossing, in meters along it, for the lower stretch to pass between the abutments.
    pub gap: f32,
}

pub struct Pad {
    pub index: usize,
    /// To the right of the center line, in meters.
    pub offset: f32,
}

pub struct Track {
    pub seed: u32,
    pub length: f32,
    pub points: Vec<Vec3>,
    pub tangents: Vec<Vec3>,
    pub rights: Vec<Vec3>,
    /// Signed curvature in 1/m, positive in left turns.
    pub curvature: Vec<f32>,
    /// How far inside a tunnel each sample is, from 0 outside to 1 a few meters in.
    pub cover: Vec<f32>,
    pub tunnels: Vec<Tunnel>,
    pub pads: Vec<Pad>,
    pub crossing: Option<Crossing>,
}

impl Track {
    /// The distance between neighboring samples.
    pub fn spacing(&self) -> f32 {
        self.length / SAMPLES as f32
    }

    pub fn wrap(index: isize) -> usize {
        index.rem_euclid(SAMPLES as isize) as usize
    }

    /// The sample nearest to `distance` along the track, counting from the start line.
    pub fn index_at(&self, distance: f32) -> usize {
        Self::wrap((distance / self.spacing()).round() as isize)
    }

    /// The center line at `distance`: the point, the direction of travel, and the right side.
    pub fn frame_at(&self, distance: f32) -> (Vec3, Vec3, Vec3) {
        let f = distance.rem_euclid(self.length) / self.spacing();
        let (i0, t) = (f.floor() as usize % SAMPLES, f.fract());
        let i1 = (i0 + 1) % SAMPLES;
        (
            self.points[i0].lerp(self.points[i1], t),
            self.tangents[i0].lerp(self.tangents[i1], t).normalize(),
            self.rights[i0].lerp(self.rights[i1], t).normalize(),
        )
    }

    /// How far `to` is ahead of `from` along the track, in samples, from -SAMPLES/2 on.
    pub fn steps(from: usize, to: usize) -> isize {
        let step = (to + SAMPLES - from) % SAMPLES;
        if step > SAMPLES / 2 { step as isize - SAMPLES as isize } else { step as isize }
    }

    /// The sample nearest to `position`, searched around `hint` first.
    pub fn nearest(&self, position: Vec3, hint: usize) -> usize {
        let distance = |i: usize| self.points[i].distance_squared(position);
        (0..120)
            .map(|step| (hint + SAMPLES + step - 40) % SAMPLES)
            .min_by(|&a, &b| distance(a).total_cmp(&distance(b)))
            .filter(|&i| distance(i) < (HALF_WIDTH * 3.0).powi(2))
            .unwrap_or_else(|| {
                (0..SAMPLES).min_by(|&a, &b| distance(a).total_cmp(&distance(b))).unwrap_or(0)
            })
    }
}

/// The `mulberry32` generator.
fn mulberry32(seed: u32) -> impl FnMut() -> f32 {
    let mut a = seed;
    move || {
        a = a.wrapping_add(0x6d2b_79f5);
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        ((t ^ (t >> 14)) as f64 / 4_294_967_296.0) as f32
    }
}

/// A random number source for the track's scenery, from the track's seed.
pub fn scenery_random(seed: u32) -> impl FnMut() -> f32 {
    mulberry32(seed.wrapping_mul(31).wrapping_add(4242))
}

/// The corners of a circuit around the origin, some of them deep in.
fn circuit_corners(random: &mut impl FnMut() -> f32) -> Vec<Vec2> {
    let count = 8 + (random() * 6.0) as usize;
    let phase = random() * TAU;
    let (rx, rz) = (230.0 + random() * 60.0, 150.0 + random() * 50.0);
    let mut radii: Vec<f32> = (0..count).map(|_| 0.8 + random() * 0.2).collect();
    // Two or three bays, each two corners deep in.
    for _ in 0..2 + (random() * 2.0) as usize {
        let first = (random() * count as f32) as usize;
        let depth = 0.25 + random() * 0.4;
        for k in first..first + 2 {
            radii[k % count] = depth * (0.9 + random() * 0.2);
        }
    }
    (0..count)
        .map(|k| {
            let angle = phase + (k as f32 + (random() - 0.5) * 0.5) / count as f32 * TAU;
            Vec2::new(angle.cos() * rx, angle.sin() * rz) * radii[k]
        })
        .collect()
}

/// The corners of a figure eight around the origin, its lobes to the sides and its crossing
/// at about a right angle, turned by a random angle.
fn figure_eight_corners(random: &mut impl FnMut() -> f32) -> Vec<Vec2> {
    let count = 10 + (random() * 5.0) as usize;
    let half_width = 190.0 + random() * 60.0;
    // Half as high as wide crosses at a right angle.
    let half_height = half_width / 2.0 * (0.9 + random() * 0.3);
    let turn = Vec2::from_angle(random() * TAU);
    (0..count)
        .map(|k| {
            let t = (k as f32 + (random() - 0.5) * 0.4) / count as f32 * TAU;
            let point = Vec2::new(t.sin() * half_width, (2.0 * t).sin() * half_height);
            turn.rotate(point * (0.8 + random() * 0.3))
        })
        .collect()
}

/// The road through `corners`: straight between them, and around each on an arc at least
/// `CORNER_RADIUS` wide, if they leave room for that.
fn round_corners(corners: &[Vec2], random: &mut impl FnMut() -> f32) -> Option<Vec<Vec3>> {
    let count = corners.len();
    let mut path = Vec::new();
    for (k, &corner) in corners.iter().enumerate() {
        let (before, after) = (corners[(k + count - 1) % count], corners[(k + 1) % count]);
        let (incoming, outgoing) = (corner - before, after - corner);
        let turn = incoming.angle_to(outgoing);
        let half_tan = (turn.abs() / 2.0).tan();
        // Each arc takes up at most half of the straights to its sides.
        let widest = incoming.length().min(outgoing.length()) / 2.0 / half_tan.max(1e-4);
        let radius = (CORNER_RADIUS + random().powi(2) * 80.0).min(widest);
        if radius < CORNER_RADIUS {
            return None;
        }
        let start = corner - incoming.normalize() * radius * half_tan;
        let center = start + incoming.normalize().perp() * radius * turn.signum();
        let steps = (radius * turn.abs() / 2.0).ceil().max(1.0) as usize;
        path.extend((0..=steps).map(|step| {
            center + Vec2::from_angle(turn * step as f32 / steps as f32).rotate(start - center)
        }));
    }
    Some(path.into_iter().map(|p| Vec3::new(p.x, 0.0, p.y)).collect())
}

/// The length of the closed `path`.
fn path_length(path: &[Vec3]) -> f32 {
    (0..path.len()).map(|i| path[i].distance(path[(i + 1) % path.len()])).sum()
}

/// `count` points evenly spaced along the closed `path`, from its start.
fn resample(path: &[Vec3], count: usize) -> Vec<Vec3> {
    let length = path_length(path);
    let segment = |i: usize| (path[i], path[(i + 1) % path.len()]);
    let (mut i, mut walked) = (0, 0.0);
    (0..count)
        .map(|k| {
            let target = k as f32 / count as f32 * length;
            let (mut a, mut b) = segment(i);
            while walked + a.distance(b) < target && i + 1 < path.len() {
                walked += a.distance(b);
                i += 1;
                (a, b) = segment(i);
            }
            a.lerp(b, ((target - walked) / a.distance(b).max(1e-6)).clamp(0.0, 1.0))
        })
        .collect()
}

/// Points every 3 m along `path`.
fn probe(path: &[Vec3]) -> Vec<Vec3> {
    resample(path, (path_length(path) / 3.0).round() as usize)
}

/// How far the road along `path` turns in all, left and right, in radians.
fn turning(path: &[Vec3]) -> f32 {
    let count = path.len();
    let direction = |i: usize| (path[(i + 1) % count] - path[i]).xz().normalize_or_zero();
    (0..count).map(|i| direction(i).angle_to(direction((i + 1) % count)).abs()).sum()
}

/// Whether the stretches along `q` keep the barriers and some grass apart, except near
/// `crossing`.
fn apart(q: &[Vec3], crossing: Option<Vec2>) -> bool {
    let count = q.len();
    let near = (2.0 * HALF_WIDTH + 18.0).powi(2);
    let at_crossing = |p: Vec3| crossing.is_some_and(|c| p.xz().distance(c) < 45.0);
    for i in 0..count {
        for j in i + 25..count {
            if count - (j - i) >= 25
                && q[i].distance_squared(q[j]) < near
                && !(at_crossing(q[i]) && at_crossing(q[j]))
            {
                return false;
            }
        }
    }
    true
}

/// Where the segments `a0`-`a1` and `b0`-`b1` cross, if they do.
fn intersection(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> Option<Vec2> {
    let (r, s) = (a1 - a0, b1 - b0);
    let denominator = r.perp_dot(s);
    if denominator.abs() < 1e-9 {
        return None;
    }
    let t = (b0 - a0).perp_dot(s) / denominator;
    let u = (b0 - a0).perp_dot(r) / denominator;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| a0 + r * t)
}

/// The crossing of a drivable figure eight: exactly one, steep enough to bridge.
fn figure_eight_crossing(path: &[Vec3]) -> Option<Vec2> {
    let q = probe(path);
    let count = q.len();
    let segment = |i: usize| (q[i].xz(), q[(i + 1) % count].xz());
    let mut crossings = Vec::new();
    for i in 0..count {
        for j in i + 2..count {
            if (j + 1) % count == i {
                continue;
            }
            let ((a0, a1), (b0, b1)) = (segment(i), segment(j));
            if let Some(point) = intersection(a0, a1, b0, b1) {
                crossings.push((point, (a1 - a0).normalize(), (b1 - b0).normalize()));
            }
        }
    }
    let [(point, a, b)] = crossings[..] else { return None };
    let angle = a.perp_dot(b).abs().asin().to_degrees();
    (angle >= MIN_CROSSING_ANGLE && apart(&q, Some(point))).then_some(point)
}

/// A closed path for `seed`: a figure eight with its crossing, or else a circuit.
fn curve(seed: u32) -> (Vec<Vec3>, Option<Vec2>) {
    let eight = mulberry32(seed ^ 0x8e1f_2a63)() < FIGURE_EIGHT_SHARE;
    for attempt in 0..(if eight { 300u32 } else { 0 }) {
        let mut random = mulberry32(seed.wrapping_mul(6007).wrapping_add(attempt * 7_919));
        let corners = figure_eight_corners(&mut random);
        if let Some(path) = round_corners(&corners, &mut random)
            && let Some(crossing) = figure_eight_crossing(&path)
        {
            return (path, Some(crossing));
        }
    }
    // The twistiest of the first few drivable circuits.
    let circuits = (0u32..).filter_map(|attempt| {
        let mut random = mulberry32(seed.wrapping_mul(4099).wrapping_add(attempt * 15_331));
        let corners = circuit_corners(&mut random);
        round_corners(&corners, &mut random).filter(|path| apart(&probe(path), None))
    });
    let circuit = circuits.take(5).max_by(|a, b| turning(a).total_cmp(&turning(b)));
    // Most candidates are drivable, so the search ends after a few.
    (circuit.expect("a drivable circuit"), None)
}

/// The height of a raised road `distance` meters from the middle of its bridge, which is
/// level for `flat` meters to each side.
fn ramp_height(distance: f32, flat: f32) -> f32 {
    let t = 1.0 - ((distance.abs() - flat) / RAMP).clamp(0.0, 1.0);
    BRIDGE_HEIGHT * t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

pub fn generate(seed: u32) -> Track {
    let (path, crossing_point) = curve(seed);
    let length = path_length(&path);
    let mut points = resample(&path, SAMPLES);
    let spacing = length / SAMPLES as f32;

    let wrap = |i: isize| Track::wrap(i);
    let tangents_of = |points: &[Vec3]| -> Vec<Vec3> {
        (0..SAMPLES as isize)
            .map(|i| (points[wrap(i + 1)] - points[wrap(i - 1)]).normalize())
            .collect()
    };
    let curvature_of = |tangents: &[Vec3]| -> Vec<f32> {
        (0..SAMPLES as isize)
            .map(|i| {
                let (a, b) = (tangents[wrap(i - 4)], tangents[wrap(i + 4)]);
                (a.z * b.x - a.x * b.z) / (8.0 * spacing)
            })
            .collect()
    };
    let mut random = mulberry32(seed.wrapping_add(77));

    // A figure eight's upper stretch rises over the lower one: level across the bridge, and
    // on ramps to each side. Nothing else goes there, nor near the lower stretch's underpass.
    let mut crossing = crossing_point.map(|point| {
        let nearest = |skip: Option<usize>| {
            (0..SAMPLES)
                .filter(|&i| skip.is_none_or(|j| Track::steps(j, i).unsigned_abs() > SAMPLES / 8))
                .min_by(|&a, &b| {
                    points[a].xz().distance(point).total_cmp(&points[b].xz().distance(point))
                })
                .unwrap_or(0)
        };
        let first = nearest(None);
        let second = nearest(Some(first));
        let (upper, lower) = if random() < 0.5 { (first, second) } else { (second, first) };
        let direction = |i: usize| (points[(i + 1) % SAMPLES] - points[i]).xz().normalize();
        let (sin, cos) = {
            let (a, b) = (direction(upper), direction(lower));
            (a.perp_dot(b).abs(), a.dot(b).abs())
        };
        // The abutments stand a meter beyond the lower stretch's barriers, and further
        // where the stretches cross at a slant, so the embankment's slopes clear them.
        let reach = EMBANKMENT_TOP + EMBANKMENT_SLOPE * BRIDGE_HEIGHT;
        let gap = (EMBANKMENT_TOP + 1.0) / sin + reach * cos / sin;
        Crossing { upper, lower, gap }
    });
    let mut forbidden = vec![false; SAMPLES];
    if let Some(crossing) = &crossing {
        let flat = crossing.gap + 6.0;
        for (i, point) in points.iter_mut().enumerate() {
            let along = Track::steps(crossing.upper, i) as f32 * spacing;
            point.y = ramp_height(along, flat);
            let to_lower = Track::steps(crossing.lower, i).unsigned_abs() as f32 * spacing;
            forbidden[i] = point.y > 0.0 || to_lower < 50.0;
        }
    }

    // The start line goes 60% into the straightest 150 m clear of the bridge, so the grid
    // has a straight behind it.
    let curvature = curvature_of(&tangents_of(&points));
    let window = (150.0 / spacing) as usize;
    let straightness =
        |i: usize| -> f32 { (0..window).map(|k| curvature[(i + k) % SAMPLES].abs()).sum() };
    let clear = |i: usize| (0..window).all(|k| !forbidden[(i + k) % SAMPLES]);
    let straightest = (0..SAMPLES)
        .filter(|&i| clear(i))
        .min_by(|&a, &b| straightness(a).total_cmp(&straightness(b)));
    let start = (straightest.unwrap_or(0) + window * 3 / 5) % SAMPLES;
    points.rotate_left(start);
    forbidden.rotate_left(start);
    if let Some(crossing) = &mut crossing {
        crossing.upper = (crossing.upper + SAMPLES - start) % SAMPLES;
        crossing.lower = (crossing.lower + SAMPLES - start) % SAMPLES;
    }
    let tangents = tangents_of(&points);
    let rights: Vec<Vec3> = tangents.iter().map(|t| t.cross(Vec3::Y).normalize()).collect();
    let curvature = curvature_of(&tangents);

    let mut tunnels = place_tunnels(&points, &curvature, &forbidden, spacing, &mut random);
    if let Some(crossing) = &crossing {
        // The lower stretch is covered where the bridge's deck is above it.
        let direction = |i: usize| tangents[i].xz().normalize();
        let sin = direction(crossing.upper).perp_dot(direction(crossing.lower)).abs();
        let half = ((EMBANKMENT_TOP / sin + 1.0) / spacing).ceil() as usize;
        tunnels.push(Tunnel {
            start: (crossing.lower + SAMPLES - half) % SAMPLES,
            samples: 2 * half + 1,
            bridge: true,
        });
    }
    let mut cover = vec![0.0f32; SAMPLES];
    // Darkens over the first few meters inside.
    let fade = (8.0 / spacing).max(1.0);
    for tunnel in &tunnels {
        for k in 0..tunnel.samples {
            let inside = k.min(tunnel.samples - 1 - k) as f32;
            cover[(tunnel.start + k) % SAMPLES] = (inside / fade).clamp(0.15, 1.0);
        }
    }

    let pads = place_pads(&curvature, &forbidden, spacing, &mut random);
    Track { seed, length, points, tangents, rights, curvature, cover, tunnels, pads, crossing }
}

/// One or two tunnels on gentle stretches away from the start, where nothing else is within
/// reach of the hill above.
fn place_tunnels(
    points: &[Vec3],
    curvature: &[f32],
    forbidden: &[bool],
    spacing: f32,
    random: &mut impl FnMut() -> f32,
) -> Vec<Tunnel> {
    let wanted = if random() < 0.6 { 2 } else { 1 };
    let length = spacing * SAMPLES as f32;
    let mut tunnels: Vec<Tunnel> = Vec::new();
    for _ in 0..80 {
        if tunnels.len() == wanted {
            break;
        }
        let samples = ((70.0 + random() * 60.0) / spacing) as usize;
        let start = (random() * SAMPLES as f32) as usize;
        let from = start as f32 * spacing;
        // Clear of the start line and the grid behind it.
        if from < 80.0 || from + samples as f32 * spacing > length - 120.0 {
            continue;
        }
        let range = start..start + samples;
        if range.clone().any(|i| curvature[i].abs() > 1.0 / 45.0) {
            continue;
        }
        // The hill reaches this far along the track past the tunnel's ends, too.
        let margin = (HILL_REACH * 1.5 / spacing) as usize;
        if (start + SAMPLES - margin..start + samples + margin).any(|i| forbidden[i % SAMPLES]) {
            continue;
        }
        let apart = (60.0 / spacing) as usize;
        let overlaps = tunnels.iter().any(|other| {
            start < other.start + other.samples + apart && other.start < start + samples + apart
        });
        if overlaps {
            continue;
        }
        let reach = (HILL_REACH + HALF_WIDTH + 6.0).powi(2);
        let near = (HILL_REACH * 2.5 / spacing) as usize;
        let crowded = range.clone().step_by(4).any(|i| {
            (0..SAMPLES).step_by(3).any(|m| {
                let apart = m.abs_diff(i).min(SAMPLES - m.abs_diff(i));
                apart > near && points[m].xz().distance_squared(points[i].xz()) < reach
            })
        });
        if !crowded {
            tunnels.push(Tunnel { start, samples, bridge: false });
        }
    }
    tunnels.sort_by_key(|tunnel| tunnel.start);
    tunnels
}

/// Four boost pads, mostly on straights, to the left, right, or middle of the track.
fn place_pads(
    curvature: &[f32],
    forbidden: &[bool],
    spacing: f32,
    random: &mut impl FnMut() -> f32,
) -> Vec<Pad> {
    let length = spacing * SAMPLES as f32;
    let mut pads: Vec<Pad> = Vec::new();
    for attempt in 0..200 {
        if pads.len() == 4 {
            break;
        }
        let index = (random() * SAMPLES as f32) as usize;
        let at = index as f32 * spacing;
        // Clear of the grid, and not right after the start.
        if at < 60.0 || at > length - 70.0 || forbidden[index] {
            continue;
        }
        if curvature[index].abs() > 1.0 / 70.0 && attempt < 150 {
            continue;
        }
        let far_enough = pads.iter().all(|pad| {
            let apart = pad.index.abs_diff(index).min(SAMPLES - pad.index.abs_diff(index));
            apart as f32 * spacing > 80.0
        });
        if far_enough {
            let offset = [-3.2, 0.0, 3.2][(random() * 3.0) as usize % 3];
            pads.push(Pad { index, offset });
        }
    }
    pads.sort_by_key(|pad| pad.index);
    pads
}
