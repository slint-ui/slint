// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Maps winit touch ids to small stable i32 finger ids.
//!
//! winit's touch id doesn't fit an i32 on every platform.
//! Native iOS stores a `UITouch` pointer address in it.
//! The web backend sign-extends Safari's negative `PointerEvent.pointerId`,
//! which lands far outside i32 range even though the id itself would fit.
//!
//! The id is also only unique within its device, so the key includes the
//! device the touch came from.

/// A winit touch, identified by its device and the id winit gave it there.
pub(crate) type TouchId = (winit::event::DeviceId, u64);

pub(crate) struct TouchFingerIdAllocator<Id = TouchId> {
    /// Index in this vector = the allocated finger id; entry = the touch
    /// currently occupying that slot.
    slots: Vec<Option<Id>>,
}

impl<Id> Default for TouchFingerIdAllocator<Id> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<Id: Copy + PartialEq> TouchFingerIdAllocator<Id> {
    /// Returns the finger id for `touch`, claiming the lowest free slot for touches not seen yet.
    /// The slots grow with the number of fingers down at once, so no touch is dropped.
    pub(crate) fn id_for(&mut self, touch: Id) -> i32 {
        let index = self
            .slot_of(touch)
            .or_else(|| self.slots.iter().position(Option::is_none))
            .unwrap_or_else(|| {
                self.slots.push(None);
                self.slots.len() - 1
            });
        self.slots[index] = Some(touch);
        index as i32
    }

    pub(crate) fn take(&mut self, touch: Id) -> Option<i32> {
        let index = self.slot_of(touch)?;
        self.slots[index] = None;
        Some(index as i32)
    }

    fn slot_of(&self, touch: Id) -> Option<usize> {
        self.slots.iter().position(|slot| *slot == Some(touch))
    }
}

#[cfg(test)]
mod tests {
    use super::TouchFingerIdAllocator;

    /// Stands in for `winit::event::DeviceId`, which only exposes one value to construct.
    type TestTouchId = (u8, u64);

    fn allocator() -> TouchFingerIdAllocator<TestTouchId> {
        TouchFingerIdAllocator::default()
    }

    #[test]
    fn allocates_and_frees_slots() {
        let mut alloc = allocator();
        assert_eq!(alloc.id_for((0, 11)), 0);
        assert_eq!(alloc.id_for((0, 22)), 1);
        // The same touch keeps its id for the rest of the gesture.
        assert_eq!(alloc.id_for((0, 11)), 0);
        assert_eq!(alloc.take((0, 11)), Some(0));
        // The freed slot is the lowest one, so the next touch reclaims it.
        assert_eq!(alloc.id_for((0, 33)), 0);
        assert_eq!(alloc.take((0, 99)), None);
    }

    #[test]
    fn handles_ids_outside_i32_range() {
        let mut alloc = allocator();
        // Converting this id used to panic.
        let sign_extended = -440423654i64 as u64;
        assert!(i32::try_from(sign_extended).is_err());
        assert_eq!(alloc.id_for((0, sign_extended)), 0);
        assert_eq!(alloc.take((0, sign_extended)), Some(0));
    }

    #[test]
    fn tracks_more_fingers_than_one_pair_of_hands() {
        let mut alloc = allocator();
        for i in 0..20 {
            assert_eq!(alloc.id_for((0, 100 + i as u64)), i);
        }
        for i in 0..20 {
            assert_eq!(alloc.take((0, 100 + i as u64)), Some(i));
        }
    }

    #[test]
    fn keeps_touches_from_different_devices_apart() {
        let mut alloc = allocator();
        // Devices number their touches independently, so the same id is two fingers here.
        assert_eq!(alloc.id_for((0, 1)), 0);
        assert_eq!(alloc.id_for((1, 1)), 1);
        assert_eq!(alloc.take((0, 1)), Some(0));
        assert_eq!(alloc.id_for((1, 1)), 1);
    }
}
