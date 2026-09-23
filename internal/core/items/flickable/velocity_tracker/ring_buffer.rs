// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A fixed-capacity ring buffer of time-stamped position deltas

use crate::animations::Instant;
use crate::lengths::{LogicalPx, LogicalVector};
use euclid::Vector2D;

/// Simple ringbuffer storing time and delta tuples
pub(crate) struct VelocityRingBuffer<const N: usize, T = Instant> {
    /// Pointing to the next free element
    curr_index: usize,
    /// Indicates if the buffer is full
    full: bool,
    values: [(T, Vector2D<f32, LogicalPx>); N],
}

impl<const N: usize, T: Copy + Default> Default for VelocityRingBuffer<N, T> {
    fn default() -> Self {
        // Placeholder timestamps; `curr_index`/`full` track which entries are real.
        Self { curr_index: 0, full: false, values: [(T::default(), Vector2D::default()); N] }
    }
}

impl<const N: usize, T: Copy + core::fmt::Debug> core::fmt::Debug for VelocityRingBuffer<N, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "VelocityRingBuffer({}): ", self.len())?;
        if self.empty() {
            writeln!(f, "Empty")
        } else {
            write!(f, "[")?;
            for e in self.iter() {
                write!(f, "{e:?},")?;
            }
            write!(f, "]")
        }
    }
}

impl<'a, const N: usize, T: Copy> VelocityRingBuffer<N, T> {
    pub fn iter(&'a self) -> VelocityRingBufferIterator<'a, N, T> {
        VelocityRingBufferIterator::new(self)
    }

    /// Indicates if the buffer is empty
    pub fn empty(&self) -> bool {
        !(self.full || self.curr_index > 0)
    }

    /// Add a new element to the ringbuffer
    pub fn push(&mut self, time: T, position_delta: LogicalVector) {
        if self.curr_index < self.values.len() {
            self.values[self.curr_index] = (time, position_delta.cast());
        }
        self.curr_index += 1;
        if self.curr_index >= N {
            self.full = true;
            self.curr_index = 0;
        }
    }

    fn next_index(&self, curr_index: usize) -> usize {
        if curr_index >= N - 1 { 0 } else { curr_index + 1 }
    }

    fn prev_index(&self, curr_index: usize) -> usize {
        if curr_index > 0 { curr_index - 1 } else { N - 1 }
    }

    /// Index of the most recent added value
    fn latest_index(&self) -> usize {
        if self.curr_index > 0 { self.curr_index - 1 } else { N - 1 }
    }

    pub fn len(&self) -> usize {
        if self.full { N } else { self.curr_index }
    }

    /// Returns the last time value added to the buffer if not empty otherwise None
    pub fn last_time(&self) -> Option<T> {
        if !self.empty() { Some(self.values[self.latest_index()].0) } else { None }
    }
}

pub(crate) struct VelocityRingBufferIterator<'a, const N: usize, T = Instant> {
    count: usize,
    curr: usize,
    curr_back: usize,
    buffer: &'a VelocityRingBuffer<N, T>,
    empty: bool,
}

impl<'a, const N: usize, T: Copy> VelocityRingBufferIterator<'a, N, T> {
    fn new(buffer: &'a VelocityRingBuffer<N, T>) -> Self {
        let curr = if buffer.full {
            // curr_index points to the oldest value which will be overwritten
            // at the next push
            buffer.curr_index
        } else {
            0
        };
        Self { empty: buffer.empty(), curr, curr_back: buffer.latest_index(), count: 0, buffer }
    }
}

impl<'a, const N: usize, T: Copy> Iterator for VelocityRingBufferIterator<'a, N, T> {
    type Item = &'a (T, Vector2D<f32, LogicalPx>);

    fn next(&mut self) -> Option<Self::Item> {
        let max_count = if self.buffer.full { N } else { self.buffer.latest_index() + 1 };
        if self.empty || self.count >= max_count {
            return None;
        }

        self.count += 1;

        let curr = self.curr;
        self.curr = self.buffer.next_index(self.curr);
        Some(&self.buffer.values[curr])
    }
}

impl<'a, const N: usize, T: Copy> DoubleEndedIterator for VelocityRingBufferIterator<'a, N, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let max_count = if self.buffer.full { N } else { self.buffer.latest_index() + 1 };
        if self.empty || self.count >= max_count {
            return None;
        }

        self.count += 1;

        let curr_back = self.curr_back;
        self.curr_back = self.buffer.prev_index(self.curr_back);
        Some(&self.buffer.values[curr_back])
    }
}

#[cfg(test)]
mod tests_ring_buffer {
    use super::*;
    use crate::animations::Instant;
    use core::time::Duration;

    #[test]
    fn test_empty_buffer() {
        let buffer: VelocityRingBuffer<5> = VelocityRingBuffer::default();
        assert!(buffer.empty());
        assert_eq!(buffer.curr_index, 0);
        assert!(!buffer.full);
        assert_eq!(buffer.last_time(), None);
    }

    #[test]
    fn test_push_single_element() {
        let mut buffer: VelocityRingBuffer<5> = VelocityRingBuffer::default();
        let time = Instant::default();
        let delta = Vector2D::new(10.0, 20.0);

        buffer.push(time, delta);

        assert!(!buffer.empty());
        assert_eq!(buffer.curr_index, 1);
        assert!(!buffer.full);
        assert_eq!(buffer.latest_index(), 0);
        assert_eq!(buffer.last_time(), Some(time));
    }

    /// Buffer not complete full
    #[test]
    fn test_push_two_elements() {
        let mut buffer: VelocityRingBuffer<5> = VelocityRingBuffer::default();
        let time = Instant::default();

        buffer.push(time, Vector2D::new(10.0, 20.0));
        buffer.push(time + Duration::from_millis(100), Vector2D::new(13.0, -5.0));

        assert!(!buffer.empty());
        assert_eq!(buffer.curr_index, 2);
        assert!(!buffer.full);
        assert_eq!(buffer.latest_index(), 1);
        assert_eq!(buffer.last_time(), Some(time + Duration::from_millis(100)));
    }

    #[test]
    fn test_push_until_full() {
        let mut buffer: VelocityRingBuffer<5> = VelocityRingBuffer::default();
        let base_time = Instant::default();

        // Push elements to fill the buffer
        for i in 0..5 {
            let time = base_time + Duration::from_millis(i * 100);
            buffer.push(time, Vector2D::new(1.0, -2.0));
        }

        assert!(!buffer.empty());
        assert_eq!(buffer.curr_index, 0);
        assert!(buffer.full);
        assert_eq!(buffer.last_time(), Some(base_time + Duration::from_millis(400)));
        assert_eq!(buffer.latest_index(), 4);
    }

    #[test]
    fn test_push_beyond_capacity() {
        const CAP: usize = 5;
        let mut buffer: VelocityRingBuffer<CAP> = VelocityRingBuffer::default();
        let base_time = Instant::default();

        // Push more than capacity
        for i in 0..(CAP + 2) {
            let time = base_time + Duration::from_millis(i as u64 * 100);
            buffer.push(time, Vector2D::new(1.0, 2.0));
        }

        assert!(!buffer.empty());
        assert!(buffer.full);
        assert_eq!(buffer.curr_index, 2);
        assert_eq!(buffer.latest_index(), 1);
        assert_eq!(buffer.last_time(), Some(base_time + Duration::from_millis(600)));
    }

    #[test]
    fn test_push_beyond_capacity_wrap_back() {
        const CAP: usize = 5;
        let mut buffer: VelocityRingBuffer<CAP> = VelocityRingBuffer::default();
        let base_time = Instant::default();

        // Push more than capacity
        for i in 0..CAP {
            let time = base_time + Duration::from_millis(i as u64 * 100);
            buffer.push(time, Vector2D::new(3.0, -2.0));
        }

        assert!(!buffer.empty());
        assert!(buffer.full);
        assert_eq!(buffer.curr_index, 0);
        assert_eq!(buffer.latest_index(), CAP - 1);
        assert_eq!(buffer.last_time(), Some(base_time + Duration::from_millis(400)));
    }

    #[test]
    fn test_len_tracks_fill_level() {
        let mut buffer: VelocityRingBuffer<4> = VelocityRingBuffer::default();
        assert_eq!(buffer.len(), 0);

        let base_time = Instant::default();
        buffer.push(base_time, Vector2D::new(1.0, 1.0));
        assert_eq!(buffer.len(), 1);

        buffer.push(base_time + Duration::from_millis(10), Vector2D::new(1.0, 1.0));
        buffer.push(base_time + Duration::from_millis(20), Vector2D::new(1.0, 1.0));
        assert_eq!(buffer.len(), 3);

        buffer.push(base_time + Duration::from_millis(30), Vector2D::new(1.0, 1.0));
        assert_eq!(buffer.len(), 4);

        // Wrapping around must not grow `len()` beyond capacity.
        buffer.push(base_time + Duration::from_millis(40), Vector2D::new(1.0, 1.0));
        assert_eq!(buffer.len(), 4);
    }

    #[test]
    fn test_next_index_wraps_at_capacity() {
        let buffer: VelocityRingBuffer<4> = VelocityRingBuffer::default();
        assert_eq!(buffer.next_index(0), 1);
        assert_eq!(buffer.next_index(1), 2);
        assert_eq!(buffer.next_index(2), 3);
        assert_eq!(buffer.next_index(3), 0);
    }

    #[test]
    fn test_iter_on_empty_buffer_yields_nothing() {
        let buffer: VelocityRingBuffer<4> = VelocityRingBuffer::default();
        assert_eq!(buffer.iter().next(), None);
    }

    #[test]
    fn test_iter_partially_filled() {
        let mut buffer: VelocityRingBuffer<5> = VelocityRingBuffer::default();
        let base_time = Instant::default();
        let v0 = Vector2D::new(1.0, 1.0);
        let v1 = Vector2D::new(2.0, 2.0);
        let v2 = Vector2D::new(3.0, 3.0);
        buffer.push(base_time, v0);
        buffer.push(base_time + Duration::from_millis(10), v1);
        buffer.push(base_time + Duration::from_millis(20), v2);

        // The buffer holds 3 of its 5 slots; `iter()` should yield exactly
        // those 3, oldest first, and then stop.
        let mut iter = buffer.iter();
        assert_eq!(iter.next().map(|(_, v)| *v), Some(v0));
        assert_eq!(iter.next().map(|(_, v)| *v), Some(v1));
        assert_eq!(iter.next().map(|(_, v)| *v), Some(v2));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_iter_full() {
        let mut buffer: VelocityRingBuffer<3> = VelocityRingBuffer::default();
        let base_time = Instant::default();
        let v0 = Vector2D::new(1.0, 1.0);
        let v1 = Vector2D::new(2.0, 2.0);
        let v2 = Vector2D::new(3.0, 3.0);
        buffer.push(base_time, v0);
        buffer.push(base_time + Duration::from_millis(10), v1);
        buffer.push(base_time + Duration::from_millis(20), v2);
        assert!(buffer.full);

        let mut iter = buffer.iter();
        assert_eq!(iter.next().map(|(_, v)| *v), Some(v0));
        assert_eq!(iter.next().map(|(_, v)| *v), Some(v1));
        assert_eq!(iter.next().map(|(_, v)| *v), Some(v2));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_iter_wrap_around() {
        let mut buffer: VelocityRingBuffer<3> = VelocityRingBuffer::default();
        let base_time = Instant::default();
        // Push 5 values into a capacity-3 buffer: the first two get
        // overwritten, so only the last 3 pushes should still be observable.
        let values: [_; 5] =
            core::array::from_fn(|i| Vector2D::new(i as f32 + 1.0, i as f32 + 1.0));
        for (i, value) in values.iter().enumerate() {
            buffer.push(base_time + Duration::from_millis(i as u64 * 10), *value);
        }

        // Oldest surviving entry first, then progressively newer.
        let mut iter = buffer.iter();
        assert_eq!(iter.next().map(|(_, v)| *v), Some(values[2]));
        assert_eq!(iter.next().map(|(_, v)| *v), Some(values[3]));
        assert_eq!(iter.next().map(|(_, v)| *v), Some(values[4]));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_iter_next_back_partially_filled() {
        let mut buffer: VelocityRingBuffer<5> = VelocityRingBuffer::default();
        let base_time = Instant::default();
        let v0 = Vector2D::new(1.0, 1.0);
        let v1 = Vector2D::new(2.0, 2.0);
        let v2 = Vector2D::new(3.0, 3.0);
        buffer.push(base_time, v0);
        buffer.push(base_time + Duration::from_millis(10), v1);
        buffer.push(base_time + Duration::from_millis(20), v2);

        // Iterating from the back yields newest first, then progressively older.
        let mut iter = buffer.iter();
        assert_eq!(iter.next_back().map(|(_, v)| *v), Some(v2));
        assert_eq!(iter.next_back().map(|(_, v)| *v), Some(v1));
        assert_eq!(iter.next_back().map(|(_, v)| *v), Some(v0));
        assert_eq!(iter.next_back(), None);
    }

    #[test]
    fn test_iter_next_back_full() {
        let mut buffer: VelocityRingBuffer<3> = VelocityRingBuffer::default();
        let base_time = Instant::default();
        let v0 = Vector2D::new(1.0, 1.0);
        let v1 = Vector2D::new(2.0, 2.0);
        let v2 = Vector2D::new(3.0, 3.0);
        buffer.push(base_time, v0);
        buffer.push(base_time + Duration::from_millis(10), v1);
        buffer.push(base_time + Duration::from_millis(20), v2);
        assert!(buffer.full);

        let mut iter = buffer.iter();
        assert_eq!(iter.next_back().map(|(_, v)| *v), Some(v2));
        assert_eq!(iter.next_back().map(|(_, v)| *v), Some(v1));
        assert_eq!(iter.next_back().map(|(_, v)| *v), Some(v0));
        assert_eq!(iter.next_back(), None);
    }

    #[test]
    fn test_iter_next_back_wrap_around() {
        let mut buffer: VelocityRingBuffer<3> = VelocityRingBuffer::default();
        let base_time = Instant::default();
        // Push 5 values into a capacity-3 buffer: the first two get
        // overwritten, so only the last 3 pushes should still be observable.
        let values: [_; 5] =
            core::array::from_fn(|i| Vector2D::new(i as f32 + 1.0, i as f32 + 1.0));
        for (i, value) in values.iter().enumerate() {
            buffer.push(base_time + Duration::from_millis(i as u64 * 10), *value);
        }

        let mut iter = buffer.iter();
        assert_eq!(iter.next_back().map(|(_, v)| *v), Some(values[4]));
        assert_eq!(iter.next_back().map(|(_, v)| *v), Some(values[3]));
        assert_eq!(iter.next_back().map(|(_, v)| *v), Some(values[2]));
        assert_eq!(iter.next_back(), None);
    }

    #[test]
    fn test_iter_rev_matches_reversed_forward_order() {
        let mut buffer: VelocityRingBuffer<3> = VelocityRingBuffer::default();
        let base_time = Instant::default();
        let v0 = Vector2D::new(1.0, 1.0);
        let v1 = Vector2D::new(2.0, 2.0);
        let v2 = Vector2D::new(3.0, 3.0);
        buffer.push(base_time, v0);
        buffer.push(base_time + Duration::from_millis(10), v1);
        buffer.push(base_time + Duration::from_millis(20), v2);

        let mut iter = buffer.iter().rev();
        assert_eq!(iter.next().map(|(_, v)| *v), Some(v2));
        assert_eq!(iter.next().map(|(_, v)| *v), Some(v1));
        assert_eq!(iter.next().map(|(_, v)| *v), Some(v0));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_iter_next_and_next_back_meet_in_the_middle() {
        let mut buffer: VelocityRingBuffer<5> = VelocityRingBuffer::default();
        let base_time = Instant::default();
        let values: [_; 4] =
            core::array::from_fn(|i| Vector2D::new(i as f32 + 1.0, i as f32 + 1.0));
        for (i, value) in values.iter().enumerate() {
            buffer.push(base_time + Duration::from_millis(i as u64 * 10), *value);
        }

        // Alternating ends should visit every real entry exactly once,
        // without overlap or running past the real data.
        let mut iter = buffer.iter();
        assert_eq!(iter.next().map(|(_, v)| *v), Some(values[0]));
        assert_eq!(iter.next_back().map(|(_, v)| *v), Some(values[3]));
        assert_eq!(iter.next().map(|(_, v)| *v), Some(values[1]));
        assert_eq!(iter.next_back().map(|(_, v)| *v), Some(values[2]));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next_back(), None);
    }
}
