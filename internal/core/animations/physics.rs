// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::animations::Animation;
use alloc::boxed::Box;

/// An animation object driven by a physics `Simulation`
///
/// Unlike a tween, the simulation integrates *in place*: each frame it reads the target's current
/// value via `get_value`, advances it, and writes it back through `set_value`. Reads the live
/// values so modifications are picked up and the animation continues smoothly
pub struct PhysicsAnimation<S> {
    simulation: S,
    running: bool,
    finished: bool,
    /// Reads the target property's current value at the start of each frame; the simulation is
    /// advanced from this value
    get_value: Box<dyn FnMut() -> crate::Coord>,
    /// Pushes each freshly computed value into the target property (once per frame).
    set_value: Box<dyn FnMut(crate::Coord)>,
    on_finished: Option<Box<dyn FnMut()>>,
}

impl<S: crate::animations::physics_simulation::Simulation> PhysicsAnimation<S> {
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
            on_finished: None,
        }
    }

    /// Reconfigures this physics animation in place with a fresh `simulation` and get/set
    /// closures, clears any previous `on_finished`, and marks it running again. Lets a
    /// persistent object be reused across restarts (e.g. Flickable's per-axis kinetic-scroll
    /// animation) instead of reallocated on every restart.
    pub fn reset(
        &mut self,
        simulation: S,
        get_value: impl FnMut() -> crate::Coord + 'static,
        set_value: impl FnMut(crate::Coord) + 'static,
    ) {
        self.simulation = simulation;
        self.get_value = Box::new(get_value);
        self.set_value = Box::new(set_value);
        self.running = true;
        self.finished = false;
        self.on_finished = None;
    }
}

impl<S: crate::animations::physics_simulation::Simulation> Animation for PhysicsAnimation<S> {
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
        // so an external adjustment since the last frame is carried forward.
        // The simulation works in `f32`; adapt to/from `Coord`.
        let mut value = (self.get_value)() as f32;
        let finished = self.simulation.step(&mut value, crate::animations::current_tick());
        let value = value as crate::Coord;
        // Push with the self-write guard so that, should the target ever carry a competing
        // (change-detector) binding, this write is treated as a self-write.
        crate::animations::with_applying_animation(|| (self.set_value)(value));
        if finished {
            self.finished = true;
            if let Some(mut on_finished) = self.on_finished.take() {
                on_finished();
            }
            false
        } else {
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.set_has_active_animations());
            true
        }
    }

    fn set_on_finished(&mut self, on_finished: Box<dyn FnMut()>) {
        self.on_finished = Some(on_finished);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::physics_simulation;
    use alloc::rc::Rc;
    use alloc::vec::Vec;
    use core::cell::{Cell, RefCell};

    #[test]
    fn test_physics_animation_reset_reuses_object_across_restarts() {
        // `reset` lets a persistent `PhysicsAnimation` be reconfigured in place and reused across
        // restarts (see Flickable's per-axis kinetic-scroll object, ANIMATION_BACKEND_PLAN.md §5
        // step 6) instead of being reallocated on every restart.
        struct StepSimulation {
            step: f32,
            remaining: u32,
        }
        impl physics_simulation::Simulation for StepSimulation {
            fn step(&mut self, current: &mut f32, _new_tick: crate::animations::Instant) -> bool {
                *current += self.step;
                self.remaining = self.remaining.saturating_sub(1);
                self.remaining == 0
            }
        }

        let observed = Rc::new(RefCell::new(Vec::new()));
        let observed_clone = observed.clone();
        let value = Rc::new(Cell::new(0.0f32));
        let value_clone = value.clone();

        let mut physics = PhysicsAnimation::new(
            StepSimulation { step: 1.0, remaining: 2 },
            move || value_clone.get() as crate::Coord,
            move |v: crate::Coord| observed_clone.borrow_mut().push(v),
        );

        assert!(physics.update());
        assert!(!physics.update());
        assert!(!physics.is_running());
        assert_eq!(observed.borrow().len(), 2);

        // Reconfigure the very same object with a fresh simulation and get/set closures.
        let value2 = Rc::new(Cell::new(10.0f32));
        let value2_clone = value2.clone();
        let observed2 = Rc::new(RefCell::new(Vec::new()));
        let observed2_clone = observed2.clone();
        physics.reset(
            StepSimulation { step: 2.0, remaining: 1 },
            move || value2_clone.get() as crate::Coord,
            move |v: crate::Coord| observed2_clone.borrow_mut().push(v),
        );
        assert!(physics.is_running());
        assert!(!physics.update(), "single remaining step finishes immediately");
        assert_eq!(*observed2.borrow().last().unwrap(), 12 as crate::Coord);
        // The old sink is untouched: `reset` fully replaced it rather than composing with it.
        assert_eq!(observed.borrow().len(), 2);
    }
}
