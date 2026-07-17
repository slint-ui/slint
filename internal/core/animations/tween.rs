// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![allow(unsafe_code)]

use crate::Property;
use crate::animations::{Animation, InterpolatingAnimation};
use crate::items::{AnimationDirection, PropertyAnimation};
use crate::properties::{AnimationSink, AnimationState, InterpolatedPropertyValue};
use alloc::boxed::Box;
/// A tween animation that interpolates a value from one state to another.
pub struct TweenAnimation<T> {
    from_value: T,
    to_value: T,
    details: PropertyAnimation,
    start_time: crate::animations::Instant,
    state: AnimationState,
    running: bool,
    sink: Option<AnimationSink<T>>,
    on_finished: Option<Box<dyn FnMut()>>,
    pending_retrigger: bool,
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
            sink: None,
            on_finished: None,
            pending_retrigger: false,
        }
    }

    /// Same as [`Self::new`], but additionally pushes each freshly computed value into
    /// `set_value` once per frame, and invokes `on_finished` exactly once when the
    /// animation completes on its own (not on an explicit [`Animation::stop`]).
    #[allow(dead_code)]
    pub fn new_with_callbacks(
        from_value: T,
        to_value: T,
        details: PropertyAnimation,
        set_value: impl FnMut(T) + 'static,
        on_finished: impl FnMut() + 'static,
    ) -> Self {
        Self {
            sink: Some(AnimationSink::Callback(Box::new(set_value))),
            on_finished: Some(Box::new(on_finished)),
            ..Self::new(from_value, to_value, details)
        }
    }

    /// Same as [`Self::new`], but pushes each freshly computed value directly into `target` via
    /// [`Property::set`], guarded by [`with_applying_animation`] so the write is recognized as a
    /// self-write rather than an external cancel.
    ///
    /// Safety: `target` must be valid for as long as this tween is registered anywhere it can be
    /// ticked (the registry, or a direct `update()` call).
    pub unsafe fn new_with_property_sink(
        from_value: T,
        to_value: T,
        details: PropertyAnimation,
        target: *const Property<T>,
    ) -> Self {
        Self {
            sink: Some(AnimationSink::Property(target)),
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
                    let t =
                        crate::animations::easings::easing_curve(&self.details.easing, progress);
                    let val = self.from_value.interpolate(&self.to_value, t);

                    (val, false)
                } else {
                    // If `time_progress` is zero, the elapsed time lands on a iteration boundary
                    // so current iteration names the iteration about to start instead of the one
                    // that just finished
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
        // Delaying and Animating are considered Running so checking running after start returns
        // true
        self.running && !matches!(self.state, AnimationState::Done { .. })
    }

    fn update(&mut self) -> bool {
        if self.pending_retrigger {
            // A change was detected but `evaluate` hasn't re-primed us with fresh endpoints yet.
            // Hold: don't push a value interpolated towards the stale `to_value`. Still report
            // "running" so the driver keeps scheduling frames until the re-prime lands.
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.set_has_active_animations());
            return true;
        }
        let (value, finished) = self.compute_interpolated_value();
        if let Some(sink) = self.sink.as_mut() {
            sink.push(value);
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

    fn pause_for_pending_retrigger(&mut self) {
        self.pending_retrigger = true;
    }
}
impl<T: InterpolatedPropertyValue + Clone> InterpolatingAnimation<T> for TweenAnimation<T> {
    fn retrigger(
        &mut self,
        from: T,
        to: T,
        details: PropertyAnimation,
        anchor: Option<crate::animations::Instant>,
    ) {
        self.from_value = from;
        self.to_value = to;
        self.details = details;
        self.state = AnimationState::Delaying;
        self.start_time = anchor.unwrap_or_else(crate::animations::current_tick);
        self.running = true;
        self.pending_retrigger = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use alloc::vec::Vec;
    use core::cell::RefCell;

    #[test]
    fn test_pending_retrigger_holds_stale_push_until_reprimed() {
        // `AnimationTrigger::mark_dirty` calls `pause_for_pending_retrigger` on the (potentially
        // still-registered, still-running) root leaf so a registry tick landing before the next
        // `evaluate` re-primes it can't push a value interpolated towards the now-stale
        // `to_value`. This is the long-lived-object replacement for the old `Rc<Cell<u64>>`
        // generation counter (there's no stale *instance* anymore, only a value to hold).
        let observed = Rc::new(RefCell::new(Vec::new()));
        let observed_clone = observed.clone();

        let start_time = crate::animations::current_tick();
        let mut tween = TweenAnimation::new_with_callbacks(
            0i32,
            100i32,
            PropertyAnimation { duration: 200, ..Default::default() },
            move |v: i32| observed_clone.borrow_mut().push(v),
            || {},
        );

        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(100))
        });
        assert!(tween.update());
        assert_eq!(*observed.borrow().last().unwrap(), 50);

        // A change is detected (what `mark_dirty` would do): pause before the retrigger lands.
        tween.pause_for_pending_retrigger();

        // A registry tick arrives before `evaluate` gets to re-prime: must hold, not push the
        // stale interpolation at this (now fully elapsed) instant.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(300))
        });
        assert!(tween.update());
        assert_eq!(
            *observed.borrow().last().unwrap(),
            50,
            "must not push a stale value while a retrigger is pending"
        );

        // `evaluate` re-primes with fresh endpoints (what `AnimationTrigger::prime` would do),
        // clearing the pending flag.
        let restart_time = crate::animations::current_tick();
        tween.retrigger(
            50,
            200,
            PropertyAnimation { duration: 200, ..Default::default() },
            Some(restart_time),
        );

        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(restart_time + core::time::Duration::from_millis(100))
        });
        assert!(tween.update());
        assert_eq!(*observed.borrow().last().unwrap(), 125);
    }
}
