// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

pub struct Landscape {
    flipped: bool,
    candidate: bool,
    stable: f32,
}

impl Landscape {
    pub const fn new() -> Self {
        Self { flipped: false, candidate: false, stable: 0. }
    }

    pub fn observe(&mut self, up: [f32; 2], dt: f32) -> bool {
        let candidate = if up[1] > 0.6 {
            true
        } else if up[1] < -0.6 {
            false
        } else {
            self.stable = 0.;
            return self.flipped;
        };
        if candidate != self.candidate {
            self.candidate = candidate;
            self.stable = 0.;
        }
        self.stable += dt.clamp(0., 0.05);
        if self.stable >= 0.4 {
            self.flipped = candidate;
        }
        self.flipped
    }
}

pub fn gate_x(x: u16, width: u16, flipped: bool) -> u16 {
    if flipped { 320 - x - width } else { x }
}

pub fn touch(x: f32, y: f32, flipped: bool) -> (f32, f32) {
    if flipped { (319. - x, 239. - y) } else { (x, y) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orientation_requires_dwell_and_keeps_its_pose_when_flat_or_sideways() {
        let mut o = Landscape::new();
        for _ in 0..10 {
            assert!(!o.observe([0., 1.], 0.016));
        }
        for _ in 0..30 {
            o.observe([0., 1.], 0.016);
        }
        for _ in 0..100 {
            assert!(o.observe([0., 0.], 0.016));
            assert!(o.observe([1., 0.], 0.016));
            assert!(o.observe([0., -0.5], 0.016));
        }
        for _ in 0..40 {
            o.observe([0., -1.], 0.016);
        }
        assert!(!o.observe([0., -1.], 0.016));
    }
    #[test]
    fn flipped_touch_and_scan_groups_are_involutions() {
        for x in 0..320 {
            let (a, b) = touch(x as f32, 80., true);
            assert_eq!(touch(a, b, true), (x as f32, 80.));
        }
        for x in (0..320).step_by(8) {
            assert_eq!(gate_x(gate_x(x, 8, true), 8, true), x);
        }
    }
}
