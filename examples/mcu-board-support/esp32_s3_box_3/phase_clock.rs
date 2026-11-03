// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering, fence};

pub(super) struct Stamp {
    sequence: AtomicU32,
    blank_us: AtomicU32,
}
impl Stamp {
    pub(super) const fn new() -> Self {
        Self { sequence: AtomicU32::new(0), blank_us: AtomicU32::new(0) }
    }
    pub(super) fn publish(&self, tick: &AtomicU32, cycles: u32, blank_us: u32) -> u32 {
        self.sequence.fetch_add(1, Ordering::AcqRel);
        self.blank_us.store(blank_us, Ordering::Relaxed);
        let next = tick.fetch_add(cycles, Ordering::Release).wrapping_add(cycles);
        self.sequence.fetch_add(1, Ordering::Release);
        next
    }
    pub(super) fn snapshot(&self, tick: &AtomicU32) -> (u32, u32) {
        loop {
            let sequence = self.sequence.load(Ordering::Acquire);
            if sequence & 1 != 0 {
                core::hint::spin_loop();
                continue;
            }
            let tick = tick.load(Ordering::Acquire);
            let blank = self.blank_us.load(Ordering::Relaxed);
            fence(Ordering::Acquire);
            if self.sequence.load(Ordering::Acquire) == sequence {
                return (tick, blank);
            }
        }
    }
}

// Leave margin for the producer after a crossed-refresh entry.
const FRESH_ENTRY_US: u32 = 500;

/// Only recover an iteration that crossed a refresh and finished too late to
/// start the next frame in that already-aged refresh. Keep a fresh arrival.
pub(crate) fn recovery_tick(source: u32, current: u32, blank_us: u32, now_us: u32) -> Option<u32> {
    if blank_us == 0 || (current.wrapping_sub(source) as i32) <= 0 {
        return None;
    }
    let age = now_us.wrapping_sub(blank_us);
    (age > FRESH_ENTRY_US && age < 40_000).then_some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_and_bootstrap_phases_are_not_discarded() {
        assert_eq!(recovery_tick(9, 10, 10000, 10500), None);
        assert_eq!(recovery_tick(9, 10, 10000, 10501), Some(10));
        assert_eq!(recovery_tick(9, 10, 0, 11001), None);
        assert_eq!(recovery_tick(10, 10, 10000, 24000), None);
    }
    #[test]
    fn recovery_is_once_and_preserves_tick_and_timestamp_wrap() {
        assert_eq!(recovery_tick(u32::MAX, 0, u32::MAX - 100, 1300), Some(0));
        assert_eq!(recovery_tick(0, 0, u32::MAX - 100, 1300), None);
        assert_eq!(recovery_tick(9, 12, 10000, 24000), Some(12));
        assert_eq!(recovery_tick(9, 8, 10000, 24000), None);
        assert_eq!(recovery_tick(9, 10, 10000, 50000), None);
    }
    #[test]
    fn phase_stamp_pairs_multi_cycle_updates() {
        let stamp = Stamp::new();
        let tick = AtomicU32::new(u32::MAX - 1);
        stamp.publish(&tick, 3, 77);
        assert_eq!(stamp.snapshot(&tick), (1, 77));
    }
    #[test]
    fn concurrent_snapshots_do_not_mix_tick_and_timestamp() {
        let shared = std::sync::Arc::new((Stamp::new(), AtomicU32::new(0)));
        let writer = shared.clone();
        let thread = std::thread::spawn(move || {
            for tick in 1..50000 {
                writer.0.publish(&writer.1, 1, tick * 7);
            }
        });
        loop {
            let (tick, blank) = shared.0.snapshot(&shared.1);
            assert_eq!(blank, tick * 7);
            if tick == 49999 {
                break;
            }
        }
        thread.join().unwrap();
    }
}
