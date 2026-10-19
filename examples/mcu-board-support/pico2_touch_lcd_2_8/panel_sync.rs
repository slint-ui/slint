// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering};

static INPUT_SEQUENCE: AtomicU32 = AtomicU32::new(0);
static INPUT_CLOCK: [AtomicU32; 3] = [const { AtomicU32::new(0) }; 3];
static INPUT_TICK: AtomicU32 = AtomicU32::new(0);

pub fn input_tick() -> u32 {
    let values = loop {
        let sequence = INPUT_SEQUENCE.load(Ordering::Acquire);
        if sequence == 0 {
            return tick();
        }
        if sequence & 1 != 0 {
            continue;
        }
        let values = INPUT_CLOCK.each_ref().map(|v| v.load(Ordering::Relaxed));
        core::sync::atomic::fence(Ordering::Acquire);
        if sequence == INPUT_SEQUENCE.load(Ordering::Relaxed) {
            break values;
        }
    };
    let candidate =
        super::input_phase::candidate(values[0], values[1], values[2], super::now_us() as u32);
    let mut previous = INPUT_TICK.load(Ordering::Relaxed);
    loop {
        let next = super::input_phase::advance(previous, candidate);
        if next == previous {
            return next;
        }
        match INPUT_TICK.compare_exchange_weak(previous, next, Ordering::Relaxed, Ordering::Relaxed)
        {
            Ok(_) => return next,
            Err(value) => previous = value,
        }
    }
}

static TICK: AtomicU32 = AtomicU32::new(0);
static PERIOD_US: AtomicU32 = AtomicU32::new(0);
static LATE_STRIPS: AtomicU32 = AtomicU32::new(0);
static READ_ERRORS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pico2-queue-profile")]
static LAST_LATE: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pico2-queue-profile")]
static LAST_LATE_US: AtomicU32 = AtomicU32::new(0);

#[cfg(feature = "pico2-queue-profile")]
pub fn last_late() -> [u32; 4] {
    let packed = LAST_LATE.load(Ordering::Relaxed);
    [packed & 511, (packed >> 9) & 511, packed >> 18, LAST_LATE_US.load(Ordering::Relaxed)]
}

#[derive(Default)]
pub struct ScanClock {
    pub tick: u32,
    scan: u16,
    last_wrap: u64,
    last_read: u64,
    last_observed: u64,
    #[cfg(feature = "pico2-queue-profile")]
    frame_start: u64,
}

impl ScanClock {
    #[unsafe(link_section = ".data.ram_func")]
    #[inline(never)]
    pub fn sample(&mut self) {
        let began = super::now_us();
        let (sample, observed) = cortex_m::interrupt::free(|_| {
            // This panel returns the previous request's snapshot; prime it before reading.
            let _ = super::panel_scan::scanline();
            let observed = super::now_us();
            let value = super::panel_scan::scanline();
            (value, observed)
        });
        if let Some(scan) = sample {
            let expected = PERIOD_US.load(Ordering::Relaxed);
            let expected = if expected == 0 { 17000 } else { expected };
            let wraps = if self.last_read == 0 {
                0
            } else {
                super::scan_phase::wraps(
                    (observed - self.last_observed).min(u32::MAX as u64) as u32,
                    self.scan,
                    scan,
                    expected,
                )
            };
            if wraps != 0 {
                self.tick = self.tick.wrapping_add(wraps);
                TICK.store(self.tick, Ordering::Release);
                let origin = observed.saturating_sub(
                    u64::from(scan) * u64::from(expected)
                        / u64::from(super::scan_budget::TOTAL_GATES),
                );
                if self.last_wrap != 0 {
                    let period = ((origin - self.last_wrap) / u64::from(wraps)) as u32;
                    if (14_000..20_000).contains(&period) {
                        let previous = PERIOD_US.load(Ordering::Relaxed);
                        PERIOD_US.store(
                            if previous == 0 { period } else { (7 * previous + period) / 8 },
                            Ordering::Relaxed,
                        );
                    }
                }
                self.last_wrap = origin;
                INPUT_SEQUENCE.fetch_add(1, Ordering::AcqRel);
                for (slot, value) in INPUT_CLOCK.iter().zip([
                    self.tick,
                    origin as u32,
                    PERIOD_US.load(Ordering::Relaxed),
                ]) {
                    slot.store(value, Ordering::Relaxed);
                }
                INPUT_SEQUENCE.fetch_add(1, Ordering::Release);
            }
            self.scan = scan;
            self.last_read = began;
            self.last_observed = observed;
        } else {
            READ_ERRORS.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn idle_delay_us(&self) -> u64 {
        let period = PERIOD_US.load(Ordering::Relaxed);
        if self.last_observed == 0 {
            return 500;
        }
        super::scan_budget::idle_delay(period, self.scan, super::now_us() - self.last_observed)
    }

    pub async fn start_frame(
        &mut self,
        columns: &[u32; super::uniform_tiles::COLUMNS],
        flipped: bool,
    ) -> u32 {
        self.sample();
        let spi_hz = super::pio_transport::hz();
        let period = PERIOD_US.load(Ordering::Relaxed);
        let period = if period == 0 { 17000 } else { period };
        let remaining = u32::from(super::scan_budget::TOTAL_GATES.saturating_sub(self.scan))
            * period
            / u32::from(super::scan_budget::TOTAL_GATES);
        let groups = super::uniform_tiles::ordered_rectangles(columns, flipped)
            .map(|(x, _, height)| (super::orientation::gate_x(x, 8, flipped), 8, height));
        if self.last_wrap == 0
            || columns.iter().all(|mask| *mask == (1 << super::uniform_tiles::ROWS) - 1)
            || !super::scan_budget::fits_regions(remaining, period, self.scan, spi_hz, groups)
        {
            let wave = self.tick;
            let started = super::now_us();
            while self.tick == wave {
                embassy_time::Timer::after_micros(50).await;
                self.sample();
                if super::now_us() - started > 40000 {
                    READ_ERRORS.fetch_add(1, Ordering::Relaxed);
                    break;
                }
            }
        }
        #[cfg(feature = "pico2-queue-profile")]
        {
            self.frame_start = super::now_us();
        }
        self.tick
    }
    fn certainly_before_deadline(&self, wave: u32, x: u16) -> bool {
        // The configured blanking and fastest accepted 14 ms bound give an early fetch deadline.
        // An inconclusive bound always falls back to a hardware scanline read.
        super::scan_budget::earliest_deadline(
            self.last_read,
            self.scan,
            self.tick.wrapping_sub(wave),
            x,
        )
        .is_some_and(|deadline| super::now_us() < deadline)
    }
    pub async fn wait_after(&mut self, wave: u32, x: u16, width: u16) {
        // MV swaps the address axes; MX mirrors physical columns, leaving logical x along gates.
        let target = super::scan_budget::after_gate(x, width);
        if self.tick == wave && self.scan >= target && self.certainly_before_deadline(wave, x) {
            return;
        }
        if self.tick.wrapping_sub(wave) == 1 && self.certainly_before_deadline(wave, x) {
            return;
        }
        self.sample();
        let started = super::now_us();
        while self.tick == wave && self.scan < target {
            embassy_time::Timer::after_micros(50).await;
            self.sample();
            if super::now_us() - started > 40_000 {
                READ_ERRORS.fetch_add(1, Ordering::Relaxed);
                break;
            }
        }
    }

    pub fn complete_strip(&mut self, wave: u32, x: u16) {
        if self.certainly_before_deadline(wave, x) {
            return;
        }
        self.sample();
        if self.tick.wrapping_sub(wave) > 1
            || (self.tick.wrapping_sub(wave) == 1
                && self.scan >= super::scan_budget::deadline_gate(x))
        {
            #[cfg(feature = "pico2-queue-profile")]
            {
                LAST_LATE.store(
                    u32::from(x)
                        | (u32::from(self.scan) << 9)
                        | (self.tick.wrapping_sub(wave) << 18),
                    Ordering::Relaxed,
                );
                LAST_LATE_US.store((super::now_us() - self.frame_start) as u32, Ordering::Relaxed);
            }
            LATE_STRIPS.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub fn tick() -> u32 {
    TICK.load(Ordering::Acquire)
}

#[cfg(feature = "pico2-queue-profile")]
pub fn counters() -> [u32; 4] {
    [
        tick(),
        PERIOD_US.load(Ordering::Relaxed),
        LATE_STRIPS.load(Ordering::Relaxed),
        READ_ERRORS.load(Ordering::Relaxed),
    ]
}
