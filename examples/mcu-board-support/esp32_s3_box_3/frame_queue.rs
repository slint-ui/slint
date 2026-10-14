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

#[cfg(feature = "box-line-damage")]
#[path = "pixel_damage.rs"]
pub(super) mod pixel_damage;

#[cfg(feature = "box-profile")]
mod pipeline_trace;

#[path = "phase_clock.rs"]
pub(super) mod phase_clock;

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
    #[cfg(feature = "box-profile")]
    hash: u32,
    #[cfg(all(feature = "box-line-damage", feature = "box-profile"))]
    verified_change: bool,
    #[cfg(feature = "box-profile")]
    ready_us: u64,
    #[cfg(feature = "box-profile")]
    target_tick: u32,
    #[cfg(feature = "box-profile")]
    pipeline: [u32; 5],
}

/// One producer owns the framebuffer; one display core owns the SPI peripheral.
/// The packet remains immutable from READY until the final DMA write completes.
pub(super) struct FrameQueue {
    packet: UnsafeCell<Packet>,
    state: AtomicU32,
    pub(super) tick: AtomicU32,
    #[cfg(feature = "box-profile")]
    pub(super) render_tick: AtomicU32,
    phase: phase_clock::Stamp,
    #[cfg(feature = "box-profile")]
    pub(super) acquire_us: AtomicU32,
    #[cfg(feature = "box-profile")]
    pub(super) acquire_blank_us: AtomicU32,
    #[cfg(feature = "box-profile")]
    trace: Option<pipeline_trace::Trace>,
    #[cfg(feature = "box-profile")]
    pub(super) presented: AtomicU32,
    #[cfg(feature = "box-profile")]
    pub(super) changed: AtomicU32,
    #[cfg(feature = "box-profile")]
    pub(super) repeats: AtomicU32,
    #[cfg(feature = "box-profile")]
    pub(super) skipped_cycles: AtomicU32,
    #[cfg(feature = "box-profile")]
    pub(super) timeouts: AtomicU32,
    #[cfg(feature = "box-profile")]
    pub(super) late_packets: AtomicU32,
    #[cfg(feature = "box-profile")]
    pub(super) late_packet_cycles: AtomicU32,
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
                #[cfg(feature = "box-profile")]
                hash: 0,
                #[cfg(all(feature = "box-line-damage", feature = "box-profile"))]
                verified_change: false,
                #[cfg(feature = "box-profile")]
                ready_us: 0,
                #[cfg(feature = "box-profile")]
                target_tick: 0,
                #[cfg(feature = "box-profile")]
                pipeline: [0; 5],
            }),
            state: AtomicU32::new(FREE),
            tick: AtomicU32::new(0),
            #[cfg(feature = "box-profile")]
            render_tick: AtomicU32::new(0),
            phase: phase_clock::Stamp::new(),
            #[cfg(feature = "box-profile")]
            acquire_us: AtomicU32::new(0),
            #[cfg(feature = "box-profile")]
            acquire_blank_us: AtomicU32::new(0),
            #[cfg(feature = "box-profile")]
            trace: Some(pipeline_trace::Trace::new()),
            #[cfg(feature = "box-profile")]
            presented: AtomicU32::new(0),
            #[cfg(feature = "box-profile")]
            changed: AtomicU32::new(0),
            #[cfg(feature = "box-profile")]
            repeats: AtomicU32::new(0),
            #[cfg(feature = "box-profile")]
            skipped_cycles: AtomicU32::new(0),
            #[cfg(feature = "box-profile")]
            timeouts: AtomicU32::new(0),
            #[cfg(feature = "box-profile")]
            late_packets: AtomicU32::new(0),
            #[cfg(feature = "box-profile")]
            late_packet_cycles: AtomicU32::new(0),
        }
    }

    pub(super) fn phase_snapshot(&self) -> (u32, u32) {
        self.phase.snapshot(&self.tick)
    }
    #[cfg(all(feature = "box-profile", not(test)))]
    pub(super) fn dump_pipeline(&self) {
        if let Some(trace) = &self.trace {
            trace.dump();
        }
    }

    #[cfg(feature = "box-profile")]
    pub(super) fn snapshot(&self) -> [u32; 5] {
        [
            self.presented.load(Ordering::Acquire),
            self.changed.load(Ordering::Acquire),
            self.repeats.load(Ordering::Acquire),
            self.skipped_cycles.load(Ordering::Acquire),
            self.timeouts.load(Ordering::Acquire),
        ]
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
            #[cfg(feature = "box-line-damage")]
            false,
        );
    }

    #[cfg(all(test, feature = "box-line-damage"))]
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
            false,
        );
    }

    #[cfg(feature = "box-line-damage")]
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
            true,
        );
    }

    fn publish_rect_iter(
        &self,
        pixels: &[Rgb565BigEndianPixel],
        rectangles: impl Iterator<Item = Rectangle>,
        fallback: Rectangle,
        #[cfg(feature = "box-line-damage")] _verified_change: bool,
    ) {
        #[cfg(feature = "box-profile")]
        let hash_start = super::now_us();
        #[cfg(feature = "box-profile")]
        let hash = {
            #[cfg(feature = "box-line-damage")]
            let sample_hash = !_verified_change;
            #[cfg(not(feature = "box-line-damage"))]
            let sample_hash = true;
            // Sampling can miss changes, so exact damage carries its own proof.
            if sample_hash {
                pixels.iter().step_by(8).fold(2166136261u32, |hash, pixel| {
                    (hash ^ pixel.0 as u32).wrapping_mul(16777619)
                })
            } else {
                0
            }
        };
        while self
            .state
            .compare_exchange(FREE, WRITING, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        #[cfg(feature = "box-profile")]
        let copy_start = super::now_us();
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
        #[cfg(feature = "box-profile")]
        {
            packet.hash = hash;
        }
        #[cfg(all(feature = "box-line-damage", feature = "box-profile"))]
        {
            packet.verified_change = _verified_change;
        }
        #[cfg(feature = "box-profile")]
        {
            packet.target_tick = self.render_tick.load(Ordering::Acquire).wrapping_add(1);
            packet.pipeline = [
                super::profile::mode(),
                self.acquire_blank_us.load(Ordering::Acquire),
                self.acquire_us.load(Ordering::Acquire),
                hash_start as u32,
                copy_start as u32,
            ];
        }
        #[cfg(feature = "box-profile")]
        {
            packet.ready_us = super::now_us();
        }
        self.state.store(READY, Ordering::Release);
    }

    #[cfg(not(test))]
    pub(super) fn display(&'static self, panel: Panel) -> ! {
        self.display_loop(panel, || {})
    }

    #[cfg(all(not(test), feature = "box-touch-worker"))]
    pub(super) fn display_with_touch(
        &'static self,
        panel: Panel,
        mut touch: super::touch_worker::TouchWorker,
    ) -> ! {
        self.display_loop(panel, || touch.poll())
    }

    #[cfg(not(test))]
    fn display_loop(&'static self, mut panel: Panel, mut after_transfer: impl FnMut()) -> ! {
        #[cfg(feature = "box-profile")]
        let mut recorder = self.trace.as_ref().map(pipeline_trace::Trace::recorder);
        let mut previous_blank = 0u64;
        #[cfg(feature = "box-profile")]
        let mut previous_hash = None;
        loop {
            let Some(blank) = panel.wait_blank() else {
                #[cfg(feature = "box-profile")]
                self.timeouts.fetch_add(1, Ordering::Relaxed);
                esp_hal::delay::Delay::new().delay_micros(1000);
                continue;
            };
            let cycles = if previous_blank == 0 {
                1
            } else {
                ((blank - previous_blank + 8350) / 16700).max(1) as u32
            };
            previous_blank = blank;
            #[cfg(feature = "box-profile")]
            self.skipped_cycles.fetch_add(cycles - 1, Ordering::Relaxed);
            let _present_tick = self.phase.publish(&self.tick, cycles, blank as u32);
            if self
                .state
                .compare_exchange(READY, ACTIVE, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                #[cfg(feature = "box-profile")]
                self.repeats.fetch_add(1, Ordering::Relaxed);
                after_transfer();
                continue;
            }
            // ACTIVE excludes producer writes until every blocking DMA chunk ends.
            let packet = unsafe { &*self.packet.get() };
            #[cfg(feature = "box-profile")]
            {
                let late = packet_late_cycles(_present_tick, packet.target_tick);
                if late != 0 {
                    self.late_packets.fetch_add(1, Ordering::Relaxed);
                    self.late_packet_cycles.fetch_add(late, Ordering::Relaxed);
                }
            }
            #[cfg(feature = "box-profile")]
            let start = super::now_us();
            let source =
                if packet.internal_used { &packet.internal[..] } else { &packet.external[..] };
            for rect in &packet.rectangles[..packet.count] {
                let data = &source[rect.offset..rect.offset + rect.width * rect.height * 2];
                panel.packed_rectangle(rect.x, rect.y, rect.width, rect.height, data).unwrap();
            }
            #[cfg(feature = "box-profile")]
            let elapsed = (super::now_us() - start) as u32;
            #[cfg(feature = "box-profile")]
            {
                self.presented.fetch_add(1, Ordering::Relaxed);
                if packet_changes_pixels(packet, &mut previous_hash) {
                    self.changed.fetch_add(1, Ordering::Relaxed);
                }
            }
            #[cfg(feature = "box-profile")]
            if packet.pipeline[0] != 0 {
                if let Some(recorder) = &mut recorder {
                    recorder.push([
                        packet.pipeline[0],
                        packet.target_tick.wrapping_sub(1),
                        packet.target_tick,
                        _present_tick,
                        packet.pipeline[1],
                        packet.pipeline[2],
                        packet.pipeline[3],
                        packet.pipeline[4],
                        packet.ready_us as u32,
                        blank as u32,
                        start as u32,
                        (start + elapsed as u64) as u32,
                        packet.rectangles[..packet.count]
                            .iter()
                            .map(|r| r.width * r.height * 2)
                            .sum::<usize>() as u32,
                    ]);
                }
            }
            self.state.store(FREE, Ordering::Release);
            after_transfer();
        }
    }
}

#[cfg(feature = "box-profile")]
fn packet_changes_pixels(packet: &Packet, previous_hash: &mut Option<u32>) -> bool {
    #[cfg(feature = "box-line-damage")]
    if packet.verified_change {
        *previous_hash = None;
        return true;
    }
    let changed = *previous_hash != Some(packet.hash);
    *previous_hash = Some(packet.hash);
    changed
}

#[cfg(test)]
#[path = "frame_queue/tests.rs"]
mod tests;

#[cfg(feature = "box-profile")]
fn packet_late_cycles(presented: u32, target: u32) -> u32 {
    (presented.wrapping_sub(target) as i32).max(0) as u32
}
