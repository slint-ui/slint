// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Optional nested CPU timing buckets for embedded renderer profiling.

use core::sync::atomic::{AtomicU32, Ordering};

/// Names for the nested timing buckets.
pub const NAMES: [&str; 6] = ["render", "dirty", "text", "texture", "rectangle", "rounded"];
/// Names for the request, font match, preparation, and submission text stages.
pub const TEXT_STAGES: [&str; 4] = ["text_request", "text_font", "text_prepare", "text_submit"];
i_slint_core::thread_local! {
    static CLOCK: core::cell::Cell<Option<fn() -> u64>> = core::cell::Cell::new(None);
}
static TIMES: [AtomicU32; 10] = [const { AtomicU32::new(0) }; 10];
static CALLS: [AtomicU32; 10] = [const { AtomicU32::new(0) }; 10];
static TEXT_CACHE: [AtomicU32; 5] = [const { AtomicU32::new(0) }; 5];

pub(crate) enum TextCacheEvent {
    Hit,
    Miss,
    Evict,
}

pub(crate) fn text_cache_event(event: TextCacheEvent) {
    TEXT_CACHE[event as usize].fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn text_cache_size(entries: usize, glyphs: usize) {
    TEXT_CACHE[3].fetch_max(entries as u32, Ordering::Relaxed);
    TEXT_CACHE[4].fetch_max(glyphs as u32, Ordering::Relaxed);
}

/// Read and reset cache hits, misses, evictions, and peak entry/glyph counts.
pub fn take_text_cache() -> [u32; 5] {
    core::array::from_fn(|i| TEXT_CACHE[i].swap(0, Ordering::Relaxed))
}

/// Read and reset text stages; submission includes its nested texture rasterization.
pub fn take_text_stages() -> ([u32; 4], [u32; 4]) {
    (
        core::array::from_fn(|i| TIMES[i + 6].swap(0, Ordering::Relaxed)),
        core::array::from_fn(|i| CALLS[i + 6].swap(0, Ordering::Relaxed)),
    )
}

/// Install a monotonic microsecond clock before rendering begins.
pub fn set_clock(clock: fn() -> u64) {
    CLOCK.with(|slot| slot.set(Some(clock)));
}

/// Read and reset buckets. Text includes its texture rasterization; rectangle
/// includes rounded rasterization. The counters include clock-call overhead.
pub fn take() -> ([u32; 6], [u32; 6]) {
    (
        core::array::from_fn(|i| TIMES[i].swap(0, Ordering::Relaxed)),
        core::array::from_fn(|i| CALLS[i].swap(0, Ordering::Relaxed)),
    )
}

pub(crate) struct Scope {
    clock: Option<fn() -> u64>,
    start: u64,
    bucket: usize,
}
impl Scope {
    pub(crate) fn new(bucket: usize) -> Self {
        let clock = CLOCK.with(|slot| slot.get());
        Self { clock, start: clock.map_or(0, |clock| clock()), bucket }
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(clock) = self.clock {
            TIMES[self.bucket].fetch_add((clock() - self.start) as u32, Ordering::Relaxed);
            CALLS[self.bucket].fetch_add(1, Ordering::Relaxed);
        }
    }
}
