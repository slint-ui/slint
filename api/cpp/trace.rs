// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore callsite csrr mhartid nomem nostack

//! Records the runtime's `tracing` spans on ESP-IDF with as little work as possible, and sends
//! them over the console only after the capture ends, so that neither recording nor sending
//! disturbs the measured frames. `idf.py trace` (api/cpp/esp-idf/slint/idf_ext.py) decodes them.
//!
//! The capture is a sequence of little-endian 32-bit words:
//! - `id | kind`, `cycles`: a span enter (0), exit (1) or event (2). `id` is the address of the
//!   call site's metadata, and `cycles` the low 32 bits of the CPU cycle counter.
//! - `3 | type << 2 | payload_words << 8`, then the payload:
//!   - type 0, a name: its address, byte length, and UTF-8 bytes padded to whole words.
//!   - type 1, a field of the next span or event: name address, value low and high words.
//!   - type 2, the clock: CPU cycles per microsecond.
//!   - type 3, the measured cost of recording one empty span, in cycles.
//!   - type 4, a span on another track, such as hardware working in the background: its name
//!     address, begin and end cycles, and the track number.
//!   - type 5, the start of the records of another CPU core: its track number. Its cycles are
//!     converted to those of the first core.
//!
//! Each packet is `\x1eB`, the escaped sequence number and capture bytes, and `\n`. `\x1eE\n`
//! ends the capture. Escaping keeps `\n` and `\r` out of packets, so they pass the console.

use alloc::alloc::{Layout, alloc};
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicUsize, Ordering::Relaxed};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::subscriber::Interest;
use tracing::{Event, Metadata, Subscriber};

/// Skips the start-up before recording.
const DELAY_US: u32 = 2_000_000;
const DURATION_US: u32 = 5_000_000;
const MAX_CAPTURE_BYTES: usize = 16 << 20;

const ENTER: u32 = 0;
const EXIT: u32 = 1;
const EVENT: u32 = 2;
const EXTENDED: u32 = 3;
const NAME: u32 = 0;
const FIELD: u32 = 1;
const CLOCK: u32 = 2;
const SPAN_COST: u32 = 3;
const TRACK_SPAN: u32 = 4;
const CORE: u32 = 5;

/// Each core records into its own part of the capture, so that recording needs no locking.
const CORES: usize = 2;
/// The Tracy track of each core's records. Track 1 is the panel transfer.
const CORE_TRACKS: [u32; CORES] = [0, 2];

#[allow(clippy::declare_interior_mutable_const)]
const NO_CAPTURE: AtomicPtr<u32> = AtomicPtr::new(core::ptr::null_mut());
#[allow(clippy::declare_interior_mutable_const)]
const ZERO: AtomicUsize = AtomicUsize::new(0);
static CAPTURE: [AtomicPtr<u32>; CORES] = [NO_CAPTURE; CORES];
static CAPACITY: AtomicUsize = AtomicUsize::new(0);
static LEN: [AtomicUsize; CORES] = [ZERO; CORES];
/// What to add to each core's cycle count to get the first core's.
static CYCLE_OFFSET: [AtomicU32; CORES] = [AtomicU32::new(0), AtomicU32::new(0)];
static RECORDING: AtomicBool = AtomicBool::new(false);
static INSTALLED: AtomicU32 = AtomicU32::new(0);
/// The time and the first core's cycle count at one moment, to align the other cores' with.
static REFERENCE_US: AtomicU32 = AtomicU32::new(0);
static REFERENCE_CYCLES: AtomicU32 = AtomicU32::new(0);
static TICKS_PER_US: AtomicU32 = AtomicU32::new(0);
static DELAY: AtomicU32 = AtomicU32::new(0);
static END: AtomicU32 = AtomicU32::new(u32::MAX);

unsafe extern "C" {
    fn esp_rom_get_cpu_ticks_per_us() -> u32;
    fn esp_timer_get_time() -> i64;
    fn printf(format: *const core::ffi::c_char, ...) -> i32;
    // Defined in slint-esp.cpp, where the chip's headers are available.
    fn slint_esp_trace_write(data: *const u8, len: usize);
    #[cfg(not(all(target_arch = "riscv32", target_feature = "f")))]
    fn slint_esp_trace_cycles() -> u32;
    #[cfg(not(all(target_arch = "riscv32", target_feature = "f")))]
    fn slint_esp_trace_core() -> u32;
}

fn write_all(data: &[u8]) {
    // Safety: writes from a valid slice.
    unsafe { slint_esp_trace_write(data.as_ptr(), data.len()) };
}

/// The ESP32-P4, the only RISC-V ESP with an FPU, has the standard cycle counter.
#[cfg(all(target_arch = "riscv32", target_feature = "f"))]
#[inline(always)]
fn core_cycles() -> u32 {
    let cycles: u32;
    // Safety: reads a counter that ESP-IDF keeps enabled on this chip.
    unsafe { core::arch::asm!("csrr {0}, cycle", out(reg) cycles, options(nomem, nostack)) };
    cycles
}

#[cfg(all(target_arch = "riscv32", target_feature = "f"))]
#[inline(always)]
fn core_id() -> usize {
    let hart: usize;
    // Safety: reads the core's id.
    unsafe { core::arch::asm!("csrr {0}, mhartid", out(reg) hart, options(nomem, nostack)) };
    hart.min(CORES - 1)
}

#[cfg(not(all(target_arch = "riscv32", target_feature = "f")))]
#[inline(always)]
fn core_cycles() -> u32 {
    // Safety: a plain function.
    unsafe { slint_esp_trace_cycles() }
}

#[cfg(not(all(target_arch = "riscv32", target_feature = "f")))]
#[inline(always)]
fn core_id() -> usize {
    // Safety: a plain function.
    (unsafe { slint_esp_trace_core() } as usize).min(CORES - 1)
}

/// The cycle count of the first core, wherever this runs.
#[inline(always)]
fn cycles() -> u32 {
    core_cycles().wrapping_add(CYCLE_OFFSET[core_id()].load(Relaxed))
}

/// Returns the time if records are being kept now, and ends the capture when it's over.
#[inline(always)]
fn recording_now() -> Option<u32> {
    if !RECORDING.load(Relaxed) {
        return None;
    }
    let now = cycles();
    let elapsed = now.wrapping_sub(INSTALLED.load(Relaxed));
    if elapsed < DELAY.load(Relaxed) {
        return None;
    }
    if elapsed >= END.load(Relaxed) {
        finish();
        return None;
    }
    Some(now)
}

#[inline(always)]
fn push(words: &[u32]) {
    let core = core_id();
    let len = LEN[core].load(Relaxed);
    if len + words.len() > CAPACITY.load(Relaxed) {
        finish();
        return;
    }
    // Safety: this core's capture has room for `words` after `len`, and only this core writes it.
    unsafe {
        core::ptr::copy_nonoverlapping(
            words.as_ptr(),
            CAPTURE[core].load(Relaxed).add(len),
            words.len(),
        )
    };
    LEN[core].store(len + words.len(), Relaxed);
}

#[inline(always)]
fn record(id: u32, kind: u32) {
    if let Some(now) = recording_now() {
        push(&[id | kind, now]);
    }
}

fn extended(kind: u32, payload_words: usize) -> u32 {
    EXTENDED | kind << 2 | (payload_words as u32) << 8
}

fn push_name(address: u32, name: &str) {
    if !RECORDING.load(Relaxed) {
        return;
    }
    let text_words = name.len().div_ceil(4);
    let core = core_id();
    let len = LEN[core].load(Relaxed);
    if len + 3 + text_words > CAPACITY.load(Relaxed) {
        finish();
        return;
    }
    push(&[extended(NAME, 2 + text_words), address, name.len() as u32]);
    let start = LEN[core].load(Relaxed);
    // Safety: checked above that the text fits after the header.
    unsafe {
        let words = CAPTURE[core].load(Relaxed).add(start);
        core::ptr::write_bytes(words, 0, text_words);
        core::ptr::copy_nonoverlapping(name.as_ptr(), words.cast::<u8>(), name.len());
    }
    LEN[core].store(start + text_words, Relaxed);
}

struct FieldRecorder;

impl FieldRecorder {
    fn push(&self, field: &Field, value: u64) {
        if recording_now().is_some() {
            let name = field.name().as_ptr() as u32;
            push(&[extended(FIELD, 3), name, value as u32, (value >> 32) as u32]);
        }
    }
}

impl Visit for FieldRecorder {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.push(field, value as u64);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.push(field, value);
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.push(field, value as i64 as u64);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.push(field, value as u64);
    }

    fn record_debug(&mut self, _: &Field, _: &dyn core::fmt::Debug) {}
}

fn metadata_id(metadata: &'static Metadata<'static>) -> u32 {
    metadata as *const Metadata<'static> as usize as u32
}

struct CaptureSubscriber;

impl Subscriber for CaptureSubscriber {
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        push_name(metadata_id(metadata), metadata.name());
        for field in metadata.fields() {
            push_name(field.name().as_ptr() as u32, field.name());
        }
        Interest::always()
    }

    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, span: &Attributes<'_>) -> Id {
        span.record(&mut FieldRecorder);
        Id::from_u64(metadata_id(span.metadata()) as u64)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        event.record(&mut FieldRecorder);
        record(metadata_id(event.metadata()), EVENT);
    }

    fn enter(&self, span: &Id) {
        record(span.into_u64() as u32, ENTER);
    }

    fn exit(&self, span: &Id) {
        record(span.into_u64() as u32, EXIT);
    }
}

/// Starts the capture. Only the first call has an effect.
pub fn install() {
    if !CAPTURE[0].load(Relaxed).is_null() {
        return;
    }
    let mut bytes = MAX_CAPTURE_BYTES;
    let capture = loop {
        // Safety: the layout has a non-zero size.
        let capture = unsafe { alloc(Layout::from_size_align(bytes, 4).unwrap()) };
        if !capture.is_null() || bytes <= 1 << 20 {
            break capture;
        }
        bytes /= 2;
    };
    if capture.is_null() {
        return;
    }
    let capacity = bytes / 4 / CORES;
    for (core, capture_of_core) in CAPTURE.iter().enumerate() {
        // Safety: each core's part is within the allocation.
        capture_of_core.store(unsafe { capture.cast::<u32>().add(core * capacity) }, Relaxed);
    }
    CAPACITY.store(capacity, Relaxed);
    // The records of the other cores start with their track.
    for (core, &track) in CORE_TRACKS.iter().enumerate().skip(1) {
        // Safety: each core's part has room for this.
        unsafe {
            let words = CAPTURE[core].load(Relaxed);
            words.write(extended(CORE, 1));
            words.add(1).write(track);
        }
        LEN[core].store(2, Relaxed);
    }
    // Safety: a plain function.
    REFERENCE_US.store(unsafe { esp_timer_get_time() } as u32, Relaxed);
    REFERENCE_CYCLES.store(cycles(), Relaxed);
    RECORDING.store(true, Relaxed);

    // Safety: a plain ROM function.
    let ticks_per_us = unsafe { esp_rom_get_cpu_ticks_per_us() };
    TICKS_PER_US.store(ticks_per_us, Relaxed);
    push(&[extended(CLOCK, 1), ticks_per_us]);
    if tracing::dispatcher::set_global_default(tracing::Dispatch::new(CaptureSubscriber)).is_err() {
        return;
    }

    // Measures the cost of one span through the whole recording path, then drops its records.
    const PROBES: u32 = 10_000;
    let core = core_id();
    let len = LEN[core].load(Relaxed);
    let start = cycles();
    for _ in 0..PROBES {
        let _span = tracing::info_span!("trace_probe").entered();
    }
    let span_cost = cycles().wrapping_sub(start) / PROBES;
    LEN[core].store(len, Relaxed);
    push(&[extended(SPAN_COST, 1), span_cost]);
    // Safety: the format matches the arguments.
    unsafe {
        printf(
            c"slint trace: recording one span costs %lu cycles, %lu ns\n".as_ptr(),
            span_cost,
            span_cost * 1000 / ticks_per_us,
        )
    };

    DELAY.store(DELAY_US * ticks_per_us, Relaxed);
    END.store((DELAY_US + DURATION_US) * ticks_per_us, Relaxed);
    INSTALLED.store(cycles(), Relaxed);
}

/// Aligns the cycle count of the core this runs on with the first core's, so that both cores'
/// records share one time line. Call it once on each other core that records, after `install`.
#[unsafe(no_mangle)]
pub extern "C" fn slint_trace_register_core() {
    let core = core_id();
    if core == 0 || REFERENCE_US.load(Relaxed) == 0 {
        return;
    }
    // Safety: a plain function.
    let elapsed_us =
        (unsafe { esp_timer_get_time() } as u32).wrapping_sub(REFERENCE_US.load(Relaxed));
    let first_core_now = REFERENCE_CYCLES
        .load(Relaxed)
        .wrapping_add(elapsed_us.wrapping_mul(TICKS_PER_US.load(Relaxed)));
    CYCLE_OFFSET[core].store(first_core_now.wrapping_sub(core_cycles()), Relaxed);
}

/// Stops recording and sends the capture. Runs on the thread that recorded last, which blocks
/// until everything is sent.
fn finish() {
    if !RECORDING.swap(false, Relaxed) {
        return;
    }

    // Each core's records follow the previous core's. Another core can still be adding records
    // after its `LEN`, which are left out.
    let capture = CAPTURE.iter().zip(&LEN).flat_map(|(capture, len)| {
        // Safety: the first `len` words of each core's part are complete.
        unsafe {
            core::slice::from_raw_parts(capture.load(Relaxed).cast::<u8>(), len.load(Relaxed) * 4)
        }
    });
    // Room for the prefix, an escaped sequence number and chunk, and the newline.
    let mut packet = [0u8; 1024];
    let mut chunk = [0u8; 480];
    let mut chunk_len = 0;
    let mut sequence = 0u32;
    let mut send = |chunk: &[u8], sequence: u32| {
        packet[..2].copy_from_slice(b"\x1eB");
        let mut len = 2;
        let mut sequence = sequence;
        while sequence >= 0x80 {
            append_escaped(&mut packet, &mut len, sequence as u8 | 0x80);
            sequence >>= 7;
        }
        append_escaped(&mut packet, &mut len, sequence as u8);
        for &byte in chunk {
            append_escaped(&mut packet, &mut len, byte);
        }
        packet[len] = b'\n';
        write_all(&packet[..len + 1]);
    };
    for &byte in capture {
        chunk[chunk_len] = byte;
        chunk_len += 1;
        if chunk_len == chunk.len() {
            send(&chunk, sequence);
            sequence += 1;
            chunk_len = 0;
        }
    }
    if chunk_len > 0 {
        send(&chunk[..chunk_len], sequence);
    }
    write_all(b"\x1eE\n");
}

fn append_escaped(packet: &mut [u8], len: &mut usize, byte: u8) {
    if matches!(byte, b'\n' | b'\r' | 0x1b | 0x1e) {
        packet[*len] = 0x1b;
        packet[*len + 1] = byte ^ 0x40;
        *len += 2;
    } else {
        packet[*len] = byte;
        *len += 1;
    }
}

/// A span or field name recorded from C++. Its address is the name's ID, like a call site's
/// metadata for the Rust spans. See `SLINT_TRACE_SPAN` in slint-esp.cpp.
#[repr(C)]
pub struct TraceSite {
    name: *const core::ffi::c_char,
    registered: bool,
}

/// Safety: `site` points to a static site with a string literal as its name, which only the
/// recording thread uses.
unsafe fn site_id(site: *mut TraceSite) -> u32 {
    // Safety: see above.
    let site = unsafe { &mut *site };
    if !site.registered && RECORDING.load(Relaxed) {
        // Safety: see above.
        let name = unsafe { core::ffi::CStr::from_ptr(site.name) };
        push_name(site as *const TraceSite as usize as u32, name.to_str().unwrap_or("?"));
        site.registered = true;
    }
    site as *const TraceSite as usize as u32
}

/// Safety: see `site_id`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_trace_enter(site: *mut TraceSite) {
    record(unsafe { site_id(site) }, ENTER);
}

/// Safety: see `site_id`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_trace_exit(site: *mut TraceSite) {
    record(unsafe { site_id(site) }, EXIT);
}

/// Records a field of the next span entered.
///
/// Safety: see `site_id`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_trace_field(site: *mut TraceSite, value: i64) {
    let id = unsafe { site_id(site) };
    if recording_now().is_some() {
        push(&[extended(FIELD, 3), id, value as u32, (value >> 32) as u32]);
    }
}

/// Records an event, such as the frame marks Tracy shows for events named "frame".
///
/// Safety: see `site_id`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_trace_event(site: *mut TraceSite) {
    record(unsafe { site_id(site) }, EVENT);
}

/// The cycle count the capture's times are based on.
#[unsafe(no_mangle)]
pub extern "C" fn slint_trace_now() -> u32 {
    cycles()
}

/// Records a span from `begin` to `end`, cycle counts from `slint_trace_now`, on track `track`
/// next to the spans of the recording thread. Use it for work that overlaps those spans, such as a
/// DMA transfer, timed in an interrupt and recorded afterwards.
///
/// Safety: see `site_id`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_trace_track_span(
    site: *mut TraceSite,
    begin: u32,
    end: u32,
    track: u32,
) {
    let id = unsafe { site_id(site) };
    if recording_now().is_some() {
        push(&[extended(TRACK_SPAN, 4), id, begin, end, track]);
    }
}
