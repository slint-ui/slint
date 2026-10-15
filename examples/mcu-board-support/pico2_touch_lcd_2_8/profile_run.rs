// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering};

pub struct RunGate {
    current: AtomicU32,
    armed: AtomicU32,
}

impl RunGate {
    pub const fn new(mode: u8) -> Self {
        Self { current: AtomicU32::new(mode as u32), armed: AtomicU32::new(u32::MAX) }
    }

    pub fn token(&self) -> u32 {
        self.current.load(Ordering::Acquire)
    }

    pub fn mode(&self) -> u32 {
        self.token() & 255
    }

    pub fn select(&self, mode: u8) -> u32 {
        let next = |run: u32| (run.wrapping_add(256) & !255) | u32::from(mode);
        let mut previous = self.token();
        loop {
            let token = next(previous);
            match self.current.compare_exchange_weak(
                previous,
                token,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return token,
                Err(current) => previous = current,
            }
        }
    }

    pub fn arm(&self, token: u32) {
        self.armed.store(token, Ordering::Release);
    }

    pub fn is_armed(&self, token: u32) -> bool {
        self.armed.load(Ordering::Acquire) == token && self.token() == token
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_starts_disarmed() {
        let gate = RunGate::new(2);
        assert_eq!(gate.mode(), 2);
        assert!(!gate.is_armed(gate.token()));
        gate.arm(gate.token());
        assert!(gate.is_armed(gate.token()));
    }

    #[test]
    fn a_stale_callback_cannot_arm_the_next_mode() {
        let gate = RunGate::new(6);
        let stale = gate.token();
        let current = gate.select(8);
        gate.arm(stale);
        assert!(!gate.is_armed(stale));
        assert!(!gate.is_armed(current));
        gate.arm(current);
        assert!(gate.is_armed(current));
    }

    #[test]
    fn repeating_a_mode_starts_a_distinct_run() {
        let gate = RunGate::new(8);
        let previous = gate.token();
        gate.arm(previous);
        let current = gate.select(8);
        assert_ne!(previous, current);
        assert_eq!(gate.mode(), 8);
        gate.arm(previous);
        assert!(!gate.is_armed(current));
    }

    #[test]
    fn returning_to_a_mode_does_not_reuse_its_previous_token() {
        let gate = RunGate::new(6);
        let previous = gate.token();
        gate.select(8);
        let current = gate.select(6);
        assert_ne!(previous, current);
        gate.arm(previous);
        assert!(!gate.is_armed(current));
    }

    #[test]
    fn generation_wrap_keeps_the_selected_mode() {
        let gate = RunGate::new(2);
        gate.current.store(0xffffff0a, Ordering::Release);
        assert_eq!(gate.select(9), 9);
        assert_eq!(gate.mode(), 9);
        assert!(!gate.is_armed(gate.token()));
    }

    #[test]
    fn delayed_cross_core_arming_does_not_start_the_new_run() {
        use std::sync::{Arc, Barrier};
        let gate = Arc::new(RunGate::new(6));
        let barrier = Arc::new(Barrier::new(2));
        let stale = gate.token();
        let producer = gate.clone();
        let ready = barrier.clone();
        let thread = std::thread::spawn(move || {
            ready.wait();
            producer.arm(stale);
        });
        let current = gate.select(8);
        barrier.wait();
        thread.join().unwrap();
        assert!(!gate.is_armed(current));
        gate.arm(current);
        assert!(gate.is_armed(current));
    }
}
