// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/// Progress ticks are discarded while the raw list position is moving.
/// The caller settles the gate after 200 ms without a position change.
#[derive(Clone, Copy, Default)]
pub struct ScrollGate {
    last_y: Option<f32>,
    blocked: bool,
}

impl ScrollGate {
    pub fn observe_cell(gate: &core::cell::Cell<Self>, raw_y: f32) -> bool {
        let mut current = gate.get();
        let moved = current.observe(raw_y);
        gate.set(current);
        moved
    }

    pub fn observe(&mut self, raw_y: f32) -> bool {
        let moved = self.last_y.is_some_and(|previous| previous != raw_y);
        self.last_y = Some(raw_y);
        if moved {
            self.blocked = true;
        }
        moved
    }

    pub fn settle(&mut self) {
        self.blocked = false;
    }

    #[cfg(test)]
    pub fn blocked(&self) -> bool {
        self.blocked
    }

    pub fn progress_tick(&self) -> bool {
        !self.blocked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sample_is_saved_by_the_cell_wrapper() {
        let gate = core::cell::Cell::new(ScrollGate::default());
        assert!(!ScrollGate::observe_cell(&gate, 0.));
        assert!(!ScrollGate::observe_cell(&gate, 0.));
        assert!(ScrollGate::observe_cell(&gate, -0.1));
        assert!(gate.get().blocked());
        assert!(!ScrollGate::observe_cell(&gate, -0.1));
    }

    #[test]
    fn fractional_inertia_remains_motion_until_settled() {
        let mut gate = ScrollGate::default();
        assert!(!gate.observe(0.));
        assert!(gate.progress_tick());
        for position in [-10., -10.4, -10.45, -10.46] {
            assert!(gate.observe(position));
            assert!(!gate.progress_tick());
        }
        assert!(!gate.observe(-10.46));
        assert!(!gate.progress_tick());
        gate.settle();
        assert!(gate.progress_tick());
    }

    #[test]
    fn stationary_auto_controller_does_not_pause_or_catch_up() {
        let mut gate = ScrollGate::default();
        gate.observe(0.);
        for _ in 0..3 {
            assert!(gate.progress_tick());
            assert!(!gate.observe(0.));
        }
        gate.observe(-1.);
        for _ in 0..8 {
            assert!(!gate.progress_tick());
        }
        gate.settle();
        assert!(gate.progress_tick());
    }
}
