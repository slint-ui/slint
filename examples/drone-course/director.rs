// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore signum trackside

//! The chase camera, and while the autopilot races, a broadcast director that cuts between
//! the chase camera, a camera behind the next gate, a trackside camera, and an overhead one.

use glam::{Quat, Vec3};

use crate::course::{Course, HALL_X, HALL_Z};
use crate::{CEILING, Phase, Race, damp, forward, looking_at};

#[derive(Clone, Copy, PartialEq)]
enum Shot {
    Chase,
    /// Behind a gate the drone flies towards, looking back at it.
    Gate,
    /// From the hall's corner nearest to the drone, with a long lens.
    Trackside,
    Overhead,
}

/// The order of the shots, and how many seconds each one runs at most.
const SHOTS: [(Shot, f32); 5] = [
    (Shot::Chase, 12.0),
    (Shot::Gate, 12.0),
    (Shot::Trackside, 10.0),
    (Shot::Chase, 11.0),
    (Shot::Overhead, 9.0),
];

pub struct View {
    pub eye: Vec3,
    pub rotation: Quat,
    pub fov_degrees: f32,
}

#[derive(Default)]
pub struct Director {
    shot: usize,
    time: f32,
    /// The gate the gate camera stands behind.
    gate: Option<usize>,
    /// Which side of the gate the gate camera stands on, alternating from gate to gate.
    side: f32,
    /// The corner the trackside camera stands in.
    corner: Vec3,
    eye: Option<Vec3>,
}

impl Director {
    /// Makes the next view start fresh instead of moving there, such as after the onboard
    /// camera.
    pub fn cut(&mut self) {
        self.eye = None;
    }

    /// The camera for `race`. With `cutting`, the director cycles through the shots; otherwise
    /// it stays with the chase camera. `shift` pans the camera sideways, relative to half the
    /// view's width, so the drone is centered in the part of the scene the panel leaves free.
    pub fn view(
        &mut self,
        race: &Race,
        course: &Course,
        cutting: bool,
        shift: f32,
        aspect: f32,
        dt: f32,
    ) -> View {
        let previous = self.shot;
        if cutting {
            self.time += dt;
            let (shot, length) = SHOTS[self.shot];
            let through_gate = shot == Shot::Gate
                && self.gate.is_some_and(|index| {
                    let gate = &course.gates[index];
                    (race.position - gate.center).dot(gate.forward) > 10.0
                });
            if self.time > length || through_gate {
                self.shot = (self.shot + 1) % SHOTS.len();
                self.time = 0.0;
                self.gate = None;
            }
        } else {
            self.shot = 0;
            self.time = 0.0;
        }
        let mut cut = self.shot != previous || race.phase == Phase::Ready;

        let drone = race.position;
        let ahead = forward(race.yaw);
        // The eye, where it looks, the field of view, and how fast the eye follows.
        let (eye, look, fov_degrees, rate) = match SHOTS[self.shot].0 {
            Shot::Chase => (
                drone - ahead * 4.6 + Vec3::Y * 1.5,
                drone + ahead * 5.0,
                62.0 + race.speed * 0.4,
                7.0,
            ),
            Shot::Gate => {
                let index = match self.gate {
                    Some(index) => index,
                    None => {
                        // The next gate, or the one after when the drone is about to pass it.
                        let next = race.next_gate;
                        let index = if course.gates[next].center.distance(drone) < 15.0 {
                            (next + 1) % course.gates.len()
                        } else {
                            next
                        };
                        self.gate = Some(index);
                        self.side = if self.side < 0.0 { 1.0 } else { -1.0 };
                        index
                    }
                };
                let gate = &course.gates[index];
                let eye = gate.center + gate.forward * 7.0 + gate.right * self.side * 2.2;
                // The eye stays put until the next shot, which cuts to it.
                (eye + Vec3::Y * 0.7, drone, 46.0, 4.0)
            }
            Shot::Trackside => {
                let corner = Vec3::new(drone.x.signum(), 1.0, drone.z.signum());
                cut |= corner != self.corner;
                self.corner = corner;
                let eye =
                    Vec3::new(corner.x * (HALL_X - 3.0), 10.5, corner.z * (HALL_Z - 3.0)) * 0.92;
                // Keeps the drone about the same size in the frame.
                let fov = (2.0 * (3.5 / eye.distance(drone)).atan()).to_degrees().clamp(6.0, 40.0);
                (eye, drone, fov, 1.5)
            }
            Shot::Overhead => {
                (drone + Vec3::Y * 16.0 - ahead * 5.0, drone + ahead * 4.0, 50.0, 4.0)
            }
        };
        let eye = Vec3::new(
            eye.x.clamp(-HALL_X + 1.0, HALL_X - 1.0),
            eye.y.clamp(0.4, CEILING),
            eye.z.clamp(-HALL_Z + 1.0, HALL_Z - 1.0),
        );
        let eye = match self.eye {
            Some(previous) if !cut => previous.lerp(eye, damp(rate, dt)),
            _ => eye,
        };
        self.eye = Some(eye);

        let rotation = looking_at(eye, look);
        let half_width = eye.distance(look) * (fov_degrees.to_radians() / 2.0).tan() * aspect;
        View { eye: eye + rotation * Vec3::X * half_width * shift, rotation, fov_degrees }
    }
}
