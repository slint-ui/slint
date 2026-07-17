// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]
//! The animation system

use crate::items::PropertyAnimation;
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::Cell;
use core::cell::RefCell;
use core::num::NonZeroUsize;
pub(crate) mod delay;
pub mod easings;
pub(crate) mod handle;
pub(crate) mod parallel;
pub(crate) mod physics;
pub(crate) mod physics_simulation;
pub(crate) mod sequential;
pub(crate) mod tween;

/// Represent an instant, in milliseconds since the AnimationDriver's initial_instant
#[repr(transparent)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Ord, PartialOrd, Eq)]
pub struct Instant(pub u64);

impl core::ops::Sub<Instant> for Instant {
    type Output = core::time::Duration;
    fn sub(self, other: Self) -> core::time::Duration {
        core::time::Duration::from_millis(self.0 - other.0)
    }
}

impl core::ops::Sub<core::time::Duration> for Instant {
    type Output = Instant;
    fn sub(self, other: core::time::Duration) -> Instant {
        Self(self.0 - other.as_millis() as u64)
    }
}

impl core::ops::Add<core::time::Duration> for Instant {
    type Output = Instant;
    fn add(self, other: core::time::Duration) -> Instant {
        Self(self.0 + other.as_millis() as u64)
    }
}

impl core::ops::AddAssign<core::time::Duration> for Instant {
    fn add_assign(&mut self, other: core::time::Duration) {
        self.0 += other.as_millis() as u64;
    }
}

impl core::ops::SubAssign<core::time::Duration> for Instant {
    fn sub_assign(&mut self, other: core::time::Duration) {
        self.0 -= other.as_millis() as u64;
    }
}

impl Instant {
    /// Returns the amount of time elapsed since an other instant.
    ///
    /// Equivalent to `self - earlier`
    pub fn duration_since(self, earlier: Instant) -> core::time::Duration {
        self - earlier
    }

    /// Wrapper around [`std::time::Instant::now()`] that delegates to the backend
    /// and allows working in no_std environments.
    pub fn now() -> Self {
        Self(Self::duration_since_start().as_millis() as u64)
    }

    fn duration_since_start() -> core::time::Duration {
        crate::context::GLOBAL_CONTEXT
            .with(|p| p.get().map(|p| p.platform().duration_since_start()))
            .unwrap_or_default()
    }

    /// Return the number of milliseconds this `Instant` is after the backend has started
    pub fn as_millis(&self) -> u64 {
        self.0
    }
}

/// The AnimationDriver
pub struct AnimationDriver {
    /// Indicate whether there are any active animations that require a future call to update_animations.
    active_animations: Cell<bool>,
    global_instant: core::pin::Pin<Box<crate::Property<Instant>>>,
}

impl Default for AnimationDriver {
    fn default() -> Self {
        AnimationDriver {
            active_animations: Cell::default(),
            global_instant: Box::pin(crate::Property::new_named(
                Instant::default(),
                "i_slint_core::AnimationDriver::global_instant",
            )),
        }
    }
}

impl AnimationDriver {
    /// Iterates through all animations based on the new time tick and updates their state. This should be called by
    /// the windowing system driver for every frame.
    pub fn update_animations(&self, new_tick: Instant) {
        let current_tick = self.global_instant.as_ref().get_untracked();
        assert!(current_tick <= new_tick, "The platform's clock is not monotonic!");
        if current_tick != new_tick {
            self.active_animations.set(false);
            self.global_instant.as_ref().set(new_tick);
        }
    }

    /// Returns true if there are any active or ready animations. This is used by the windowing system to determine
    /// if a new animation frame is required or not. Returns false otherwise.
    pub fn has_active_animations(&self) -> bool {
        self.active_animations.get()
    }

    /// Tell the driver that there are active animations
    pub fn set_has_active_animations(&self) {
        self.active_animations.set(true);
    }
    /// The current instant that is to be used for animation
    /// using this function register the current binding as a dependency
    pub fn current_tick(&self) -> Instant {
        self.global_instant.as_ref().get()
    }
}

crate::thread_local!(
/// This is the default instance of the animation driver that's used to advance all property animations
/// at the same time.
pub static CURRENT_ANIMATION_DRIVER : AnimationDriver = AnimationDriver::default()
);

/// The current instant that is to be used for animation
/// using this function register the current binding as a dependency
pub fn current_tick() -> Instant {
    CURRENT_ANIMATION_DRIVER.with(|driver| driver.current_tick())
}

/// Same as [`current_tick`], but also register that one should be running animation
/// on next frame
pub fn animation_tick() -> u64 {
    CURRENT_ANIMATION_DRIVER.with(|driver| {
        driver.set_has_active_animations();
        driver.current_tick().0
    })
}

/// Advance the global animation clock to the current platform time (honoring
/// `SLINT_SLOW_ANIMATIONS`) and reset the active-animations flag for the new frame.
/// Should run first so timers and change handlers see the correct value
pub fn advance_animation_clock() {
    CURRENT_ANIMATION_DRIVER.with(|driver| {
        #[allow(unused_mut)]
        let mut duration = Instant::duration_since_start().as_millis() as u64;
        #[cfg(feature = "std")]
        if let Ok(val) = std::env::var("SLINT_SLOW_ANIMATIONS") {
            let factor = val.parse().unwrap_or(2).max(1);
            duration /= factor;
        };
        driver.update_animations(Instant(duration))
    });
}

/// Global registry of live animation objects keyed by an id that is never reused.
/// Necessary because an `AnimationHandle` can outlive its entry
///
/// Entries are `Rc<RefCell<dyn Animation>>` so an animation can be removed while the creator keeps
/// the animation alive while not reallocating a new animation on retrigger
#[derive(Default)]
struct AnimationRegistry {
    next_id: usize,
    animations: alloc::collections::BTreeMap<usize, Rc<RefCell<dyn Animation>>>,
}

impl AnimationRegistry {
    /// Insert `animation` under a fresh, never-reused id (always `>= 1`).
    fn insert(&mut self, animation: Rc<RefCell<dyn Animation>>) -> NonZeroUsize {
        self.next_id = self.next_id.checked_add(1).expect("animation id overflow");
        let id = NonZeroUsize::new(self.next_id).unwrap();
        self.animations.insert(id.get(), animation);
        id
    }
}
crate::thread_local!(static CURRENT_ANIMATIONS: RefCell<AnimationRegistry> = RefCell::default());

crate::thread_local!(
    /// Set so the change detector can differentiate between our animation updates
    /// and external writes.
    pub(crate) static APPLYING_ANIMATION: Cell<bool> = const { Cell::new(false) }
);

/// Run `f` with [`APPLYING_ANIMATION`] set, so writes it performs on an animated property are
/// treated as self-writes (the binding is kept) rather than external overrides.
pub(crate) fn with_applying_animation<R>(f: impl FnOnce() -> R) -> R {
    APPLYING_ANIMATION.with(|g| {
        let previous = g.replace(true);
        let r = f();
        g.set(previous);
        r
    })
}

/// Base trait for all animation objects
// `start`/`set_iteration_count`/`add_child`/`set_on_finished` have no caller yet
#[allow(dead_code)]
pub(crate) trait Animation {
    /// Start the animation
    fn start(&mut self);
    /// Stop the animation
    fn stop(&mut self);
    /// Restart the animation from the beginning
    fn restart(&mut self);
    /// Check if the animation is currently running
    fn is_running(&self) -> bool;
    /// Advance the animation state by one frame
    /// Returns true if the animation is still running.
    /// Default implementation returns `is_running()` with no state change.
    fn update(&mut self) -> bool {
        self.is_running()
    }
    /// Sets iteration count for the animation
    fn set_iteration_count(&mut self, _iteration_count: f64) {}
    /// Add a child animation to a Parallel/Sequential container
    fn add_child(&mut self, _child: Box<dyn Animation>) {}
    /// Register a callback invoked exactly once, the first time `update()` reports that this
    /// animation is no longer running
    fn set_on_finished(&mut self, _on_finished: Box<dyn FnMut()>) {}
    /// Called by [`AnimationTrigger`] when it has detected a change but hasn't re-primed this
    /// animation with fresh endpoints yet (that happens on the next `evaluate`). A registry tick
    /// landing in this window must not push a value: it would be interpolating towards the
    /// *stale* endpoint. Sink-owning leaves (`TweenAnimation`) pause their push until re-primed;
    /// this replaces the old `Rc<Cell<u64>>` generation counter, which existed only because a
    /// stale *instance* could still be registered when its replacement was born — with one
    /// long-lived object there is no such instance to go stale, only a value to pause.
    fn pause_for_pending_retrigger(&mut self) {}
}

/// Shared retrigger trait for endpoint leaves.
/// Gets called to (re)prime a long-lived leaf instead of allocating a new animation
pub(crate) trait InterpolatingAnimation<T>: Animation {
    /// Reset the animation's clock/state, adopt new endpoints and config, and honor `anchor`
    /// (the change instant, or a transition's explicit start time) as the new start time.
    fn retrigger(
        &mut self,
        from: T,
        to: T,
        details: PropertyAnimation,
        anchor: Option<crate::animations::Instant>,
    );
}

/// Update all active animation objects by one tick and should be called once per frame.
pub fn update_animation_objects() {
    CURRENT_ANIMATIONS.with(|anims| {
        let snapshot: Vec<(usize, Rc<RefCell<dyn Animation>>)> = {
            let anims_ref = anims.borrow();
            anims_ref.animations.iter().map(|(id, anim)| (*id, anim.clone())).collect()
        };

        let mut finished_ids = Vec::new();
        for (id, anim) in snapshot {
            match anim.try_borrow_mut() {
                Ok(mut anim) => {
                    if !anim.update() {
                        finished_ids.push(id);
                    }
                }
                Err(_) => {
                    // Contended: this animation is being reached from within its own
                    // `update()`. Skip it this frame; it'll be ticked again next frame.
                }
            }
        }

        // Remove finished animations. An id may already be gone so a missing entry is not an error.
        let mut anims_mut = anims.borrow_mut();
        for id in finished_ids {
            anims_mut.animations.remove(&id);
        }
    });
}
