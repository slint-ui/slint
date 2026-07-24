// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore signum underdamped

#[cfg(test)]
use crate::animations::simulations::assert_approx_eq;

#[cfg(not(feature = "std"))]
use num_traits::Float;

/// How close to the target position/velocity a `SpringSimulation` must get before it is
/// considered settled and snaps to rest. This is the "settling duration" and is distinct from the
/// user-facing `duration` that fixes the natural frequency
pub(crate) const SPRING_SETTLE_POSITION_EPSILON: f32 = 0.001;
pub(crate) const SPRING_SETTLE_VELOCITY_EPSILON: f32 = 0.05;

/// Converts a springs configuration into the `(natural_frequency, damping_ratio)` pair
/// that the `SpringSimulation` solves the ODE with.
pub trait SpringParameters {
    /// Returns `(w_n, zeta)`
    fn to_natural_frequency_and_damping_ratio(&self) -> (f32, f32);
}

/// `duration` decides the natural frequency and bounce decides the damping
#[derive(Debug, Clone, Copy)]
pub struct SpringDurationBounceParameters {
    /// Fixes the spring's natural frequency, independent of `bounce`.
    pub duration_secs: f32,
    /// Expected range `-1.0..=1.0`, but not clamped here.
    pub bounce: f32,
}

impl SpringDurationBounceParameters {
    /// Creates new `duration`/`bounce`-style spring parameters.
    pub fn new(duration_secs: f32, bounce: f32) -> Self {
        Self { duration_secs, bounce }
    }
}

impl SpringParameters for SpringDurationBounceParameters {
    fn to_natural_frequency_and_damping_ratio(&self) -> (f32, f32) {
        debug_assert!(self.duration_secs > 0., "duration must be greater than zero");
        let w_n = 2. * core::f32::consts::PI / self.duration_secs;
        let zeta = 1. - self.bounce;
        (w_n, zeta)
    }
}

/// `mass`/`stiffness`/`damping`-style spring configuration
#[derive(Debug, Clone, Copy)]
pub struct SpringPhysicalParameters {
    /// The mass attached to the spring
    pub mass: f32,
    /// The spring's stiffness (spring constant)
    pub stiffness: f32,
    /// The spring's damping coefficient
    pub damping: f32,
}

impl SpringPhysicalParameters {
    /// Creates new `mass`/`stiffness`/`damping`-style spring parameters.
    pub fn new(mass: f32, stiffness: f32, damping: f32) -> Self {
        Self { mass, stiffness, damping }
    }
}

impl SpringParameters for SpringPhysicalParameters {
    fn to_natural_frequency_and_damping_ratio(&self) -> (f32, f32) {
        debug_assert!(self.mass > 0., "mass must be greater than zero");
        debug_assert!(self.stiffness >= 0., "stiffness must not be negative");
        let w_n = f32::sqrt(self.stiffness / self.mass);
        let critical_damping = 2. * f32::sqrt(self.mass * self.stiffness);
        let zeta = if critical_damping > 0. { self.damping / critical_damping } else { 0. };
        (w_n, zeta)
    }
}

/// Precomputed coefficients for a spring, one variant per damping regime
/// All are relative to `target` (`x_rel = x - target`)
/// `x_rel(0) = start_value - target`
/// `vel(0) = initial_velocity`
#[derive(Debug, Clone, Copy)]
pub enum SpringRegime {
    /// `zeta < 1`: oscillates while decaying. `x_rel(t) = e^(-zeta*w_n*t) * (c1*cos(w_d*t) + c2*sin(w_d*t))`
    Underdamped { w_n: f32, zeta: f32, w_d: f32, c1: f32, c2: f32 },
    /// `zeta == 1`: fastest non-oscillating approach. `x_rel(t) = (c1 + c2*t) * e^(-w_n*t)`
    Critical { w_n: f32, c1: f32, c2: f32 },
    /// `zeta > 1`: slow, non-oscillating approach. `x_rel(t) = c1*e^(r1*t) + c2*e^(r2*t)`
    Overdamped { r1: f32, r2: f32, c1: f32, c2: f32 },
}

impl SpringRegime {
    /// `zeta` values within this distance of `1.0` are treated as critically damped, to avoid
    /// `w_d` (underdamped) or `sqrt(zeta^2 - 1)` (overdamped) blowing up near the boundary.
    const CRITICAL_ZETA_EPSILON: f32 = 1e-3;

    pub(crate) fn new(x0: f32, v0: f32, w_n: f32, zeta: f32) -> Self {
        if (zeta - 1.).abs() < Self::CRITICAL_ZETA_EPSILON {
            Self::Critical { w_n, c1: x0, c2: v0 + w_n * x0 }
        } else if zeta < 1. {
            let w_d = w_n * f32::sqrt(1. - zeta * zeta);
            Self::Underdamped { w_n, zeta, w_d, c1: x0, c2: (v0 + zeta * w_n * x0) / w_d }
        } else {
            let disc = f32::sqrt(zeta * zeta - 1.);
            let r1 = w_n * (-zeta + disc);
            let r2 = w_n * (-zeta - disc);
            let c1 = (v0 - r2 * x0) / (r1 - r2);
            Self::Overdamped { r1, r2, c1, c2: x0 - c1 }
        }
    }

    /// Evaluates the closed form at elapsed time `t`, returning `(x_rel, vel)`.
    pub(crate) fn evaluate(&self, t: f32) -> (f32, f32) {
        match *self {
            Self::Underdamped { w_n, zeta, w_d, c1, c2 } => {
                let decay = f32::exp(-zeta * w_n * t);
                let (s, c) = f32::sin_cos(w_d * t);
                let pos = decay * (c1 * c + c2 * s);
                let vel =
                    decay * ((-zeta * w_n * c1 + w_d * c2) * c + (-zeta * w_n * c2 - w_d * c1) * s);
                (pos, vel)
            }
            Self::Critical { w_n, c1, c2 } => {
                let decay = f32::exp(-w_n * t);
                let pos = decay * (c1 + c2 * t);
                let vel = decay * (c2 - w_n * (c1 + c2 * t));
                (pos, vel)
            }
            Self::Overdamped { r1, r2, c1, c2 } => {
                let pos = c1 * f32::exp(r1 * t) + c2 * f32::exp(r2 * t);
                let vel = c1 * r1 * f32::exp(r1 * t) + c2 * r2 * f32::exp(r2 * t);
                (pos, vel)
            }
        }
    }
}

#[cfg(test)]
mod spring_regime_tests {
    use super::*;

    const W_N: f32 = 10.;
    const X0: f32 = 5.;
    const V0: f32 = -3.;

    #[test]
    fn regime_matches_initial_conditions() {
        let regime = SpringRegime::new(X0, V0, W_N, 0.3);
        let (pos, vel) = regime.evaluate(0.);
        assert_approx_eq!(pos, X0);
        assert_approx_eq!(vel, V0);

        let regime = SpringRegime::new(X0, V0, W_N, 1.);
        let (pos, vel) = regime.evaluate(0.);
        assert_approx_eq!(pos, X0);
        assert_approx_eq!(vel, V0);

        let regime = SpringRegime::new(X0, V0, W_N, 1.8);
        let (pos, vel) = regime.evaluate(0.);
        assert_approx_eq!(pos, X0);
        assert_approx_eq!(vel, V0);
    }

    #[test]
    fn regime_decays_to_rest_over_time() {
        let regime = SpringRegime::new(X0, V0, W_N, 0.3);
        let (pos, vel) = regime.evaluate(10.);
        assert_approx_eq!(pos, 0.);
        assert_approx_eq!(vel, 0.);

        let regime = SpringRegime::new(X0, V0, W_N, 1.);
        let (pos, vel) = regime.evaluate(10.);
        assert_approx_eq!(pos, 0.);
        assert_approx_eq!(vel, 0.);

        let regime = SpringRegime::new(X0, V0, W_N, 1.8);
        let (pos, vel) = regime.evaluate(10.);
        assert_approx_eq!(pos, 0.);
        assert_approx_eq!(vel, 0.);
    }

    #[test]
    fn undamped_oscillates_without_decay() {
        // zeta == 0: pure oscillation, amplitude must be preserved over a full period
        let regime = SpringRegime::new(X0, 0., W_N, 0.);
        let period = 2. * core::f32::consts::PI / W_N;
        let (pos, vel) = regime.evaluate(period);
        assert_approx_eq!(pos, X0);
        assert_approx_eq!(vel, 0.);

        // Quarter period: position crosses zero, velocity is at its (negative) extreme
        let (pos, vel) = regime.evaluate(period / 4.);
        assert_approx_eq!(pos, 0.);
        assert_approx_eq!(vel, -X0 * W_N);
    }
}
