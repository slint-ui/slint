// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU32, Ordering};

const FREE: u32 = 0;
const BUILDING: u32 = 1;
const READY: u32 = 2;
const ACTIVE: u32 = 3;

pub struct Slot<T> {
    state: AtomicU32,
    value: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for Slot<T> {}

impl<T> Slot<T> {
    pub const fn new(value: T) -> Self {
        Self { state: AtomicU32::new(FREE), value: UnsafeCell::new(value) }
    }

    pub fn try_write(&self) -> Option<Write<'_, T>> {
        self.state
            .compare_exchange(FREE, BUILDING, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| Write { slot: self, published: false, ownership: core::marker::PhantomData })
    }

    pub fn try_read(&self) -> Option<Read<'_, T>> {
        self.state
            .compare_exchange(READY, ACTIVE, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| Read { slot: self, ownership: core::marker::PhantomData })
    }
}

pub struct Write<'a, T> {
    slot: &'a Slot<T>,
    published: bool,
    ownership: core::marker::PhantomData<&'a mut T>,
}

impl<T> Write<'_, T> {
    pub fn publish(mut self) {
        self.published = true;
        self.slot.state.store(READY, Ordering::Release);
    }
}

impl<T> Deref for Write<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.slot.value.get() }
    }
}
impl<T> DerefMut for Write<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.slot.value.get() }
    }
}
impl<T> Drop for Write<'_, T> {
    fn drop(&mut self) {
        if !self.published {
            self.slot.state.store(FREE, Ordering::Release);
        }
    }
}

pub struct Read<'a, T> {
    slot: &'a Slot<T>,
    ownership: core::marker::PhantomData<&'a mut T>,
}
impl<T> Deref for Read<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.slot.value.get() }
    }
}
impl<T> Drop for Read<'_, T> {
    fn drop(&mut self) {
        self.slot.state.store(FREE, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dma_lease_keeps_the_packet_immutable_until_completion() {
        let slot = Slot::new([0; 1024]);
        let mut packet = slot.try_write().unwrap();
        packet.fill(37);
        assert!(slot.try_read().is_none());
        assert!(slot.try_write().is_none());
        packet.publish();
        assert!(slot.try_write().is_none());
        let dma = slot.try_read().unwrap();
        assert!(slot.try_read().is_none());
        assert!(slot.try_write().is_none());
        assert_eq!(*dma, [37; 1024]);
        drop(dma);
        assert!(slot.try_write().is_some());
    }

    #[test]
    fn cancelled_build_is_reusable_without_becoming_readable() {
        let slot = Slot::new(0);
        *slot.try_write().unwrap() = 12;
        assert!(slot.try_read().is_none());
        let mut next = slot.try_write().unwrap();
        *next = 44;
        next.publish();
        assert_eq!(*slot.try_read().unwrap(), 44);
    }

    #[test]
    fn cores_preserve_fifo_and_never_observe_partial_packets() {
        for count in [2, 3] {
            let slots =
                std::sync::Arc::new((0..count).map(|_| Slot::new([0u32; 128])).collect::<Vec<_>>());
            let producer = slots.clone();
            let writer = std::thread::spawn(move || {
                for sequence in 0..20000u32 {
                    let slot = &producer[sequence as usize % producer.len()];
                    let mut packet = loop {
                        if let Some(packet) = slot.try_write() {
                            break packet;
                        }
                        std::thread::yield_now();
                    };
                    packet.fill(sequence);
                    packet.publish();
                }
            });
            for sequence in 0..20000u32 {
                let slot = &slots[sequence as usize % slots.len()];
                let packet = loop {
                    if let Some(packet) = slot.try_read() {
                        break packet;
                    }
                    std::thread::yield_now();
                };
                assert_eq!(*packet, [sequence; 128]);
            }
            writer.join().unwrap();
        }
    }
}
