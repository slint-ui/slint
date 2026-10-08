// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Rival drones that race the course next to the player's. They fly along the course line,
//! each at its own pace and with its own line through the turns, and keep inside the gates.

use glam::{Vec2, Vec3};

use crate::course::{Course, GATE_HALF, SAMPLES};
use crate::textures::smoothstep;
use crate::{
    DRONE_SPAN, GROUND_CLEARANCE, LAUNCH_DISTANCE, PAD_SIZE, TURN_RATE, damp, launch_index,
    launch_pad, wrap_angle, yaw_towards,
};

/// How far apart drones keep their centers, a little more than they're wide.
const SPACING: f32 = DRONE_SPAN + 0.4;

/// Another drone that a rival keeps clear of.
#[derive(Clone, Copy)]
pub struct Neighbor {
    pub position: Vec3,
    pub speed: f32,
}

pub struct Pilot {
    pub name: &'static str,
    pub color: u32,
    /// Meters per second, before the pace varies.
    speed: f32,
    /// The preferred line, sideways and up from the course line, in meters.
    line: Vec2,
    phase: f32,
    frequency: f32,
}

pub const PILOTS: [Pilot; 4] = [
    Pilot {
        name: "Volta",
        color: 0xff8a3d,
        speed: 10.3,
        line: Vec2::new(0.6, 0.2),
        phase: 0.0,
        frequency: 0.45,
    },
    Pilot {
        name: "Kestrel",
        color: 0xf2c94c,
        speed: 10.0,
        line: Vec2::new(-0.7, -0.1),
        phase: 2.1,
        frequency: 0.37,
    },
    Pilot {
        name: "Nix",
        color: 0x3fd0a8,
        speed: 9.7,
        line: Vec2::new(0.2, 0.4),
        phase: 4.0,
        frequency: 0.52,
    },
    Pilot {
        name: "Wren",
        color: 0xd060e0,
        speed: 10.15,
        line: Vec2::new(-0.3, -0.3),
        phase: 5.3,
        frequency: 0.41,
    },
];

/// A launch pad on the grid and its distance along the course, which is negative before the
/// start gate. Slot 0 is the player's pad; rivals stand beside and behind it.
pub fn grid_slot(course: &Course, slot: usize) -> (Vec3, f32) {
    let index = launch_index(course);
    let (side, row) = match slot {
        0 => (0.0, 0.0),
        _ => {
            // Pads side by side keep a gap, so they don't overlap.
            let side = PAD_SIZE + 0.7;
            (if slot % 2 == 1 { side } else { -side }, ((slot - 1) / 2) as f32 * 4.5)
        }
    };
    let back = -course.tangents[index].with_y(0.0).normalize();
    (launch_pad(course) + course.rights[index] * side + back * row, -LAUNCH_DISTANCE - row)
}

/// The course line at `distance`: the point, the direction of travel, and the right side.
fn frame_at(course: &Course, distance: f32) -> (Vec3, Vec3, Vec3) {
    let f = distance.rem_euclid(course.length) / course.spacing();
    let (i0, t) = (f.floor() as usize % SAMPLES, f.fract());
    let i1 = (i0 + 1) % SAMPLES;
    (
        course.points[i0].lerp(course.points[i1], t),
        course.tangents[i0].lerp(course.tangents[i1], t).normalize(),
        course.rights[i0].lerp(course.rights[i1], t).normalize(),
    )
}

pub struct Rival {
    pub pilot: &'static Pilot,
    pad: Vec3,
    /// How far along the course the rival is, counting every lap.
    pub distance: f32,
    pub speed: f32,
    offset: Vec2,
    offset_velocity: Vec2,
    /// From 0 on the pad to 1 once on the course line.
    lift: f32,
    pub position: Vec3,
    pub yaw: f32,
    /// Like the player's stick, from -1 (left) to 1 (right).
    pub roll: f32,
}

impl Rival {
    pub fn new(course: &Course, slot: usize) -> Self {
        let (pad, distance) = grid_slot(course, slot);
        let (_, tangent, _) = frame_at(course, distance);
        Self {
            pilot: &PILOTS[slot - 1],
            pad,
            distance,
            speed: 0.0,
            offset: Vec2::ZERO,
            offset_velocity: Vec2::ZERO,
            lift: 0.0,
            position: pad.with_y(GROUND_CLEARANCE),
            yaw: yaw_towards(tangent),
            roll: 0.0,
        }
    }

    /// Flies `dt` seconds further, `time` seconds into the race, with the pace scaled by `push`.
    /// It keeps `SPACING` from `others` like a pilot would: it moves aside where there's
    /// room, and otherwise queues up behind a drone just ahead, such as through a gate.
    pub fn fly(&mut self, course: &Course, time: f32, push: f32, others: &[Neighbor], dt: f32) {
        let pilot = self.pilot;
        self.lift = (self.lift + dt / 1.4).min(1.0);
        let pace = 1.0 + 0.04 * (time * pilot.frequency + pilot.phase).sin();
        let mut target = pilot.speed * pace * push * (0.35 + 0.65 * self.lift);

        let (_, tangent, right) = frame_at(course, self.distance);
        let up = right.cross(tangent).normalize();
        let mut aside = Vec2::ZERO;
        for other in others {
            let delta = other.position - self.position;
            let ahead = delta.dot(tangent);
            let across = Vec2::new(delta.dot(right), delta.dot(up));
            let gap = across.length();
            if ahead.abs() > SPACING * 1.5 || gap > SPACING {
                continue;
            }
            // Away from the other drone, or to this pilot's side when right behind it.
            let away =
                if gap > 0.01 { -across / gap } else { Vec2::new(pilot.line.x.signum(), 0.0) };
            aside += away * (SPACING - gap);
            if ahead > 0.0 {
                target = target.min(other.speed * (0.8 + 0.2 * ahead / (SPACING * 1.5)));
            }
        }
        self.speed += (target - self.speed).clamp(-32.0 * dt, 17.0 * dt);
        self.distance += self.speed * dt;

        let along = self.distance.rem_euclid(course.length);
        let near_gate = course.gates.iter().any(|gate| {
            let gap = (gate.index as f32 * course.spacing() - along).rem_euclid(course.length);
            gap.min(course.length - gap) < 9.0
        });
        // Through a gate, the propellers stay clear of its frame.
        let limit = if near_gate { GATE_HALF - 0.1 - DRONE_SPAN / 2.0 } else { 2.2 };
        let wave = |scale: f32| (time * pilot.frequency * scale + pilot.phase).sin();
        let aim = (pilot.line + Vec2::new(0.7 * wave(1.3), 0.35 * wave(1.7)) + aside * 1.5)
            .clamp(Vec2::splat(-limit), Vec2::splat(limit));
        self.offset_velocity += ((aim - self.offset) * 7.0 - self.offset_velocity * 4.0) * dt;
        self.offset =
            (self.offset + self.offset_velocity * dt).clamp(Vec2::splat(-2.6), Vec2::splat(2.6));

        let (point, tangent, right) = frame_at(course, self.distance);
        let up = right.cross(tangent).normalize();
        let path = point + right * self.offset.x + up * self.offset.y;
        let ease = smoothstep(0.0, 1.0, self.lift);
        let position = self.pad.with_y(GROUND_CLEARANCE).lerp(path, ease);
        self.position = position.with_y(position.y.max(GROUND_CLEARANCE));

        let yaw = yaw_towards(tangent);
        let turn = wrap_angle(yaw - self.yaw) / dt.max(1e-3);
        self.yaw = yaw;
        let roll = (-turn / TURN_RATE).clamp(-1.0, 1.0);
        self.roll += (roll - self.roll) * damp(5.0, dt);
    }
}
