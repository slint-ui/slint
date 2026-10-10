// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore trackside

//! The chase and the bumper camera, and while the autopilot races, a broadcast director
//! that cuts between the chase camera, trackside cameras, and a helicopter.

use glam::{Quat, Vec3, Vec3Swizzles};

use crate::cars::{BOOST_SPEED, Car};
use crate::track::{
    EMBANKMENT_SLOPE, EMBANKMENT_TOP, HALF_WIDTH, HILL_REACH, SAMPLES, TUNNEL_HEIGHT, Track,
};
use crate::{damp, looking_at};

#[derive(Clone, Copy, PartialEq)]
enum Shot {
    Chase,
    /// Beside the track ahead of the car, with a long lens.
    Trackside,
    Helicopter,
}

/// The order of the shots, and how many seconds each one runs at most.
const SHOTS: [(Shot, f32); 4] =
    [(Shot::Chase, 10.0), (Shot::Trackside, 9.0), (Shot::Chase, 8.0), (Shot::Helicopter, 8.0)];

pub struct View {
    pub eye: Vec3,
    pub rotation: Quat,
    pub fov_degrees: f32,
}

#[derive(Default)]
pub struct Director {
    shot: usize,
    time: f32,
    /// Where the trackside camera stands, and on which side of the track the next one does.
    trackside: Option<Vec3>,
    side: f32,
    eye: Option<Vec3>,
}

impl Director {
    /// Makes the next view start fresh instead of moving there.
    pub fn cut(&mut self) {
        self.eye = None;
    }

    /// The view from the bumper.
    pub fn bumper(&mut self, car: &Car) -> View {
        self.cut();
        let ahead = car.forward();
        let rotation = Quat::from_rotation_y(car.yaw) * Quat::from_rotation_x(-0.03);
        let fov = 70.0 + car.speed.max(0.0) / BOOST_SPEED * 12.0;
        View { eye: car.position + ahead * 0.6 + Vec3::Y * 1.05, rotation, fov_degrees: fov }
    }

    /// The camera that follows `car`. With `cutting`, the director cycles through the shots;
    /// otherwise it stays with the chase camera.
    pub fn view(&mut self, car: &Car, track: &Track, cutting: bool, dt: f32) -> View {
        let previous = self.shot;
        if cutting {
            self.time += dt;
            let (shot, length) = SHOTS[self.shot];
            let passed = shot == Shot::Trackside
                && self
                    .trackside
                    .is_some_and(|eye| (eye - car.position).dot(car.forward()) < -25.0);
            if self.time > length || passed {
                self.shot = (self.shot + 1) % SHOTS.len();
                self.time = 0.0;
                self.trackside = None;
            }
        } else {
            self.shot = 0;
            self.time = 0.0;
        }
        let cut = self.shot != previous;

        let position = car.position;
        let ahead = car.forward();
        let speed = car.speed.max(0.0);
        // The eye, where it looks, the field of view, and how fast the eye follows.
        let (eye, look, fov, rate) = match SHOTS[self.shot].0 {
            Shot::Chase => {
                let eye = position - ahead * 7.5 + Vec3::Y * 2.7;
                let fov = 58.0 + speed / BOOST_SPEED * 22.0;
                (self.inside(track, car, eye), position + ahead * 8.0 + Vec3::Y * 0.6, fov, 8.0)
            }
            Shot::Trackside => {
                let eye = *self.trackside.get_or_insert_with(|| {
                    self.side = if self.side < 0.0 { 1.0 } else { -1.0 };
                    // The next place about 80 m ahead where the camera stands in the open.
                    let place = |distance: f32| {
                        let (point, _, right) = track.frame_at(distance);
                        point + right * self.side * (HALF_WIDTH + 4.0) + Vec3::Y * 3.2
                    };
                    let mut distance = car.progress() + 80.0;
                    for _ in 0..30 {
                        if in_the_open(track, place(distance)) {
                            break;
                        }
                        distance += 15.0;
                    }
                    place(distance)
                });
                // Keeps the car about the same size in the frame.
                let fov = (2.0 * (5.0 / eye.distance(position)).atan()).to_degrees();
                (eye, position + Vec3::Y * 0.6, fov.clamp(10.0, 55.0), 100.0)
            }
            Shot::Helicopter => {
                let side = car.forward().cross(Vec3::Y) * 10.0;
                let eye = position - ahead * 24.0 + side + Vec3::Y * 18.0;
                (eye, position + ahead * 10.0, 45.0, 2.5)
            }
        };
        let eye = match self.eye {
            Some(previous) if !cut => previous.lerp(eye, damp(rate, dt)),
            _ => eye,
        };
        let eye = self.inside(track, car, eye);
        self.eye = Some(eye);
        View { eye, rotation: looking_at(eye, look), fov_degrees: fov }
    }

    /// Keeps a camera near `car` inside the tunnel's walls and below its roof.
    fn inside(&self, track: &Track, car: &Car, eye: Vec3) -> Vec3 {
        if eye.distance(car.position) > 15.0 {
            return eye;
        }
        let index = track.nearest(eye, car.index);
        // From a few meters before a portal, so the camera doesn't swing into the hill.
        let near_portal =
            (-12..=12).any(|step| track.cover[Track::wrap(index as isize + step)] > 0.0);
        if !near_portal {
            return eye;
        }
        let (point, right) = (track.points[index], track.rights[index]);
        let lateral = (eye - point).dot(right);
        let limit = HALF_WIDTH - 0.5;
        let eye = eye - right * (lateral - lateral.clamp(-limit, limit));
        if track.cover[index] == 0.0 {
            return eye;
        }
        eye.with_y(eye.y.min(point.y + TUNNEL_HEIGHT - 1.0))
    }
}

/// Whether `eye` is clear of the hills over the tunnels and of the embankment under a
/// raised road.
fn in_the_open(track: &Track, eye: Vec3) -> bool {
    let hills = track.tunnels.iter().filter(|tunnel| !tunnel.bridge).flat_map(|tunnel| {
        (0..tunnel.samples).step_by(4).map(move |k| (tunnel.start + k) % SAMPLES)
    });
    let on_hill =
        hills.into_iter().any(|i| track.points[i].xz().distance(eye.xz()) < HILL_REACH + 3.0);
    let in_embankment = (0..SAMPLES).step_by(3).any(|i| {
        let point = track.points[i];
        let reach = EMBANKMENT_TOP + EMBANKMENT_SLOPE * point.y + 3.0;
        point.y > 0.0 && eye.y < point.y + 2.0 && point.xz().distance(eye.xz()) < reach
    });
    let near_crossing = track
        .crossing
        .as_ref()
        .is_some_and(|crossing| track.points[crossing.lower].xz().distance(eye.xz()) < 40.0);
    !on_hill && !in_embankment && !near_crossing
}
