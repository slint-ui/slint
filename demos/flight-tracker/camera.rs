// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore slerp

//! The camera around the globe, which has a radius of 1, and its transitions between views.

use std::f32::consts::PI;

use glam::{Mat4, Vec2, Vec3};

pub const FOV_Y: f32 = 30.0 * PI / 180.0;
/// The closest the camera gets to the ground, where the imagery, 4096 texels wide, still looks
/// sharp.
pub const MIN_RANGE: f32 = 0.27;
const MAX_RANGE: f32 = 6.0;

/// The point on the unit sphere at `lon` and `lat`, in radians.
pub fn surface(lon: f32, lat: f32) -> Vec3 {
    Vec3::new(lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos())
}

/// The directions north and east along the ground at `point`.
pub fn north_east(point: Vec3) -> (Vec3, Vec3) {
    let north = (Vec3::Y - point * point.y).try_normalize().unwrap_or(Vec3::NEG_Z);
    (north, north.cross(point))
}

/// The direction a fraction `t` of the way from `a` to `b` on the great circle through both.
pub fn slerp(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    let angle = a.angle_between(b);
    if angle < 1e-4 {
        return a.lerp(b, t).normalize();
    }
    // Opposite points have no single great circle; take the one over the pole.
    if angle > PI - 1e-3 {
        let pole = if a.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
        let middle = (pole - a * a.dot(pole)).normalize();
        return if t < 0.5 { slerp(a, middle, 2.0 * t) } else { slerp(middle, b, 2.0 * t - 1.0) };
    }
    (a * ((1.0 - t) * angle).sin() + b * (t * angle).sin()) / angle.sin()
}

pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

/// Where the camera is: looking at `target` on the ground from `range` earth radii away.
#[derive(Clone, Copy)]
pub struct Pose {
    pub target: Vec3,
    /// The compass direction that points up on the screen, in radians clockwise from north.
    pub heading: f32,
    /// 0 looks straight down, larger angles look towards the horizon.
    pub tilt: f32,
    pub range: f32,
}

impl Pose {
    pub fn top_down(target: Vec3, range: f32) -> Self {
        Self { target, heading: 0.0, tilt: 0.0, range }
    }

    /// The direction of `heading` along the ground, and the one to its right.
    fn ground_axes(&self) -> (Vec3, Vec3) {
        let (north, east) = north_east(self.target);
        let (sin, cos) = self.heading.sin_cos();
        (north * cos + east * sin, east * cos - north * sin)
    }

    pub fn eye(&self) -> Vec3 {
        let (forward, _) = self.ground_axes();
        let (sin, cos) = self.tilt.sin_cos();
        self.target + (self.target * cos - forward * sin) * self.range
    }

    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        let (forward, _) = self.ground_axes();
        let (sin, cos) = self.tilt.sin_cos();
        let up = forward * cos + self.target * sin;
        let eye = self.eye();
        let height = (eye.length() - 1.0).max(1e-3);
        let projection = Mat4::perspective_rh(FOV_Y, aspect, height * 0.5, eye.length() + 1.0);
        projection * Mat4::look_at_rh(eye, self.target, up)
    }

    fn lerp(&self, other: &Pose, t: f32) -> Pose {
        let turn = (other.heading - self.heading + PI).rem_euclid(2.0 * PI) - PI;
        Pose {
            target: slerp(self.target, other.target, t),
            heading: self.heading + turn * t,
            tilt: self.tilt + (other.tilt - self.tilt) * t,
            range: (self.range.ln() + (other.range.ln() - self.range.ln()) * t).exp(),
        }
    }
}

struct Transition {
    from: Pose,
    time: f32,
    duration: f32,
}

pub struct Camera {
    pub pose: Pose,
    transition: Option<Transition>,
    /// Logical pixels per second that the ground keeps moving after a drag.
    velocity: Vec2,
}

impl Camera {
    pub fn new(pose: Pose) -> Self {
        Self { pose, transition: None, velocity: Vec2::ZERO }
    }

    /// Makes the next poses blend in from the current one over `duration` seconds.
    pub fn transition(&mut self, duration: f32) {
        self.velocity = Vec2::ZERO;
        self.transition = Some(Transition { from: self.pose, time: 0.0, duration });
    }

    pub fn is_moving(&self) -> bool {
        self.transition.is_some() || self.velocity.length() > 1.0
    }

    /// Moves to `goal`, or towards it while a transition runs.
    pub fn follow(&mut self, goal: Pose, dt: f32) {
        self.pose = match &mut self.transition {
            Some(transition) => {
                transition.time += dt;
                let t = transition.time / transition.duration;
                let pose = transition.from.lerp(&goal, ease(t));
                if t >= 1.0 {
                    self.transition = None;
                }
                pose
            }
            None => goal,
        };
    }

    /// Moves the ground under the camera by `delta` logical pixels, for a view `height`
    /// logical pixels high.
    fn pan(&mut self, delta: Vec2, height: f32) {
        let focal = height / 2.0 / (FOV_Y / 2.0).tan();
        let angle = self.pose.range / focal;
        let (forward, right) = self.pose.ground_axes();
        let step = (-right * delta.x + forward * delta.y) * angle;
        self.pose.target = (self.pose.target + step).normalize();
    }

    /// Follows a drag by `delta` logical pixels, `dt` seconds after the previous one.
    pub fn drag(&mut self, delta: Vec2, height: f32, dt: f32) {
        self.transition = None;
        self.pan(delta, height);
        if dt > 0.0 {
            self.velocity = self.velocity.lerp(delta / dt, 0.5);
        }
    }

    /// Stops the movement from earlier drags.
    pub fn hold(&mut self) {
        self.velocity = Vec2::ZERO;
    }

    /// Keeps the ground moving after a drag.
    pub fn coast(&mut self, height: f32, dt: f32) {
        self.pan(self.velocity * dt, height);
        self.velocity *= (-3.0 * dt).exp();
    }

    /// The range after zooming in by `factor` from `range`.
    pub fn zoomed(range: f32, factor: f32) -> f32 {
        (range / factor).clamp(MIN_RANGE, MAX_RANGE)
    }
}
