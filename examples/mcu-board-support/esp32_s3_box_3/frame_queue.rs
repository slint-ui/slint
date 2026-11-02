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

// The viewer also needs internal SRAM for the interpreter and CPU0 stack.
pub(super) const INTERNAL_PACKET_BYTES: usize = 32 * 1024;

const FREE: u32 = 0;
const READY: u32 = 1;
const ACTIVE: u32 = 2;
const WRITING: u32 = 3;
const MAX_RECTS: usize = 64;

#[derive(Clone, Copy, Default)]
struct Rectangle {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    offset: usize,
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
}

// A producer reserves WRITING before mutation. The consumer reads an ACTIVE packet.
// Release/Acquire transitions protect all packet data, including metadata.
unsafe impl Sync for FrameQueue {}

impl FrameQueue {
    pub(super) fn new(internal: &'static mut [u8]) -> Self {
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
        }
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

    #[cfg(test)]
    pub(super) fn publish_rectangles(
        &self,
        pixels: &[Rgb565BigEndianPixel],
        rects: &[pixel_damage::Rect],
    ) {
        self.publish_rect_iter(
            pixels,
            rects.iter().map(|rect| Rectangle {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
                offset: 0,
            }),
            Rectangle { x: 0, y: 0, width: 320, height: 240, offset: 0 },
        );
    }

    pub(super) fn publish_damage(
        &self,
        pixels: &[Rgb565BigEndianPixel],
        plan: &pixel_damage::Plan,
    ) {
        // Exact comparison proved that there is no new image to publish.
        // Do not wait for ownership of the previous in-flight packet.
        if plan.rectangles().is_empty() {
            return;
        }
        self.publish_rect_iter(
            pixels,
            plan.rectangles().iter().map(|rect| Rectangle {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
                offset: 0,
            }),
            Rectangle { x: 0, y: 0, width: 320, height: 240, offset: 0 },
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
        if bytes > packet.external.len() {
            packet.rectangles[0] = fallback;
            packet.count = 1;
            bytes = fallback.width * fallback.height * 2;
        }
        assert!(bytes <= packet.external.len());
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
            let Some(blank) = panel.wait_blank() else {
                esp_hal::delay::Delay::new().delay_micros(1000);
                continue;
            };
            let cycles = if previous_blank == 0 {
                1
            } else {
                ((blank - previous_blank + 8350) / 16700).max(1) as u32
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
            let started = super::now_us();
            for rect in &packet.rectangles[..packet.count] {
                let data = &source[rect.offset..rect.offset + rect.width * rect.height * 2];
                panel.packed_rectangle(rect.x, rect.y, rect.width, rect.height, data).unwrap();
            }
            #[cfg(feature = "box-display-profile")]
            log::info!(
                "BOX_TRANSFER,bytes={},transfer_us={}",
                packet.rectangles[..packet.count]
                    .iter()
                    .map(|rect| rect.width * rect.height * 2)
                    .sum::<usize>(),
                super::now_us() - started
            );

            self.state.store(FREE, Ordering::Release);
            after_transfer();
        }
    }
}

#[cfg(test)]
#[path = "frame_queue/tests.rs"]
mod tests;
