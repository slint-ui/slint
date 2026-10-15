// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub static STATS_MAX_US: AtomicU32 = AtomicU32::new(0);
pub static RUN: super::profile_run::RunGate = super::profile_run::RunGate::new(2);
static WARM_RUN: AtomicU32 = AtomicU32::new(u32::MAX);
static WARM_STARTED: AtomicU32 = AtomicU32::new(0);

pub fn begin_run(run: u32) {
    if RUN.token() == run && WARM_RUN.load(Ordering::Acquire) != run {
        WARM_STARTED.store(now_us() as u32, Ordering::Relaxed);
        WARM_RUN.store(run, Ordering::Release);
    }
}

pub fn phase(run: u32, phase: u32) {
    if RUN.token() != run {
        return;
    }
    if !RUN.is_armed(run) {
        begin_run(run);
        let warming =
            (now_us() as u32).wrapping_sub(WARM_STARTED.load(Ordering::Relaxed)) < 2_000_000;
        if run & 255 != 2 && warming {
            return;
        }
    }
    PHASE.store(phase, Ordering::Relaxed);
    RUN.arm(run);
}
pub static TOUCH_PRESSES: AtomicU32 = AtomicU32::new(0);
pub static PHASE: AtomicU32 = AtomicU32::new(0);
pub static INK_STATE: AtomicU32 = AtomicU32::new(0);
static SEQUENCE: AtomicU32 = AtomicU32::new(0);
static REPORT: [AtomicU32; 61] = [const { AtomicU32::new(0) }; 61];

static ANIMATION_US: AtomicU32 = AtomicU32::new(0);
static INK_TIMES: [AtomicU32; 4] = [const { AtomicU32::new(0) }; 4];
pub fn ink_times(times: [u64; 4]) {
    for (slot, us) in INK_TIMES.iter().zip(times) {
        slot.fetch_add(us as u32, Ordering::Relaxed);
    }
}

fn take_ink_times() -> [u32; 4] {
    core::array::from_fn(|i| INK_TIMES[i].swap(0, Ordering::Relaxed))
}
pub fn animation(us: u64) {
    ANIMATION_US.fetch_add(us as u32, Ordering::Relaxed);
}

static INPUT: [AtomicU32; 4] = [const { AtomicU32::new(0) }; 4];
pub fn input(touch_us: u64, dispatch_us: u64, dispatched: bool) {
    INPUT[0].fetch_add(touch_us as u32, Ordering::Relaxed);
    INPUT[1].fetch_add(dispatch_us as u32, Ordering::Relaxed);
    INPUT[2].fetch_add(1, Ordering::Relaxed);
    INPUT[3].fetch_add(u32::from(dispatched), Ordering::Relaxed);
}
pub fn dispatch(us: u64) {
    INPUT[1].fetch_add(us as u32, Ordering::Relaxed);
    INPUT[3].fetch_add(1, Ordering::Relaxed);
}
fn take_input() -> [u32; 4] {
    core::array::from_fn(|i| INPUT[i].swap(0, Ordering::Relaxed))
}

pub use super::now_us;

#[derive(Default)]
pub struct FrameStats {
    pub raster_us: u64,
    pub crc_us: u64,
    pub wait_us: u64,
    pub window_us: u64,
    pub pixels: u32,
    pub lines: u32,
    pub plan_us: u64,
}

pub struct Profiler {
    start: u64,
    last_frame: u64,
    last_phase: u32,
    run: u32,
    values: [u32; 61],
    transport_start: [u32; 3],
    presented_start: u32,
    changing_start: u32,
    panel_start: [u32; 4],
}

impl Default for Profiler {
    fn default() -> Self {
        Self {
            start: 0,
            last_frame: 0,
            last_phase: 0,
            run: u32::MAX,
            values: [0; 61],
            transport_start: [0; 3],
            presented_start: 0,
            changing_start: 0,
            panel_start: [0; 4],
        }
    }
}
impl Profiler {
    pub fn begin(&mut self) {
        let run = RUN.token();
        if self.start != 0 && self.run != run {
            *self = Self::default();
        }
        if self.start == 0 && RUN.is_armed(run) {
            #[cfg(feature = "pico2-renderer-profile")]
            i_slint_renderer_software::embedded_profile::take();
            #[cfg(feature = "pico2-renderer-profile")]
            i_slint_renderer_software::embedded_profile::take_text_cache();
            #[cfg(feature = "pico2-renderer-profile")]
            i_slint_renderer_software::embedded_profile::take_text_stages();
            take_input();
            ANIMATION_US.store(0, Ordering::Relaxed);
            take_ink_times();
            take_latency();
            self.run = run;
            self.values[33] = run & 255;
            self.values[42] = run;
            self.values[56] = KIOSK_STATE.load(Ordering::Acquire) & 255;
            self.start = now_us();
            self.last_frame = self.start;
            self.last_phase = PHASE.load(Ordering::Relaxed);
            let v = super::frame_queue::counters();
            self.presented_start = v[0];
            self.changing_start = v[1];
            self.panel_start = super::panel_sync::counters();
            self.transport_start = super::frame_queue::transport_counters();
        }
    }
    pub fn frame(&mut self, draw_us: u64, stats: &FrameStats) {
        if !RUN.is_armed(self.run) {
            *self = Self::default();
            return;
        }
        let now = now_us();
        let phase = PHASE.load(Ordering::Relaxed);
        let interval = (now - self.last_frame) as u32;
        self.last_frame = now;
        self.values[1] += 1;
        self.values[2] += draw_us as u32;
        self.values[3] += stats.raster_us as u32;
        self.values[32] += stats.crc_us as u32;
        self.values[4] += stats.wait_us as u32;
        self.values[5] += stats.window_us as u32;
        self.values[6] += stats.pixels;
        self.values[7] += stats.lines;
        self.values[57] += stats.plan_us as u32;
        if draw_us as u32 >= self.values[8] {
            self.values[8] = draw_us as u32;
            #[cfg(not(feature = "pico2-renderer-profile"))]
            self.values[13..18].copy_from_slice(&[
                stats.raster_us as u32,
                stats.crc_us as u32,
                stats.plan_us as u32,
                stats.window_us as u32,
                stats.wait_us as u32,
            ]);
        }
        self.values[9] = self.values[9].max(interval);
        self.values[10] += phase.saturating_sub(self.last_phase).saturating_sub(1);
        self.last_phase = phase;
        let window_us = if self.run & 255 == 11 { 7_000_000 } else { 10_000_000 };
        if now - self.start >= window_us {
            self.values[0] = (now - self.start) as u32;
            let counts = super::frame_queue::counters();
            self.values[11] = counts[0].wrapping_sub(self.presented_start);
            self.values[12] = counts[1].wrapping_sub(self.changing_start);
            #[cfg(feature = "pico2-renderer-profile")]
            {
                let (times, calls) = i_slint_renderer_software::embedded_profile::take();
                self.values[13..19].copy_from_slice(&times);
                self.values[19..25].copy_from_slice(&calls);
                self.values[43..48].copy_from_slice(
                    &i_slint_renderer_software::embedded_profile::take_text_cache(),
                );
                let (times, calls) =
                    i_slint_renderer_software::embedded_profile::take_text_stages();
                self.values[48..52].copy_from_slice(&times);
                self.values[52..56].copy_from_slice(&calls);
            }
            self.values[25..29].copy_from_slice(&take_input());
            self.values[34] = ANIMATION_US.swap(0, Ordering::Relaxed);
            self.values[38..42].copy_from_slice(&take_ink_times());
            self.values[35..38].copy_from_slice(&take_latency());
            let panel = super::panel_sync::counters();
            self.values[29] = panel[0].wrapping_sub(self.panel_start[0]);
            self.values[30] = panel[2].wrapping_sub(self.panel_start[2]);
            self.values[31] = panel[3].wrapping_sub(self.panel_start[3]);
            let transport = super::frame_queue::transport_counters();
            self.values[58] = transport[0].wrapping_sub(self.transport_start[0]);
            self.values[59] = transport[1].wrapping_sub(self.transport_start[1]);
            self.values[60] = transport[2];
            SEQUENCE.fetch_add(1, Ordering::AcqRel);
            for (slot, value) in REPORT.iter().zip(self.values) {
                slot.store(value, Ordering::Relaxed);
            }
            SEQUENCE.fetch_add(1, Ordering::Release);
            *self = Self::default();
        }
    }
}

pub fn snapshot() -> (u32, [u32; 61]) {
    loop {
        let before = SEQUENCE.load(Ordering::Acquire);
        if before & 1 != 0 {
            continue;
        }
        let values = core::array::from_fn(|i| REPORT[i].load(Ordering::Relaxed));
        core::sync::atomic::fence(Ordering::Acquire);
        if SEQUENCE.load(Ordering::Relaxed) == before {
            return (before / 2, values);
        }
    }
}

const STACK_MARKER: u32 = 0xdadb_abab;
static STACK_BOTTOM: AtomicU32 = AtomicU32::new(0);
static STACK_LIMIT: AtomicU32 = AtomicU32::new(0);

pub fn initialize_stack_probe() {
    unsafe extern "C" {
        static __euninit: u8;
    }
    cortex_m::interrupt::free(|_| {
        let bottom = core::ptr::addr_of!(__euninit) as usize;
        let limit = (cortex_m::register::msp::read() as usize).saturating_sub(512) & !3;
        // Mark only unused main-core stack memory, before USB starts core 1.
        if bottom < limit {
            for address in (bottom..limit).step_by(4) {
                unsafe {
                    core::ptr::write_volatile(address as *mut u32, STACK_MARKER);
                }
            }
            STACK_BOTTOM.store(bottom as u32, Ordering::Relaxed);
            STACK_LIMIT.store(limit as u32, Ordering::Release);
        }
    });
}

pub fn stack_free() -> u32 {
    let limit = STACK_LIMIT.load(Ordering::Acquire);
    let bottom = STACK_BOTTOM.load(Ordering::Relaxed);
    for address in (bottom..limit).step_by(4) {
        if unsafe { core::ptr::read_volatile(address as *const u32) } != STACK_MARKER {
            return address - bottom;
        }
    }
    limit.saturating_sub(bottom)
}

static REPLAY_KIND: AtomicU32 = AtomicU32::new(0);
pub static KIOSK_STATE: AtomicU32 = AtomicU32::new(0);
static REPLAY_X: AtomicU32 = AtomicU32::new(0);
static REPLAY_US: AtomicU32 = AtomicU32::new(0);
static REPLAY_Y: AtomicU32 = AtomicU32::new(0);
static SAMPLE_US: AtomicU32 = AtomicU32::new(0);
static SAMPLE_PENDING: AtomicBool = AtomicBool::new(false);
static LATENCY: [AtomicU32; 3] = [const { AtomicU32::new(0) }; 3];

pub fn schedule_pointer(x: f32, y: f32, pressed: bool) {
    schedule_pointer_kind(x, y, if pressed { 1 } else { 2 });
}

pub fn schedule_pointer_kind(x: f32, y: f32, kind: u32) {
    REPLAY_X.store(x.to_bits(), Ordering::Relaxed);
    REPLAY_Y.store(y.to_bits(), Ordering::Relaxed);
    REPLAY_US.store(now_us() as u32, Ordering::Relaxed);
    REPLAY_KIND.store(kind, Ordering::Release);
}

pub fn take_pointer() -> Option<(f32, f32, u32, u32)> {
    let kind = REPLAY_KIND.swap(0, Ordering::AcqRel);
    (kind != 0).then(|| {
        (
            f32::from_bits(REPLAY_X.load(Ordering::Relaxed)),
            f32::from_bits(REPLAY_Y.load(Ordering::Relaxed)),
            kind,
            REPLAY_US.load(Ordering::Relaxed),
        )
    })
}

pub fn sampled_input(time: u32) {
    SAMPLE_US.store(time, Ordering::Relaxed);
    SAMPLE_PENDING.store(true, Ordering::Release);
}

pub fn begin_raster() {
    if SAMPLE_PENDING.swap(false, Ordering::Acquire) {
        let age = (now_us() as u32).wrapping_sub(SAMPLE_US.load(Ordering::Relaxed));
        LATENCY[0].fetch_add(age, Ordering::Relaxed);
        LATENCY[1].fetch_max(age, Ordering::Relaxed);
        LATENCY[2].fetch_add(1, Ordering::Relaxed);
    }
}

fn take_latency() -> [u32; 3] {
    core::array::from_fn(|index| LATENCY[index].swap(0, Ordering::Relaxed))
}
