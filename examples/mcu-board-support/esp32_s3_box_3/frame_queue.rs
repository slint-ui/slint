// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[cfg(not(test))]
use super::panel::Panel;
use alloc::{boxed::Box, vec};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, Ordering},
};
use slint::platform::software_renderer::{PhysicalRegion, Rgb565BigEndianPixel};

#[path = "pixel_damage.rs"]
pub(super) mod pixel_damage;

#[path = "phase_clock.rs"]
pub(super) mod phase_clock;

#[path = "band_stream.rs"]
pub(super) mod band_stream;

/// A streamed band's pixels, which the SPI DMA reads in place.
#[cfg(not(test))]
pub(super) type Slot = esp_hal::dma::DmaTxBuf;
#[cfg(test)]
pub(super) type Slot = alloc::vec::Vec<u8>;

#[cfg(not(test))]
impl band_stream::SlotBuffer for esp_hal::dma::DmaTxBuf {
    fn pixels(&mut self) -> &mut [u8] {
        self.as_mut_slice()
    }
}

// The viewer also needs internal SRAM for the interpreter and CPU0 stack.
pub(super) const INTERNAL_PACKET_BYTES: usize = 32 * 1024;

const FREE: u32 = 0;
const READY: u32 = 1;
const ACTIVE: u32 = 2;
const WRITING: u32 = 3;
const MAX_RECTS: usize = 64;
// Larger streamed frames wait for the scan to start at the top, so the write stays behind it.
const UNSYNCHRONIZED_FRAME_BYTES: usize = 320 * 240 * 2 / 4;

#[derive(Clone, Copy, Default)]
struct Rectangle {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    offset: usize,
}

impl Rectangle {
    fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self {
            x,
            y,
            width: (self.x + self.width).max(other.x + other.width) - x,
            height: (self.y + self.height).max(other.y + other.height) - y,
            offset: 0,
        }
    }
}

struct Packet {
    internal: &'static mut [u8],
    external: Box<[u8]>,
    rectangles: [Rectangle; MAX_RECTS],
    count: usize,
    internal_used: bool,
}

/// One producer owns the framebuffer; one display core owns the SPI peripheral.
/// The packet remains immutable from READY until the final DMA write completes.
pub(super) struct FrameQueue {
    packet: UnsafeCell<Packet>,
    state: AtomicU32,
    pub(super) tick: AtomicU32,
    phase: phase_clock::Stamp,
    bands: band_stream::BandQueue<Slot>,
}

// A producer reserves WRITING before mutation. The consumer reads an ACTIVE packet.
// Release/Acquire transitions protect all packet data, including metadata.
unsafe impl Sync for FrameQueue {}

impl FrameQueue {
    pub(super) fn new(internal: &'static mut [u8], slots: [Slot; band_stream::SLOTS]) -> Self {
        Self {
            packet: UnsafeCell::new(Packet {
                internal,
                external: vec![0; 320 * 240 * 2].into_boxed_slice(),
                rectangles: [Rectangle::default(); MAX_RECTS],
                count: 0,
                internal_used: false,
            }),
            state: AtomicU32::new(FREE),
            tick: AtomicU32::new(0),
            phase: phase_clock::Stamp::new(),
            bands: band_stream::BandQueue::new(slots),
        }
    }

    /// The band stream, once no packet is waiting for or in transfer.
    pub(super) fn bands(&self) -> &band_stream::BandQueue<Slot> {
        while self.state.load(Ordering::Acquire) != FREE {
            core::hint::spin_loop();
        }
        &self.bands
    }

    pub(super) fn phase_snapshot(&self) -> (u32, u32) {
        self.phase.snapshot(&self.tick)
    }

    pub(super) fn publish(&self, pixels: &[Rgb565BigEndianPixel], region: &PhysicalRegion) {
        let origin = region.bounding_box_origin();
        let size = region.bounding_box_size();
        let fallback = Rectangle {
            x: origin.x as usize,
            y: origin.y as usize,
            width: size.width as usize,
            height: size.height as usize,
            offset: 0,
        };
        self.publish_rect_iter(
            pixels,
            region.iter().map(|(origin, size)| Rectangle {
                x: origin.x as usize,
                y: origin.y as usize,
                width: size.width as usize,
                height: size.height as usize,
                offset: 0,
            }),
            fallback,
        );
    }

    fn publish_rect_iter(
        &self,
        pixels: &[Rgb565BigEndianPixel],
        rectangles: impl Iterator<Item = Rectangle>,
        fallback: Rectangle,
    ) {
        while self
            .state
            .compare_exchange(FREE, WRITING, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }

        // WRITING excludes every other publisher and the display consumer.
        let packet = unsafe { &mut *self.packet.get() };
        packet.count = 0;
        let mut bytes = 0;
        for mut rect in rectangles {
            if packet.count == MAX_RECTS {
                packet.rectangles[0] = fallback;
                packet.count = 1;
                bytes = fallback.width * fallback.height * 2;
                break;
            }
            rect.offset = bytes;
            bytes += rect.width * rect.height * 2;
            packet.rectangles[packet.count] = rect;
            packet.count += 1;
        }
        if packet.count == 0 {
            self.state.store(FREE, Ordering::Release);
            return;
        }
        // The LCD scans its RAM during SPI writes, exposing boundaries between dense rectangles.
        if packet.count > 1 && bytes > packet.internal.len() {
            let bounds = packet.rectangles[1..packet.count]
                .iter()
                .copied()
                .fold(packet.rectangles[0], Rectangle::union);
            let full_bytes = bounds.width * bounds.height * 2;
            if bytes >= full_bytes / 2 {
                packet.rectangles[0] = bounds;
                packet.count = 1;
                bytes = full_bytes;
            }
        }
        if bytes > packet.external.len() {
            packet.rectangles[0] = fallback;
            packet.count = 1;
            bytes = fallback.width * fallback.height * 2;
        }
        assert!(bytes <= packet.external.len());
        packet.rectangles[..packet.count].sort_unstable_by_key(|rect| (rect.y, rect.x));
        let mut offset = 0;
        for rect in &mut packet.rectangles[..packet.count] {
            rect.offset = offset;
            offset += rect.width * rect.height * 2;
        }
        packet.internal_used = bytes <= packet.internal.len();
        let output = if packet.internal_used {
            &mut packet.internal[..bytes]
        } else {
            &mut packet.external[..bytes]
        };
        for rect in &packet.rectangles[..packet.count] {
            for row in 0..rect.height {
                let source = &pixels
                    [(rect.y + row) * 320 + rect.x..(rect.y + row) * 320 + rect.x + rect.width];
                // The renderer's transparent BE pixels are already in wire order.
                let source = unsafe {
                    core::slice::from_raw_parts(source.as_ptr().cast::<u8>(), rect.width * 2)
                };
                let start = rect.offset + row * rect.width * 2;
                output[start..start + source.len()].copy_from_slice(source);
            }
        }

        self.state.store(READY, Ordering::Release);
    }

    #[cfg(not(test))]
    pub(super) fn display_with_touch(
        &'static self,
        panel: Panel,
        mut touch: super::touch_worker::TouchWorker,
    ) -> ! {
        self.display_loop(panel, || touch.poll())
    }

    #[cfg(not(test))]
    fn display_loop(&'static self, mut panel: Panel, mut after_transfer: impl FnMut()) -> ! {
        let mut previous_blank = 0u64;

        loop {
            if self.bands.pending() {
                #[cfg(feature = "box-display-profile")]
                let wait_started = super::now_us();
                let small = || self.bands.frame_within(UNSYNCHRONIZED_FRAME_BYTES) == Some(true);
                let small_frame = small();
                if !small_frame {
                    panel.wait_scan_start(small);
                }
                #[cfg(feature = "box-display-profile")]
                let started = super::now_us();
                let _bytes = self.bands.drain(|band, run| panel.stream(band, run)).unwrap();
                #[cfg(feature = "box-display-profile")]
                log::info!(
                    "BOX_TRANSFER,bytes={},transfer_us={},wait_us={},small={}",
                    _bytes,
                    super::now_us() - started,
                    started - wait_started,
                    small_frame
                );
                after_transfer();
                continue;
            }
            let Some(blank) = panel.wait_blank(|| self.bands.pending()) else {
                if !self.bands.pending() {
                    esp_hal::delay::Delay::new().delay_micros(1000);
                }
                continue;
            };
            let cycles = if previous_blank == 0 {
                1
            } else {
                let refresh = u64::from(super::panel::REFRESH_US);
                ((blank - previous_blank + refresh / 2) / refresh).max(1) as u32
            };
            previous_blank = blank;

            let _present_tick = self.phase.publish(&self.tick, cycles, blank as u32);
            if self
                .state
                .compare_exchange(READY, ACTIVE, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                after_transfer();
                continue;
            }
            // ACTIVE excludes producer writes until every blocking DMA chunk ends.
            let packet = unsafe { &*self.packet.get() };

            let source =
                if packet.internal_used { &packet.internal[..] } else { &packet.external[..] };
            #[cfg(feature = "box-display-profile")]
            let _ = panel.take_dma_profile();
            #[cfg(feature = "box-display-profile")]
            let started = super::now_us();
            for rect in &packet.rectangles[..packet.count] {
                let data = &source[rect.offset..rect.offset + rect.width * rect.height * 2];
                panel.packed_rectangle(rect.x, rect.y, rect.width, rect.height, data).unwrap();
            }
            #[cfg(feature = "box-display-profile")]
            let transfer_us = super::now_us() - started;
            #[cfg(feature = "box-display-profile")]
            let dma_profile = panel.take_dma_profile();
            #[cfg(feature = "box-display-profile")]
            log::info!(
                "BOX_DMA,copy_us={},setup_us={},wait_us={},chunks={},overlap_us={}",
                dma_profile.copy_us,
                dma_profile.setup_us,
                dma_profile.wait_us,
                dma_profile.chunks,
                dma_profile.overlap_us
            );
            #[cfg(feature = "box-display-profile")]
            log::info!(
                "BOX_TRANSFER,bytes={},transfer_us={}",
                packet.rectangles[..packet.count]
                    .iter()
                    .map(|rect| rect.width * rect.height * 2)
                    .sum::<usize>(),
                transfer_us
            );
            #[cfg(feature = "box-display-profile")]
            {
                let bytes = packet.rectangles[..packet.count]
                    .iter()
                    .map(|rect| rect.width * rect.height * 2)
                    .sum::<usize>();
                let hash = source[..bytes].iter().fold(0x811c9dc5u32, |hash, byte| {
                    (hash ^ u32::from(*byte)).wrapping_mul(16777619)
                });
                log::info!("BOX_PIXELS,hash={:08x}", hash);
            }
            #[cfg(feature = "box-display-profile")]
            for (index, rect) in packet.rectangles[..packet.count].iter().enumerate() {
                log::info!(
                    "BOX_RECT,index={},x={},y={},width={},height={}",
                    index,
                    rect.x,
                    rect.y,
                    rect.width,
                    rect.height
                );
            }

            self.state.store(FREE, Ordering::Release);
            after_transfer();
        }
    }
}

#[cfg(test)]
#[path = "frame_queue/tests.rs"]
mod tests;
