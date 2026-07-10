// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use alloc::vec::Vec;
use alloc::boxed::Box;
use core::num::NonZeroUsize;
use core::cell::RefCell;
use crate::{
    animations::physics_simulation::{self, Simulation},
    items::{AnimationDirection, PropertyAnimation},
    lengths::LogicalLength,
};
use euclid::Length;
#[cfg(not(feature = "std"))]
use num_traits::Float;

crate::thread_local!(static CURRENT_COMPOSITE_ANIMATIONS: RefCell<slab::Slab<Box<dyn Animation>>> = RefCell::default());

/// Base trait for all animation objects
pub trait Animation {
    /// Start the animation
    fn start(&mut self);
    /// Stop the animation
    fn stop(&mut self);
    /// Restart the animation from the beginning
    fn restart(&mut self);
    /// Check if the animation is currently running
    fn is_running(&self) -> bool;
    /// Advance the animation state by one frame: a tween updates its target property,
    /// a composite (sequential/parallel) updates its children. Returns true if the
    /// animation is still running.
    /// Default implementation returns `is_running()` with no state change.
    fn update(&mut self) -> bool {
        self.is_running()
    }
}

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
    running: bool,
}

impl<S> PropertyPhysicsAnimationData<S>
where
    S: physics_simulation::Simulation,
{
    pub fn new(simulation: S) -> PropertyPhysicsAnimationData<S> {
        PropertyPhysicsAnimationData { simulation, state: AnimationState::Delaying, running: true }
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

impl<S: physics_simulation::Simulation> Animation for PropertyPhysicsAnimationData<S> {
    fn start(&mut self) {
        self.running = true;
        if matches!(self.state, AnimationState::Done { .. }) {
            self.state = AnimationState::Delaying;
        }
    }

    fn stop(&mut self) {
        self.running = false;
    }

    fn restart(&mut self) {
        self.state = AnimationState::Delaying;
        self.running = true;
    }

    fn is_running(&self) -> bool {
        self.running && matches!(self.state, AnimationState::Animating { .. })
    }
}

/// A tween animation that interpolates a value from one state to another.
///
/// This is the "data" half of the tween's handle+data pattern: codegen holds a
/// [`CompositeAnimationHandle`] field and, on each frame where the Slint `running`
/// property is true, builds a fresh `TweenAnimation` (via
/// [`new_with_callbacks`](Self::new_with_callbacks)) and hands it to
/// [`CompositeAnimationHandle::start`]/[`restart`](CompositeAnimationHandle::restart).
/// `set_value`/`on_finished` are only populated on that path; the other two use sites
/// ([`AnimatedBindingCallable`] and `Property::set_animated_value`) call
/// `compute_interpolated_value()` directly and leave them `None`.
pub struct TweenAnimation<T> {
    from_value: T,
    to_value: T,
    details: PropertyAnimation,
    start_time: crate::animations::Instant,
    state: AnimationState,
    running: bool,
    set_value: Option<Box<dyn FnMut(T)>>,
    on_finished: Option<Box<dyn FnMut()>>,
}

impl<T: InterpolatedPropertyValue + Clone> TweenAnimation<T> {
    /// Creates a new tween interpolating `from_value` to `to_value` according to `details`.
    pub fn new(from_value: T, to_value: T, details: PropertyAnimation) -> Self {
        let start_time = crate::animations::current_tick();

        Self {
            from_value,
            to_value,
            details,
            start_time,
            state: AnimationState::Delaying,
            running: true,
            set_value: None,
            on_finished: None,
        }
    }

    /// Same as [`Self::new`], but additionally pushes each freshly computed value into
    /// `set_value` once per frame, and invokes `on_finished` exactly once when the
    /// animation completes on its own (not on an explicit [`Animation::stop`]).
    pub fn new_with_callbacks(
        from_value: T,
        to_value: T,
        details: PropertyAnimation,
        set_value: impl FnMut(T) + 'static,
        on_finished: impl FnMut() + 'static,
    ) -> Self {
        Self {
            set_value: Some(Box::new(set_value)),
            on_finished: Some(Box::new(on_finished)),
            ..Self::new(from_value, to_value, details)
        }
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
                    // `current_iteration` is `floor(total_elapsed / duration)`. When
                    // `time_progress` (the remainder) is zero, the total elapsed time lands
                    // exactly on an iteration boundary, so `current_iteration` actually names
                    // the iteration about to start rather than the one just completed -- back
                    // up by one to get the direction of the iteration that just finished.
                    // Otherwise (a fractional `iteration_count` truncating mid-iteration),
                    // `current_iteration` already names the iteration being cut short.
                    let finished_iteration = if time_progress == 0 {
                        current_iteration.max(1) - 1
                    } else {
                        current_iteration
                    };
                    self.state = AnimationState::Done { iteration_count: finished_iteration };
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

impl<T: InterpolatedPropertyValue + Clone> Animation for TweenAnimation<T> {
    fn start(&mut self) {
        self.running = true;
        if matches!(self.state, AnimationState::Done { .. }) {
            self.reset();
        }
    }

    fn stop(&mut self) {
        self.running = false;
    }

    fn restart(&mut self) {
        self.reset();
        self.running = true;
    }

    fn is_running(&self) -> bool {
        self.running && matches!(self.state, AnimationState::Animating { .. })
    }

    fn update(&mut self) -> bool {
        let (value, finished) = self.compute_interpolated_value();
        if let Some(set_value) = self.set_value.as_mut() {
            (set_value)(value);
        }
        if finished {
            if let Some(mut on_finished) = self.on_finished.take() {
                on_finished();
            }
        } else {
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.set_has_active_animations());
        }
        !finished
    }
}

// TODO remove
#[cfg(feature = "ffi")]
pub(super) type PropertyValueAnimationData<T> = TweenAnimation<T>;

/// Delays before starting the next animation
pub struct DelayAnimation {
    delay_ms: u64,
    start_time: crate::animations::Instant,
    running: bool,
}

impl DelayAnimation {
    /// Creates a new delay of `delay_ms` milliseconds, starting now.
    pub fn new(delay_ms: u64) -> Self {
        Self { delay_ms, start_time: crate::animations::current_tick(), running: true }
    }

    /// Returns true once `delay_ms` has elapsed since the delay started (or restarted).
    pub fn is_finished(&self) -> bool {
        let elapsed = crate::animations::current_tick().duration_since(self.start_time);
        elapsed.as_millis() as u64 >= self.delay_ms
    }
}

impl Animation for DelayAnimation {
    fn start(&mut self) {
        self.running = true;
    }

    fn stop(&mut self) {
        self.running = false;
    }

    fn restart(&mut self) {
        self.start_time = crate::animations::current_tick();
        self.running = true;
    }

    fn is_running(&self) -> bool {
        self.running && !self.is_finished()
    }

    fn update(&mut self) -> bool {
        let running = self.is_running();
        if running {
            // Nothing to compute while delaying, but the frame loop must keep
            // updating us so `is_finished()` gets observed once the delay elapses.
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.set_has_active_animations());
        }
        running
    }
}

/// Runs animations sequentially, one after another
pub struct SequentialAnimation {
    animations: Vec<Box<dyn Animation>>,
    current_index: usize,
    running: bool,
}

impl SequentialAnimation {
    /// Creates an empty sequence of animations.
    pub fn new() -> Self {
        Self { animations: Vec::new(), current_index: 0, running: true }
    }

    /// Appends `animation` to the end of the sequence.
    pub fn add_animation(&mut self, animation: Box<dyn Animation>) {
        self.animations.push(animation);
    }

    /// Returns the animation currently being run, if any.
    pub fn current_animation_mut(&mut self) -> Option<&mut Box<dyn Animation>> {
        self.animations.get_mut(self.current_index)
    }

    /// Advances to and restarts the next animation in the sequence, if any remain.
    pub fn advance_to_next(&mut self) {
        self.current_index += 1;
        if self.current_index < self.animations.len() {
            if let Some(anim) = self.current_animation_mut() {
                // `restart()`, not `start()`: children are constructed up front (e.g.
                // a tween queued behind a delay) but only actually activated once
                // their turn comes, potentially much later, so their clock must be
                // reset to "now" rather than keep counting from construction time.
                anim.restart();
            }
        }
    }

    /// Returns true once every animation in the sequence has run to completion.
    pub fn is_finished(&self) -> bool {
        self.current_index >= self.animations.len()
    }
}

impl Default for SequentialAnimation {
    fn default() -> Self {
        Self::new()
    }
}

impl Animation for SequentialAnimation {
    fn start(&mut self) {
        self.running = true;
        if !self.animations.is_empty() && self.current_index == 0 {
            // See `advance_to_next()`: `restart()` gives the first child a fresh
            // clock, covering any gap between when it was constructed and now.
            self.animations[0].restart();
        }
    }

    fn stop(&mut self) {
        self.running = false;
        for anim in &mut self.animations {
            anim.stop();
        }
    }

    fn restart(&mut self) {
        self.current_index = 0;
        self.running = true;
        for anim in &mut self.animations {
            anim.restart();
        }
    }

    fn is_running(&self) -> bool {
        self.running && !self.is_finished()
    }

    fn update(&mut self) -> bool {
        if !self.running {
            return false;
        }
        // `update()`, not `is_running()`: children like a tween only advance their own
        // state (and push their interpolated value) from inside `update()`. Loop so that
        // a child finishing (e.g. a DelayAnimation elapsing) advances to and updates the
        // next child within the same frame, instead of leaving it un-updated until the
        // next call and visibly lagging a frame behind.
        while let Some(current_anim) = self.animations.get_mut(self.current_index) {
            if current_anim.update() {
                break;
            }
            self.advance_to_next();
        }
        self.is_running()
    }
}

/// Runs animations in parallel
pub struct ParallelAnimation {
    animations: Vec<Box<dyn Animation>>,
    running: bool,
}

impl ParallelAnimation {
    /// Creates an empty group of animations to run in parallel.
    pub fn new() -> Self {
        Self { animations: Vec::new(), running: true }
    }

    /// Adds `animation` to the group.
    pub fn add_animation(&mut self, animation: Box<dyn Animation>) {
        self.animations.push(animation);
    }

    /// Returns true once every animation in the group has finished running.
    pub fn all_finished(&self) -> bool {
        self.animations.is_empty() || self.animations.iter().all(|a| !a.is_running())
    }
}

impl Default for ParallelAnimation {
    fn default() -> Self {
        Self::new()
    }
}

impl Animation for ParallelAnimation {
    fn start(&mut self) {
        self.running = true;
        for anim in &mut self.animations {
            anim.start();
        }
    }

    fn stop(&mut self) {
        self.running = false;
        for anim in &mut self.animations {
            anim.stop();
        }
    }

    fn restart(&mut self) {
        for anim in &mut self.animations {
            anim.restart();
        }
        self.running = true;
    }

    fn is_running(&self) -> bool {
        self.running && !self.all_finished()
    }

    fn update(&mut self) -> bool {
        if !self.running {
            return false;
        }
        for anim in &mut self.animations {
            anim.update();
        }
        self.is_running()
    }
}

/// Handle to a composite animation (Sequential/Parallel/Delay).
/// Analogous to `crate::timers::Timer`, this is a lightweight id-holding handle
/// that the codegen can store as a component field.
#[derive(Default)]
pub struct CompositeAnimationHandle {
    id: core::cell::Cell<Option<NonZeroUsize>>,
    _phantom: core::marker::PhantomData<*mut ()>,
}

impl CompositeAnimationHandle {
    /// Register a new composite animation in the global registry.
    pub fn register(animation: Box<dyn Animation>) -> Self {
        let id = CURRENT_COMPOSITE_ANIMATIONS.with(|anims| {
            let mut anims = anims.borrow_mut();
            NonZeroUsize::new(anims.insert(animation) + 1).expect("slab index too large")
        });
        Self { id: core::cell::Cell::new(Some(id)), _phantom: core::marker::PhantomData }
    }

    /// Start driving `animation`. No-op if something is already running on this
    /// handle: codegen re-runs the whole `update_animations()` block whenever *any*
    /// animation's `running` property changes, so a still-running animation would
    /// otherwise be restarted (its clock reset) every time a sibling animation starts
    /// or finishes. Use [`restart`](Self::restart) to force a running animation back
    /// to the beginning.
    pub fn start(&self, animation: Box<dyn Animation>) {
        if self.is_running() {
            return;
        }
        self.replace(animation);
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.set_has_active_animations());
    }

    /// Force `animation` to (re)start from the beginning, even if something is
    /// already running on this handle (unlike [`start`](Self::start), which is a
    /// no-op in that case).
    pub fn restart(&self, animation: Box<dyn Animation>) {
        self.clear();
        self.start(animation);
    }

    /// Stop and deregister whatever's running, freezing the target property at its
    /// current value.
    pub fn stop(&self) {
        self.clear();
    }

    /// Check if the composite animation is running.
    pub fn is_running(&self) -> bool {
        if let Some(id) = self.id.get() {
            CURRENT_COMPOSITE_ANIMATIONS.with(|anims| {
                anims.borrow().get(id.get() - 1).map(|a| a.is_running()).unwrap_or(false)
            })
        } else {
            false
        }
    }

    /// Remove any previously registered animation and register `animation` in its place.
    pub fn replace(&self, animation: Box<dyn Animation>) {
        self.clear();
        let id = CURRENT_COMPOSITE_ANIMATIONS.with(|anims| {
            let mut anims = anims.borrow_mut();
            NonZeroUsize::new(anims.insert(animation) + 1).expect("slab index too large")
        });
        self.id.set(Some(id));
    }

    /// Deregister the animation, if any. Leaves the handle empty.
    pub fn clear(&self) {
        if let Some(id) = self.id.take() {
            CURRENT_COMPOSITE_ANIMATIONS.with(|anims| {
                let _ = anims.borrow_mut().try_remove(id.get() - 1);
            });
        }
    }
}

impl Drop for CompositeAnimationHandle {
    fn drop(&mut self) {
        if let Some(id) = self.id.get() {
            CURRENT_COMPOSITE_ANIMATIONS.with(|anims| {
                let _ = anims.borrow_mut().try_remove(id.get() - 1);
            });
        }
    }
}

/// Update all active composite animations by one tick.
/// This should be called once per frame, similar to `crate::timers::TimerList::maybe_activate_timers`.
pub fn update_composite_animations() {
    CURRENT_COMPOSITE_ANIMATIONS.with(|anims| {
        let mut finished_ids = Vec::new();
        {
            let mut anims_mut = anims.borrow_mut();
            for (id, anim) in anims_mut.iter_mut() {
                if !anim.update() {
                    finished_ids.push(id);
                }
            }
        }
        // Remove finished animations
        let mut anims_mut = anims.borrow_mut();
        for id in finished_ids {
            let _ = anims_mut.try_remove(id);
        }
    });
}


#[cfg(test)]
mod animation_architecture_tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn test_sequential_animation_structure() {
        let mut seq = SequentialAnimation::new();
        seq.add_animation(Box::new(DelayAnimation::new(100)));

        assert_eq!(seq.animations.len(), 1);
        assert!(!seq.is_finished());

        seq.start();
        assert!(seq.is_running());
    }

    #[test]
    fn test_sequential_delay_then_tween_advances_on_tick() {
        // A DelayAnimation followed by a tween: update() must drive the currently-active
        // child's own state (not just poll is_running()), otherwise the tween would
        // never advance past its initial value.
        let observed = Rc::new(RefCell::new(Vec::new()));
        let observed_clone = observed.clone();

        let start_time = crate::animations::current_tick();

        let mut seq = SequentialAnimation::new();
        seq.add_animation(Box::new(DelayAnimation::new(100)));
        seq.add_animation(Box::new(TweenAnimation::new_with_callbacks(
            0i32,
            100i32,
            PropertyAnimation { duration: 200, ..Default::default() },
            move |v: i32| observed_clone.borrow_mut().push(v),
            || {},
        )));
        seq.start();

        // Still delaying: the tween hasn't started, nothing observed yet.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + core::time::Duration::from_millis(50)));
        assert!(seq.update());
        assert!(observed.borrow().is_empty());

        // The delay elapses: the tween is activated (with a freshly-reset clock,
        // hence progress 0) and ticked within this same call.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + core::time::Duration::from_millis(100)));
        assert!(seq.update());
        assert_eq!(*observed.borrow().last().unwrap(), 0);

        // 100ms into the tween's own (200ms) duration: halfway.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + core::time::Duration::from_millis(200)));
        assert!(seq.update());
        assert_eq!(*observed.borrow().last().unwrap(), 50);

        // Past the tween's duration: finished, sequence reports not-running.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + core::time::Duration::from_millis(300)));
        assert!(!seq.update());
        assert_eq!(*observed.borrow().last().unwrap(), 100);
    }

    #[test]
    fn test_composite_handle_start_restart_tween() {
        // Mirrors how codegen drives a tween: a CompositeAnimationHandle field (the
        // "handle") and, per frame, a freshly-built TweenAnimation (the "data") handed
        // to start()/restart().
        let observed = Rc::new(RefCell::new(Vec::new()));
        let observed_clone = observed.clone();
        let finished = Rc::new(core::cell::Cell::new(false));
        let finished_clone = finished.clone();

        let start_time = crate::animations::current_tick();
        let handle = CompositeAnimationHandle::default();

        let tween = TweenAnimation::new_with_callbacks(
            0i32,
            100i32,
            PropertyAnimation { duration: 200, ..Default::default() },
            move |v: i32| observed_clone.borrow_mut().push(v),
            move || finished_clone.set(true),
        );
        handle.start(Box::new(tween));

        // First tick: moves the tween past its (zero) delay into Animating, at which
        // point is_running() reports true.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time));
        crate::animations::update_composite_animations();
        assert!(handle.is_running());

        // A second start() while already running is a no-op: codegen relies on this to
        // avoid resetting the clock every time a sibling animation's `running` changes.
        let no_op_tween = TweenAnimation::new_with_callbacks(
            0i32,
            999i32,
            PropertyAnimation { duration: 200, ..Default::default() },
            |_: i32| panic!("start() must not replace a still-running animation"),
            || {},
        );
        handle.start(Box::new(no_op_tween));

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + core::time::Duration::from_millis(100)));
        crate::animations::update_composite_animations();
        assert_eq!(*observed.borrow().last().unwrap(), 50);

        // restart() forces a fresh tween in, even though the handle is still running.
        let observed_clone = observed.clone();
        let finished_clone = finished.clone();
        let restarted_tween = TweenAnimation::new_with_callbacks(
            0i32,
            100i32,
            PropertyAnimation { duration: 200, ..Default::default() },
            move |v: i32| observed_clone.borrow_mut().push(v),
            move || finished_clone.set(true),
        );
        handle.restart(Box::new(restarted_tween));

        let start_time = crate::animations::current_tick();
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + core::time::Duration::from_millis(200)));
        crate::animations::update_composite_animations();
        assert_eq!(*observed.borrow().last().unwrap(), 100);
        assert!(finished.get());
        assert!(!handle.is_running());
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
    pub(super) animation_data: RefCell<TweenAnimation<T>>,
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
    pub fn set_animated_value(&self, value: T, animation_data: PropertyAnimation) {
        // FIXME if the current value is a dirty binding, we must run it, but we do not have the context
        let d = RefCell::new(properties_animations::TweenAnimation::new(
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
            animation_data: RefCell::new(properties_animations::TweenAnimation::new(
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
    pub fn set_physic_animation_value<
        S: physics_simulation::Simulation + 'static,
        AD: physics_simulation::Parameter<Output = S>,
    >(
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
        compo.width.set_animated_value(200, animation_details);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        assert_eq!(get_prop_value(&compo.width), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION * 2));
        assert_eq!(get_prop_value(&compo.width), 100);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION * 2 + DURATION / 4));
        assert_eq!(get_prop_value(&compo.width), 125);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION * 2 + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);

        // Stays there past the end.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION * 3));
        assert_eq!(get_prop_value(&compo.width), 200);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
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
}
