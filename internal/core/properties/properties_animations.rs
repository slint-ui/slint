// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use alloc::vec::Vec;
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::num::NonZeroUsize;
use core::cell::RefCell;
use crate::{
    animations::physics_simulation,
    items::{AnimationDirection, PropertyAnimation},
    lengths::LogicalLength,
};
#[cfg(not(feature = "std"))]
use num_traits::Float;

/// Global registry of live animation objects, keyed by a **monotonically increasing** id that is
/// never reused. Ids must never be reused because an [`AnimationHandle`] can outlive its entry:
/// when `update_animation_objects` drops a finished animation the handle keeps its id, and if that
/// id were later handed to a *different* animation the stale handle's `clear`/`Drop` would remove
/// the wrong one. A monotonic key makes a stale id simply refer to an absent entry (a no-op).
#[derive(Default)]
struct AnimationRegistry {
    next_id: usize,
    animations: alloc::collections::BTreeMap<usize, Box<dyn Animation>>,
}

impl AnimationRegistry {
    /// Insert `animation` under a fresh, never-reused id (always `>= 1`).
    fn insert(&mut self, animation: Box<dyn Animation>) -> NonZeroUsize {
        self.next_id = self.next_id.checked_add(1).expect("animation id overflow");
        let id = NonZeroUsize::new(self.next_id).unwrap();
        self.animations.insert(id.get(), animation);
        id
    }
}

crate::thread_local!(static CURRENT_ANIMATIONS: RefCell<AnimationRegistry> = RefCell::default());

crate::thread_local!(
    /// Set while an object-driven animated binding is pushing its interpolated value back into
    /// its own target property via `Property::set`. `AnimatedBindingObjectCallable::intercept_set`
    /// reads this so that the animation's own writes keep the (change-detector) binding installed,
    /// while an *external* write (guard clear) still removes it — preserving the legacy
    /// "an imperative set cancels the animated binding" semantics.
    static APPLYING_ANIMATION: Cell<bool> = const { Cell::new(false) }
);

/// Run `f` with [`APPLYING_ANIMATION`] set, so writes it performs on an animated property are
/// treated as self-writes (the binding is kept) rather than external overrides.
fn with_applying_animation<R>(f: impl FnOnce() -> R) -> R {
    APPLYING_ANIMATION.with(|g| {
        let previous = g.replace(true);
        let r = f();
        g.set(previous);
        r
    })
}

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

/// A tween animation that interpolates a value from one state to another.
///
/// This is the "data" half of the tween's handle+data pattern: codegen holds a
/// [`AnimationHandle`] field and, on each frame where the Slint `running`
/// property is true, builds a fresh `TweenAnimation` (via
/// [`new_with_callbacks`](Self::new_with_callbacks)) and hands it to
/// [`AnimationHandle::start`]/[`restart`](AnimationHandle::restart).
/// `set_value`/`on_finished` are only populated on that path; [`AnimatedBindingObjectCallable`]
/// drives its tween itself and calls `compute_interpolated_value()` directly, leaving them `None`.
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
        // Not just `Animating`: a freshly registered tween sits in `Delaying` (never yet
        // ticked by `update()`) but is very much active. Codegen's `AnimationHandle::start`
        // relies on this to no-op a redundant `start()` on a tween that hasn't ticked even
        // once yet -- e.g. one registered synchronously by an explicit `.start()` call, then
        // immediately redundantly re-triggered by the `running` property's own (deferred)
        // change tracker on the next driver tick. Treating `Delaying` as not-running would let
        // that second call replace it with a freshly time-stamped tween, silently resetting
        // progress that was never actually observed as reset.
        self.running && !matches!(self.state, AnimationState::Done { .. })
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

/// An animation object driven by a physics [`Simulation`](physics_simulation::Simulation)
/// (e.g. constant-deceleration friction or a spring-damper) rather than by tween interpolation.
///
/// Like [`TweenAnimation`], it is registered in the `CURRENT_ANIMATIONS` registry (via an
/// [`AnimationHandle`]) and, on each frame, steps its simulation and *pushes* the resulting value
/// into the target property through `set_value`. `Flickable` uses it for kinetic scrolling.
///
/// Unlike a tween, the simulation integrates *in place*: each frame it reads the target's current
/// value via `get_value`, advances it, and writes it back through `set_value`. Reading the live
/// value (rather than an internally-cached one) means an external write to the target between
/// frames — e.g. a `ListView` re-adjusting `viewport-y` after refining its item-height estimates —
/// is picked up and the simulation continues smoothly from there instead of snapping back to the
/// trajectory it would otherwise have followed. This mirrors the legacy pull-based physics binding,
/// which integrated on the property's own cell value.
pub struct PhysicsAnimation<S> {
    simulation: S,
    running: bool,
    finished: bool,
    /// Reads the target property's current value at the start of each frame; the simulation is
    /// advanced from this value (see the type-level note on in-place integration).
    get_value: Box<dyn FnMut() -> crate::Coord>,
    /// Pushes each freshly computed value into the target property (once per frame).
    set_value: Box<dyn FnMut(crate::Coord)>,
}

impl<S: physics_simulation::Simulation> PhysicsAnimation<S> {
    /// Creates a physics animation stepping `simulation`, reading the target's current value each
    /// frame via `get_value` and pushing the advanced value back through `set_value`.
    pub fn new(
        simulation: S,
        get_value: impl FnMut() -> crate::Coord + 'static,
        set_value: impl FnMut(crate::Coord) + 'static,
    ) -> Self {
        Self {
            simulation,
            running: true,
            finished: false,
            get_value: Box::new(get_value),
            set_value: Box::new(set_value),
        }
    }
}

impl<S: physics_simulation::Simulation> Animation for PhysicsAnimation<S> {
    fn start(&mut self) {
        self.running = true;
    }

    fn stop(&mut self) {
        self.running = false;
    }

    fn restart(&mut self) {
        self.running = true;
        self.finished = false;
    }

    fn is_running(&self) -> bool {
        self.running && !self.finished
    }

    fn update(&mut self) -> bool {
        if !self.running {
            return false;
        }
        // Integrate in place on the target's *live* value: read it now, step, and write it back,
        // so an external adjustment since the last frame is carried forward (see the type note).
        // The simulation works in `f32`; adapt to/from `Coord`.
        let mut value = (self.get_value)() as f32;
        let finished = self.simulation.step(&mut value, crate::animations::current_tick());
        let value = value as crate::Coord;
        // Push with the self-write guard so that, should the target ever carry a competing
        // (change-detector) binding, this write is treated as a self-write. `Flickable`'s viewport
        // carries no binding while flicking, but this keeps the push consistent with the tween path.
        with_applying_animation(|| (self.set_value)(value));
        if finished {
            self.finished = true;
            false
        } else {
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.set_has_active_animations());
            true
        }
    }
}

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

/// Handle to a registered animation object (e.g. `TweenAnimation`).
/// Analogous to `crate::timers::Timer`, this is a lightweight id-holding handle
/// that the codegen can store as a component field.
#[derive(Default)]
pub struct AnimationHandle {
    id: core::cell::Cell<Option<NonZeroUsize>>,
    _phantom: core::marker::PhantomData<*mut ()>,
}

impl AnimationHandle {
    /// Register a new animation object in the global registry.
    pub fn register(animation: Box<dyn Animation>) -> Self {
        let id = CURRENT_ANIMATIONS.with(|anims| anims.borrow_mut().insert(animation));
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

    /// Check if the animation is running.
    pub fn is_running(&self) -> bool {
        if let Some(id) = self.id.get() {
            CURRENT_ANIMATIONS.with(|anims| {
                anims.borrow().animations.get(&id.get()).map(|a| a.is_running()).unwrap_or(false)
            })
        } else {
            false
        }
    }

    /// Remove any previously registered animation and register `animation` in its place.
    pub fn replace(&self, animation: Box<dyn Animation>) {
        self.clear();
        let id = CURRENT_ANIMATIONS.with(|anims| anims.borrow_mut().insert(animation));
        self.id.set(Some(id));
    }

    /// Deregister the animation, if any. Leaves the handle empty.
    pub fn clear(&self) {
        if let Some(id) = self.id.take() {
            CURRENT_ANIMATIONS.with(|anims| {
                anims.borrow_mut().animations.remove(&id.get());
            });
        }
    }
}

impl Drop for AnimationHandle {
    fn drop(&mut self) {
        if let Some(id) = self.id.get() {
            CURRENT_ANIMATIONS.with(|anims| {
                anims.borrow_mut().animations.remove(&id.get());
            });
        }
    }
}

/// Update all active animation objects by one tick.
/// This should be called once per frame, similar to `crate::timers::TimerList::maybe_activate_timers`.
pub fn update_animation_objects() {
    CURRENT_ANIMATIONS.with(|anims| {
        let mut finished_ids = Vec::new();
        {
            let mut anims_mut = anims.borrow_mut();
            for (id, anim) in anims_mut.animations.iter_mut() {
                if !anim.update() {
                    finished_ids.push(*id);
                }
            }
        }
        // Remove finished animations
        let mut anims_mut = anims.borrow_mut();
        for id in finished_ids {
            anims_mut.animations.remove(&id);
        }
    });
}

#[cfg(feature = "ffi")]
pub(crate) mod animation_object_ffi {
    #![allow(unsafe_code)]

    use super::*;
    use core::cell::Cell;
    use core::ffi::c_void;

    struct WrapFn {
        callback: extern "C" fn(*mut c_void),
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    }

    impl Drop for WrapFn {
        fn drop(&mut self) {
            if let Some(x) = self.drop_user_data {
                x(self.user_data)
            }
        }
    }

    impl WrapFn {
        fn call(&self) {
            (self.callback)(self.user_data)
        }
    }

    struct SetValueWrapFn<T> {
        callback: extern "C" fn(*mut c_void, *const T),
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    }

    impl<T> Drop for SetValueWrapFn<T> {
        fn drop(&mut self) {
            if let Some(x) = self.drop_user_data {
                x(self.user_data)
            }
        }
    }

    impl<T> SetValueWrapFn<T> {
        fn call(&self, value: T) {
            (self.callback)(self.user_data, &value as *const T)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn animation_handle_start_impl<T: InterpolatedPropertyValue + Clone + 'static>(
        id: usize,
        from: T,
        to: T,
        details: PropertyAnimation,
        set_value: extern "C" fn(*mut c_void, *const T),
        set_value_user_data: *mut c_void,
        set_value_drop_user_data: Option<extern "C" fn(*mut c_void)>,
        on_finished: extern "C" fn(*mut c_void),
        on_finished_user_data: *mut c_void,
        on_finished_drop_user_data: Option<extern "C" fn(*mut c_void)>,
        restart: bool,
    ) -> usize {
        let set_value_wrap = SetValueWrapFn {
            callback: set_value,
            user_data: set_value_user_data,
            drop_user_data: set_value_drop_user_data,
        };
        let on_finished_wrap = WrapFn {
            callback: on_finished,
            user_data: on_finished_user_data,
            drop_user_data: on_finished_drop_user_data,
        };
        let tween = TweenAnimation::new_with_callbacks(
            from,
            to,
            details,
            move |value| set_value_wrap.call(value),
            move || on_finished_wrap.call(),
        );
        let handle = AnimationHandle::default();
        if id != 0 {
            handle.id.set(NonZeroUsize::new(id));
        }
        if restart {
            handle.restart(Box::new(tween));
        } else {
            handle.start(Box::new(tween));
        }
        handle.id.take().map(usize::from).unwrap_or(0)
    }

    // cbindgen does not expand macros, so the 8 monomorphized start/restart functions below
    // are written out explicitly rather than generated via macro_rules!.

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_animation_handle_start_int(
        id: usize,
        from: i32,
        to: i32,
        details: &PropertyAnimation,
        set_value: extern "C" fn(*mut c_void, *const i32),
        set_value_user_data: *mut c_void,
        set_value_drop_user_data: Option<extern "C" fn(*mut c_void)>,
        on_finished: extern "C" fn(*mut c_void),
        on_finished_user_data: *mut c_void,
        on_finished_drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) -> usize {
        animation_handle_start_impl(
            id,
            from,
            to,
            details.clone(),
            set_value,
            set_value_user_data,
            set_value_drop_user_data,
            on_finished,
            on_finished_user_data,
            on_finished_drop_user_data,
            false,
        )
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_animation_handle_restart_int(
        id: usize,
        from: i32,
        to: i32,
        details: &PropertyAnimation,
        set_value: extern "C" fn(*mut c_void, *const i32),
        set_value_user_data: *mut c_void,
        set_value_drop_user_data: Option<extern "C" fn(*mut c_void)>,
        on_finished: extern "C" fn(*mut c_void),
        on_finished_user_data: *mut c_void,
        on_finished_drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) -> usize {
        animation_handle_start_impl(
            id,
            from,
            to,
            details.clone(),
            set_value,
            set_value_user_data,
            set_value_drop_user_data,
            on_finished,
            on_finished_user_data,
            on_finished_drop_user_data,
            true,
        )
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_animation_handle_start_float(
        id: usize,
        from: f32,
        to: f32,
        details: &PropertyAnimation,
        set_value: extern "C" fn(*mut c_void, *const f32),
        set_value_user_data: *mut c_void,
        set_value_drop_user_data: Option<extern "C" fn(*mut c_void)>,
        on_finished: extern "C" fn(*mut c_void),
        on_finished_user_data: *mut c_void,
        on_finished_drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) -> usize {
        animation_handle_start_impl(
            id,
            from,
            to,
            details.clone(),
            set_value,
            set_value_user_data,
            set_value_drop_user_data,
            on_finished,
            on_finished_user_data,
            on_finished_drop_user_data,
            false,
        )
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_animation_handle_restart_float(
        id: usize,
        from: f32,
        to: f32,
        details: &PropertyAnimation,
        set_value: extern "C" fn(*mut c_void, *const f32),
        set_value_user_data: *mut c_void,
        set_value_drop_user_data: Option<extern "C" fn(*mut c_void)>,
        on_finished: extern "C" fn(*mut c_void),
        on_finished_user_data: *mut c_void,
        on_finished_drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) -> usize {
        animation_handle_start_impl(
            id,
            from,
            to,
            details.clone(),
            set_value,
            set_value_user_data,
            set_value_drop_user_data,
            on_finished,
            on_finished_user_data,
            on_finished_drop_user_data,
            true,
        )
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_animation_handle_start_color(
        id: usize,
        from: crate::Color,
        to: crate::Color,
        details: &PropertyAnimation,
        set_value: extern "C" fn(*mut c_void, *const crate::Color),
        set_value_user_data: *mut c_void,
        set_value_drop_user_data: Option<extern "C" fn(*mut c_void)>,
        on_finished: extern "C" fn(*mut c_void),
        on_finished_user_data: *mut c_void,
        on_finished_drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) -> usize {
        animation_handle_start_impl(
            id,
            from,
            to,
            details.clone(),
            set_value,
            set_value_user_data,
            set_value_drop_user_data,
            on_finished,
            on_finished_user_data,
            on_finished_drop_user_data,
            false,
        )
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_animation_handle_restart_color(
        id: usize,
        from: crate::Color,
        to: crate::Color,
        details: &PropertyAnimation,
        set_value: extern "C" fn(*mut c_void, *const crate::Color),
        set_value_user_data: *mut c_void,
        set_value_drop_user_data: Option<extern "C" fn(*mut c_void)>,
        on_finished: extern "C" fn(*mut c_void),
        on_finished_user_data: *mut c_void,
        on_finished_drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) -> usize {
        animation_handle_start_impl(
            id,
            from,
            to,
            details.clone(),
            set_value,
            set_value_user_data,
            set_value_drop_user_data,
            on_finished,
            on_finished_user_data,
            on_finished_drop_user_data,
            true,
        )
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_animation_handle_start_brush(
        id: usize,
        from: crate::Brush,
        to: crate::Brush,
        details: &PropertyAnimation,
        set_value: extern "C" fn(*mut c_void, *const crate::Brush),
        set_value_user_data: *mut c_void,
        set_value_drop_user_data: Option<extern "C" fn(*mut c_void)>,
        on_finished: extern "C" fn(*mut c_void),
        on_finished_user_data: *mut c_void,
        on_finished_drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) -> usize {
        animation_handle_start_impl(
            id,
            from,
            to,
            details.clone(),
            set_value,
            set_value_user_data,
            set_value_drop_user_data,
            on_finished,
            on_finished_user_data,
            on_finished_drop_user_data,
            false,
        )
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_animation_handle_restart_brush(
        id: usize,
        from: crate::Brush,
        to: crate::Brush,
        details: &PropertyAnimation,
        set_value: extern "C" fn(*mut c_void, *const crate::Brush),
        set_value_user_data: *mut c_void,
        set_value_drop_user_data: Option<extern "C" fn(*mut c_void)>,
        on_finished: extern "C" fn(*mut c_void),
        on_finished_user_data: *mut c_void,
        on_finished_drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) -> usize {
        animation_handle_start_impl(
            id,
            from,
            to,
            details.clone(),
            set_value,
            set_value_user_data,
            set_value_drop_user_data,
            on_finished,
            on_finished_user_data,
            on_finished_drop_user_data,
            true,
        )
    }

    /// Stop and deregister whatever animation is running on this handle.
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_animation_handle_stop(id: usize) {
        if id == 0 {
            return;
        }
        let handle =
            AnimationHandle { id: Cell::new(NonZeroUsize::new(id)), _phantom: Default::default() };
        handle.stop();
        handle.id.take();
    }

    /// Returns true if the animation on this handle is running.
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_animation_handle_is_running(id: usize) -> bool {
        if id == 0 {
            return false;
        }
        let handle =
            AnimationHandle { id: Cell::new(NonZeroUsize::new(id)), _phantom: Default::default() };
        let running = handle.is_running();
        handle.id.take();
        running
    }

    /// Drop (deregister) the animation handle. Called from the C++ destructor.
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_animation_handle_drop(id: usize) {
        if id == 0 {
            return;
        }
        let handle =
            AnimationHandle { id: Cell::new(NonZeroUsize::new(id)), _phantom: Default::default() };
        drop(handle);
    }
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
    fn test_animation_handle_start_restart_tween() {
        // Mirrors how codegen drives a tween: an AnimationHandle field (the
        // "handle") and, per frame, a freshly-built TweenAnimation (the "data") handed
        // to start()/restart().
        let observed = Rc::new(RefCell::new(Vec::new()));
        let observed_clone = observed.clone();
        let finished = Rc::new(core::cell::Cell::new(false));
        let finished_clone = finished.clone();

        let start_time = crate::animations::current_tick();
        let handle = AnimationHandle::default();

        let tween = TweenAnimation::new_with_callbacks(
            0i32,
            100i32,
            PropertyAnimation { duration: 200, ..Default::default() },
            move |v: i32| observed_clone.borrow_mut().push(v),
            move || finished_clone.set(true),
        );
        handle.start(Box::new(tween));

        // First tick: moves the tween past its (zero) delay into Animating. `is_running()`
        // already reported true before this tick too (registered, not yet Done).
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time));
        crate::animations::update_animation_objects();
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
        crate::animations::update_animation_objects();
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
        crate::animations::update_animation_objects();
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

pub(super) type AnimationDetail = (PropertyAnimation, Option<crate::animations::Instant>);

/// The change-detector binding used by [`Property::set_animated_binding_object`].
/// It is installed as the target property's binding and
/// acts purely as a **change detector + from/to capturer**: when the wrapped `original_binding`
/// goes dirty it (re)starts a `TweenAnimation` registered in the shared `CURRENT_ANIMATIONS`
/// registry (driven each frame by [`update_animation_objects`]), which pushes interpolated values
/// back into the property. This routes `animate x` through the same object backend as the explicit
/// `TweenAnimation` element while keeping the exact trigger-on-property-change semantics.
#[pin_project::pin_project]
pub(super) struct AnimatedBindingObjectCallable<T, A> {
    #[pin]
    pub(super) original_binding: PropertyHandle,
    pub(super) state: Cell<AnimatedBindingState>,
    pub(super) compute_animation_details: A,
    /// The instant the change was detected (captured in `mark_dirty`). The animation is anchored
    /// here, not at the first `.get()`, so that changing a value and then elapsing time animates
    /// correctly even without an intervening read (matching the legacy `reset()`-in-`mark_dirty`
    /// behaviour). Overridden by an explicit start_time from `compute_animation_details` (state
    /// transitions).
    pub(super) trigger_time: Cell<Option<crate::animations::Instant>>,
    /// Monotonic counter bumped by `mark_dirty` on every detected change. Each registered tween
    /// captures the value current when it was built and only pushes while it still matches, so a
    /// *stale* tween (one superseded by a newer change but not yet replaced by the next
    /// `evaluate`) cannot push its endpoint over the fresh value. This is done with a plain `Cell`
    /// rather than by stopping the old tween, because `mark_dirty` can run re-entrantly from
    /// inside `update_animation_objects` (a dependent animated property) where touching the
    /// `CURRENT_ANIMATIONS` registry would panic.
    pub(super) generation: Rc<Cell<u64>>,
    /// Handle owning the registry-driven tween. Its `Drop` deregisters the tween, so it is torn
    /// down together with this binding (and thus before `target` — see `target` below).
    pub(super) handle: AnimationHandle,
    /// Raw pointer to the property this binding is installed on, used by the tween's `set_value`
    /// to push interpolated values. Valid for as long as this binding lives: the binding is owned
    /// by the property's handle, and the registry tween (the only holder of a copy of this
    /// pointer) is dropped via `handle` when this binding drops.
    pub(super) target: *const Property<T>,
}

unsafe impl<T: InterpolatedPropertyValue + Clone, A: Fn() -> AnimationDetail> BindingCallable<T>
    for AnimatedBindingObjectCallable<T, A>
{
    fn evaluate(self: Pin<&Self>, value: &mut T) -> BindingResult {
        let original_binding = self.project_ref().original_binding;
        original_binding.register_as_dependency_to_current_binding(
            #[cfg(slint_debug_property)]
            "<AnimatedBindingObjectCallable>",
        );
        match self.state.get() {
            // The tween pushes values directly into the property cell (see `set_value` below), so
            // once running there is nothing to compute here; keep the current cell value.
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

                let (details, start_time) = (self.compute_animation_details)();
                let target = self.target;
                let generation = self.generation.clone();
                let my_generation = generation.get();
                let set_value = move |v: T| {
                    // A newer change bumped the generation: this tween is stale, don't clobber the
                    // fresh value (the next `evaluate` will `restart` the handle with a new tween).
                    if generation.get() != my_generation {
                        return;
                    }
                    // Safety: `target` is valid while this closure lives; the closure is owned by
                    // the registry tween, which is deregistered (via `handle`) before the property.
                    with_applying_animation(|| unsafe { (*target).set(v) });
                };
                let mut tween = TweenAnimation::new_with_callbacks(
                    from_value,
                    to_value,
                    details,
                    set_value,
                    || {},
                );
                // Anchor at the change instant (or the transition's explicit start_time), not at
                // this `.get()`.
                if let Some(start_time) = start_time.or_else(|| self.trigger_time.take()) {
                    tween.start_time = start_time;
                }

                // Compute the initial value for this same `.get()`; this also advances the tween
                // past `Delaying` so a degenerate (disabled/zero-duration/negative-delay) animation
                // snaps to its endpoint immediately, matching the legacy path.
                let (initial, finished) = tween.compute_interpolated_value();
                *value = initial;
                if finished {
                    self.state.set(AnimatedBindingState::NotAnimating);
                } else {
                    self.state.set(AnimatedBindingState::Animating);
                    self.handle.restart(Box::new(tween));
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
            self.trigger_time.set(Some(crate::animations::current_tick()));
            // Invalidate any still-running tween so it stops pushing before the next `evaluate`
            // builds its replacement. Cell-only: safe to call re-entrantly (see `generation`).
            self.generation.set(self.generation.get().wrapping_add(1));
        }
    }

    fn intercept_set(self: Pin<&Self>, _value: &T) -> bool {
        // Keep this binding when the write is the animation pushing its own value; let an external
        // write fall through to remove it (cancelling the animation), as the legacy path does.
        APPLYING_ANIMATION.with(|g| g.get())
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
    /// Install an animated binding: an [`AnimatedBindingObjectCallable`] change-detector whose
    /// triggered animation is a `TweenAnimation` registered in the shared `CURRENT_ANIMATIONS`
    /// registry (driven each frame by [`update_animation_objects`]). This is the backend used by
    /// the Rust generator for `animate x` and state transitions.
    pub fn set_animated_binding_object(
        &self,
        binding: impl Binding<T> + 'static,
        compute_animation_details: impl Fn() -> (PropertyAnimation, Option<crate::animations::Instant>)
        + 'static,
    ) {
        let binding_callable = properties_animations::AnimatedBindingObjectCallable::<T, _> {
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
            generation: Rc::new(Cell::new(0)),
            handle: properties_animations::AnimationHandle::default(),
            target: self as *const Property<T>,
        };

        // Safety: the `AnimatedBindingObjectCallable`'s type matches the property type
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

    /// Animate the property from its current value to `value` through the consolidated registry
    /// backend, driven each frame by [`update_animation_objects`]. An
    /// imperative animated assignment is just a bound animation with a *constant* target and an
    /// immediate trigger, so it reuses [`AnimatedBindingObjectCallable`] with the state forced to
    /// `ShouldStart`. Re-assigning replaces this binding (dropping its handle, deregistering the
    /// previous tween), so a mid-flight re-assignment cleanly restarts from the current value.
    pub fn set_animated_value_object(&self, value: T, animation_data: PropertyAnimation) {
        let binding_callable = properties_animations::AnimatedBindingObjectCallable::<T, _> {
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
            generation: Rc::new(Cell::new(0)),
            handle: properties_animations::AnimationHandle::default(),
            target: self as *const Property<T>,
        };

        // Safety: the `AnimatedBindingObjectCallable`'s type matches the property type
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

        let w = Rc::downgrade(&compo);
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


    // ---- Object-backed animated binding (set_animated_binding_object) ----
    // These mirror the legacy `*_triggered_by_binding` tests but drive the consolidated object
    // backend: values are pushed by `update_animation_objects()` each frame rather than pulled
    // lazily, so each frame the test advances the clock *and* calls `update_animation_objects()`.

    #[test]
    fn object_animation_triggered_by_binding() {
        let compo = Component::new_test_component();
        let start_time = crate::animations::current_tick();

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: 1.,
            ..PropertyAnimation::default()
        };

        let w = Rc::downgrade(&compo);
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

        let w = Rc::downgrade(&compo);
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

        let w = Rc::downgrade(&compo);
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

        let w = Rc::downgrade(&compo);
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

        let w = Rc::downgrade(&compo);
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
