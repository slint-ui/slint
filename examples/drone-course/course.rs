// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore catmull

//! Seeded race courses inside the hall: closed Lissajous curves, mostly figure-eights whose
//! crossings are flown over and under, with gates on the flatter stretches and LED towers on
//! the inside of the tightest turns. The same seed always produces the same course.

use std::f32::consts::TAU;

use glam::Vec3;

/// The hall's half extents and height, in meters.
pub const HALL_X: f32 = 64.0;
pub const HALL_Z: f32 = 36.0;
pub const HALL_HEIGHT: f32 = 22.0;

/// The three.js version of the course uses 6.28 rather than 2π for some phases.
#[allow(clippy::approx_constant)]
const PHASE_RANGE: f32 = 6.28;

/// How many points the course is sampled at, evenly spaced along its length.
pub const SAMPLES: usize = 1400;
/// Half the size of a gate's opening.
pub const GATE_HALF: f32 = 1.6;

pub struct Gate {
    pub center: Vec3,
    /// The course sample at the gate, counted on past `SAMPLES` for gates beyond the
    /// course's first sample in a lap, so the gates' indices increase.
    pub index: usize,
    /// The horizontal direction the course passes through the gate.
    pub forward: Vec3,
    pub right: Vec3,
    pub half: f32,
    /// A second frame stacked above the first.
    pub double: bool,
    pub start: bool,
}

impl Gate {
    /// The heights of the gate's frame centers.
    pub fn frame_heights(&self) -> Vec<f32> {
        let mut heights = vec![self.center.y];
        if self.double {
            heights.push(self.center.y + 2.0 * GATE_HALF + 0.5);
        }
        heights
    }

    pub fn yaw(&self) -> f32 {
        self.forward.x.atan2(self.forward.z)
    }
}

pub struct Course {
    pub seed: u32,
    pub length: f32,
    pub points: Vec<Vec3>,
    pub tangents: Vec<Vec3>,
    pub rights: Vec<Vec3>,
    pub gates: Vec<Gate>,
    pub towers: Vec<Vec3>,
}

impl Course {
    /// The distance between neighboring samples.
    pub fn spacing(&self) -> f32 {
        self.length / SAMPLES as f32
    }
}

/// The `mulberry32` generator, so seeds match the three.js version of this course.
fn mulberry32(seed: u32) -> impl FnMut() -> f32 {
    let mut a = seed;
    move || {
        a = a.wrapping_add(0x6d2b_79f5);
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        ((t ^ (t >> 14)) as f64 / 4_294_967_296.0) as f32
    }
}

/// A closed centripetal Catmull-Rom spline, parameterized like three.js's `CatmullRomCurve3`.
struct Spline {
    points: Vec<Vec3>,
    /// Cumulative arc lengths at evenly spaced parameter values.
    lengths: Vec<f32>,
}

impl Spline {
    const DIVISIONS: usize = 200;

    fn new(points: Vec<Vec3>) -> Self {
        let mut spline = Self { points, lengths: Vec::new() };
        let mut total = 0.0;
        let mut previous = spline.point(0.0);
        spline.lengths.push(0.0);
        for i in 1..=Self::DIVISIONS {
            let current = spline.point(i as f32 / Self::DIVISIONS as f32);
            total += current.distance(previous);
            spline.lengths.push(total);
            previous = current;
        }
        spline
    }

    fn length(&self) -> f32 {
        *self.lengths.last().unwrap()
    }

    fn point(&self, t: f32) -> Vec3 {
        let count = self.points.len();
        let p = count as f32 * t;
        let segment = p.floor() as usize;
        let weight = p - p.floor();
        let at = |offset: isize| {
            self.points[(segment as isize + offset).rem_euclid(count as isize) as usize]
        };
        let (p0, p1, p2, p3) = (at(-1), at(0), at(1), at(2));
        let mut dt1 = p1.distance_squared(p2).powf(0.25);
        let mut dt0 = p0.distance_squared(p1).powf(0.25);
        let mut dt2 = p2.distance_squared(p3).powf(0.25);
        if dt1 < 1e-4 {
            dt1 = 1.0;
        }
        if dt0 < 1e-4 {
            dt0 = dt1;
        }
        if dt2 < 1e-4 {
            dt2 = dt1;
        }
        let axis = |x0: f32, x1: f32, x2: f32, x3: f32| {
            let t1 = ((x1 - x0) / dt0 - (x2 - x0) / (dt0 + dt1) + (x2 - x1) / dt1) * dt1;
            let t2 = ((x2 - x1) / dt1 - (x3 - x1) / (dt1 + dt2) + (x3 - x2) / dt2) * dt1;
            let c2 = -3.0 * x1 + 3.0 * x2 - 2.0 * t1 - t2;
            let c3 = 2.0 * x1 - 2.0 * x2 + t1 + t2;
            x1 + t1 * weight + c2 * weight * weight + c3 * weight * weight * weight
        };
        Vec3::new(
            axis(p0.x, p1.x, p2.x, p3.x),
            axis(p0.y, p1.y, p2.y, p3.y),
            axis(p0.z, p1.z, p2.z, p3.z),
        )
    }

    /// The parameter at fraction `u` of the arc length.
    fn parameter_at(&self, u: f32) -> f32 {
        let target = u * self.length();
        let i = self.lengths.partition_point(|&length| length < target).clamp(1, Self::DIVISIONS);
        let (before, after) = (self.lengths[i - 1], self.lengths[i]);
        let fraction = if after > before { (target - before) / (after - before) } else { 0.0 };
        (i as f32 - 1.0 + fraction) / Self::DIVISIONS as f32
    }

    fn point_at(&self, u: f32) -> Vec3 {
        self.point(self.parameter_at(u))
    }

    fn tangent_at(&self, u: f32) -> Vec3 {
        let t = self.parameter_at(u);
        let delta = 0.0001;
        (self.point((t + delta).min(1.0)) - self.point((t - delta).max(0.0))).normalize()
    }
}

fn generate_curve(seed: u32) -> Spline {
    let mut spline = None;
    for attempt in 0..300u32 {
        let mut random = mulberry32(
            seed.wrapping_mul(7919).wrapping_add(attempt.wrapping_mul(104_729)).wrapping_add(3),
        );
        let figure_eight = attempt < 220 && random() < 0.8;
        let b = if figure_eight { 2.0 } else { 1.0 };
        let phase = random() * TAU;
        let amplitude_x = 46.0 + random() * 8.0;
        let amplitude_z = 22.0 + random() * 5.0;
        let height_harmonic = 1.0 + (random() * 3.0).floor();
        let height_phase = random() * PHASE_RANGE;
        let base_height = 6.5 + random() * 1.5;
        let height_swing = 3.0 + random() * 2.5;
        let wobble: [[f32; 4]; 3] = std::array::from_fn(|_| {
            let harmonic = 2.0 + (random() * 4.0).floor();
            let wobble_phase = random() * PHASE_RANGE;
            let amount = if figure_eight { 1.0 } else { 2.5 } + random() * 1.5;
            [harmonic, wobble_phase, amount, random() * PHASE_RANGE]
        });

        let points = (0..36)
            .map(|k| {
                let t = k as f32 / 36.0 * TAU;
                let x = amplitude_x * (t + phase).sin()
                    + wobble[0][2] * (wobble[0][0] * t + wobble[0][1]).sin();
                let z = amplitude_z * (b * t).sin()
                    + wobble[1][2] * (wobble[1][0] * t + wobble[1][1]).sin() * 0.7;
                let y = base_height
                    + height_swing * (height_harmonic * t + height_phase).sin()
                    + 1.2 * (wobble[2][0] * t + wobble[2][1]).sin();
                Vec3::new(x, y, z)
            })
            .collect();
        let candidate = Spline::new(points);
        let valid = is_flyable(&candidate);
        spline = Some(candidate);
        if valid {
            break;
        }
    }
    spline.unwrap()
}

/// Rejects courses with turns tighter than 5 m, steep climbs, points outside the hall, or
/// crossings closer than 4.2 m in height.
fn is_flyable(spline: &Spline) -> bool {
    let count = (spline.length() / 2.0).round() as usize;
    let q: Vec<Vec3> = (0..count).map(|i| spline.point_at(i as f32 / count as f32)).collect();
    for i in 0..count {
        let (p, a, b) = (q[i], q[(i + 1) % count], q[(i + 2) % count]);
        let (ab, bc, ca) = (a.distance(p), b.distance(a), b.distance(p));
        let s = (ab + bc + ca) / 2.0;
        let area = (s * (s - ab) * (s - bc) * (s - ca)).max(1e-12).sqrt();
        if ab * bc * ca / (4.0 * area) < 5.0
            || (a.y - p.y).abs() / ab > 0.42
            || p.x.abs() > HALL_X - 5.0
            || p.z.abs() > HALL_Z - 4.0
            || !(1.8..=14.0).contains(&p.y)
        {
            return false;
        }
    }
    for i in 0..count {
        for j in i + 20..count {
            if count - (j - i) < 20 {
                continue;
            }
            let (dx, dz) = (q[i].x - q[j].x, q[i].z - q[j].z);
            if dx * dx + dz * dz < 25.0 && (q[i].y - q[j].y).abs() < 4.2 {
                return false;
            }
        }
    }
    true
}

pub fn generate(seed: u32) -> Course {
    let spline = generate_curve(seed);
    let length = spline.length();
    let spacing = length / SAMPLES as f32;
    let points: Vec<Vec3> =
        (0..SAMPLES).map(|i| spline.point_at(i as f32 / SAMPLES as f32)).collect();
    let tangents: Vec<Vec3> =
        (0..SAMPLES).map(|i| spline.tangent_at(i as f32 / SAMPLES as f32)).collect();
    let rights: Vec<Vec3> = tangents.iter().map(|t| t.cross(Vec3::Y).normalize()).collect();
    let wrap = |i: isize| i.rem_euclid(SAMPLES as isize) as usize;
    // Signed curvature in the horizontal plane.
    let curvature: Vec<f32> = (0..SAMPLES as isize)
        .map(|i| {
            let (a, b) = (tangents[wrap(i - 5)], tangents[wrap(i + 5)]);
            (a.z * b.x - a.x * b.z) / (10.0 * spacing)
        })
        .collect();

    let mut random = mulberry32(seed.wrapping_add(77));
    let gate_count = ((length / 30.0).round() as usize).clamp(8, 15);
    // Keeps gates away from the other stretch of a crossing.
    let clear = |j: usize| {
        (0..SAMPLES).step_by(4).all(|m| {
            let separation = m.abs_diff(j).min(SAMPLES - m.abs_diff(j)) as f32 * spacing;
            let (dx, dz) = (points[m].x - points[j].x, points[m].z - points[j].z);
            separation < 20.0 || dx * dx + dz * dz >= 64.0
        })
    };
    let suits_gate = |i: usize| tangents[i % SAMPLES].y.abs() <= 0.2 && clear(i % SAMPLES);
    // Gates keep this many samples apart, so each comes after the one before in a lap.
    let gap = (10.0 / spacing).ceil() as usize;
    let mut start = 0;
    while start < 180 && !suits_gate(start) {
        start += 3;
    }
    // Sample indices from the start gate on, so the last gate comes before the finish.
    let mut indices = vec![start];
    for k in 1..gate_count {
        let limit = start + SAMPLES - gap;
        let mut i = (k * SAMPLES / gate_count).max(indices[k - 1] + gap);
        if i >= limit {
            break;
        }
        for _ in 0..60 {
            if suits_gate(i) || i + 3 >= limit {
                break;
            }
            i += 3;
        }
        indices.push(i);
    }
    let gates = indices
        .iter()
        .enumerate()
        .map(|(k, &index)| {
            let i = index % SAMPLES;
            let double = k > 0 && random() < 0.22 && points[i].y < 8.0;
            let forward = tangents[i].with_y(0.0).normalize();
            Gate {
                center: points[i],
                index,
                forward,
                right: forward.cross(Vec3::Y).normalize() * -1.0,
                half: if k == 0 { GATE_HALF * 1.8 } else { GATE_HALF },
                double,
                start: k == 0,
            }
        })
        .collect();

    let mut order: Vec<usize> = (0..SAMPLES).collect();
    order.sort_by(|&a, &b| curvature[b].abs().total_cmp(&curvature[a].abs()));
    let mut used: Vec<usize> = Vec::new();
    let mut towers = Vec::new();
    for i in order {
        if towers.len() >= 3 {
            break;
        }
        if used.iter().any(|&u| u.abs_diff(i).min(SAMPLES - u.abs_diff(i)) < SAMPLES / 6) {
            continue;
        }
        let side = if curvature[i] < 0.0 { 1.0 } else { -1.0 };
        let radius = 1.0 / curvature[i].abs().max(1e-3);
        let p = points[i] + rights[i] * side * (radius * 0.6).min(5.0);
        if p.x.abs() > HALL_X - 3.0 || p.z.abs() > HALL_Z - 3.0 {
            continue;
        }
        let hits_course = (0..SAMPLES).step_by(3).any(|m| {
            let (dx, dz) = (points[m].x - p.x, points[m].z - p.z);
            dx * dx + dz * dz < 9.0
        });
        if hits_course {
            continue;
        }
        used.push(i);
        towers.push(p.with_y(0.0));
    }

    Course { seed, length, points, tangents, rights, gates, towers }
}
