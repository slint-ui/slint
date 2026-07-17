// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use crate::{
    animations::{
        APPLYING_ANIMATION, Animation, InterpolatingAnimation, handle::AnimationHandle,
        tween::TweenAnimation,
    },
    items::PropertyAnimation,
    lengths::LogicalLength,
};
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
#[cfg(not(feature = "std"))]
use num_traits::Float;

pub(crate) enum AnimationState {
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

/// Where a leaf animation's per-frame value goes.
pub(crate) enum AnimationSink<T> {
    /// Pushes straight into a property's cell via [`Property::set`], guarded by
    /// `with_applying_animation` so the write reads as a self-write to any binding still
    /// occupying that property's slot
    Property(*const Property<T>),
    /// Pushes via an arbitrary callback
    #[allow(dead_code)]
    // constructed by `TweenAnimation::new_with_callbacks`, unit-tested only
    Callback(Box<dyn FnMut(T)>),
}

impl<T: Clone + PartialEq> AnimationSink<T> {
    pub(crate) fn push(&mut self, value: T) {
        match self {
            AnimationSink::Property(target) => {
                // Safety: `target` is only ever populated by
                // `TweenAnimation::new_with_property_sink`, whose safety contract requires the
                // pointer to stay valid for as long as the tween is registered anywhere it can be
                // ticked. `push` only runs from such a tick, so the pointer is still valid here.
                crate::animations::with_applying_animation(|| unsafe { (**target).set(value) });
            }
            AnimationSink::Callback(set_value) => set_value(value),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub(super) enum AnimatedBindingState {
    Animating,
    NotAnimating,
    ShouldStart,
}

pub(super) type AnimationDetail = (PropertyAnimation, Option<crate::animations::Instant>);

/// What an external (non-animation) `set` on an [`AnimationTrigger`]'s watched property means.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub(super) enum SetPolicy {
    /// An external write cancels the animation, it removes the binding and snaps to the new value
    Cancel,
    /// (Re)starts the animation to animate towards the new target
    #[allow(dead_code)]
    Retrigger,
}

/// Watches the property's binding slot. When it detects a change, it hands it to `prime` to
/// (re)prime the concrete leaf.
#[pin_project::pin_project]
pub(super) struct AnimationTrigger<T, A> {
    #[pin]
    pub(super) original_binding: PropertyHandle,
    pub(super) state: Cell<AnimatedBindingState>,
    pub(super) compute_animation_details: A,
    /// The instant the change was detected
    pub(super) trigger_time: Cell<Option<crate::animations::Instant>>,
    /// The root animation to (re)start
    pub(super) root: Rc<RefCell<dyn Animation>>,
    /// Registers/deregisters `root` in the registry. Registry membership always tracks this
    /// trigger's lifetime. Only a trigger, which has one `handle`, registers its `root`
    pub(super) handle: AnimationHandle,
    /// Captures (from, to, anchor) and (re)primes the animation in `root`.
    /// Returns the value to publish for this `evaluate` along with if the animation is already
    /// finished.
    /// `None` for a plain root trigger and just (re)starts the animation
    #[allow(clippy::type_complexity)]
    pub(super) prime: Option<
        Box<dyn Fn(&T, &T, PropertyAnimation, Option<crate::animations::Instant>) -> (T, bool)>,
    >,
    pub(super) on_external_set: SetPolicy,
}

unsafe impl<T: Clone + 'static, A: Fn() -> AnimationDetail> BindingCallable<T>
    for AnimationTrigger<T, A>
{
    fn evaluate(self: Pin<&Self>, value: &mut T) -> BindingResult {
        let original_binding = self.project_ref().original_binding;
        original_binding.register_as_dependency_to_current_binding(
            #[cfg(slint_debug_property)]
            "<AnimationTrigger>",
        );
        match self.state.get() {
            // The leaf pushes values directly into the property cell via its sink, so once
            // running there is nothing to compute here
            AnimatedBindingState::Animating => {}
            AnimatedBindingState::NotAnimating => {
                // Safety: `value` is a valid mutable reference
                unsafe { self.original_binding.update(value as *mut T) };
            }
            AnimatedBindingState::ShouldStart => {
                let from_value = value.clone();
                // Capture the new target value by evaluating the wrapped binding (also refreshes
                // its dependency registration so a later change re-triggers `mark_dirty`).
                let mut to_value = from_value.clone();
                // Safety: `to_value` is a valid mutable reference
                unsafe { self.original_binding.update((&mut to_value) as *mut T) };

                match self.prime.as_ref() {
                    Some(prime) => {
                        let (details, start_time) = (self.compute_animation_details)();
                        // Anchor at the change instant (or the transition's explicit start_time),
                        // not at this `.get()`.
                        let anchor = start_time.or_else(|| self.trigger_time.take());
                        // Re-primes the animation in place
                        let (initial, finished) = (prime)(&from_value, &to_value, details, anchor);
                        *value = initial;
                        if finished {
                            self.state.set(AnimatedBindingState::NotAnimating);
                        } else {
                            self.state.set(AnimatedBindingState::Animating);
                            self.handle.restart(self.root.clone());
                            crate::animations::CURRENT_ANIMATION_DRIVER
                                .with(|driver| driver.set_has_active_animations());
                        }
                    }
                    None => {
                        // (re)start the animation as-is
                        *value = to_value;
                        self.restart_plain_root();
                    }
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
            self.trigger_time.set(Some(crate::animations::current_tick()));
            // Pause the root so a registry tick landing before the next `evaluate` doesn't push a
            // value interpolated towards the now-stale endpoint. If `root` is
            // momentarily borrowed, skip the pause: the worst case is one stale
            // frame before the next `evaluate` re-primes and overwrites it anyway.
            if let Ok(mut root) = self.root.try_borrow_mut() {
                root.pause_for_pending_retrigger();
            }
        }
    }

    fn intercept_set(self: Pin<&Self>, _value: &T) -> bool {
        if APPLYING_ANIMATION.with(|g| g.get()) {
            // Self-write: the animation pushing its own value. Keep this binding.
            return true;
        }
        match self.on_external_set {
            SetPolicy::Cancel => false,
            SetPolicy::Retrigger => {
                // No lazy `ShouldStart` defer needed here, this is already a direct, eager write,
                // so (re)start the root synchronously. `Property::set` writes the new value into
                // the cell right after this returns, regardless of what we do here.
                self.restart_plain_root();
                true
            }
        }
    }
}

impl<T, A> AnimationTrigger<T, A> {
    /// Shared by the plain (no-capture) root-trigger path and (re)start
    /// `root` as-is. Called both lazily, from `evaluate`'s `ShouldStart`/`None` branch (a
    /// dependency changed), and eagerly, from `intercept_set` under `SetPolicy::Retrigger`
    fn restart_plain_root(&self) {
        self.trigger_time.take();
        self.root.borrow_mut().restart();
        if self.root.borrow().is_running() {
            self.state.set(AnimatedBindingState::Animating);
            self.handle.restart(self.root.clone());
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.set_has_active_animations());
        } else {
            self.state.set(AnimatedBindingState::NotAnimating);
        }
    }
}

/// Builds a fresh long-lived tween sinking into `target`, plus the `prime` closure
/// [`AnimationTrigger`] uses to (re)prime it in place on every trigger.
/// Shared by `Property::set_animated_binding_object`/`set_animated_value_object`:
/// both install a [`SetPolicy::Cancel`] trigger over a `TweenAnimation` sink.
#[allow(clippy::type_complexity)]
pub(super) fn new_tween_trigger_root<T: InterpolatedPropertyValue + Clone>(
    target: *const Property<T>,
) -> (
    Rc<RefCell<dyn Animation>>,
    Box<dyn Fn(&T, &T, PropertyAnimation, Option<crate::animations::Instant>) -> (T, bool)>,
) {
    // Safety: forwarded to the caller's own obligation on `target` (see
    // `TweenAnimation::new_with_property_sink`); both callers satisfy it, as the tween ends up
    // owned (via `root`) by this same property's own binding.
    let tween: Rc<RefCell<TweenAnimation<T>>> = Rc::new(RefCell::new(unsafe {
        TweenAnimation::new_with_property_sink(
            T::default(),
            T::default(),
            PropertyAnimation::default(),
            target,
        )
    }));
    let prime_tween = tween.clone();
    let prime: Box<
        dyn Fn(&T, &T, PropertyAnimation, Option<crate::animations::Instant>) -> (T, bool),
    > = Box::new(move |from: &T, to: &T, details, anchor| {
        let mut t = prime_tween.borrow_mut();
        t.retrigger(from.clone(), to.clone(), details, anchor);
        t.compute_interpolated_value()
    });
    let root: Rc<RefCell<dyn Animation>> = tween;
    (root, prime)
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
}

impl InterpolatedPropertyValue for f32 {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        self + t * (target_value - self)
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
    /// Install an animated binding: an `AnimationTrigger` change-detector wrapping `binding`,
    /// whose triggered leaf is a long-lived `TweenAnimation` registered in the shared
    /// `CURRENT_ANIMATIONS` registry (driven each frame by [`update_animation_objects`]) and
    /// (re)primed in place on every detected change.
    pub fn set_animated_binding_object(
        &self,
        binding: impl Binding<T> + 'static,
        compute_animation_details: impl Fn() -> (PropertyAnimation, Option<crate::animations::Instant>)
        + 'static,
    ) {
        let (root, prime) =
            properties_animations::new_tween_trigger_root(self as *const Property<T>);
        let binding_callable = properties_animations::AnimationTrigger::<T, _> {
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
            compute_animation_details,
            trigger_time: Cell::new(None),
            root,
            handle: properties_animations::AnimationHandle::default(),
            prime: Some(prime),
            on_external_set: properties_animations::SetPolicy::Cancel,
        };

        // Safety: the `AnimationTrigger`'s type matches the property type
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

    /// Animate the property from its current value to `value`.
    pub fn set_animated_value_object(&self, value: T, animation_data: PropertyAnimation) {
        // Force the existing binding to run before it gets replaced below: otherwise, if it
        // was never evaluated (e.g. assigning to an animated property in `init`, before its
        // binding ever ran), the animation would start from the type's default value instead
        // of the binding's actual value.
        unsafe { Pin::new_unchecked(self) }.get();
        let (root, prime) =
            properties_animations::new_tween_trigger_root(self as *const Property<T>);
        let binding_callable = properties_animations::AnimationTrigger::<T, _> {
            original_binding: PropertyHandle {
                handle: Cell::new(
                    (alloc_binding_holder(move |val: &mut T| {
                        *val = value.clone();
                        BindingResult::KeepBinding
                    }) as *mut ())
                        .map_addr(|a| a | 0b10),
                ),
            },
            // Force the animation to start on the next evaluation (the assignment is the trigger;
            // there is no dependency change to drive `mark_dirty`). `from` is captured then as the
            // property's current value; the constant binding above supplies `to`.
            state: Cell::new(properties_animations::AnimatedBindingState::ShouldStart),
            compute_animation_details: move || (animation_data.clone(), None),
            trigger_time: Cell::new(Some(crate::animations::current_tick())),
            root,
            handle: properties_animations::AnimationHandle::default(),
            prime: Some(prime),
            on_external_set: properties_animations::SetPolicy::Cancel,
        };

        // Safety: the `AnimationTrigger`'s type matches the property type
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

impl<T: Clone + 'static> Property<T> {
    /// Install a plain root trigger: (re)start `root` whenever this property's value changes,
    /// with no endpoint capture. This is a plain restart-on-change, not tied to any
    /// interpolated value. This will be used by future Sequential/Parallel objects
    #[allow(dead_code)]
    pub(crate) fn set_root_trigger(
        &self,
        binding: impl Binding<T> + 'static,
        root: Rc<RefCell<dyn Animation>>,
    ) {
        let binding_callable = AnimationTrigger {
            original_binding: PropertyHandle {
                handle: Cell::new(
                    (alloc_binding_holder(move |val: &mut T| {
                        *val = binding.evaluate(val);
                        BindingResult::KeepBinding
                    }) as *mut ())
                        .map_addr(|a| a | 0b10),
                ),
            },
            state: Cell::new(AnimatedBindingState::NotAnimating),
            compute_animation_details: (|| (PropertyAnimation::default(), None))
                as fn() -> AnimationDetail,
            trigger_time: Cell::new(None),
            root,
            handle: AnimationHandle::default(),
            prime: None::<
                Box<
                    dyn Fn(
                        &T,
                        &T,
                        PropertyAnimation,
                        Option<crate::animations::Instant>,
                    ) -> (T, bool),
                >,
            >,
            on_external_set: SetPolicy::Retrigger,
        };

        // Safety: the `AnimationTrigger`'s type matches the property type
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

#[cfg(test)]
mod animation_tests {
    use super::*;
    use crate::items::AnimationDirection;
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

        compo.width.set_animated_value_object(200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);
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

        compo.width.set_animated_value_object(200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);
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

        compo.width.set_animated_value_object(200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);
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

        compo.width.set_animated_value_object(200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // (fractal) end of animation
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 4));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 125);
        assert_eq!(get_prop_value(&compo.width_times_two), 250);

        // End of animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);
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

        compo.width.set_animated_value_object(200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // No animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);
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

        compo.width.set_animated_value_object(200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // No animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);
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

        compo.width.set_animated_value_object(200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // No animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);
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

        compo.width.set_animated_value_object(200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In animation (again):
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + 500 * DURATION));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + DELAY + 50000 * DURATION + DURATION / 2)
        });
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);
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

        compo.width.set_animated_value_object(200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // Overshoot: Always from_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);
    }

    #[test]
    fn properties_test_animation_alternate_fractional_iteration_triggered_by_set() {
        // Iteration 0: forward 100 -> 200. Iteration 1 (odd, reversed for Alternate):
        // 200 -> 100. Iteration 2 (even, forward again) is cut short at its halfway
        // point by the 2.5 iteration_count: it must snap to *its own* direction's
        // endpoint (200, forward), not the previous iteration's (100, reversed).
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: 2.5,
            direction: AnimationDirection::Alternate,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        let start_time = crate::animations::current_tick();
        compo.width.set_animated_value_object(200, animation_details);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION * 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION * 2 + DURATION / 4));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 125);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION * 2 + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);

        // Stays there past the end.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION * 3));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
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

        compo.width.set_animated_value_object(200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 150);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 150);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION * 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
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
        compo.width.set_animated_binding_object(
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
        crate::animations::update_animation_objects();

        assert_eq!(get_prop_value(&compo.width), 150);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        crate::animations::update_animation_objects();

        assert_eq!(get_prop_value(&compo.width), 100);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));
        crate::animations::update_animation_objects();

        assert_eq!(get_prop_value(&compo.width), 150);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + 2 * DURATION));
        crate::animations::update_animation_objects();

        assert_eq!(get_prop_value(&compo.width), 200);

        // Overshoot a bit:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + 2 * DURATION + DURATION / 2));
        crate::animations::update_animation_objects();

        assert_eq!(get_prop_value(&compo.width), 200);

        // Restart the animation by setting a new value.

        let start_time = crate::animations::current_tick();

        compo.feed_property.set(300);
        assert_eq!(get_prop_value(&compo.width), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        crate::animations::update_animation_objects();

        assert_eq!(get_prop_value(&compo.width), 250);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        crate::animations::update_animation_objects();

        assert_eq!(get_prop_value(&compo.width), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));
        crate::animations::update_animation_objects();

        assert_eq!(get_prop_value(&compo.width), 250);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + 2 * DURATION));
        crate::animations::update_animation_objects();

        assert_eq!(get_prop_value(&compo.width), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + 2 * DURATION + DURATION / 2));
        crate::animations::update_animation_objects();

        assert_eq!(get_prop_value(&compo.width), 300);
    }

    // These drive the consolidated object backend: values are pushed by `update_animation_objects()`
    // each frame rather than pulled lazily, so each frame the test advances the clock
    // *and* calls `update_animation_objects()`.

    #[test]
    fn object_animation_triggered_by_binding() {
        let compo = Component::new_test_component();
        let start_time = crate::animations::current_tick();

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: 1.,
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        compo.width.set_animated_binding_object(
            move || {
                let compo = w.upgrade().unwrap();
                get_prop_value(&compo.feed_property)
            },
            move || (animation_details.clone(), None),
        );

        compo.feed_property.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // A new target value: the next `.get()` runs the ShouldStart path and registers the tween.
        compo.feed_property.set(200);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);
    }

    #[test]
    fn object_delayed_animation_triggered_by_binding() {
        let compo = Component::new_test_component();
        let start_time = crate::animations::current_tick();

        let animation_details = PropertyAnimation {
            delay: DELAY.as_millis() as _,
            duration: DURATION.as_millis() as _,
            iteration_count: 1.0,
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        compo.width.set_animated_binding_object(
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

        // Still within the delay: value stays at `from`.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);

        // Delay elapsed, animation begins.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 150);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
    }

    #[test]
    fn object_animation_external_set_cancels() {
        let compo = Component::new_test_component();
        let start_time = crate::animations::current_tick();

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: 1.,
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        compo.width.set_animated_binding_object(
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
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 150);

        // An external imperative set (guard clear) cancels the animation: the change-detector
        // binding is removed and its owned tween is deregistered.
        compo.width.set(999);
        assert_eq!(get_prop_value(&compo.width), 999);
        compo.width.handle.access(|binding| assert!(binding.is_none()));

        // Further frames don't animate the now-plain property.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 999);
    }

    #[test]
    fn object_animation_registered_late_then_completes() {
        // Mirrors tests/cases/properties/animation_bindings_reactive.slint: the value is changed,
        // time elapses *without* a read (so nothing is registered yet), then the first read
        // registers the tween mid-flight, and a further elapse must run it to completion.
        let compo = Component::new_test_component();
        let start_time = crate::animations::current_tick();

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: 1.,
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        compo.width.set_animated_binding_object(
            move || {
                let compo = w.upgrade().unwrap();
                get_prop_value(&compo.feed_property)
            },
            move || (animation_details.clone(), None),
        );

        compo.feed_property.set(0);
        assert_eq!(get_prop_value(&compo.width), 0);

        // Change, then elapse *without* reading (nothing registered yet).
        compo.feed_property.set(100);
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        crate::animations::update_animation_objects();

        // First read registers the tween mid-flight (anchored at the change instant).
        assert_eq!(get_prop_value(&compo.width), 50);

        // A further frame must run it to completion.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 100);
    }

    #[test]
    fn object_animation_transition_start_time() {
        // A state-transition-style animation whose start_time is in the past: at trigger it is
        // already partway through, exactly as the `Option<Instant>` start_time is meant to allow.
        let compo = Component::new_test_component();
        // Advance the clock to a positive base first so a past start_time doesn't underflow Instant.
        let base = crate::animations::current_tick();
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(base + DURATION));
        let start_time = crate::animations::current_tick();

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: 1.,
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        let details_clone = animation_details.clone();
        compo.width.set_animated_binding_object(
            move || {
                let compo = w.upgrade().unwrap();
                get_prop_value(&compo.feed_property)
            },
            move || (details_clone.clone(), Some(start_time - DURATION / 2)),
        );

        compo.feed_property.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);

        // Trigger: start_time is half a duration in the past, so it snaps straight to the midpoint.
        compo.feed_property.set(200);
        assert_eq!(get_prop_value(&compo.width), 150);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        crate::animations::update_animation_objects();
        assert_eq!(get_prop_value(&compo.width), 200);
    }
}
