// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::animations::Animation;
use alloc::boxed::Box;
use alloc::vec::Vec;
/// Runs animations in parallel
#[allow(dead_code)]
pub struct ParallelAnimation {
    animations: Vec<Box<dyn Animation>>,
    running: bool,
    iteration_count: f64,
    completed_iterations: u64,
    on_finished: Option<Box<dyn FnMut()>>,
}

#[allow(dead_code)]
impl ParallelAnimation {
    /// Creates an empty group of animations to run in parallel, run once by default.
    pub fn new() -> Self {
        Self {
            animations: Vec::new(),
            running: true,
            iteration_count: 1.0,
            completed_iterations: 0,
            on_finished: None,
        }
    }

    /// Sets how many times the whole group should run (negative loops forever).
    pub fn set_iteration_count(&mut self, iteration_count: f64) {
        self.iteration_count = iteration_count;
    }

    /// Adds `animation` to the group.
    pub fn add_animation(&mut self, animation: Box<dyn Animation>) {
        self.animations.push(animation);
    }

    /// Returns true once every animation in the group has finished running (this iteration).
    pub fn all_finished(&self) -> bool {
        self.animations.is_empty() || self.animations.iter().all(|a| !a.is_running())
    }

    fn more_iterations_remaining(&self) -> bool {
        self.iteration_count < 0. || (self.completed_iterations as f64) < self.iteration_count
    }

    /// Returns true once the group has run `iteration_count` times to completion.
    pub fn is_finished(&self) -> bool {
        self.animations.is_empty()
            || self.iteration_count == 0.
            || (self.all_finished() && !self.more_iterations_remaining())
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
        if self.is_finished() {
            self.completed_iterations = 0;
        }
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
        self.completed_iterations = 0;
        for anim in &mut self.animations {
            anim.restart();
        }
        self.running = true;
    }

    fn is_running(&self) -> bool {
        self.running && !self.is_finished()
    }

    fn update(&mut self) -> bool {
        if !self.running {
            return false;
        }
        for anim in &mut self.animations {
            anim.update();
        }
        if !self.animations.is_empty() && self.all_finished() {
            self.completed_iterations += 1;
            if self.more_iterations_remaining() {
                for anim in &mut self.animations {
                    anim.restart();
                }
            }
        }
        let running = self.is_running();
        if !running {
            if let Some(mut on_finished) = self.on_finished.take() {
                on_finished();
            }
        }
        running
    }

    fn set_on_finished(&mut self, on_finished: Box<dyn FnMut()>) {
        self.on_finished = Some(on_finished);
    }

    fn set_iteration_count(&mut self, iteration_count: f64) {
        ParallelAnimation::set_iteration_count(self, iteration_count);
    }

    fn add_child(&mut self, child: Box<dyn Animation>) {
        self.add_animation(child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::delay::DelayAnimation;
    use crate::animations::tween::TweenAnimation;
    use crate::items::PropertyAnimation;
    use alloc::rc::Rc;
    use core::cell::RefCell;

    #[test]
    fn test_parallel_animation_structure() {
        let mut par = ParallelAnimation::new();
        par.add_animation(Box::new(DelayAnimation::new(100)));
        par.add_animation(Box::new(DelayAnimation::new(200)));

        assert!(!par.is_finished());

        par.start();
        assert!(par.is_running());
    }

    #[test]
    fn test_parallel_two_tweens_finish_together() {
        // Two children with different durations: the group must keep reporting
        // running until the *longer* one finishes, not the shorter one.
        let short_observed = Rc::new(RefCell::new(Vec::new()));
        let short_observed_clone = short_observed.clone();
        let long_observed = Rc::new(RefCell::new(Vec::new()));
        let long_observed_clone = long_observed.clone();

        let start_time = crate::animations::current_tick();

        let mut par = ParallelAnimation::new();
        par.add_animation(Box::new(TweenAnimation::new_with_callbacks(
            0i32,
            100i32,
            PropertyAnimation { duration: 100, ..Default::default() },
            move |v: i32| short_observed_clone.borrow_mut().push(v),
            || {},
        )));
        par.add_animation(Box::new(TweenAnimation::new_with_callbacks(
            0i32,
            100i32,
            PropertyAnimation { duration: 200, ..Default::default() },
            move |v: i32| long_observed_clone.borrow_mut().push(v),
            || {},
        )));
        par.start();

        // Halfway through the shorter tween, both are still running.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(50))
        });
        assert!(par.update());
        assert_eq!(*short_observed.borrow().last().unwrap(), 50);
        assert_eq!(*long_observed.borrow().last().unwrap(), 25);

        // The shorter tween finishes, but the group keeps running because the
        // longer one hasn't finished yet.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(100))
        });
        assert!(par.update());
        assert_eq!(*short_observed.borrow().last().unwrap(), 100);
        assert_eq!(*long_observed.borrow().last().unwrap(), 50);

        // Both finish: the group reports done.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(200))
        });
        assert!(!par.update());
        assert_eq!(*long_observed.borrow().last().unwrap(), 100);
        assert!(!par.is_running());
    }

    #[test]
    fn test_parallel_iteration_count_loops() {
        let start_time = crate::animations::current_tick();

        let mut par = ParallelAnimation::new();
        par.set_iteration_count(2.0);
        par.add_animation(Box::new(DelayAnimation::new(100)));
        par.start();

        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(100))
        });
        assert!(par.update());

        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(200))
        });
        assert!(!par.update());
        assert!(!par.is_running());
    }
}
