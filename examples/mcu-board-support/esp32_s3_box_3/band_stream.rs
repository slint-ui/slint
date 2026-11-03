// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Streams changed rows to the display core while the renderer produces the rows below them.

use alloc::{boxed::Box, vec};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, Ordering},
};

const STRIDE: usize = 320;
pub(crate) const SLOT_BYTES: usize = 16 * 1024;
// More slots than the 48 KiB internal heap holds, so the allocation lands in PSRAM.
const SLOTS: usize = 4;

/// Pixel rows `y..y + height`, columns `x..x + width`. A zero height ends the frame.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(crate) struct Band {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Band {
    fn bytes(self) -> usize {
        self.width * self.height * 2
    }
}

/// A single-producer, single-consumer ring of packed bands.
pub(crate) struct BandQueue {
    pixels: *mut u8,
    bands: [UnsafeCell<Band>; SLOTS],
    head: AtomicU32,
    tail: AtomicU32,
}

// The producer writes a slot only while it's outside `tail..head`; the consumer only reads inside.
unsafe impl Sync for BandQueue {}
unsafe impl Send for BandQueue {}

impl BandQueue {
    pub(crate) fn new() -> Self {
        Self {
            pixels: Box::leak(vec![0u8; SLOTS * SLOT_BYTES].into_boxed_slice()).as_mut_ptr(),
            bands: core::array::from_fn(|_| UnsafeCell::new(Band::default())),
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
        }
    }

    fn push(&self, band: Band, fill: impl FnOnce(&mut [u8])) {
        debug_assert!(band.bytes() <= SLOT_BYTES);
        let head = self.head.load(Ordering::Relaxed);
        while head.wrapping_sub(self.tail.load(Ordering::Acquire)) as usize == SLOTS {
            core::hint::spin_loop();
        }
        let slot = head as usize % SLOTS;
        unsafe {
            *self.bands[slot].get() = band;
            fill(core::slice::from_raw_parts_mut(self.pixels.add(slot * SLOT_BYTES), band.bytes()));
        }
        self.head.store(head.wrapping_add(1), Ordering::Release);
    }

    pub(crate) fn pending(&self) -> bool {
        self.head.load(Ordering::Acquire) != self.tail.load(Ordering::Relaxed)
    }

    /// Passes each band of one frame to `send` as it arrives.
    /// Returns the bytes sent, or the first error.
    pub(crate) fn drain<E>(
        &self,
        mut send: impl FnMut(Band, &[u8]) -> Result<(), E>,
    ) -> Result<usize, E> {
        let mut sent = 0;
        let mut error = None;
        loop {
            let tail = self.tail.load(Ordering::Relaxed);
            while self.head.load(Ordering::Acquire) == tail {
                core::hint::spin_loop();
            }
            let slot = tail as usize % SLOTS;
            let band = unsafe { *self.bands[slot].get() };
            if band.height == 0 {
                self.tail.store(tail.wrapping_add(1), Ordering::Release);
                return error.map_or(Ok(sent), Err);
            }
            let data = unsafe {
                core::slice::from_raw_parts(self.pixels.add(slot * SLOT_BYTES), band.bytes())
            };
            match send(band, data) {
                Ok(()) => sent += data.len(),
                Err(err) => {
                    error.get_or_insert(err);
                }
            }
            self.tail.store(tail.wrapping_add(1), Ordering::Release);
        }
    }
}

/// Collects the changed rows of a frame, in rendering order, into bands.
pub(crate) struct BandWriter {
    band: Option<Band>,
    line: usize,
    changed: Option<(usize, usize)>,
    published: bool,
}

impl BandWriter {
    pub(crate) const fn new() -> Self {
        Self { band: None, line: usize::MAX, changed: None, published: false }
    }

    /// Records that columns `changed` of `line` differ from the previous frame.
    /// `pixels` is the framebuffer in wire order, already updated for every finished line.
    pub(crate) fn line(
        &mut self,
        line: usize,
        changed: Option<(usize, usize)>,
        pixels: &[u8],
        queue: &BandQueue,
    ) {
        if line != self.line {
            self.end_line(pixels, queue);
            self.line = line;
        }
        if let Some((left, right)) = changed {
            let merged = self.changed.map_or((left, right), |(l, r)| (l.min(left), r.max(right)));
            self.changed = Some(merged);
        }
    }

    /// Sends the remaining rows and ends the frame, if it changed anything.
    pub(crate) fn finish(&mut self, pixels: &[u8], queue: &BandQueue) {
        self.end_line(pixels, queue);
        self.flush(pixels, queue);
        if core::mem::take(&mut self.published) {
            queue.push(Band::default(), |_| {});
        }
    }

    fn end_line(&mut self, pixels: &[u8], queue: &BandQueue) {
        let Some((left, right)) = self.changed.take() else {
            self.flush(pixels, queue);
            return;
        };
        let y = self.line;
        if let Some(band) = &mut self.band {
            let x = band.x.min(left);
            let width = (band.x + band.width).max(right) - x;
            if band.y + band.height == y && width * (band.height + 1) * 2 <= SLOT_BYTES {
                *band = Band { x, y: band.y, width, height: band.height + 1 };
                return;
            }
            self.flush(pixels, queue);
        }
        self.band = Some(Band { x: left, y, width: right - left, height: 1 });
    }

    fn flush(&mut self, pixels: &[u8], queue: &BandQueue) {
        let Some(band) = self.band.take() else { return };
        queue.push(band, |output| {
            for (row, output) in output.chunks_exact_mut(band.width * 2).enumerate() {
                let start = ((band.y + row) * STRIDE + band.x) * 2;
                output.copy_from_slice(&pixels[start..start + band.width * 2]);
            }
        });
        self.published = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, thread, vec::Vec};

    /// Writes each band into its address window like the ILI9342C.
    struct FakePanel {
        memory: Vec<u8>,
    }

    impl FakePanel {
        fn new() -> Self {
            Self { memory: vec![0; STRIDE * 240 * 2] }
        }

        fn send(&mut self, band: Band, data: &[u8]) -> Result<(), ()> {
            assert_eq!(data.len(), band.bytes());
            for (row, pixels) in data.chunks_exact(band.width * 2).enumerate() {
                let start = ((band.y + row) * STRIDE + band.x) * 2;
                self.memory[start..start + pixels.len()].copy_from_slice(pixels);
            }
            Ok(())
        }
    }

    /// Renders `next` over `framebuffer` line by line like the BOX producer.
    fn render(
        framebuffer: &mut [u8],
        next: &[u8],
        lines: impl Iterator<Item = usize>,
        writer: &mut BandWriter,
        queue: &BandQueue,
    ) {
        for line in lines {
            let row = line * STRIDE * 2..(line + 1) * STRIDE * 2;
            let changed = (0..STRIDE)
                .filter(|&x| {
                    next[row.start + x * 2..][..2] != framebuffer[row.start + x * 2..][..2]
                })
                .fold(None, |span: Option<(usize, usize)>, x| {
                    Some(span.map_or((x, x + 1), |(left, _)| (left, x + 1)))
                });
            framebuffer[row.clone()].copy_from_slice(&next[row]);
            writer.line(line, changed, framebuffer, queue);
        }
        writer.finish(framebuffer, queue);
    }

    fn frame(seed: u8, area: impl Fn(usize, usize) -> bool) -> Vec<u8> {
        let mut pixels = vec![0; STRIDE * 240 * 2];
        for y in 0..240 {
            for x in 0..STRIDE {
                if area(x, y) {
                    let offset = (y * STRIDE + x) * 2;
                    pixels[offset] = seed;
                    pixels[offset + 1] = (x ^ y) as u8;
                }
            }
        }
        pixels
    }

    #[test]
    fn concurrent_frames_reach_the_panel_exactly() {
        let queue = Arc::new(BandQueue::new());
        let frames: Vec<Vec<u8>> = vec![
            frame(1, |_, _| true),
            frame(2, |x, y| (40..90).contains(&y) && (10..200).contains(&x)),
            frame(3, |x, y| y % 37 < 5 || (x > 300 && y > 200)),
            frame(4, |_, _| true),
        ];
        let consumer = {
            let queue = queue.clone();
            let count = frames.len();
            thread::spawn(move || {
                let mut panel = FakePanel::new();
                for _ in 0..count {
                    queue.drain(|band, data| panel.send(band, data)).unwrap();
                }
                panel
            })
        };
        let mut framebuffer = vec![0; STRIDE * 240 * 2];
        let mut writer = BandWriter::new();
        for next in &frames {
            render(&mut framebuffer, next, 0..240, &mut writer, &queue);
        }
        let panel = consumer.join().unwrap();
        assert!(panel.memory == framebuffer);
        assert!(!queue.pending());
    }

    #[test]
    fn full_screen_change_fills_each_band() {
        let queue = BandQueue::new();
        let mut framebuffer = vec![0; STRIDE * 240 * 2];
        let mut writer = BandWriter::new();
        let next = frame(9, |_, _| true);
        let consumer = thread::scope(|scope| {
            let consumer = scope.spawn(|| {
                let mut panel = FakePanel::new();
                let mut bands = Vec::new();
                let sent = queue
                    .drain(|band, data| {
                        bands.push(band);
                        panel.send(band, data)
                    })
                    .unwrap();
                (panel, bands, sent)
            });
            render(&mut framebuffer, &next, 0..240, &mut writer, &queue);
            consumer.join().unwrap()
        });
        let (panel, bands, sent) = consumer;
        assert_eq!(sent, STRIDE * 240 * 2);
        assert_eq!(bands.len(), 240usize.div_ceil(SLOT_BYTES / (STRIDE * 2)));
        assert!(bands.iter().all(|band| band.width == STRIDE && band.bytes() <= SLOT_BYTES));
        assert!(panel.memory == framebuffer);
    }

    #[test]
    fn unchanged_rows_and_skipped_lines_split_bands() {
        let queue = BandQueue::new();
        let mut framebuffer = vec![0; STRIDE * 240 * 2];
        let mut writer = BandWriter::new();
        let next = frame(5, |x, y| (10..14).contains(&y) || ((30..34).contains(&y) && x < 8));
        let bands = thread::scope(|scope| {
            let consumer = scope.spawn(|| {
                let mut bands = Vec::new();
                queue
                    .drain(|band, _: &[u8]| {
                        bands.push(band);
                        Ok::<_, ()>(())
                    })
                    .unwrap();
                bands
            });
            render(&mut framebuffer, &next, (0..20).chain(28..40), &mut writer, &queue);
            consumer.join().unwrap()
        });
        assert_eq!(
            bands,
            [
                Band { x: 0, y: 10, width: 320, height: 4 },
                Band { x: 0, y: 30, width: 8, height: 4 },
            ]
        );
    }

    #[test]
    fn unchanged_frame_sends_nothing() {
        let queue = BandQueue::new();
        let mut framebuffer = frame(5, |_, _| true);
        let next = framebuffer.clone();
        let mut writer = BandWriter::new();
        render(&mut framebuffer, &next, 0..240, &mut writer, &queue);
        assert!(!queue.pending());
    }
}
