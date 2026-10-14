// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use alloc::boxed::Box;
use core::cell::RefCell;

i_slint_core::thread_local! {
    static HANDLER: RefCell<Option<Box<dyn FnMut(u64)>>> = RefCell::new(None);
}

/// Set the callback that prepares animation state for the next panel refresh.
/// Call this on the Slint thread. The callback runs there once per acquired
/// panel tick, before Slint timers, input dispatch, and rendering.
pub(super) fn set_handler(callback: impl FnMut(u64) + 'static) {
    HANDLER.with(|slot| *slot.borrow_mut() = Some(Box::new(callback)));
}

pub(super) fn invoke(frame: u64) {
    let Some(mut callback) = HANDLER.with(|slot| slot.borrow_mut().take()) else { return };
    callback(frame);
    HANDLER.with(|slot| {
        // A callback may install its successor without borrowing its own slot.
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(callback);
        }
    });
}

#[derive(Default)]
pub(super) struct FrameClock {
    previous: Option<u32>,
    frame: u64,
}

impl FrameClock {
    pub(super) fn acquire(&mut self, tick: u32) -> Option<u64> {
        if self.previous == Some(tick) {
            return None;
        }
        // Zero is the worker's bootstrap value, before the first scanline tick.
        if self.previous.is_none() && tick == 0 {
            self.previous = Some(0);
            return None;
        }
        self.frame = match self.previous {
            Some(previous) => self.frame + tick.wrapping_sub(previous) as u64,
            None => tick as u64,
        };
        self.previous = Some(tick);
        Some(self.frame + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameClock, invoke, set_handler};
    #[test]
    fn callback_can_install_its_successor() {
        use alloc::rc::Rc;
        use core::cell::Cell;
        let first = Rc::new(Cell::new(0));
        let next = Rc::new(Cell::new(0));
        let a = first.clone();
        let b = next.clone();
        set_handler(move |frame| {
            a.set(frame);
            let b = b.clone();
            set_handler(move |frame| b.set(frame));
        });
        invoke(11);
        invoke(12);
        assert_eq!(first.get(), 11);
        assert_eq!(next.get(), 12);
    }

    #[test]
    fn repeated_ticks_do_not_invoke_another_frame() {
        let mut clock = FrameClock::default();
        assert_eq!(clock.acquire(0), None);
        assert_eq!(clock.acquire(0), None);
        assert_eq!(clock.acquire(1), Some(2));
        assert_eq!(clock.acquire(1), None);
    }
    #[test]
    fn skipped_ticks_advance_time_without_catch_up_callbacks() {
        let mut clock = FrameClock::default();
        assert_eq!(clock.acquire(9), Some(10));
        assert_eq!(clock.acquire(12), Some(13));
        assert_eq!(clock.acquire(12), None);
    }
    #[cfg(feature = "box-parallel")]
    #[test]
    fn recovering_an_aged_tick_advances_without_a_catch_up_callback() {
        let mut clock = FrameClock::default();
        let pending_target = clock.acquire(10);
        assert_eq!(pending_target, Some(11));
        let recovered = super::super::frame_queue::phase_clock::recovery_tick(10, 11, 10000, 10501);
        assert_eq!(recovered, Some(11));
        // No callback is made for the abandoned acquisition of tick 11.
        assert_eq!(clock.acquire(12), Some(13));
        assert_eq!(clock.acquire(12), None);
        assert_eq!(pending_target, Some(11));
    }
    #[test]
    fn tick_wrap_preserves_monotonic_animation_time() {
        let mut clock = FrameClock::default();
        assert_eq!(clock.acquire(u32::MAX - 1), Some(u32::MAX as u64));
        assert_eq!(clock.acquire(1), Some(u32::MAX as u64 + 3));
        assert_eq!(clock.acquire(2), Some(u32::MAX as u64 + 4));
    }
}
