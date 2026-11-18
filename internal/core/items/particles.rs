// Copyright © 2026 Klarälvdalens Datakonsult AB, a KDAB Group company <info@kdab.com>, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains the `Particles` item and its CPU simulation.

When adding an item or a property, it needs to be kept in sync with different place.
Lookup the [`crate::items`] module documentation.
*/

use super::{
    AttractorFalloff, Item, ItemConsts, ItemRc, ItemRendererRef, ParticleAffector,
    ParticleAffectorKind, ParticleEmitter, ParticleShape, RenderingResult,
};
use crate::animations::{CURRENT_ANIMATION_DRIVER, EasingCurve, easing_curve};
use crate::graphics::{Color, Image};
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, InternalKeyEvent,
    KeyEventResult, MouseEvent,
};
use crate::item_rendering::CachedRenderingData;
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalRect, LogicalSize};
use crate::model::{Model, ModelRc};
use crate::once_box::OnceBox;
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowAdapter;
use crate::{Coord, Property};
use alloc::rc::Rc;
use alloc::vec::Vec;
use const_field_offset::FieldOffsets;
use core::cell::{Ref, RefCell, RefMut};
use core::pin::Pin;
use euclid::num::Zero;
use fastrand::Rng;
use i_slint_core_macros::*;
#[allow(unused_imports)]
use num_traits::Float;

/// Upper bound for `max-particles`, which keeps a frame of particles within the
/// software renderer's `u16` scene indices.
const MAX_PARTICLES: usize = 16384;

/// Longest time span simulated in one frame, in seconds.
/// After a stall, the effect pauses instead of jumping.
const MAX_FRAME_TIME: f32 = 0.1;

/// The simulation integrates in steps of this length, in seconds, so that the result
/// doesn't depend on the frame rate.
const SUBSTEP: f32 = 1. / 60.;

/// The distance, in logical pixels, at which an `Attractor`'s acceleration equals its `strength`.
const ATTRACTOR_REFERENCE_DISTANCE: f32 = 100.;

/// The implementation of the `Particles` element
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct Particles {
    pub source: Property<Image>,
    pub max_particles: Property<i32>,
    pub running: Property<bool>,
    pub clip: Property<bool>,
    pub start_size: Property<LogicalLength>,
    pub end_size: Property<LogicalLength>,
    pub size_variation: Property<LogicalLength>,
    pub start_color: Property<Color>,
    pub end_color: Property<Color>,
    pub color_variation: Property<f32>,
    pub start_opacity: Property<f32>,
    pub end_opacity: Property<f32>,
    pub easing: Property<EasingCurve>,
    pub rotation: Property<f32>,
    pub rotation_variation: Property<f32>,
    pub angular_velocity: Property<f32>,
    pub angular_velocity_variation: Property<f32>,
    pub align_to_velocity: Property<bool>,
    pub seed: Property<i32>,
    pub particle_count: Property<i32>,
    pub emitters: Property<ModelRc<ParticleEmitter>>,
    pub affectors: Property<ModelRc<ParticleAffector>>,
    /// Changed by `burst()` and `reset()`, so that a step reading it schedules a frame.
    generation: Property<u32>,
    pub cached_rendering_data: CachedRenderingData,
    state: OnceBox<RefCell<ParticleState>>,
}

impl Item for Particles {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn deinit(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _cross_axis_constraint: Coord,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut super::MouseCursorInner,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut super::MouseCursorInner,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn capture_key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(
        self: Pin<&Self>,
        _: &FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> FocusEventResult {
        FocusEventResult::FocusIgnored
    }

    fn render(
        self: Pin<&Self>,
        backend: &mut ItemRendererRef,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        let clip = self.clip();
        if clip {
            (*backend).save_state();
            (*backend).combine_clip(size.into(), LogicalBorderRadius::zero());
        }
        (*backend).draw_particles(self, self_rc, size);
        if clip {
            (*backend).restore_state();
        }
        RenderingResult::ContinueRenderingChildren
    }

    fn bounding_rect(
        self: core::pin::Pin<&Self>,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        geometry: LogicalRect,
    ) -> LogicalRect {
        if self.clip() {
            return geometry;
        }
        // Stepping here, before the dirty region is computed, makes the dirty region cover
        // where the particles are drawn in this frame.
        self.step();
        let state = self.state().borrow();
        match state.bounds {
            Some(bounds) => geometry.union(&bounds.translate(geometry.origin.to_vector())),
            None => geometry,
        }
    }

    fn clips_children(self: core::pin::Pin<&Self>) -> bool {
        false
    }
}

impl ItemConsts for Particles {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Particles,
        CachedRenderingData,
    > = Particles::FIELD_OFFSETS.cached_rendering_data().as_unpinned_projection();
}

/// A particle as the renderers draw it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleInstance {
    /// The center, relative to the `Particles` item.
    pub center: LogicalPoint,
    /// The length of the longer side of the image.
    pub size: LogicalLength,
    /// The clockwise rotation, in degrees.
    pub rotation: f32,
    /// When set, the image is an alpha mask in this opaque color, as with `Image::colorize`.
    /// It's set for all particles of a `Particles` item, or for none.
    pub colorize: Option<Color>,
    /// From 0 to 1.
    pub opacity: f32,
}

impl Particles {
    /// Advances the simulation to the current animation tick, and returns the live particles.
    /// The iterator borrows the simulation, so drop it before calling other functions of the item.
    pub fn instances(
        self: Pin<&Self>,
    ) -> impl ExactSizeIterator<Item = ParticleInstance> + use<'_> {
        self.step();
        let appearance = Appearance::new(self);
        let pool = Ref::map(self.state().borrow(), |state| state.pool.as_slice());
        (0..pool.len()).map(move |i| appearance.instance(&pool[i]))
    }

    /// Emits `count` particles from each enabled emitter.
    pub fn burst(self: Pin<&Self>, count: i32) {
        let count = count.max(0) as usize;
        let emitters = self.emitters();
        let max_particles = self.max_particle_count();
        let params = SpawnParams::new(self);
        let mut state = self.state().borrow_mut();
        for emitter in emitters.iter().filter(|e| e.enabled) {
            for _ in 0..count {
                if state.pool.len() >= max_particles {
                    break;
                }
                state.spawn(&emitter, &params);
            }
        }
        self.pool_changed(state);
        self.bump_generation();
    }

    /// Removes all particles.
    pub fn reset(self: Pin<&Self>) {
        let mut state = self.state().borrow_mut();
        state.pool.clear();
        state.bounds = None;
        drop(state);
        self.particle_count.set(0);
        self.bump_generation();
    }

    fn bump_generation(self: Pin<&Self>) {
        let generation = Self::FIELD_OFFSETS.generation().apply_pin(self);
        generation.set(generation.get_untracked().wrapping_add(1));
    }

    fn state(self: Pin<&Self>) -> &RefCell<ParticleState> {
        self.get_ref().state.get_or_init(|| {
            let seed = match self.seed() {
                0 => {
                    CURRENT_ANIMATION_DRIVER.with(|driver| driver.current_tick().as_nanos())
                        ^ (self.get_ref() as *const Self as u64)
                }
                seed => seed as u64,
            };
            RefCell::new(ParticleState {
                pool: Vec::new(),
                last_tick: None,
                emit_accumulator: Vec::new(),
                rng: Rng::with_seed(seed),
                bounds: None,
            })
        })
    }

    fn pool_changed(self: Pin<&Self>, mut state: RefMut<ParticleState>) {
        state.update_bounds(&Appearance::new(self));
        let count = state.pool.len() as i32;
        drop(state);
        self.particle_count.set(count);
    }

    fn max_particle_count(self: Pin<&Self>) -> usize {
        (self.max_particles().max(0) as usize).min(MAX_PARTICLES)
    }

    /// Advances the simulation from the last step to the current animation tick.
    /// Calling it again in the same tick does nothing.
    fn step(self: Pin<&Self>) {
        Self::FIELD_OFFSETS.generation().apply_pin(self).get();
        let emitters: Vec<ParticleEmitter> = self.emitters().iter().collect();
        let running = self.running();
        let has_emission = emitters.iter().any(|e| e.enabled && e.rate > 0.);

        let mut state = self.state().borrow_mut();
        let idle = state.pool.is_empty() && !has_emission;
        if !running || idle {
            // Resuming later must not simulate the time spent paused.
            state.last_tick = None;
            return;
        }

        let now = CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.set_has_active_animations();
            driver.current_tick().as_nanos()
        });
        let Some(last) = state.last_tick.replace(now) else { return };
        let dt = (now.saturating_sub(last) as f32 / 1e9).min(MAX_FRAME_TIME);
        if dt <= 0. {
            return;
        }

        let affectors: Vec<Affector> =
            self.affectors().iter().filter(|a| a.enabled).map(Affector::from).collect();
        let params = SpawnParams::new(self);
        let max_particles = self.max_particle_count();
        state.emit_accumulator.resize(emitters.len(), 0.);

        let mut remaining = dt;
        while remaining > 0. {
            let h = remaining.min(SUBSTEP);
            remaining -= h;
            state.integrate(h, &affectors);
            state.emit(h, &emitters, &params, max_particles);
        }
        self.pool_changed(state);
    }
}

#[derive(Clone, Copy, Debug)]
struct Particle {
    position: [f32; 2],
    velocity: [f32; 2],
    acceleration: [f32; 2],
    /// In seconds.
    age: f32,
    /// In seconds.
    lifetime: f32,
    size_offset: f32,
    rotation: f32,
    /// In degrees per second.
    angular_velocity: f32,
    color_offset: [f32; 3],
}

struct ParticleState {
    pool: Vec<Particle>,
    /// In nanoseconds of the animation tick.
    last_tick: Option<u64>,
    /// The fractional particles that each emitter has yet to emit.
    emit_accumulator: Vec<f32>,
    rng: Rng,
    /// The bounding rectangle of the live particles, relative to the item.
    bounds: Option<LogicalRect>,
}

impl ParticleState {
    fn emit(
        &mut self,
        h: f32,
        emitters: &[ParticleEmitter],
        params: &SpawnParams,
        max_particles: usize,
    ) {
        for (index, emitter) in emitters.iter().enumerate() {
            if !emitter.enabled || emitter.rate <= 0. {
                self.emit_accumulator[index] = 0.;
                continue;
            }
            let pending = self.emit_accumulator[index] + emitter.rate * h;
            let count = pending.floor();
            self.emit_accumulator[index] = pending - count;
            for _ in 0..count as usize {
                if self.pool.len() >= max_particles {
                    // Emission pauses instead of catching up once the pool has room again.
                    self.emit_accumulator[index] = 0.;
                    break;
                }
                self.spawn(emitter, params);
            }
        }
    }

    fn spawn(&mut self, emitter: &ParticleEmitter, params: &SpawnParams) {
        let rng = &mut self.rng;
        let position = spawn_position(emitter, rng);
        let direction = (emitter.direction + emitter.spread * symmetric(rng) / 2.).to_radians();
        let speed = emitter.speed as f32 + emitter.speed_variation as f32 * symmetric(rng);
        let acceleration_direction = emitter.acceleration_direction.to_radians();
        let acceleration = emitter.acceleration as f32;
        let lifetime =
            (emitter.lifetime + emitter.lifetime_variation * symmetric(rng)).max(0.) / 1000.;
        if lifetime <= 0. {
            return;
        }
        self.pool.push(Particle {
            position,
            velocity: [speed * direction.cos(), speed * direction.sin()],
            acceleration: [
                acceleration * acceleration_direction.cos(),
                acceleration * acceleration_direction.sin(),
            ],
            age: 0.,
            lifetime,
            size_offset: params.size_variation * symmetric(rng),
            rotation: params.rotation + params.rotation_variation * symmetric(rng),
            angular_velocity: params.angular_velocity
                + params.angular_velocity_variation * symmetric(rng),
            color_offset: [
                params.color_variation * symmetric(rng),
                params.color_variation * symmetric(rng),
                params.color_variation * symmetric(rng),
            ],
        });
    }

    /// Advances the live particles by `h` seconds with semi-implicit Euler, and removes the dead ones.
    fn integrate(&mut self, h: f32, affectors: &[Affector]) {
        let mut i = 0;
        while i < self.pool.len() {
            let p = &mut self.pool[i];
            let mut acceleration = p.acceleration;
            for affector in affectors {
                if affector.contains(p.position) {
                    affector.apply(p, &mut acceleration, h);
                }
            }
            p.velocity[0] += acceleration[0] * h;
            p.velocity[1] += acceleration[1] * h;
            p.position[0] += p.velocity[0] * h;
            p.position[1] += p.velocity[1] * h;
            p.rotation += p.angular_velocity * h;
            p.age += h;
            if p.age >= p.lifetime {
                self.pool.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn update_bounds(&mut self, appearance: &Appearance) {
        self.bounds = self
            .pool
            .iter()
            .map(|p| {
                // Half the diagonal leaves room for any rotation.
                let extent = appearance.max_size(p) * core::f32::consts::FRAC_1_SQRT_2;
                LogicalRect::new(
                    LogicalPoint::new(
                        (p.position[0] - extent) as Coord,
                        (p.position[1] - extent) as Coord,
                    ),
                    LogicalSize::new((2. * extent) as Coord, (2. * extent) as Coord),
                )
            })
            .reduce(|a, b| a.union(&b));
    }
}

/// A `ParticleAffector` with the values that the simulation uses, in seconds and radians.
struct Affector {
    /// The left, top, right, and bottom edges of the area.
    area: Option<[f32; 4]>,
    effect: Effect,
}

enum Effect {
    Gravity([f32; 2]),
    Friction { factor: f32, threshold: f32 },
    Attractor { point: [f32; 2], strength: f32, falloff: AttractorFalloff },
    Sink { lifetime_left: f32 },
}

impl From<ParticleAffector> for Affector {
    fn from(a: ParticleAffector) -> Self {
        let (x, y) = (a.x as f32, a.y as f32);
        let effect = match a.kind {
            ParticleAffectorKind::Gravity => {
                let (sin, cos) = a.direction.to_radians().sin_cos();
                let magnitude = a.magnitude as f32;
                Effect::Gravity([magnitude * cos, magnitude * sin])
            }
            ParticleAffectorKind::Friction => {
                Effect::Friction { factor: a.factor, threshold: a.threshold as f32 }
            }
            ParticleAffectorKind::Attractor => Effect::Attractor {
                point: [a.point_x as f32, a.point_y as f32],
                strength: a.strength as f32,
                falloff: a.falloff,
            },
            ParticleAffectorKind::Sink => Effect::Sink { lifetime_left: a.lifetime_left / 1000. },
        };
        Self { area: a.has_area.then_some([x, y, x + a.width as f32, y + a.height as f32]), effect }
    }
}

impl Affector {
    fn contains(&self, [x, y]: [f32; 2]) -> bool {
        self.area.is_none_or(|[left, top, right, bottom]| {
            x >= left && y >= top && x < right && y < bottom
        })
    }

    fn apply(&self, p: &mut Particle, acceleration: &mut [f32; 2], h: f32) {
        match self.effect {
            Effect::Gravity([ax, ay]) => {
                acceleration[0] += ax;
                acceleration[1] += ay;
            }
            Effect::Friction { factor, threshold } => {
                let [vx, vy] = p.velocity;
                let speed = (vx * vx + vy * vy).sqrt();
                if speed > threshold {
                    let new_speed = (speed * (1. - factor * h)).max(threshold);
                    let scale = new_speed / speed;
                    p.velocity = [vx * scale, vy * scale];
                }
            }
            Effect::Attractor { point, strength, falloff } => {
                let dx = point[0] - p.position[0];
                let dy = point[1] - p.position[1];
                let distance = (dx * dx + dy * dy).sqrt();
                if distance > 0. {
                    // Clamped, so that inverse falloffs stay finite at the point.
                    let r = (distance / ATTRACTOR_REFERENCE_DISTANCE).max(0.1);
                    let factor = match falloff {
                        AttractorFalloff::Constant => 1.,
                        AttractorFalloff::Linear => r,
                        AttractorFalloff::Quadratic => r * r,
                        AttractorFalloff::InverseLinear => 1. / r,
                        AttractorFalloff::InverseQuadratic => 1. / (r * r),
                    };
                    let magnitude = strength * factor / distance;
                    acceleration[0] += dx * magnitude;
                    acceleration[1] += dy * magnitude;
                }
            }
            Effect::Sink { lifetime_left } => {
                p.age = p.age.max(p.lifetime - lifetime_left);
            }
        }
    }
}

/// A random point in the emitter's area, relative to the `Particles` item.
fn spawn_position(emitter: &ParticleEmitter, rng: &mut Rng) -> [f32; 2] {
    let (x, y) = (emitter.x as f32, emitter.y as f32);
    let (w, h) = (emitter.width as f32, emitter.height as f32);
    match emitter.shape {
        ParticleShape::Line => {
            let t = rng.f32();
            [x + t * w, y + t * h]
        }
        ParticleShape::Ellipse => {
            let angle = rng.f32() * core::f32::consts::TAU;
            // The square root spreads the points evenly over the area.
            let radius = if emitter.filled { rng.f32().sqrt() } else { 1. };
            [x + w / 2. * (1. + radius * angle.cos()), y + h / 2. * (1. + radius * angle.sin())]
        }
        _ if emitter.filled => [x + rng.f32() * w, y + rng.f32() * h],
        _ => {
            // A point on the perimeter, with each side weighted by its length.
            let t = rng.f32() * 2. * (w + h);
            if t < w {
                [x + t, y]
            } else if t < w + h {
                [x + w, y + t - w]
            } else if t < 2. * w + h {
                [x + 2. * w + h - t, y + h]
            } else {
                [x, y + 2. * (w + h) - t]
            }
        }
    }
}

/// The properties of the item that apply to newly emitted particles.
struct SpawnParams {
    size_variation: f32,
    rotation: f32,
    rotation_variation: f32,
    angular_velocity: f32,
    angular_velocity_variation: f32,
    color_variation: f32,
}

impl SpawnParams {
    fn new(particles: Pin<&Particles>) -> Self {
        Self {
            size_variation: particles.size_variation().get() as f32,
            rotation: particles.rotation(),
            rotation_variation: particles.rotation_variation(),
            angular_velocity: particles.angular_velocity(),
            angular_velocity_variation: particles.angular_velocity_variation(),
            color_variation: particles.color_variation(),
        }
    }
}

/// The properties of the item that set how particles look over their lifetime.
struct Appearance {
    start_size: f32,
    end_size: f32,
    /// Premultiplied, or `None` when not colorizing.
    colors: Option<([f32; 4], [f32; 4])>,
    start_opacity: f32,
    end_opacity: f32,
    easing: EasingCurve,
    align_to_velocity: bool,
}

impl Appearance {
    fn new(particles: Pin<&Particles>) -> Self {
        let start_color = particles.start_color();
        let end_color = particles.end_color();
        let premultiplied = |c: Color| {
            let a = c.alpha() as f32 / 255.;
            [c.red() as f32 / 255. * a, c.green() as f32 / 255. * a, c.blue() as f32 / 255. * a, a]
        };
        Self {
            start_size: particles.start_size().get() as f32,
            end_size: particles.end_size().get() as f32,
            colors: (start_color.alpha() > 0 || end_color.alpha() > 0)
                .then(|| (premultiplied(start_color), premultiplied(end_color))),
            start_opacity: particles.start_opacity(),
            end_opacity: particles.end_opacity(),
            easing: particles.easing(),
            align_to_velocity: particles.align_to_velocity(),
        }
    }

    fn instance(&self, p: &Particle) -> ParticleInstance {
        let t = easing_curve(&self.easing, (p.age / p.lifetime).clamp(0., 1.));
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        let mut opacity = lerp(self.start_opacity, self.end_opacity).clamp(0., 1.);
        // Interpolating premultiplied fades a color to transparent without darkening it.
        let colorize = self.colors.map(|(start, end)| {
            let alpha = lerp(start[3], end[3]);
            opacity *= alpha;
            let channel = |i: usize| {
                let premultiplied = lerp(start[i], end[i]);
                let value = if alpha > 0. { premultiplied / alpha } else { 0. };
                (value + p.color_offset[i]).clamp(0., 1.)
            };
            Color::from_argb_f32(1., channel(0), channel(1), channel(2))
        });
        let mut rotation = p.rotation;
        if self.align_to_velocity {
            rotation += p.velocity[1].atan2(p.velocity[0]).to_degrees();
        }
        ParticleInstance {
            center: LogicalPoint::new(p.position[0] as Coord, p.position[1] as Coord),
            size: LogicalLength::new(
                (lerp(self.start_size, self.end_size) + p.size_offset).max(0.) as Coord,
            ),
            rotation,
            colorize,
            opacity,
        }
    }

    /// The largest size a particle can reach over its lifetime.
    fn max_size(&self, p: &Particle) -> f32 {
        (self.start_size.max(self.end_size) + p.size_offset).max(0.)
    }
}

/// A random number in `[-1, 1)`.
fn symmetric(rng: &mut Rng) -> f32 {
    rng.f32() * 2. - 1.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::Instant;
    use crate::model::VecModel;
    use alloc::boxed::Box;
    use alloc::vec;

    fn particles(
        emitters: Vec<ParticleEmitter>,
        affectors: Vec<ParticleAffector>,
    ) -> Pin<Box<Particles>> {
        let p = Box::pin(Particles::default());
        p.max_particles.set(1000);
        p.running.set(true);
        p.seed.set(1);
        p.start_size.set(LogicalLength::new(10.));
        p.end_size.set(LogicalLength::new(10.));
        p.start_opacity.set(1.);
        p.end_opacity.set(1.);
        p.emitters.set(ModelRc::new(VecModel::from(emitters)));
        p.affectors.set(ModelRc::new(VecModel::from(affectors)));
        p
    }

    fn emitter(rate: f32, lifetime_ms: f32) -> ParticleEmitter {
        ParticleEmitter { rate, lifetime: lifetime_ms, ..Default::default() }
    }

    fn step_at(p: &Pin<Box<Particles>>, ms: u64) {
        CURRENT_ANIMATION_DRIVER.with(|d| d.update_animations(Instant::from_millis(ms)));
        p.as_ref().step();
    }

    fn run(p: &Pin<Box<Particles>>, from_ms: u64, to_ms: u64) {
        for ms in (from_ms..=to_ms).step_by(16) {
            step_at(p, ms);
        }
    }

    #[test]
    fn emits_at_rate() {
        let p = particles(vec![emitter(100., 10_000.)], vec![]);
        run(&p, 0, 1000);
        assert!(
            (98..=100).contains(&p.as_ref().particle_count()),
            "{}",
            p.as_ref().particle_count()
        );
    }

    #[test]
    fn removes_dead_particles() {
        let p = particles(vec![emitter(10., 500.)], vec![]);
        run(&p, 0, 3000);
        assert!((4..=6).contains(&p.as_ref().particle_count()), "{}", p.as_ref().particle_count());
    }

    #[test]
    fn paused_freezes() {
        let p = particles(vec![emitter(100., 10_000.)], vec![]);
        run(&p, 0, 500);
        let count = p.as_ref().particle_count();
        let instances: Vec<_> = p.as_ref().instances().collect();
        p.running.set(false);
        run(&p, 500, 1000);
        assert_eq!(p.as_ref().particle_count(), count);
        assert_eq!(p.as_ref().instances().collect::<Vec<_>>(), instances);
        // Resuming doesn't simulate the paused time.
        p.running.set(true);
        run(&p, 1008, 1024);
        assert!(p.as_ref().particle_count() <= count + 2);
    }

    #[test]
    fn stall_is_clamped() {
        let p = particles(vec![emitter(100., 10_000.)], vec![]);
        step_at(&p, 0);
        step_at(&p, 5000);
        assert_eq!(p.as_ref().particle_count(), (MAX_FRAME_TIME * 100.) as i32);
    }

    #[test]
    fn gravity_accelerates() {
        let gravity = ParticleAffector {
            kind: ParticleAffectorKind::Gravity,
            enabled: true,
            direction: 90.,
            magnitude: 100.,
            ..Default::default()
        };
        let p = particles(vec![emitter(0., 10_000.)], vec![gravity]);
        p.as_ref().burst(1);
        run(&p, 0, 1000);
        let y = p.as_ref().instances().next().unwrap().center.y;
        // y = a * t² / 2 after 992ms
        assert!((y - 49.2).abs() < 1., "{y}");
        let bounds = p.as_ref().state().borrow().bounds.unwrap();
        assert!(bounds.contains(LogicalPoint::new(0., y)));
    }

    #[test]
    fn sink_kills_in_area() {
        let sink = ParticleAffector {
            kind: ParticleAffectorKind::Sink,
            enabled: true,
            has_area: true,
            x: 50.,
            width: 100.,
            height: 100.,
            ..Default::default()
        };
        let mut left = emitter(0., 10_000.);
        left.height = 10.;
        let mut right = left.clone();
        right.x = 60.;
        let p = particles(vec![left, right], vec![sink]);
        p.as_ref().burst(5);
        run(&p, 0, 100);
        assert_eq!(p.as_ref().particle_count(), 5);
        assert!(p.as_ref().instances().all(|i| i.center.x < 50.));
    }

    #[test]
    fn seed_is_reproducible() {
        let mut e = emitter(50., 1000.);
        e.width = 100.;
        e.height = 100.;
        e.speed = 20.;
        e.spread = 360.;
        let a = particles(vec![e.clone()], vec![]);
        run(&a, 0, 500);
        let a: Vec<_> = a.as_ref().instances().collect();
        let b = particles(vec![e], vec![]);
        // The same thread reuses the animation driver, so restart the clock relative to it.
        let now = CURRENT_ANIMATION_DRIVER.with(|d| d.current_tick().as_millis());
        run(&b, now, now + 500);
        assert_eq!(b.as_ref().instances().collect::<Vec<_>>(), a);
    }
}
