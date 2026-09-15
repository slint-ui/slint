//! Ported from Flutter's `ClampingScrollSimulation`
//! (scroll_simulation.dart.dart), which is:
//! Copyright 2014 The Flutter Authors. All rights reserved.
//!
//! Use of the original source is governed by a BSD-style license
//!
//! Original: <https://github.com/flutter/flutter/blob/d6bed8ff6135cdd414f14edc3063f761d47ca846/packages/flutter/lib/src/widgets/scroll_physics.dart>

use crate::animations::Instant;
use crate::animations::simulations::spring::{
    SpringParameters, SpringPhysicalParameters, SpringRegime,
};
use crate::animations::simulations::{PositionSimulation, Simulation};
#[cfg(not(feature = "std"))]
use num_traits::Float;

const DEFAULT_MASS: f32 = 0.5;
const DEFAULT_STIFFNESS: f32 = 100.;
const DEFAULT_RATIO: f32 = 1.1;

const ZERO_TOLERANCE: f32 = 1e-3;

#[derive(Debug)]
pub struct SpringSimulation {
    limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    start_time: Instant,
    traveled: f32,
    data: SpringRegime,
    init_pos: f32,
}

impl SpringSimulation {
    pub fn new_with_default_parameters(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    ) -> Self {
        let l = limit_value.as_ref().get();
        let (w_n, zeta) = SpringPhysicalParameters::new_with_damping_ratio(
            DEFAULT_MASS,
            DEFAULT_STIFFNESS,
            DEFAULT_RATIO,
        )
        .to_natural_frequency_and_damping_ratio();
        let init_pos = l - start_value;
        let spring = SpringRegime::new(init_pos, 0., w_n, zeta);

        Self {
            limit_value,
            start_time: crate::animations::current_tick(),
            traveled: 0.,
            data: spring,
            init_pos,
        }
    }

    fn step_internal(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        let t = new_tick.duration_since(self.start_time).as_secs_f32();
        let (new_pos, new_vel) = self.data.evaluate(t);
        let new_traveled = self.init_pos - new_pos;
        *current += new_traveled - self.traveled;
        self.traveled = new_traveled;

        

        new_pos.abs() < ZERO_TOLERANCE && new_vel.abs() < ZERO_TOLERANCE
    }
}

impl Simulation for SpringSimulation {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        self.step_internal(current, new_tick)
    }
}

impl PositionSimulation for SpringSimulation {
    fn remaining_distance(&self, time_elapsed: core::time::Duration) -> f32 {
        self.data.current_position(time_elapsed.as_secs_f32())
    }

    fn remaining_velocity(&self, time_elapsed: core::time::Duration) -> f32 {
        self.data.current_velocity(time_elapsed.as_secs_f32())
    }

    fn overshoot_allowed(&self) -> bool {
        false
    }
}
