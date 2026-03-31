// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains a simple ringbuffer to store time and location tuples. It is used in the flickable to
//! determine the initial velocity of the animation

use crate::Coord;
use crate::animations::Instant;
use crate::lengths::LogicalPoint;
use crate::lengths::LogicalPx;
use core::time::Duration;
use euclid::Vector2D;

/// Simple ringbuffer storing time and position tuples
#[derive(Debug)]
pub(crate) struct PositionTimeRingBuffer<const N: usize> {
    /// Pointing to the next free element
    curr_index: usize,
    /// Indicates if the buffer is full
    full: bool,
    values: [(Instant, LogicalPoint); N],
}

impl<const N: usize> Default for PositionTimeRingBuffer<N> {
    fn default() -> Self {
        Self { curr_index: 0, full: false, values: [(Instant::now(), LogicalPoint::default()); N] }
    }
}

impl<const N: usize> PositionTimeRingBuffer<N> {
    /// Indicates if the buffer is empty
    pub fn empty(&self) -> bool {
        !(self.full || self.curr_index > 0)
    }

    /// Add a new element to the ringbuffer
    pub fn push(&mut self, time: Instant, value: LogicalPoint) {
        if self.curr_index < self.values.len() {
            self.values[self.curr_index] = (time, value);
        }
        self.curr_index += 1;
        if self.curr_index >= N {
            self.full = true;
            self.curr_index = 0;
        }
    }

    /// Index of the most recent added value
    fn latest_index(&self) -> usize {
        if self.curr_index > 0 { self.curr_index - 1 } else { N - 1 }
    }

    /// Returns the last time value added to the buffer if not empty otherwise None
    pub fn last_time(&self) -> Option<Instant> {
        if !self.empty() { Some(self.values[self.latest_index()].0) } else { None }
    }

    /// Returns the difference between the oldest and the newest point
    pub fn diff(&self) -> (Duration, Vector2D<Coord, LogicalPx>) {
        if self.full {
            let oldest = self.values[self.curr_index];
            let newest = if self.curr_index > 0 {
                self.values[self.curr_index - 1]
            } else {
                self.values[self.values.len() - 1]
            };
            (newest.0.duration_since(oldest.0), newest.1 - oldest.1)
        } else {
            let oldest = self.values[0];
            let newest = self.values[usize::max(0, self.curr_index - 1)];
            (newest.0.duration_since(oldest.0), newest.1 - oldest.1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::Instant;
    use crate::lengths::LogicalPoint;
    use core::time::Duration;

    #[test]
    fn test_empty_buffer() {
        let buffer: PositionTimeRingBuffer<5> = PositionTimeRingBuffer::default();
        assert!(buffer.empty());
        assert_eq!(buffer.curr_index, 0);
        assert!(!buffer.full);
        assert_eq!(buffer.last_time(), None);
    }

    #[test]
    fn test_push_single_element() {
        let mut buffer: PositionTimeRingBuffer<5> = PositionTimeRingBuffer::default();
        let time = Instant::now();
        let point = LogicalPoint::new(10.0, 20.0);

        buffer.push(time, point);

        assert!(!buffer.empty());
        assert_eq!(buffer.curr_index, 1);
        assert!(!buffer.full);
        assert_eq!(buffer.latest_index(), 0);
        assert_eq!(buffer.last_time(), Some(time));

        assert_eq!(buffer.diff(), (Duration::from_millis(0), Vector2D::new(0., 0.)));
    }

    /// Buffer not complete full
    #[test]
    fn test_push_two_elements() {
        let mut buffer: PositionTimeRingBuffer<5> = PositionTimeRingBuffer::default();
        let time = Instant::now();

        buffer.push(time, LogicalPoint::new(10.0, 20.0));
        buffer.push(time + Duration::from_millis(13), LogicalPoint::new(13.0, -5.0));

        assert!(!buffer.empty());
        assert_eq!(buffer.curr_index, 2);
        assert!(!buffer.full);
        assert_eq!(buffer.latest_index(), 1);
        assert_eq!(buffer.last_time(), Some(time + Duration::from_millis(13)));

        assert_eq!(buffer.diff(), (Duration::from_millis(13), Vector2D::new(3., -25.)));
    }

    #[test]
    fn test_push_until_full() {
        let mut buffer: PositionTimeRingBuffer<5> = PositionTimeRingBuffer::default();
        let base_time = Instant::now();

        // Push elements to fill the buffer
        for i in 0..5 {
            let time = base_time + Duration::from_millis(i * 3 as u64);
            let point = LogicalPoint::new(i as f32, -2. * i as f32);
            buffer.push(time, point);
        }

        assert!(!buffer.empty());
        assert_eq!(buffer.curr_index, 0);
        assert!(buffer.full);
        assert_eq!(buffer.last_time(), Some(base_time + Duration::from_millis(4 * 3)));
        assert_eq!(buffer.latest_index(), 4);

        assert_eq!(buffer.diff(), (Duration::from_millis(12), Vector2D::new(4., -8.)));
    }

    #[test]
    fn test_push_beyond_capacity() {
        const CAP: usize = 5;
        let mut buffer: PositionTimeRingBuffer<CAP> = PositionTimeRingBuffer::default();
        let base_time = Instant::now();

        // Push more than capacity
        for i in 0..(CAP + 2) {
            let time = base_time + Duration::from_millis(i as u64);
            let point = LogicalPoint::new(i as f32, i as f32 * 2. + 100.);
            buffer.push(time, point);
        }

        assert!(!buffer.empty());
        assert!(buffer.full);
        assert_eq!(buffer.curr_index, 2);
        assert_eq!(buffer.latest_index(), 1);
        assert_eq!(buffer.last_time(), Some(base_time + Duration::from_millis(6)));

        assert_eq!(buffer.diff(), (Duration::from_millis(4), Vector2D::new(4., 4. * 2.)));
    }

    #[test]
    fn test_push_beyond_capacity_wrap_back() {
        const CAP: usize = 5;
        let mut buffer: PositionTimeRingBuffer<CAP> = PositionTimeRingBuffer::default();
        let base_time = Instant::now();

        // Push more than capacity
        for i in 0..CAP {
            let time = base_time + Duration::from_millis(i as u64);
            let point = LogicalPoint::new(i as f32 * 3., i as f32 * -2. + 100.);
            buffer.push(time, point);
        }

        assert!(!buffer.empty());
        assert!(buffer.full);
        assert_eq!(buffer.curr_index, 0);
        assert_eq!(buffer.latest_index(), CAP - 1);
        assert_eq!(buffer.last_time(), Some(base_time + Duration::from_millis(4)));

        // Wrapping back must be done
        assert_eq!(buffer.diff(), (Duration::from_millis(4), Vector2D::new(4. * 3., 4. * -2.)));
    }
}
