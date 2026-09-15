// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Maps winit touch ids to small stable i32 finger ids.
//!
//! winit's touch id doesn't fit an i32 on every platform.
//! Native iOS stores a `UITouch` pointer address in it.
//! The web backend sign-extends Safari's negative `PointerEvent.pointerId`,
//! which lands far outside i32 range even though the id itself would fit.

/// Fingers tracked at once, chosen with headroom over the five the core's
/// gesture recognition follows.
const SLOTS: usize = 8;

#[derive(Default)]
pub(crate) struct TouchFingerIdAllocator {
    /// Index in this array = the allocated finger id; entry = the winit touch
    /// id currently occupying that slot.
    slots: [Option<u64>; SLOTS],
}

impl TouchFingerIdAllocator {
    /// Returns the finger id for `winit_id`, claiming the lowest free slot for ids not seen yet.
    /// Returns `None` once every slot is taken, which drops the event.
    pub(crate) fn id_for(&mut self, winit_id: u64) -> Option<i32> {
        let index =
            self.slot_of(winit_id).or_else(|| self.slots.iter().position(Option::is_none))?;
        self.slots[index] = Some(winit_id);
        Some(index as i32)
    }

    pub(crate) fn take(&mut self, winit_id: u64) -> Option<i32> {
        let index = self.slot_of(winit_id)?;
        self.slots[index] = None;
        Some(index as i32)
    }

    fn slot_of(&self, winit_id: u64) -> Option<usize> {
        self.slots.iter().position(|slot| *slot == Some(winit_id))
    }
}

#[cfg(test)]
mod tests {
    use super::{SLOTS, TouchFingerIdAllocator};

    #[test]
    fn allocates_and_frees_slots() {
        let mut alloc = TouchFingerIdAllocator::default();
        assert_eq!(alloc.id_for(11), Some(0));
        assert_eq!(alloc.id_for(22), Some(1));
        // The same touch keeps its id for the rest of the gesture.
        assert_eq!(alloc.id_for(11), Some(0));
        assert_eq!(alloc.take(11), Some(0));
        // The freed slot is the lowest one, so the next touch reclaims it.
        assert_eq!(alloc.id_for(33), Some(0));
        assert_eq!(alloc.take(99), None);
    }

    #[test]
    fn handles_ids_outside_i32_range() {
        let mut alloc = TouchFingerIdAllocator::default();
        // Converting this id used to panic.
        let sign_extended = -440423654i64 as u64;
        assert!(i32::try_from(sign_extended).is_err());
        assert_eq!(alloc.id_for(sign_extended), Some(0));
        assert_eq!(alloc.take(sign_extended), Some(0));
    }

    #[test]
    fn drops_events_once_every_slot_is_taken() {
        let mut alloc = TouchFingerIdAllocator::default();
        for i in 0..SLOTS {
            assert_eq!(alloc.id_for(i as u64), Some(i as i32));
        }
        assert_eq!(alloc.id_for(999), None);
    }
}
