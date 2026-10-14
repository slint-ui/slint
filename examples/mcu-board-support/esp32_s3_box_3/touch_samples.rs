// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicU32, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TouchState {
    Contact { x: u16, y: u16 },
    Released,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TouchSample {
    pub(super) state: TouchState,
    pub(super) completed_us: u64,
}

pub(super) struct TouchSamples<const N: usize = 8> {
    slots: UnsafeCell<[MaybeUninit<TouchSample>; N]>,
    written: AtomicU32,
    consumed: AtomicU32,
}

// Split handles make each cursor exclusive. Release/Acquire publication protects
// slot access between the producer and consumer.
unsafe impl<const N: usize> Sync for TouchSamples<N> {}

pub(super) struct TouchProducer<'a, const N: usize = 8> {
    samples: &'a TouchSamples<N>,
}

pub(super) struct TouchConsumer<'a, const N: usize = 8> {
    samples: &'a TouchSamples<N>,
}

pub(super) struct TouchReceiver<'a, const N: usize = 8> {
    consumer: TouchConsumer<'a, N>,
    pending: Option<TouchSample>,
}

impl<'a, const N: usize> TouchReceiver<'a, N> {
    pub(super) fn new(consumer: TouchConsumer<'a, N>) -> Self {
        Self { consumer, pending: None }
    }

    /// Keep contact edges in order and use the latest queued position during a drag.
    pub(super) fn next(&mut self, pressed: bool) -> Option<TouchSample> {
        let mut sample = self.pending.take().or_else(|| self.consumer.pop())?;
        if pressed && matches!(sample.state, TouchState::Contact { .. }) {
            // Bound work even if the other core continues publishing samples.
            for _ in 1..N {
                let Some(next) = self.consumer.pop() else { break };
                if matches!(next.state, TouchState::Released) {
                    self.pending = Some(next);
                    break;
                }
                sample = next;
            }
        }
        Some(sample)
    }
}

impl<const N: usize> TouchSamples<N> {
    pub(super) const fn new() -> Self {
        assert!(N.is_power_of_two() && N <= (1 << 30));
        Self {
            slots: UnsafeCell::new([MaybeUninit::uninit(); N]),
            written: AtomicU32::new(0),
            consumed: AtomicU32::new(0),
        }
    }

    pub(super) fn split(&mut self) -> (TouchProducer<'_, N>, TouchConsumer<'_, N>) {
        let samples = &*self;
        (TouchProducer { samples }, TouchConsumer { samples })
    }
}

impl<const N: usize> TouchProducer<'_, N> {
    pub(super) fn push(&mut self, sample: TouchSample) -> Result<(), TouchSample> {
        let written = self.samples.written.load(Ordering::Relaxed);
        let consumed = self.samples.consumed.load(Ordering::Acquire);
        if written.wrapping_sub(consumed) == N as u32 {
            return Err(sample);
        }
        let index = written as usize & (N - 1);
        // Only this producer writes the unconsumed slot. The consumer releases
        // the slot before this cursor can wrap back to it.
        unsafe { (*self.samples.slots.get())[index].write(sample) };
        self.samples.written.store(written.wrapping_add(1), Ordering::Release);
        Ok(())
    }
}

impl<const N: usize> TouchConsumer<'_, N> {
    pub(super) fn pop(&mut self) -> Option<TouchSample> {
        let consumed = self.samples.consumed.load(Ordering::Relaxed);
        let written = self.samples.written.load(Ordering::Acquire);
        if consumed == written {
            return None;
        }
        let index = consumed as usize & (N - 1);
        // Acquire observes initialization; the producer cannot reuse this slot
        // until the consumed cursor is released below.
        let sample = unsafe { (*self.samples.slots.get())[index].assume_init_read() };
        self.samples.consumed.store(consumed.wrapping_add(1), Ordering::Release);
        Some(sample)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contact(value: u16) -> TouchSample {
        TouchSample {
            state: TouchState::Contact { x: value, y: !value },
            completed_us: (value as u64) << 32 | (!value as u64),
        }
    }

    #[test]
    fn preserves_contacts_and_release_in_order() {
        let mut samples = TouchSamples::<8>::new();
        let (mut producer, mut consumer) = samples.split();
        assert_eq!(consumer.pop(), None);
        let release = TouchSample { state: TouchState::Released, completed_us: u64::MAX };
        let expected = [contact(0), contact(u16::MAX), release, contact(123)];
        for sample in expected {
            assert_eq!(producer.push(sample), Ok(()));
        }
        for sample in expected {
            assert_eq!(consumer.pop(), Some(sample));
        }
        assert_eq!(consumer.pop(), None);
    }

    #[test]
    fn full_queue_returns_release_for_retry_without_overwriting() {
        let mut samples = TouchSamples::<2>::new();
        let (mut producer, mut consumer) = samples.split();
        producer.push(contact(1)).unwrap();
        producer.push(contact(2)).unwrap();
        let release = TouchSample { state: TouchState::Released, completed_us: 900 };
        assert_eq!(producer.push(release), Err(release));
        assert_eq!(consumer.pop(), Some(contact(1)));
        producer.push(release).unwrap();
        assert_eq!(consumer.pop(), Some(contact(2)));
        assert_eq!(consumer.pop(), Some(release));
        assert_eq!(consumer.pop(), None);
    }

    #[test]
    fn receiver_keeps_press_release_and_next_press_separate() {
        let mut samples = TouchSamples::<8>::new();
        let (mut producer, consumer) = samples.split();
        let mut receiver = TouchReceiver::new(consumer);
        let release = TouchSample { state: TouchState::Released, completed_us: 99 };
        for sample in [contact(1), contact(2), contact(3), release, contact(4), contact(5)] {
            producer.push(sample).unwrap();
        }
        assert_eq!(receiver.next(false), Some(contact(1)));
        assert_eq!(receiver.next(true), Some(contact(3)));
        assert_eq!(receiver.next(true), Some(release));
        assert_eq!(receiver.next(false), Some(contact(4)));
        assert_eq!(receiver.next(true), Some(contact(5)));
        assert_eq!(receiver.next(true), None);
    }

    #[test]
    fn receiver_retains_release_when_new_reports_arrive() {
        let mut samples = TouchSamples::<4>::new();
        let (mut producer, consumer) = samples.split();
        let mut receiver = TouchReceiver::new(consumer);
        let release = TouchSample { state: TouchState::Released, completed_us: 99 };
        producer.push(contact(1)).unwrap();
        producer.push(release).unwrap();
        assert_eq!(receiver.next(true), Some(contact(1)));
        for value in 2..6 {
            producer.push(contact(value)).unwrap();
        }
        assert_eq!(receiver.next(true), Some(release));
        assert_eq!(receiver.next(false), Some(contact(2)));
        assert_eq!(receiver.next(true), Some(contact(5)));
        assert_eq!(receiver.next(true), None);
    }

    #[test]
    fn receiver_forwards_release_without_pending_movement() {
        let mut samples = TouchSamples::<1>::new();
        let (mut producer, consumer) = samples.split();
        let mut receiver = TouchReceiver::new(consumer);
        let release = TouchSample { state: TouchState::Released, completed_us: 99 };
        producer.push(release).unwrap();
        assert_eq!(receiver.next(true), Some(release));
        producer.push(contact(1)).unwrap();
        assert_eq!(receiver.next(false), Some(contact(1)));
        assert_eq!(receiver.next(true), None);
    }

    #[test]
    fn cursors_wrap_without_losing_slots() {
        let mut samples = TouchSamples::<8>::new();
        samples.written.store(u32::MAX - 3, Ordering::Relaxed);
        samples.consumed.store(u32::MAX - 3, Ordering::Relaxed);
        let (mut producer, mut consumer) = samples.split();
        for cycle in 0..64u16 {
            for index in 0..8 {
                producer.push(contact(cycle * 8 + index)).unwrap();
            }
            assert_eq!(producer.push(contact(999)), Err(contact(999)));
            for index in 0..8 {
                assert_eq!(consumer.pop(), Some(contact(cycle * 8 + index)));
            }
            assert_eq!(consumer.pop(), None);
        }
    }

    #[test]
    fn repeated_splits_resume_pending_samples() {
        let mut samples = TouchSamples::<2>::new();
        {
            let (mut producer, mut consumer) = samples.split();
            producer.push(contact(5)).unwrap();
            assert_eq!(consumer.pop(), Some(contact(5)));
            producer.push(contact(6)).unwrap();
        }
        let (mut producer, mut consumer) = samples.split();
        assert_eq!(consumer.pop(), Some(contact(6)));
        producer.push(contact(7)).unwrap();
        assert_eq!(consumer.pop(), Some(contact(7)));
    }

    #[test]
    fn transfers_complete_samples_between_threads() {
        let mut samples = TouchSamples::<8>::new();
        let (mut producer, mut consumer) = samples.split();
        std::thread::scope(|scope| {
            scope.spawn(move || {
                for value in 0..50_000u16 {
                    let sample = contact(value);
                    while producer.push(sample).is_err() {
                        std::thread::yield_now();
                    }
                }
            });
            for value in 0..50_000u16 {
                loop {
                    if let Some(sample) = consumer.pop() {
                        assert_eq!(sample, contact(value));
                        break;
                    }
                    std::thread::yield_now();
                }
            }
            assert_eq!(consumer.pop(), None);
        });
    }

    #[test]
    #[should_panic]
    fn rejects_capacity_that_breaks_cursor_wrap() {
        TouchSamples::<3>::new();
    }

    #[test]
    #[should_panic]
    fn rejects_zero_capacity() {
        TouchSamples::<0>::new();
    }
}
