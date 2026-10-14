// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use super::{now_us, queue};
use alloc::{string::String, vec::Vec};
use core::{
    fmt::Write,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};
use slint::platform::software_renderer::{
    RepaintBufferType, Rgb565BigEndianPixel, SoftwareRenderer,
};

static MODE: AtomicU32 = AtomicU32::new(0);
static RAW_Y: AtomicU32 = AtomicU32::new(0);
static CAPTURE: AtomicBool = AtomicBool::new(false);

pub(super) fn mode() -> u32 {
    MODE.load(Ordering::Relaxed)
}
pub fn profile_mode(mode: u32) {
    MODE.store(mode, Ordering::Relaxed);
}
pub fn profile_scroll_y(y: f32) {
    RAW_Y.store(y.to_bits(), Ordering::Relaxed);
}
pub(super) fn request_capture() {
    CAPTURE.store(true, Ordering::Relaxed);
}

#[derive(Default, Clone, Copy)]
pub(super) struct FrameStats {
    pub raster_us: u64,
    pub queue_us: u64,
}

fn external_vec<T: Copy>(count: usize, value: T) -> Vec<T> {
    let layout = core::alloc::Layout::array::<T>(count).unwrap();
    let pointer =
        unsafe { esp_alloc::HEAP.alloc_caps(esp_alloc::MemoryCapability::External.into(), layout) }
            .cast::<T>();
    assert!(!pointer.is_null(), "benchmark PSRAM allocation");
    for index in 0..count {
        unsafe {
            pointer.add(index).write(value);
        }
    }
    unsafe { Vec::from_raw_parts(pointer, count, count) }
}

#[derive(Clone, Copy)]
struct Snapshot {
    display: [u32; 5],
    late: [u32; 2],
    touch: [u32; 2],
}
impl Snapshot {
    fn capture() -> Self {
        let q = queue();
        Self {
            #[cfg(feature = "box-touch-worker")]
            touch: [
                super::touch_worker::READS.load(Ordering::Acquire),
                super::touch_worker::ERRORS.load(Ordering::Acquire),
            ],
            #[cfg(not(feature = "box-touch-worker"))]
            touch: [0; 2],
            display: q.snapshot(),
            late: [
                q.late_packets.load(Ordering::Acquire),
                q.late_packet_cycles.load(Ordering::Acquire),
            ],
        }
    }
}

pub(super) struct Profile {
    mode: u32,
    start_us: u64,
    start: Snapshot,
    frames: u32,
    draw_us: u64,
    draw_max: u64,
    stats: FrameStats,
    motion: Vec<[u64; 9]>,
    truncated: u32,
    reference: Vec<Rgb565BigEndianPixel>,
}
impl Profile {
    pub(super) fn new() -> Self {
        let mut motion = external_vec(1024, [0; 9]);
        motion.clear();
        Self {
            mode: 0,
            start_us: now_us(),
            start: Snapshot::capture(),
            frames: 0,
            draw_us: 0,
            draw_max: 0,
            stats: FrameStats::default(),
            motion,
            truncated: 0,
            reference: external_vec(320 * 240, Rgb565BigEndianPixel(0)),
        }
    }
    pub(super) fn select(&mut self) {
        let mode = mode();
        if mode == self.mode {
            return;
        }
        let end_us = now_us();
        let end = Snapshot::capture();
        if self.mode != 0 {
            let display: [u32; 5] =
                core::array::from_fn(|i| end.display[i].wrapping_sub(self.start.display[i]));
            let late = end.late[0].wrapping_sub(self.start.late[0]);
            let cycles = end.late[1].wrapping_sub(self.start.late[1]);
            esp_println::println!(
                "PRINTER_RESULT,mode={},elapsed_us={},frames={},draw_total_us={},draw_max_us={},raster_total_us={},queue_total_us={},presented={},changed_hash={},repeated_panel_frames={},skipped_panel_cycles={},timeouts={},late_packets={},late_cycles={},truncated={}",
                self.mode,
                end_us - self.start_us,
                self.frames,
                self.draw_us,
                self.draw_max,
                self.stats.raster_us,
                self.stats.queue_us,
                display[0],
                display[1],
                display[2],
                display[3],
                display[4],
                late,
                cycles,
                self.truncated
            );
            esp_println::println!(
                "PRINTER_TOUCH_WORKER,mode={},reads={},errors={}",
                self.mode,
                end.touch[0].wrapping_sub(self.start.touch[0]),
                end.touch[1].wrapping_sub(self.start.touch[1])
            );
            esp_println::println!(
                "PRINTER_MOTION_TRACE_BEGIN,mode={},samples={},columns=phase;start_us;raw_y_bits;drawn;draw_us;event_us;touch_us;raster_us;queue_us",
                self.mode,
                self.motion.len()
            );
            for row in &self.motion {
                esp_println::println!(
                    "PRINTER_MOTION_TRACE,{};{};{};{};{};{};{};{};{}",
                    row[0],
                    row[1],
                    row[2],
                    row[3],
                    row[4],
                    row[5],
                    row[6],
                    row[7],
                    row[8]
                );
            }
            esp_println::println!("PRINTER_MOTION_TRACE_END");
        }
        self.mode = mode;
        self.start_us = now_us();
        self.start = Snapshot::capture();
        self.frames = 0;
        self.draw_us = 0;
        self.draw_max = 0;
        self.stats = FrameStats::default();
        self.motion.clear();
        self.truncated = 0;
    }
    pub(super) fn frame(&mut self, start_us: u64, draw_us: u64, drawn: bool, stats: FrameStats) {
        if self.mode == 0 {
            return;
        }
        if self.motion.len() < self.motion.capacity() {
            self.motion.push([
                super::profile_animation_tick(),
                start_us,
                RAW_Y.load(Ordering::Relaxed) as u64,
                drawn as u64,
                draw_us,
                0,
                0,
                stats.raster_us,
                stats.queue_us,
            ]);
        } else {
            self.truncated += 1;
        }
        if drawn {
            self.frames += 1;
            self.draw_us += draw_us;
            self.draw_max = self.draw_max.max(draw_us);
            self.stats.raster_us += stats.raster_us;
            self.stats.queue_us += stats.queue_us;
        }
    }
    pub(super) fn capture_requested(&self) -> bool {
        CAPTURE.load(Ordering::Relaxed)
    }
    pub(super) fn capture(&mut self, renderer: &SoftwareRenderer, pixels: &[Rgb565BigEndianPixel]) {
        CAPTURE.store(false, Ordering::Relaxed);
        let old = renderer.repaint_buffer_type();
        renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
        renderer.render(&mut self.reference, 320);
        renderer.set_repaint_buffer_type(old);
        let differences = pixels.iter().zip(&self.reference).filter(|(a, b)| a.0 != b.0).count();
        esp_println::println!("PRINTER_CAPTURE_PARITY,pixels=76800,differences={}", differences);
        for (index, (actual, reference)) in
            pixels.iter().zip(&self.reference).enumerate().filter(|(_, (a, b))| a.0 != b.0).take(8)
        {
            esp_println::println!(
                "PRINTER_CAPTURE_DIFFERENCE,x={},y={},actual={:04x},canonical={:04x}",
                index % 320,
                index / 320,
                u16::from_be(actual.0),
                u16::from_be(reference.0)
            );
        }
        esp_println::println!("PRINTER_CAPTURE_BEGIN,width=320,height=240");
        for (y, row) in pixels.chunks_exact(320).enumerate() {
            let mut text = String::with_capacity(1280);
            for pixel in row {
                write!(&mut text, "{:04x}", u16::from_be(pixel.0)).unwrap();
            }
            esp_println::println!("PRINTER_CAPTURE,{y},{text}");
        }
        queue().dump_pipeline();
        esp_println::println!("PRINTER_CAPTURE_END");
    }
}
