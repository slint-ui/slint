// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::animations::Animation;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Runs animations sequentially, one after another
#[allow(dead_code)]
pub struct SequentialAnimation {
    pub(crate) animations: Vec<Box<dyn Animation>>,
    current_index: usize,
    running: bool,
    iteration_count: f64,
    completed_iterations: u64,
    on_finished: Option<Box<dyn FnMut()>>,
}

#[allow(dead_code)]
impl SequentialAnimation {
    /// Creates an empty sequence of animations, run once by default.
    pub fn new() -> Self {
        Self {
            animations: Vec::new(),
            current_index: 0,
            running: true,
            iteration_count: 1.0,
            completed_iterations: 0,
            on_finished: None,
        }
    }

    /// Sets how many times the whole sequence should run (negative loops forever).
    pub fn set_iteration_count(&mut self, iteration_count: f64) {
        self.iteration_count = iteration_count;
    }

    /// Appends `animation` to the end of the sequence.
    pub fn add_animation(&mut self, animation: Box<dyn Animation>) {
        self.animations.push(animation);
    }

    /// Returns the animation currently being run, if any.
    pub fn current_animation_mut(&mut self) -> Option<&mut Box<dyn Animation>> {
        self.animations.get_mut(self.current_index)
    }

    fn more_iterations_remaining(&self) -> bool {
        self.iteration_count < 0. || (self.completed_iterations as f64) < self.iteration_count
    }

    /// Advances to and restarts the next animation in the sequence
    /// Once the last one finishes, loops to the beginning if more iterations in `iteration_count`
    pub fn advance_to_next(&mut self) {
        self.current_index += 1;
        if self.current_index < self.animations.len() {
            if let Some(anim) = self.current_animation_mut() {
                // `restart` as children are constructed at the beginning but only activated now
                anim.restart();
            }
            return;
        }

        self.completed_iterations += 1;
        if !self.animations.is_empty() && self.more_iterations_remaining() {
            self.current_index = 0;
            for anim in &mut self.animations {
                anim.restart();
            }
        }
    }

    /// Returns true once the sequence has run `iteration_count` times to completion.
    pub fn is_finished(&self) -> bool {
        self.animations.is_empty()
            || self.iteration_count == 0.
            || self.current_index >= self.animations.len()
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
        if self.is_finished() {
            self.current_index = 0;
            self.completed_iterations = 0;
        }
        if !self.animations.is_empty() && self.current_index == 0 {
            // `restart` as children are constructed at the beginning but only activated now
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
        self.completed_iterations = 0;
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
        // Loop so a child finishing begins the next child (or the next iteration) in the same frame
        while let Some(current_anim) = self.animations.get_mut(self.current_index) {
            if current_anim.update() {
                break;
            }
            self.advance_to_next();
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
        SequentialAnimation::set_iteration_count(self, iteration_count);
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
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(50))
        });
        assert!(seq.update());
        assert!(observed.borrow().is_empty());

        // The delay elapses: the tween is activated (with a freshly-reset clock,
        // hence progress 0) and ticked within this same call.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(100))
        });
        assert!(seq.update());
        assert_eq!(*observed.borrow().last().unwrap(), 0);

        // 100ms into the tween's own (200ms) duration: halfway.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(200))
        });
        assert!(seq.update());
        assert_eq!(*observed.borrow().last().unwrap(), 50);

        // Past the tween's duration: finished, sequence reports not-running.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(300))
        });
        assert!(!seq.update());
        assert_eq!(*observed.borrow().last().unwrap(), 100);
    }

    #[test]
    fn test_sequential_iteration_count_loops() {
        let start_time = crate::animations::current_tick();

        let mut seq = SequentialAnimation::new();
        seq.set_iteration_count(2.0);
        seq.add_animation(Box::new(DelayAnimation::new(100)));
        seq.start();

        // First pass finishes...
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(100))
        });
        // ...but a second iteration is due, so the sequence is still running.
        assert!(seq.update());

        // Second pass finishes: no iterations remain.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(200))
        });
        assert!(!seq.update());
        assert!(!seq.is_running());
    }

    #[test]
    fn test_dyn_animation_set_iteration_count_and_add_child_delegate_on_containers() {
        // Mirrors how the C++ FFI layer builds a tree: only ever through `Box<dyn Animation>`,
        // never the concrete `SequentialAnimation`/`ParallelAnimation` types.
        let start_time = crate::animations::current_tick();

        let mut seq: Box<dyn Animation> = Box::new(SequentialAnimation::new());
        seq.set_iteration_count(1.0);
        seq.add_child(Box::new(DelayAnimation::new(100)));
        seq.add_child(Box::new(DelayAnimation::new(100)));
        seq.start();
        assert!(seq.is_running());

        // First child's delay elapses; the second child hasn't started yet, so the
        // sequence (having advanced to it) is still running.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(100))
        });
        assert!(seq.update());

        // Both children's delays have now elapsed.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(210))
        });
        assert!(!seq.update());
        assert!(!seq.is_running());
    }
}
