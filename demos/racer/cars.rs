// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! The arcade car model, shared by the player's car and the rivals, and the driver that
//! steers the rivals and the autopilot.

use glam::{Vec2, Vec3};

use crate::track::{HALF_WIDTH, PAD_HALF, Track};
use crate::{damp, forward, wrap_angle, yaw_towards};

/// Meters per second: 130 km/h flat out, 180 km/h on a boost.
pub const TOP_SPEED: f32 = 36.0;
pub const BOOST_SPEED: f32 = 50.0;
/// Seconds of boost a pad gives.
pub const BOOST_TIME: f32 = 1.8;
const ACCELERATION: f32 = 12.0;
const BOOST_ACCELERATION: f32 = 30.0;
const BRAKING: f32 = 30.0;
const REVERSE_SPEED: f32 = 8.0;
/// Radians per second at full lock, at speed.
const TURN_RATE: f32 = 1.9;
pub const HALF_LENGTH: f32 = 2.2;
pub const HALF_WIDTH_CAR: f32 = 0.95;
pub const WHEEL_RADIUS: f32 = 0.36;
/// How far the barriers' faces are from the center line.
pub const WALL: f32 = HALF_WIDTH + 0.2;
/// How far apart cars keep their centers when they touch.
const CONTACT: f32 = 2.5;

/// The pedals and the steering wheel, from -1 to 1. A negative throttle brakes.
#[derive(Clone, Copy, Default)]
pub struct Controls {
    pub throttle: f32,
    /// Positive to the right.
    pub steer: f32,
}

#[derive(Clone)]
pub struct Car {
    pub position: Vec3,
    pub yaw: f32,
    pub speed: f32,
    /// The steering wheel, eased towards the controls.
    pub steer: f32,
    /// Seconds of boost left.
    pub boost: f32,
    /// Scales the top speed.
    pub power: f32,
    /// The track sample nearest to the car.
    pub index: usize,
    /// How far along the track the car is at the nearest sample, counting every lap.
    pub distance: f32,
    /// How far the car is past the nearest sample, along the track.
    pub past: f32,
    /// To the right of the center line, in meters.
    pub lateral: f32,
    pub braking: bool,
    pub wheel_angle: f32,
    /// The pad the car is on, so it boosts once per pad.
    pad: Option<usize>,
}

impl Car {
    /// A car standing `distance` along the track, `lateral` meters right of the center line.
    pub fn new(track: &Track, distance: f32, lateral: f32, power: f32) -> Self {
        let (point, tangent, right) = track.frame_at(distance);
        let at_sample = (distance / track.spacing()).round() * track.spacing();
        Self {
            position: point + right * lateral,
            yaw: yaw_towards(tangent),
            speed: 0.0,
            steer: 0.0,
            boost: 0.0,
            power,
            index: track.index_at(distance),
            distance: at_sample,
            past: distance - at_sample,
            lateral,
            braking: false,
            wheel_angle: 0.0,
            pad: None,
        }
    }

    /// How far along the track the car is, counting every lap.
    pub fn progress(&self) -> f32 {
        self.distance + self.past
    }

    pub fn forward(&self) -> Vec3 {
        forward(self.yaw)
    }

    /// Drives `dt` seconds with `controls`. Returns whether the car drove onto a boost pad.
    pub fn drive(&mut self, track: &Track, controls: Controls, dt: f32) -> bool {
        self.boost = (self.boost - dt).max(0.0);
        let boosting = self.boost > 0.0;
        let top = if boosting { BOOST_SPEED } else { TOP_SPEED * self.power };
        let throttle = controls.throttle.clamp(-1.0, 1.0);
        self.braking = throttle < -0.05 && self.speed > 0.5;
        let acceleration = if throttle < 0.0 && self.speed > 0.0 {
            BRAKING * throttle
        } else if throttle < 0.0 {
            // Reverses slowly once stopped.
            if self.speed > -REVERSE_SPEED { 6.0 * throttle } else { 0.0 }
        } else if self.speed > top {
            // Eases back after a boost.
            -7.0
        } else if boosting {
            BOOST_ACCELERATION
        } else {
            // Rolls out without throttle, and pulls less towards the top speed.
            ACCELERATION * throttle * (1.0 - 0.6 * self.speed / top) - 2.0 * (1.0 - throttle)
        };
        let speed = self.speed + acceleration * dt;
        // Braking stops the car instead of reversing it.
        self.speed = if self.speed > 0.0 && throttle < 0.0 { speed.max(0.0) } else { speed };

        self.steer += (controls.steer.clamp(-1.0, 1.0) - self.steer) * damp(10.0, dt);
        // Steering needs the car to roll, and is a little calmer at high speed.
        let grip =
            (self.speed / 8.0).clamp(-1.0, 1.0) * (1.0 - 0.3 * self.speed.abs() / BOOST_SPEED);
        self.yaw -= self.steer * TURN_RATE * grip * dt;
        self.position += forward(self.yaw) * self.speed * dt;
        self.wheel_angle += self.speed / WHEEL_RADIUS * dt;

        self.follow(track);
        self.keep_off_barriers(track, dt);
        self.check_pads(track)
    }

    /// Finds the nearest sample, and how far along the track the car got.
    fn follow(&mut self, track: &Track) {
        let index = track.nearest(self.position, self.index);
        let step = Track::steps(self.index, index);
        self.distance += step as f32 * track.spacing();
        self.index = index;
        let half = track.spacing() / 2.0;
        let offset = self.position - track.points[index];
        self.past = offset.dot(track.tangents[index]).clamp(-half, half);
        self.lateral = offset.dot(track.rights[index]);
        // On the road's surface, which rises on the ramps to a bridge.
        self.position.y = track.points[index].y + track.tangents[index].y * self.past;
    }

    /// Pushes the car back from a barrier, turns it along the barrier, and takes away speed
    /// by how head-on it hit.
    fn keep_off_barriers(&mut self, track: &Track, dt: f32) {
        let limit = WALL - HALF_WIDTH_CAR;
        if self.lateral.abs() <= limit {
            return;
        }
        let right = track.rights[self.index];
        let side = self.lateral.signum();
        self.position -= right * (self.lateral - side * limit);
        self.lateral = side * limit;
        let into = self.forward().dot(right) * side * self.speed.signum();
        if into > 0.0 {
            self.speed *= 1.0 - (into * 1.2).min(0.7);
            let along = yaw_towards(track.tangents[self.index] * self.speed.signum());
            self.yaw += wrap_angle(along - self.yaw) * 0.7;
        }
        // Scraping along the barrier.
        self.speed -= self.speed.signum() * 5.0 * dt;
    }

    fn check_pads(&mut self, track: &Track) -> bool {
        let on = track.pads.iter().position(|pad| {
            let along = Track::steps(pad.index, self.index) as f32 * track.spacing() + self.past;
            along.abs() < PAD_HALF.x + HALF_LENGTH * 0.5
                && (self.lateral - pad.offset).abs() < PAD_HALF.y + HALF_WIDTH_CAR * 0.5
        });
        let boosted = on.is_some() && on != self.pad && self.speed > 0.0;
        if boosted {
            self.boost = BOOST_TIME;
        }
        self.pad = on;
        boosted
    }
}

/// Whether `a` and `b` can touch, rather than one being on a bridge and the other under it.
fn same_level(a: &Car, b: &Car) -> bool {
    (a.position.y - b.position.y).abs() < 2.0
}

/// Pushes touching cars apart, and slows the one that ran into the other.
pub fn separate(cars: &mut [Car]) {
    for i in 0..cars.len() {
        for j in i + 1..cars.len() {
            if !same_level(&cars[i], &cars[j]) {
                continue;
            }
            let delta = (cars[j].position - cars[i].position).with_y(0.0);
            // Cars are longer than wide, so they touch sooner end to end.
            let along = delta.normalize_or_zero().dot(cars[i].forward()).abs();
            let contact = CONTACT * (0.85 + 0.75 * along);
            let gap = delta.length();
            if gap >= contact || gap < 1e-4 {
                continue;
            }
            let normal = delta / gap;
            let push = normal * (contact - gap) / 2.0;
            cars[i].position -= push;
            cars[j].position += push;
            let closing = cars[i].forward().dot(normal) * cars[i].speed
                - cars[j].forward().dot(normal) * cars[j].speed;
            if closing > 0.0 {
                // The car behind gives some speed to the car ahead.
                let (behind, ahead) =
                    if cars[i].forward().dot(normal) > 0.0 { (i, j) } else { (j, i) };
                cars[behind].speed -= closing * 0.5;
                cars[ahead].speed += closing * 0.2;
            }
        }
    }
}

/// How a driver drives.
#[derive(Clone, Copy)]
pub struct Style {
    /// The preferred line, right of the center line, in meters.
    pub line: f32,
    /// The sideways acceleration the driver dares in turns, in m/s².
    pub grip: f32,
    /// Whether the driver goes for the boost pads ahead.
    pub pads: bool,
}

/// The controls that drive `car`, one of `cars`, along the track: it steers towards a point ahead on its
/// line, or on a boost pad ahead, and brakes for the turns ahead in time.
pub fn drive(car: &Car, track: &Track, style: Style, cars: &[Car]) -> Controls {
    let spacing = track.spacing();
    let progress = car.progress();
    let pad_ahead = track.pads.iter().find_map(|pad| {
        let ahead = Track::steps(car.index, pad.index) as f32 * spacing - car.past;
        (style.pads && (-PAD_HALF.x..80.0).contains(&ahead)).then_some(pad.offset)
    });
    let mut line = pad_ahead.unwrap_or(style.line);

    // Moves over for a slower car just ahead on the same line.
    let others = cars.iter().filter(|&other| !std::ptr::eq(other, car));
    for other in others.filter(|other| same_level(car, other)) {
        let delta = other.position - car.position;
        let ahead = delta.dot(car.forward());
        let beside = other.lateral - line;
        if (0.0..18.0).contains(&ahead) && beside.abs() < 2.4 && other.speed < car.speed + 1.0 {
            let side = if other.lateral > 0.0 { -1.0 } else { 1.0 };
            line = other.lateral + side * 3.0;
        }
    }
    let limit = WALL - HALF_WIDTH_CAR - 0.6;
    let line = line.clamp(-limit, limit);

    let look = 5.0 + car.speed.abs() * 0.4;
    let (point, _, right) = track.frame_at(progress + look);
    let target = point + right * line;
    let error = wrap_angle(yaw_towards(target - car.position) - car.yaw);
    let steer = (-error * 3.0).clamp(-1.0, 1.0);

    // The fastest speed from which the car can still slow down for every turn ahead.
    let mut wanted = BOOST_SPEED;
    for step in 0..=20 {
        let ahead = step as f32 * 5.0;
        let index = Track::wrap(car.index as isize + (ahead / spacing) as isize);
        let curvature = track.curvature[index].abs().max(1e-4);
        let corner = (style.grip / curvature).sqrt();
        wanted = wanted.min((corner * corner + 2.0 * 20.0 * ahead).sqrt());
    }
    let throttle =
        if car.speed > wanted + 1.0 { -((car.speed - wanted) / 6.0).min(1.0) } else { 1.0 };
    Controls { throttle, steer }
}

/// A rival's wish to change its line now and then, slowly, so the field doesn't drive in
/// single file.
pub fn wander(style: Style, time: f32, phase: f32) -> Style {
    let wave = (time * 0.13 + phase).sin() * 1.6 + (time * 0.31 + phase * 2.0).sin() * 0.8;
    Style { line: style.line + wave, ..style }
}

/// The grid behind the start line: two columns, staggered.
pub fn grid_slot(slot: usize) -> Vec2 {
    let side = if slot.is_multiple_of(2) { -2.6 } else { 2.6 };
    // Along the track (negative behind the line), and to the right.
    Vec2::new(-10.0 - slot as f32 * 7.0, side)
}
