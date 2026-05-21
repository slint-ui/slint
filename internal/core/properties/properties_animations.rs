// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use crate::{
    animations::physics_simulation,
    items::{AngleInterpolation, AnimationDirection, PropertyAnimation},
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
    S: physics_simulation::Simulation,
{
    pub fn new(simulation: S) -> PropertyPhysicsAnimationData<S> {
        PropertyPhysicsAnimationData { simulation, state: AnimationState::Delaying }
    }

    /// Single iteration of the animation
    pub fn compute_interpolated_value(&mut self) -> (crate::Coord, bool) {
        match self.state {
            AnimationState::Delaying => {
                // Decide on next state:
                self.state = AnimationState::Animating { current_iteration: 0 };
                self.compute_interpolated_value()
            }
            AnimationState::Animating { current_iteration: _ } => {
                let (val, finished) = self.simulation.step(crate::animations::current_tick());
                if finished {
                    self.state = AnimationState::Done { iteration_count: 0 };
                    self.compute_interpolated_value()
                } else {
                    (val as crate::Coord, false)
                }
            }
            AnimationState::Done { iteration_count: _ } => {
                (self.simulation.curr_value() as crate::Coord, true)
            }
        }
    }
}

pub(super) struct PropertyValueAnimationData<T> {
    from_value: T,
    to_value: T,
    details: PropertyAnimation,
    start_time: crate::animations::Instant,
    state: AnimationState,
}

impl<T: InterpolatedPropertyValue + Clone> PropertyValueAnimationData<T> {
    pub fn new(from_value: T, to_value: T, details: PropertyAnimation) -> Self {
        let start_time = crate::animations::current_tick();

        Self { from_value, to_value, details, start_time, state: AnimationState::Delaying }
    }

    /// Single iteration of the animation
    pub fn compute_interpolated_value(&mut self) -> (T, bool) {
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
                    let val = self.from_value.interpolate_with_mode(
                        &self.to_value,
                        t,
                        self.details.angle_interpolation,
                    );

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
                // animation_data.details.iteration_count = 1.;
                animation_data.from_value = value.clone();
                let (details, start_time) = (self.compute_animation_details)();
                if let Some(start_time) = start_time {
                    animation_data.start_time = start_time;
                }
                animation_data.details = details;

                // Safety: `animation_data.to_value` is a valid mutable reference
                unsafe { self.original_binding.update((&mut animation_data.to_value) as *mut T) };
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
            self.state.set(AnimatedBindingState::ShouldStart);
            self.animation_data.borrow_mut().reset();
        }
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

    /// Returns the interpolated value with a specific interpolation mode.
    /// This is particularly useful for angle values where the interpolation path matters.
    /// The default implementation ignores the mode and uses linear interpolation.
    #[must_use]
    fn interpolate_with_mode(
        &self,
        target_value: &Self,
        t: f32,
        _mode: AngleInterpolation,
    ) -> Self {
        self.interpolate(target_value, t)
    }
}

impl InterpolatedPropertyValue for f32 {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        self + t * (target_value - self)
    }

    fn interpolate_with_mode(&self, target_value: &Self, t: f32, mode: AngleInterpolation) -> Self {
        const FULL_ROTATION: f32 = 360.0;
        const HALF_ROTATION: f32 = 180.0;

        // Difference normalized to (-180, 180]; for an exact 180° opposite, the sign of the
        // original delta is preserved so the traversal direction follows the user's input.
        let shorter = || {
            let raw = target_value - self;
            let d = raw.rem_euclid(FULL_ROTATION);
            if d > HALF_ROTATION || (d == HALF_ROTATION && raw < 0.0) {
                d - FULL_ROTATION
            } else {
                d
            }
        };

        let diff = match mode {
            AngleInterpolation::Linear => return self.interpolate(target_value, t),
            AngleInterpolation::Shorter => shorter(),
            AngleInterpolation::Longer => {
                let s = shorter();
                if s > 0.0 {
                    s - FULL_ROTATION
                } else if s < 0.0 {
                    s + FULL_ROTATION
                } else {
                    // shorter() returned 0: either target == self (no motion) or
                    // the endpoints are coterminal (raw is a non-zero multiple of 360°).
                    // For the coterminal case, take a full rotation in the direction
                    // of the user-supplied delta.
                    let raw = target_value - self;
                    if raw > 0.0 {
                        FULL_ROTATION
                    } else if raw < 0.0 {
                        -FULL_ROTATION
                    } else {
                        0.0
                    }
                }
            }
            AngleInterpolation::Increasing => {
                let d = (target_value - self).rem_euclid(FULL_ROTATION);
                if d == 0.0 && target_value != self { FULL_ROTATION } else { d }
            }
            AngleInterpolation::Decreasing => {
                let d = (target_value - self).rem_euclid(FULL_ROTATION);
                if d == 0.0 {
                    // target == self: no motion; otherwise a full negative rotation
                    if target_value == self { 0.0 } else { -FULL_ROTATION }
                } else {
                    d - FULL_ROTATION
                }
            }
        };
        self + t * diff
    }
}

impl InterpolatedPropertyValue for i32 {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        self + (t * (target_value - self) as f32).round() as i32
    }
}

impl InterpolatedPropertyValue for i64 {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        self + (t * (target_value - self) as f32).round() as Self
    }
}

impl InterpolatedPropertyValue for u8 {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        ((*self as f32) + (t * ((*target_value as f32) - (*self as f32)))).round().clamp(0., 255.)
            as u8
    }
}

impl InterpolatedPropertyValue for LogicalLength {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        LogicalLength::new(self.get().interpolate(&target_value.get(), t))
    }
}

impl<T: Clone + InterpolatedPropertyValue + 'static> Property<T> {
    /// Change the value of this property, by animating (interpolating) from the current property's value
    /// to the specified parameter value. The animation is done according to the parameters described by
    /// the PropertyAnimation object.
    ///
    /// If other properties have binding depending of this property, these properties will
    /// be marked as dirty.
    pub fn set_animated_value(&self, value: T, animation_data: PropertyAnimation) {
        // FIXME if the current value is a dirty binding, we must run it, but we do not have the context
        let d = RefCell::new(properties_animations::PropertyValueAnimationData::new(
            self.get_internal(),
            value,
            animation_data,
        ));
        // Safety: the BindingCallable will cast its argument to T
        unsafe {
            self.handle.set_binding(
                move |val: &mut T| {
                    let (value, finished) = d.borrow_mut().compute_interpolated_value();
                    *val = value;
                    if finished {
                        BindingResult::RemoveBinding
                    } else {
                        crate::animations::CURRENT_ANIMATION_DRIVER
                            .with(|driver| driver.set_has_active_animations());
                        BindingResult::KeepBinding
                    }
                },
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

impl<T> Property<Length<crate::Coord, T>> {
    /// Change the value by using a physics animation
    pub fn set_physic_animation_value<
        S: physics_simulation::Simulation + 'static,
        AD: physics_simulation::Parameter<Output = S>,
    >(
        &self,
        value: Length<crate::Coord, T>,
        simulation_data: AD,
    ) {
        let d = RefCell::new(PropertyPhysicsAnimationData::new(
            simulation_data.simulation(self.get_internal().0 as f32, value.0 as f32),
        ));
        // Safety: the BindingCallable will cast its argument to T
        unsafe {
            self.handle.set_binding(
                move |val: &mut Length<crate::Coord, T>| {
                    let (value, finished) = d.borrow_mut().compute_interpolated_value();
                    *val = Length::new(value);
                    if finished {
                        BindingResult::RemoveBinding
                    } else {
                        crate::animations::CURRENT_ANIMATION_DRIVER
                            .with(|driver| driver.set_has_active_animations());
                        BindingResult::KeepBinding
                    }
                },
                #[cfg(slint_debug_property)]
                self.debug_name.borrow().as_str(),
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
    use std::rc::Rc;

    #[derive(Default)]
    struct Component {
        width: Property<i32>,
        width_times_two: Property<i32>,
        feed_property: Property<i32>, // used by binding to feed values into width
    }

    impl Component {
        fn new_test_component() -> Rc<Self> {
            let compo = Rc::new(Component::default());
            let w = Rc::downgrade(&compo);
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

        compo.width.set_animated_value(200, animation_details);
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

        compo.width.set_animated_value(200, animation_details);
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

        compo.width.set_animated_value(200, animation_details);
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

        compo.width.set_animated_value(200, animation_details);
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

        compo.width.set_animated_value(200, animation_details);
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

        compo.width.set_animated_value(200, animation_details);
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

        compo.width.set_animated_value(200, animation_details);
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

        compo.width.set_animated_value(200, animation_details);
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

        compo.width.set_animated_value(200, animation_details);
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

        let w = Rc::downgrade(&compo);
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

        let w = Rc::downgrade(&compo);
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

        compo.width.set_animated_value(200, animation_details);
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

        let w = Rc::downgrade(&compo);
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

    // Tests for AngleInterpolation modes (angle interpolation)
    // Note: Slint stores angles in DEGREES (not radians)
    mod interpolation_tests {
        use super::*;

        const EPSILON: f32 = 0.1; // 0.1 degree tolerance

        fn approx_eq(a: f32, b: f32) -> bool {
            (a - b).abs() < EPSILON
        }

        #[test]
        fn test_linear_interpolation() {
            // Linear interpolation: 10° to 350° should go through 180° (340° rotation)
            let from = 10.0_f32;
            let to = 350.0_f32;

            let mid = from.interpolate_with_mode(&to, 0.5, AngleInterpolation::Linear);
            let expected = 180.0_f32;
            assert!(approx_eq(mid, expected), "Linear: expected {}°, got {}°", expected, mid);
        }

        #[test]
        fn test_shorter_interpolation() {
            // Shorter arc: 10° to 350° goes via the angle decreasing (20° total)
            let from = 10.0_f32;
            let to = 350.0_f32;

            let mid = from.interpolate_with_mode(&to, 0.5, AngleInterpolation::Shorter);
            assert!(approx_eq(mid, 0.0), "Shorter mid: expected 0°, got {}°", mid);

            let end = from.interpolate_with_mode(&to, 1.0, AngleInterpolation::Shorter);
            assert!(approx_eq(end, -10.0), "Shorter end: expected -10°, got {}°", end);
        }

        #[test]
        fn test_longer_interpolation() {
            // Longer arc: 10° to 100° normally takes 90° (shorter)
            // Longer takes 270° in the decreasing direction
            let from = 10.0_f32;
            let to = 100.0_f32;

            let mid = from.interpolate_with_mode(&to, 0.5, AngleInterpolation::Longer);
            assert!(approx_eq(mid, -125.0), "Longer mid: expected -125°, got {}°", mid);
        }

        #[test]
        fn test_increasing_interpolation() {
            // Increasing: angle value always increases
            // From 350° to 10°, goes 350° -> 360° -> 370° (20° total)
            let from = 350.0_f32;
            let to = 10.0_f32;

            let mid = from.interpolate_with_mode(&to, 0.5, AngleInterpolation::Increasing);
            assert!(approx_eq(mid, 360.0), "Increasing mid: expected 360°, got {}°", mid);

            let end = from.interpolate_with_mode(&to, 1.0, AngleInterpolation::Increasing);
            assert!(approx_eq(end, 370.0), "Increasing end: expected 370°, got {}°", end);
        }

        #[test]
        fn test_decreasing_interpolation() {
            // Decreasing: angle value always decreases
            // From 10° to 350°, goes 10° -> 0° -> -10° (20° total)
            let from = 10.0_f32;
            let to = 350.0_f32;

            let mid = from.interpolate_with_mode(&to, 0.5, AngleInterpolation::Decreasing);
            assert!(approx_eq(mid, 0.0), "Decreasing mid: expected 0°, got {}°", mid);

            let end = from.interpolate_with_mode(&to, 1.0, AngleInterpolation::Decreasing);
            assert!(approx_eq(end, -10.0), "Decreasing end: expected -10°, got {}°", end);
        }

        #[test]
        fn test_shorter_already_shortest() {
            // When the linear path is already shortest, Shorter matches Linear
            let from = 10.0_f32;
            let to = 100.0_f32;

            let mid_linear = from.interpolate_with_mode(&to, 0.5, AngleInterpolation::Linear);
            let mid_shorter = from.interpolate_with_mode(&to, 0.5, AngleInterpolation::Shorter);

            assert!(
                approx_eq(mid_linear, mid_shorter),
                "Shorter should equal Linear when path is already shortest: {}° vs {}°",
                mid_linear,
                mid_shorter
            );
        }

        #[test]
        fn test_shorter_180_tie_preserves_sign() {
            // For an exact 180° opposite, the traversal direction follows the sign of
            // the user-supplied delta: positive raw delta increases, negative decreases.

            // 90° -> 270° (raw +180): value increases through 180° to 270°.
            let mid = 90.0_f32.interpolate_with_mode(&270.0, 0.5, AngleInterpolation::Shorter);
            assert!(approx_eq(mid, 180.0), "Shorter +180 tie mid: expected 180°, got {}°", mid);
            let end = 90.0_f32.interpolate_with_mode(&270.0, 1.0, AngleInterpolation::Shorter);
            assert!(approx_eq(end, 270.0), "Shorter +180 tie end: expected 270°, got {}°", end);

            // 270° -> 90° (raw -180): value decreases through 180° to 90°.
            // Mid lands on 180° but the underlying delta is -180, not +180.
            let mid = 270.0_f32.interpolate_with_mode(&90.0, 0.5, AngleInterpolation::Shorter);
            assert!(approx_eq(mid, 180.0), "Shorter -180 tie mid: expected 180°, got {}°", mid);
            let end = 270.0_f32.interpolate_with_mode(&90.0, 1.0, AngleInterpolation::Shorter);
            assert!(approx_eq(end, 90.0), "Shorter -180 tie end: expected 90°, got {}°", end);

            // Longer takes the opposite arc, so for raw +180 it goes negative
            // (90 + 0.5 * -180 = 0).
            let mid = 90.0_f32.interpolate_with_mode(&270.0, 0.5, AngleInterpolation::Longer);
            assert!(approx_eq(mid, 0.0), "Longer +180 tie mid: expected 0°, got {}°", mid);

            // For raw -180 Longer goes positive (270 + 0.5 * 180 = 360).
            let mid = 270.0_f32.interpolate_with_mode(&90.0, 0.5, AngleInterpolation::Longer);
            assert!(approx_eq(mid, 360.0), "Longer -180 tie mid: expected 360°, got {}°", mid);
        }

        #[test]
        fn test_no_motion_when_target_equals_self() {
            // When the target value equals the current value, every mode must stay put
            // for the whole duration of the animation (no spurious full rotation).
            for mode in [
                AngleInterpolation::Linear,
                AngleInterpolation::Shorter,
                AngleInterpolation::Longer,
                AngleInterpolation::Increasing,
                AngleInterpolation::Decreasing,
            ] {
                for &v in &[0.0_f32, 100.0, -45.0] {
                    let mid = v.interpolate_with_mode(&v, 0.5, mode);
                    assert!(
                        approx_eq(mid, v),
                        "{:?} from {}° to {}° at t=0.5: expected {}°, got {}°",
                        mode,
                        v,
                        v,
                        v,
                        mid
                    );
                    let end = v.interpolate_with_mode(&v, 1.0, mode);
                    assert!(
                        approx_eq(end, v),
                        "{:?} from {}° to {}° at t=1.0: expected {}°, got {}°",
                        mode,
                        v,
                        v,
                        v,
                        end
                    );
                }
            }
        }

        #[test]
        fn test_increasing_decreasing_full_rotation() {
            // When `to - from` is a non-zero multiple of 360 (visually the same angle),
            // Increasing/Decreasing should still rotate a full turn.
            let mid_inc =
                10.0_f32.interpolate_with_mode(&370.0, 0.5, AngleInterpolation::Increasing);
            assert!(
                approx_eq(mid_inc, 190.0),
                "Increasing 10→370 mid: expected 190°, got {}°",
                mid_inc
            );

            let mid_dec =
                10.0_f32.interpolate_with_mode(&370.0, 0.5, AngleInterpolation::Decreasing);
            assert!(
                approx_eq(mid_dec, -170.0),
                "Decreasing 10→370 mid: expected -170°, got {}°",
                mid_dec
            );
        }

        #[test]
        fn test_longer_coterminal_full_rotation() {
            // For coterminal endpoints (raw delta is a non-zero multiple of 360°), Longer
            // takes a full rotation in the direction of the user-supplied delta. Shorter
            // intentionally stays put because the endpoints are visually identical.
            let mid_long = 10.0_f32.interpolate_with_mode(&370.0, 0.5, AngleInterpolation::Longer);
            assert!(
                approx_eq(mid_long, 190.0),
                "Longer 10→370 mid: expected 190°, got {}°",
                mid_long
            );

            let mid_long_neg =
                10.0_f32.interpolate_with_mode(&-350.0, 0.5, AngleInterpolation::Longer);
            assert!(
                approx_eq(mid_long_neg, -170.0),
                "Longer 10→-350 mid: expected -170°, got {}°",
                mid_long_neg
            );
        }
    }
}
