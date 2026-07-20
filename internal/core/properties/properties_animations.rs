// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use crate::{
    animations::simulations::{
        Parameter, Simulation,
        spring::{
            SpringDurationBounceParameters, SpringParameters, SpringPhysicalParameters,
            SpringRegime,
        },
    },
    items::{AnimationDirection, PropertyAnimation},
    lengths::LogicalLength,
};
use euclid::Length;
#[cfg(not(feature = "std"))]
use num_traits::Float;

enum AnimationState {
    /// The animation will start after the delay is finished
    Delaying,
    /// Actual animation
    Animating {
        current_iteration: u64,
    },
    Done {
        iteration_count: u64,
    },
}

pub(super) struct PropertyPhysicsAnimationData<S> {
    simulation: S,
    state: AnimationState,
}

impl<S> PropertyPhysicsAnimationData<S>
where
    S: Simulation,
{
    pub fn new(simulation: S) -> PropertyPhysicsAnimationData<S> {
        PropertyPhysicsAnimationData { simulation, state: AnimationState::Delaying }
    }

    /// Single iteration of the animation
    pub fn update_value(&mut self, target: &mut crate::Coord) -> bool {
        match self.state {
            AnimationState::Delaying => {
                // Decide on next state:
                self.state = AnimationState::Animating { current_iteration: 0 };
                self.update_value(target)
            }
            AnimationState::Animating { current_iteration: _ } => {
                // TODO: Pass in Coord directly?
                let mut value: f32 = *target as f32;
                let finished = self.simulation.step(&mut value, crate::animations::current_tick());
                *target = value as crate::Coord;
                if finished {
                    self.state = AnimationState::Done { iteration_count: 0 };
                    true
                } else {
                    false
                }
            }
            AnimationState::Done { iteration_count: _ } => true,
        }
    }
}

pub(super) struct PropertyValueAnimationData<T> {
    from_value: T,
    to_value: T,
    details: PropertyAnimation,
    start_time: crate::animations::Instant,
    state: AnimationState,
    spring: Option<SpringRegime>,
}

impl<T: InterpolatedPropertyValue + Clone> PropertyValueAnimationData<T> {
    pub fn new(from_value: T, to_value: T, details: PropertyAnimation) -> Self {
        Self::new_with_velocity(from_value, to_value, details, 0.0)
    }

    /// Used to carry velocity over across a retarget.
    pub fn new_with_velocity(
        from_value: T,
        to_value: T,
        details: PropertyAnimation,
        initial_velocity: f32,
    ) -> Self {
        let start_time = crate::animations::current_tick();
        let spring = Self::compute_spring(&details, &from_value, &to_value, initial_velocity);
        Self { from_value, to_value, details, start_time, state: AnimationState::Delaying, spring }
    }

    /// A spring with duration <= 0 (and no mass/stiffness/damping override) can't be simulated
    fn compute_spring(
        details: &PropertyAnimation,
        from_value: &T,
        to_value: &T,
        initial_velocity: f32,
    ) -> Option<SpringRegime> {
        matches!(details.easing, crate::animations::EasingCurve::Spring)
            .then(|| {
                let (w_n, zeta) = if details.mass > 0. {
                    Some(
                        SpringPhysicalParameters::new(
                            details.mass,
                            details.stiffness,
                            details.damping,
                        )
                        .to_natural_frequency_and_damping_ratio(),
                    )
                } else if details.duration > 0 {
                    Some(
                        SpringDurationBounceParameters::new(
                            details.duration as f32 / 1000.0,
                            details.bounce,
                        )
                        .to_natural_frequency_and_damping_ratio(),
                    )
                } else {
                    None
                }?;

                // -1 so that the spring knows to go to 0; re-express the carried-over velocity
                // (in property units/sec) in the spring's -1..=0-relative units.
                let delta = from_value.scalar_delta(to_value);
                let v0 = if delta != 0.0 { initial_velocity / delta } else { 0.0 };
                Some(SpringRegime::new(-1.0, v0, w_n, zeta))
            })
            .flatten()
    }

    /// The current velocity (in property units per second) of a live spring animation
    fn current_velocity(&self) -> Option<f32> {
        if !matches!(self.state, AnimationState::Animating { .. }) {
            return None;
        }
        let spring = self.spring.as_ref()?;
        let elapsed_secs =
            crate::animations::current_tick().duration_since(self.start_time).as_millis() as f32
                / 1000.0;
        let (_, rel_vel) = spring.evaluate(elapsed_secs);
        Some(rel_vel * self.from_value.scalar_delta(&self.to_value))
    }

    /// Single iteration of the animation
    pub fn compute_interpolated_value(&mut self) -> (T, bool) {
        // If animation is disabled, immediately return the target value
        if !self.details.enabled {
            return (self.to_value.clone(), true);
        }

        let new_tick = crate::animations::current_tick();
        let mut time_progress = new_tick.duration_since(self.start_time).as_millis() as u64;
        let reversed = |iteration: u64| -> bool {
            #[allow(clippy::manual_is_multiple_of)] // keep symmetry
            match self.details.direction {
                AnimationDirection::Normal => false,
                AnimationDirection::Reverse => true,
                AnimationDirection::Alternate => iteration % 2 == 1,
                AnimationDirection::AlternateReverse => iteration % 2 == 0,
            }
        };

        match self.state {
            AnimationState::Delaying => {
                if self.details.delay <= 0 {
                    self.state = AnimationState::Animating { current_iteration: 0 };
                    return self.compute_interpolated_value();
                }

                let delay = self.details.delay as u64;

                if time_progress < delay {
                    if reversed(0) {
                        (self.to_value.clone(), false)
                    } else {
                        (self.from_value.clone(), false)
                    }
                } else {
                    self.start_time =
                        new_tick - core::time::Duration::from_millis(time_progress - delay);

                    // Decide on next state:
                    self.state = AnimationState::Animating { current_iteration: 0 };
                    self.compute_interpolated_value()
                }
            }
            AnimationState::Animating { mut current_iteration } => {
                // A spring runs in real time and ends only once it settles.
                if matches!(self.details.easing, crate::animations::EasingCurve::Spring) {
                    return if let Some(spring) = self.spring.as_ref() {
                        let elapsed_secs = time_progress as f32 / 1000.0;
                        let (t, settled) =
                            crate::animations::spring_settle_progress(spring, elapsed_secs);
                        if settled {
                            self.state = AnimationState::Done { iteration_count: 0 };
                            (self.to_value.clone(), true)
                        } else {
                            (self.from_value.interpolate(&self.to_value, t), false)
                        }
                    } else {
                        self.state = AnimationState::Done { iteration_count: 0 };
                        self.compute_interpolated_value()
                    };
                }

                if self.details.duration <= 0 || self.details.iteration_count == 0. {
                    self.state = AnimationState::Done { iteration_count: 0 };
                    return self.compute_interpolated_value();
                }

                let duration = self.details.duration as u64;
                if time_progress >= duration {
                    // wrap around
                    current_iteration += time_progress / duration;
                    time_progress %= duration;
                    self.start_time = new_tick - core::time::Duration::from_millis(time_progress);
                }

                if (self.details.iteration_count < 0.)
                    || (((current_iteration * duration) + time_progress) as f64)
                        < ((self.details.iteration_count as f64) * (duration as f64))
                {
                    self.state = AnimationState::Animating { current_iteration };

                    let progress = {
                        let progress =
                            (time_progress as f32 / self.details.duration as f32).clamp(0., 1.);
                        if reversed(current_iteration) { 1. - progress } else { progress }
                    };
                    let t = crate::animations::easing_curve(&self.details.easing, progress);
                    let val = self.from_value.interpolate(&self.to_value, t);

                    (val, false)
                } else {
                    self.state =
                        AnimationState::Done { iteration_count: current_iteration.max(1) - 1 };
                    self.compute_interpolated_value()
                }
            }
            AnimationState::Done { iteration_count } => {
                if reversed(iteration_count) {
                    (self.from_value.clone(), true)
                } else {
                    (self.to_value.clone(), true)
                }
            }
        }
    }

    fn reset(&mut self) {
        self.state = AnimationState::Delaying;
        self.start_time = crate::animations::current_tick();
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub(super) enum AnimatedBindingState {
    Animating,
    NotAnimating,
    ShouldStart,
}

#[pin_project::pin_project]
pub(super) struct AnimatedBindingCallable<T, A> {
    #[pin]
    pub(super) original_binding: PropertyHandle,
    pub(super) state: Cell<AnimatedBindingState>,
    pub(super) animation_data: RefCell<PropertyValueAnimationData<T>>,
    pub(super) compute_animation_details: A,
    pub(crate) carried_velocity: Cell<f32>,
}

pub(super) type AnimationDetail = (PropertyAnimation, Option<crate::animations::Instant>);

unsafe impl<T: InterpolatedPropertyValue + Clone, A: Fn() -> AnimationDetail> BindingCallable<T>
    for AnimatedBindingCallable<T, A>
{
    fn evaluate(self: Pin<&Self>, value: &mut T) -> BindingResult {
        let original_binding = self.project_ref().original_binding;
        original_binding.register_as_dependency_to_current_binding(
            #[cfg(slint_debug_property)]
            "<AnimatedBindingCallable>",
        );
        match self.state.get() {
            AnimatedBindingState::Animating => {
                let (val, finished) = self.animation_data.borrow_mut().compute_interpolated_value();
                *value = val;
                if finished {
                    self.state.set(AnimatedBindingState::NotAnimating)
                } else {
                    crate::animations::CURRENT_ANIMATION_DRIVER
                        .with(|driver| driver.set_has_active_animations());
                }
            }
            AnimatedBindingState::NotAnimating => {
                // Safety: `value` is a valid mutable reference
                unsafe { self.original_binding.update(value as *mut T) };
            }
            AnimatedBindingState::ShouldStart => {
                self.state.set(AnimatedBindingState::Animating);
                let mut animation_data = self.animation_data.borrow_mut();

                // get the live value because if the target is updated every frame, the old value
                // isn't valid as it never goes through the Animating branch
                let live_value = matches!(animation_data.state, AnimationState::Animating { .. })
                    .then(|| animation_data.compute_interpolated_value().0);
                // animation_data.details.iteration_count = 1.;
                animation_data.from_value = live_value.unwrap_or_else(|| value.clone());
                let (details, start_time) = (self.compute_animation_details)();
                if let Some(start_time) = start_time {
                    animation_data.start_time = start_time;
                }
                animation_data.details = details;

                // Safety: `animation_data.to_value` is a valid mutable reference
                unsafe { self.original_binding.update((&mut animation_data.to_value) as *mut T) };
                animation_data.spring = PropertyValueAnimationData::<T>::compute_spring(
                    &animation_data.details,
                    &animation_data.from_value,
                    &animation_data.to_value,
                    self.carried_velocity.get(),
                );
                let (val, finished) = animation_data.compute_interpolated_value();
                *value = val;
                if finished {
                    self.state.set(AnimatedBindingState::NotAnimating)
                } else {
                    crate::animations::CURRENT_ANIMATION_DRIVER
                        .with(|driver| driver.set_has_active_animations());
                }
            }
        };
        BindingResult::KeepBinding
    }
    fn mark_dirty(self: Pin<&Self>) {
        if self.state.get() == AnimatedBindingState::ShouldStart {
            return;
        }
        let original_dirty = self.original_binding.access(|b| b.unwrap().dirty.get());
        if original_dirty {
            self.carried_velocity
                .set(self.animation_data.borrow().current_velocity().unwrap_or(0.0));
            self.state.set(AnimatedBindingState::ShouldStart);
            self.animation_data.borrow_mut().reset();
        }
    }

    fn velocity(self: Pin<&Self>) -> Option<f32> {
        self.animation_data.borrow().current_velocity()
    }
}

/// InterpolatedPropertyValue is a trait used to enable properties to be used with
/// animations that interpolate values. The basic requirement is the ability to apply
/// a progress that's typically between 0 and 1 to a range.
pub trait InterpolatedPropertyValue: PartialEq + Default + 'static {
    /// Returns the interpolated value between self and target_value according to the
    /// progress parameter t that's usually between 0 and 1. With certain animation
    /// easing curves it may over- or undershoot though.
    #[must_use]
    fn interpolate(&self, target_value: &Self, t: f32) -> Self;

    /// Returns `target_value - self` as a scalar.
    /// Types with no natural single-scalar notion of velocity keep the default `0.0`
    fn scalar_delta(&self, _target_value: &Self) -> f32 {
        0.0
    }
}

impl InterpolatedPropertyValue for f32 {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        self + t * (target_value - self)
    }

    fn scalar_delta(&self, target_value: &Self) -> f32 {
        target_value - self
    }
}

impl InterpolatedPropertyValue for i32 {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        self + (t * (target_value - self) as f32).round() as i32
    }

    fn scalar_delta(&self, target_value: &Self) -> f32 {
        (target_value - self) as f32
    }
}

impl InterpolatedPropertyValue for i64 {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        self + (t * (target_value - self) as f32).round() as Self
    }

    fn scalar_delta(&self, target_value: &Self) -> f32 {
        (target_value - self) as f32
    }
}

impl InterpolatedPropertyValue for u8 {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        ((*self as f32) + (t * ((*target_value as f32) - (*self as f32)))).round().clamp(0., 255.)
            as u8
    }

    fn scalar_delta(&self, target_value: &Self) -> f32 {
        (*target_value as f32) - (*self as f32)
    }
}

impl InterpolatedPropertyValue for LogicalLength {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        LogicalLength::new(self.get().interpolate(&target_value.get(), t))
    }

    fn scalar_delta(&self, target_value: &Self) -> f32 {
        (target_value.get() - self.get()) as f32
    }
}

/// Binding installed by `Property::set_animated_value`.
/// A type so a retarget can report the current velocity
struct AnimatedValueBinding<T> {
    animation_data: RefCell<PropertyValueAnimationData<T>>,
}

unsafe impl<T: InterpolatedPropertyValue + Clone + 'static> BindingCallable<T>
    for AnimatedValueBinding<T>
{
    fn evaluate(self: Pin<&Self>, value: &mut T) -> BindingResult {
        let (val, finished) = self.animation_data.borrow_mut().compute_interpolated_value();
        *value = val;
        if finished {
            BindingResult::RemoveBinding
        } else {
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.set_has_active_animations());
            BindingResult::KeepBinding
        }
    }

    fn velocity(self: Pin<&Self>) -> Option<f32> {
        self.animation_data.borrow().current_velocity()
    }
}

impl<T: Clone + InterpolatedPropertyValue + 'static> Property<T> {
    /// Evaluate the property and remove the (animation) binding of this property.
    ///
    /// Note that a binding can intercept this via intercept_set_binding and still remain on the property.
    /// (e.g. two-way-bindings will not be removed with this call!)
    pub fn remove_binding(self: Pin<&Self>) {
        // FIXME: This is a bit of a hack, set_animated_value will call set_binding on the internal handle,
        // which will call intercept_set_binding, which will check if the binding should be removed or not.
        // In the case of two-way bindings, we want to keep the binding, but reset the value to the current one,
        // so that any animation binding is removed, but the two-way-binding is kept.
        self.set_animated_value(self.get(), PropertyAnimation::default());
    }

    /// Change the value of this property, by animating (interpolating) from the current property's value
    /// to the specified parameter value. The animation is done according to the parameters described by
    /// the PropertyAnimation object.
    ///
    /// If other properties have binding depending of this property, these properties will
    /// be marked as dirty.
    pub fn set_animated_value(self: Pin<&Self>, value: T, animation_data: PropertyAnimation) {
        // Carry over the outgoing binding's velocity
        let carried_velocity = self.handle.current_velocity().unwrap_or(0.0);
        let binding = properties_animations::AnimatedValueBinding {
            animation_data: RefCell::new(
                properties_animations::PropertyValueAnimationData::new_with_velocity(
                    self.get(),
                    value,
                    animation_data,
                    carried_velocity,
                ),
            ),
        };
        // Safety: the BindingCallable will cast its argument to T
        unsafe {
            self.handle.set_binding(
                binding,
                #[cfg(slint_debug_property)]
                self.debug_name.borrow().as_str(),
            );
        }
        self.handle.mark_dirty(
            #[cfg(slint_debug_property)]
            self.debug_name.borrow().as_str(),
        );
    }

    /// Set a binding to this property, providing a callback for the animation and an optional
    /// start_time (relevant for state transitions).
    pub fn set_animated_binding(
        &self,
        binding: impl Binding<T> + 'static,
        compute_animation_details: impl Fn() -> (PropertyAnimation, Option<crate::animations::Instant>)
        + 'static,
    ) {
        let binding_callable = properties_animations::AnimatedBindingCallable::<T, _> {
            original_binding: PropertyHandle {
                handle: Cell::new(
                    (alloc_binding_holder(move |val: &mut T| {
                        *val = binding.evaluate(val);
                        BindingResult::KeepBinding
                    }) as *mut ())
                        .map_addr(|a| a | 0b10),
                ),
            },
            state: Cell::new(properties_animations::AnimatedBindingState::NotAnimating),
            animation_data: RefCell::new(properties_animations::PropertyValueAnimationData::new(
                T::default(),
                T::default(),
                PropertyAnimation::default(),
            )),
            compute_animation_details,
            carried_velocity: Cell::new(0.0),
        };

        // Safety: the `AnimatedBindingCallable`'s type match the property type
        unsafe {
            self.handle.set_binding(
                binding_callable,
                #[cfg(slint_debug_property)]
                self.debug_name.borrow().as_str(),
            )
        };
        self.handle.mark_dirty(
            #[cfg(slint_debug_property)]
            self.debug_name.borrow().as_str(),
        );
    }
}

unsafe impl<Unit, S: Simulation> BindingCallable<Length<crate::Coord, Unit>>
    for RefCell<PropertyPhysicsAnimationData<S>>
{
    fn evaluate(self: Pin<&Self>, value: &mut Length<crate::Coord, Unit>) -> BindingResult {
        let finished = self.borrow_mut().update_value(&mut value.0);
        if finished {
            BindingResult::RemoveBinding
        } else {
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.set_has_active_animations());
            BindingResult::KeepBinding
        }
    }

    // This binding should not be removed if the value is updated externally.
    fn intercept_set(self: Pin<&Self>, _value: &Length<crate::Coord, Unit>) -> bool {
        true
    }
}

impl<Unit> Property<Length<crate::Coord, Unit>> {
    /// Change the value by using a physics animation
    pub fn set_physic_animation_value<S: Simulation + 'static, AD: Parameter<Output = S>>(
        &self,
        limit_value: Pin<Box<Property<f32>>>,
        simulation_data: AD,
    ) {
        // Safety: the BindingCallable will cast its argument to T
        unsafe {
            self.handle.set_binding::<Length<crate::Coord, Unit>, core::cell::RefCell<PropertyPhysicsAnimationData<S>>>(RefCell::new(PropertyPhysicsAnimationData::new(
                    simulation_data.simulation(self.get_internal().0 as f32, limit_value),
                )),
                #[cfg(slint_debug_property)]
                self.debug_name.borrow().as_str()
            );
        }
        self.handle.mark_dirty(
            #[cfg(slint_debug_property)]
            self.debug_name.borrow().as_str(),
        );
    }
}

#[cfg(test)]
mod animation_tests {
    use super::*;
    use pin_weak::rc::PinWeak;
    use std::rc::Rc;

    #[derive(Default)]
    struct Component {
        width: Property<i32>,
        width_times_two: Property<i32>,
        feed_property: Property<i32>, // used by binding to feed values into width
    }

    impl Component {
        fn new_test_component() -> Pin<Rc<Self>> {
            let compo = Rc::pin(Component::default());
            let w = PinWeak::downgrade(compo.clone());
            compo.width_times_two.set_binding(move || {
                let compo = w.upgrade().unwrap();
                get_prop_value(&compo.width) * 2
            });

            compo
        }
    }

    const DURATION: std::time::Duration = std::time::Duration::from_millis(10000);
    const DELAY: std::time::Duration = std::time::Duration::from_millis(800);

    // Helper just for testing
    fn get_prop_value<T: Clone>(prop: &Property<T>) -> T {
        unsafe { Pin::new_unchecked(prop).get() }
    }

    // Helper just for testing: the property lives in a pinned `Rc<Component>`.
    fn set_animated_value<T: Clone + InterpolatedPropertyValue + 'static>(
        prop: &Property<T>,
        value: T,
        animation_data: PropertyAnimation,
    ) {
        unsafe { Pin::new_unchecked(prop) }.set_animated_value(value, animation_data);
    }

    #[test]
    fn properties_test_animation_negative_delay_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: -25,
            duration: DURATION.as_millis() as _,
            iteration_count: 1.,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn properties_test_animation_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: 1.,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn properties_test_delayed_animation_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: DELAY.as_millis() as _,
            iteration_count: 1.,
            duration: DURATION.as_millis() as _,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn properties_test_delayed_animation_fractal_iteration_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: DELAY.as_millis() as _,
            iteration_count: 1.5,
            duration: DURATION.as_millis() as _,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // (fractal) end of animation
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 4));
        assert_eq!(get_prop_value(&compo.width), 125);
        assert_eq!(get_prop_value(&compo.width_times_two), 250);

        // End of animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }
    #[test]
    fn properties_test_delayed_animation_null_duration_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: DELAY.as_millis() as _,
            iteration_count: 1.0,
            duration: 0,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // No animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn properties_test_delayed_animation_negative_duration_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: DELAY.as_millis() as _,
            iteration_count: 1.0,
            duration: -25,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // No animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn properties_test_delayed_animation_no_iteration_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: DELAY.as_millis() as _,
            iteration_count: 0.0,
            duration: DURATION.as_millis() as _,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // No animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn properties_test_delayed_animation_negative_iteration_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: DELAY.as_millis() as _,
            iteration_count: -42., // loop forever!
            duration: DURATION.as_millis() as _,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In animation (again):
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + 500 * DURATION));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + DELAY + 50000 * DURATION + DURATION / 2)
        });
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        // the binding should not be removed as it is still animating!
        compo.width.handle.access(|binding| assert!(binding.is_some()));
    }

    #[test]
    fn properties_test_animation_direction_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: -25,
            duration: DURATION.as_millis() as _,
            direction: AnimationDirection::AlternateReverse,
            iteration_count: 1.,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // Overshoot: Always from_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn properties_test_animation_triggered_by_binding() {
        let compo = Component::new_test_component();

        let start_time = crate::animations::current_tick();

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: 1.,
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        compo.width.set_animated_binding(
            move || {
                let compo = w.upgrade().unwrap();
                get_prop_value(&compo.feed_property)
            },
            move || (animation_details.clone(), None),
        );

        compo.feed_property.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        compo.feed_property.set(200);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);
    }

    #[test]
    fn properties_test_delayed_animation_triggered_by_binding() {
        let compo = Component::new_test_component();

        let start_time = crate::animations::current_tick();

        let animation_details = PropertyAnimation {
            delay: DELAY.as_millis() as _,
            duration: DURATION.as_millis() as _,
            iteration_count: 1.0,
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        compo.width.set_animated_binding(
            move || {
                let compo = w.upgrade().unwrap();
                get_prop_value(&compo.feed_property)
            },
            move || (animation_details.clone(), None),
        );

        compo.feed_property.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        compo.feed_property.set(200);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);
    }

    #[test]
    fn test_loop() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: 2.,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        assert_eq!(get_prop_value(&compo.width), 100);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION * 2));
        assert_eq!(get_prop_value(&compo.width), 200);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn test_loop_via_binding() {
        // Loop twice, restart the animation and still loop twice.

        let compo = Component::new_test_component();

        let start_time = crate::animations::current_tick();

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: 2.,
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        compo.width.set_animated_binding(
            move || {
                let compo = w.upgrade().unwrap();
                get_prop_value(&compo.feed_property)
            },
            move || (animation_details.clone(), None),
        );

        compo.feed_property.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);

        compo.feed_property.set(200);
        assert_eq!(get_prop_value(&compo.width), 100);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));

        assert_eq!(get_prop_value(&compo.width), 150);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));

        assert_eq!(get_prop_value(&compo.width), 100);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));

        assert_eq!(get_prop_value(&compo.width), 150);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + 2 * DURATION));

        assert_eq!(get_prop_value(&compo.width), 200);

        // Overshoot a bit:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + 2 * DURATION + DURATION / 2));

        assert_eq!(get_prop_value(&compo.width), 200);

        // Restart the animation by setting a new value.

        let start_time = crate::animations::current_tick();

        compo.feed_property.set(300);
        assert_eq!(get_prop_value(&compo.width), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));

        assert_eq!(get_prop_value(&compo.width), 250);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));

        assert_eq!(get_prop_value(&compo.width), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));

        assert_eq!(get_prop_value(&compo.width), 250);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + 2 * DURATION));

        assert_eq!(get_prop_value(&compo.width), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + 2 * DURATION + DURATION / 2));

        assert_eq!(get_prop_value(&compo.width), 300);
    }

    #[test]
    fn spring_retarget_carries_velocity() {
        // A retarget mid-flight must carry the outgoing spring's velocity into the new one,
        // instead of restarting it at rest (which would produce a visible "pop").
        let compo = Component::new_test_component();

        let spring_details = PropertyAnimation {
            duration: 1000,
            bounce: 0.0,
            easing: crate::animations::EasingCurve::Spring,
            ..PropertyAnimation::default()
        };

        compo.width.set(0);
        let start_time = crate::animations::current_tick();
        set_animated_value(&compo.width, 1000, spring_details.clone());

        // Let the spring run for a while so it picks up meaningful velocity.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(300))
        });
        let before_a = get_prop_value(&compo.width) as f32;
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(310))
        });
        let before_b = get_prop_value(&compo.width) as f32;
        let slope_before = before_b - before_a; // units per 10ms, just prior to the retarget

        // Retarget to a new value while the spring is still moving.
        set_animated_value(&compo.width, 2000, spring_details);
        assert_eq!(
            get_prop_value(&compo.width) as f32,
            before_b,
            "retarget must not snap the value"
        );

        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(320))
        });
        let after = get_prop_value(&compo.width) as f32;
        let slope_after = after - before_b; // units per 10ms, just after the retarget

        // With velocity carried over, the slope right after the retarget should be close to the
        // slope right before it (same order of magnitude, same direction). Without the fix, the
        // new spring starts at rest (v0 == 0), so `slope_after` would be near zero here.
        assert!(slope_before > 0.5, "sanity check: spring should be moving before retarget");
        assert!(
            slope_after > slope_before * 0.5,
            "velocity was not carried over: slope_before={slope_before}, slope_after={slope_after}"
        );
    }

    #[test]
    fn spring_retarget_via_binding_carries_velocity() {
        let compo = Component::new_test_component();

        let spring_details = PropertyAnimation {
            duration: 1000,
            bounce: 0.0,
            easing: crate::animations::EasingCurve::Spring,
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        let details = spring_details.clone();
        compo.width.set_animated_binding(
            move || {
                let compo = w.upgrade().unwrap();
                get_prop_value(&compo.feed_property)
            },
            move || (details.clone(), None),
        );

        // Establish the dependency and a baseline value (the very first read never animates).
        compo.feed_property.set(0);
        assert_eq!(get_prop_value(&compo.width), 0);

        let start_time = crate::animations::current_tick();
        compo.feed_property.set(1000);
        assert_eq!(get_prop_value(&compo.width), 0);

        // Let the spring run for a while so it picks up meaningful velocity.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(300))
        });
        let before_a = get_prop_value(&compo.width) as f32;
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(310))
        });
        let before_b = get_prop_value(&compo.width) as f32;
        let slope_before = before_b - before_a; // units per 10ms, just prior to the retarget

        // Retarget mid-flight by changing the value the animated binding reads.
        compo.feed_property.set(2000);
        assert_eq!(
            get_prop_value(&compo.width) as f32,
            before_b,
            "retarget must not snap the value"
        );

        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(320))
        });
        let after = get_prop_value(&compo.width) as f32;
        let slope_after = after - before_b; // units per 10ms, just after the retarget

        assert!(slope_before > 0.5, "sanity check: spring should be moving before retarget");
        assert!(
            slope_after > slope_before * 0.5,
            "velocity was not carried over through the binding-triggered retarget path: slope_before={slope_before}, slope_after={slope_after}"
        );
    }

    #[test]
    fn spring_continuous_retarget_keeps_advancing() {
        let compo = Component::new_test_component();

        let spring_details = PropertyAnimation {
            mass: 1.0,
            stiffness: 2.0,
            damping: 0.7,
            easing: crate::animations::EasingCurve::Spring,
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        let details = spring_details.clone();
        compo.width.set_animated_binding(
            move || {
                let compo = w.upgrade().unwrap();
                get_prop_value(&compo.feed_property)
            },
            move || (details.clone(), None),
        );

        compo.feed_property.set(0);
        assert_eq!(get_prop_value(&compo.width), 0);

        let start_time = crate::animations::current_tick();
        let mut mouse_x = 0i32;
        let mut final_width = 0f32;
        for frame in 1..=200 {
            mouse_x += 5; // simulate a steady mouse drag, 5px per frame
            compo.feed_property.set(mouse_x);
            let t = start_time + core::time::Duration::from_millis(frame * 16);
            crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| driver.update_animations(t));
            // Read the property every frame, like a renderer would when painting each frame --
            // this is what actually drives `evaluate()` (property evaluation is lazy).
            final_width = get_prop_value(&compo.width) as f32;
        }

        assert!(
            final_width > 500.0,
            "spring should have tracked the continuously-moving target by now, got {final_width}"
        );
    }
}
