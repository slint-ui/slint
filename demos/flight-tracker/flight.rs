// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore slerp

//! A flight along the great circle between two airports, sped up for the demo.

use glam::Vec3;

use crate::camera::{north_east, slerp};

pub const EARTH_RADIUS_KM: f32 = 6371.0;
const CRUISE_KMH: f32 = 880.0;
/// The ground speed right after takeoff and before landing.
const SLOW_KMH: f32 = 300.0;
const CLIMB_KM: f32 = 220.0;
const DESCENT_KM: f32 = 280.0;
/// The part of a flight's demo time spent speeding up after takeoff, and slowing down
/// before landing.
const RAMP: f32 = 0.08;

pub struct Flight {
    pub origin: usize,
    pub destination: usize,
    from: Vec3,
    to: Vec3,
    pub distance_km: f32,
    /// Seconds the demo takes for the flight.
    duration: f32,
    time: f32,
}

impl Flight {
    pub fn new(origin: usize, from: Vec3, destination: usize, to: Vec3) -> Self {
        let distance_km = from.angle_between(to) * EARTH_RADIUS_KM;
        let duration = (18.0 + distance_km / 250.0).clamp(25.0, 75.0);
        Self { origin, destination, from, to, distance_km, duration, time: 0.0 }
    }

    pub fn advance(&mut self, dt: f32) {
        self.time = (self.time + dt).min(self.duration);
    }

    pub fn landed(&self) -> bool {
        self.time >= self.duration
    }

    /// The flown part of the distance, from 0 to 1. The aircraft speeds up from the start and
    /// slows down at the end, at a constant rate.
    pub fn progress(&self) -> f32 {
        let s = self.time / self.duration;
        let top = 1.0 / (1.0 - RAMP);
        if s < RAMP {
            top * s * s / (2.0 * RAMP)
        } else if s > 1.0 - RAMP {
            1.0 - top * (1.0 - s) * (1.0 - s) / (2.0 * RAMP)
        } else {
            top * (s - RAMP / 2.0)
        }
    }

    /// The point on the route a fraction `t` of the way.
    pub fn point(&self, t: f32) -> Vec3 {
        slerp(self.from, self.to, t)
    }

    pub fn position(&self) -> Vec3 {
        self.point(self.progress())
    }

    pub fn flown_km(&self) -> f32 {
        self.progress() * self.distance_km
    }

    pub fn remaining_km(&self) -> f32 {
        self.distance_km - self.flown_km()
    }

    /// How far into the climb and how far from the descent the aircraft is, from 0 on the
    /// ground to 1 at cruise.
    fn cruising(&self) -> f32 {
        let smooth = |x: f32| {
            let x = x.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        smooth(self.flown_km() / CLIMB_KM) * smooth(self.remaining_km() / DESCENT_KM)
    }

    pub fn altitude_m(&self) -> f32 {
        let cruise = (self.distance_km * 15.0).clamp(7000.0, 11600.0);
        cruise * self.cruising()
    }

    pub fn ground_speed_kmh(&self) -> f32 {
        if self.landed() || self.time == 0.0 {
            return 0.0;
        }
        SLOW_KMH + (CRUISE_KMH - SLOW_KMH) * self.cruising()
    }

    /// The temperature of the standard atmosphere at the aircraft's altitude.
    pub fn outside_celsius(&self) -> f32 {
        (15.0 - 6.5 * self.altitude_m() / 1000.0).max(-56.5)
    }

    /// The hours a real flight would still take.
    pub fn remaining_hours(&self) -> f32 {
        if self.landed() {
            return 0.0;
        }
        self.remaining_km() / CRUISE_KMH + 0.3 * (self.remaining_km() / DESCENT_KM).min(1.0)
    }

    /// The direction the aircraft flies in, along the ground.
    pub fn track(&self) -> Vec3 {
        let position = self.position();
        let axis = self.from.cross(self.to).try_normalize();
        axis.map_or_else(|| north_east(position).0, |axis| axis.cross(position).normalize())
    }

    /// The compass heading, in degrees clockwise from north.
    pub fn heading_degrees(&self) -> f32 {
        let (north, east) = north_east(self.position());
        let track = self.track();
        track.dot(east).atan2(track.dot(north)).to_degrees().rem_euclid(360.0)
    }
}
