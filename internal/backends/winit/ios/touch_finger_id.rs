// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Maps iOS winit touch ids (UITouch pointer addresses) to small stable i32
//! finger ids, without ever casting the u64.

#[derive(Default)]
pub(crate) struct TouchFingerIdAllocator {
    /// Index in this array = the allocated finger id; entry = the winit touch
    /// id currently occupying that slot.
    slots: [Option<u64>; 8],
}

impl TouchFingerIdAllocator {
    /// Returns the finger id for `winit_id`, claiming the lowest free slot for
    /// ids not seen yet. Returns `None` when all slots are taken (the event is
    /// then dropped; the core's gesture recognition tracks at most 5 touches anyway).
    pub(crate) fn id_for(&mut self, winit_id: u64) -> Option<i32> {
        // Re-use the slot already claimed by this touch id.
        for (slot, id) in self.slots.iter().zip(0i32..) {
            if *slot == Some(winit_id) {
                return Some(id);
            }
        }
        // Otherwise claim the lowest free slot.
        for (slot, id) in self.slots.iter_mut().zip(0i32..) {
            if slot.is_none() {
                *slot = Some(winit_id);
                return Some(id);
            }
        }
        None
    }

    /// Returns the finger id for `winit_id` and frees its slot.
    pub(crate) fn take(&mut self, winit_id: u64) -> Option<i32> {
        for (slot, id) in self.slots.iter_mut().zip(0i32..) {
            if *slot == Some(winit_id) {
                *slot = None;
                return Some(id);
            }
        }
        None
    }
}
