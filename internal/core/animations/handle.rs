// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::animations::{Animation, CURRENT_ANIMATIONS};
use alloc::rc::Rc;
use core::cell::RefCell;
use core::num::NonZeroUsize;

/// Handle to a registered animation object
/// Analogous to `crate::timers::Timer`, this is a lightweight id-holding handle
/// that the codegen can store as a component field.
#[derive(Default)]
pub struct AnimationHandle {
    id: core::cell::Cell<Option<NonZeroUsize>>,
    _phantom: core::marker::PhantomData<*mut ()>,
}

impl AnimationHandle {
    /// Register a new animation object in the global registry.
    #[allow(dead_code)] // exercised by unit tests only
    pub fn register(animation: Rc<RefCell<dyn Animation>>) -> Self {
        let id = CURRENT_ANIMATIONS.with(|anims| anims.borrow_mut().insert(animation));
        Self { id: core::cell::Cell::new(Some(id)), _phantom: core::marker::PhantomData }
    }

    /// Start driving `animation`. No-op if something is already running on this handle: this
    /// lets a caller unconditionally call `start` on every occasion that might mean "this should
    /// be running"
    pub fn start(&self, animation: Rc<RefCell<dyn Animation>>) {
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
    pub fn restart(&self, animation: Rc<RefCell<dyn Animation>>) {
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
                anims
                    .borrow()
                    .animations
                    .get(&id.get())
                    .map(|a| a.borrow().is_running())
                    .unwrap_or(false)
            })
        } else {
            false
        }
    }

    /// Remove any previously registered animation and register `animation` in its place.
    pub fn replace(&self, animation: Rc<RefCell<dyn Animation>>) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::tween::TweenAnimation;
    use crate::items::PropertyAnimation;
    use alloc::vec::Vec;
    use core::cell::Cell;

    #[test]
    fn test_animation_handle_start_restart_tween() {
        // Mirrors how codegen drives a tween: an AnimationHandle field (the
        // "handle") and, per frame, a freshly-built TweenAnimation (the "data") handed
        // to start()/restart().
        let observed = Rc::new(RefCell::new(Vec::new()));
        let observed_clone = observed.clone();
        let finished = Rc::new(Cell::new(false));
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
        handle.start(Rc::new(RefCell::new(tween)));

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
        handle.start(Rc::new(RefCell::new(no_op_tween)));

        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(100))
        });
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
        handle.restart(Rc::new(RefCell::new(restarted_tween)));

        let start_time = crate::animations::current_tick();
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(200))
        });
        crate::animations::update_animation_objects();
        assert_eq!(*observed.borrow().last().unwrap(), 100);
        assert!(finished.get());
        assert!(!handle.is_running());
    }
}
