// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering};

static COUNTS: [AtomicU32; 10] = [const { AtomicU32::new(0) }; 10];
static POSITION: AtomicU32 = AtomicU32::new(u32::MAX);
static LAST_CONTACT: AtomicU32 = AtomicU32::new(0);
static MAX_GAP: AtomicU32 = AtomicU32::new(0);
static SEQUENCE: AtomicU32 = AtomicU32::new(0);
static TRACE: [[AtomicU32; 9]; 16] = [const { [const { AtomicU32::new(0) }; 9] }; 16];

pub fn report(data: &[u8; 27], report: super::touch_packet::Report) {
    let now = super::now_us() as u32;
    COUNTS[0].fetch_add(1, Ordering::Relaxed);
    if let super::touch_packet::Report::Contact(x, y) = report {
        COUNTS[1].fetch_add(1, Ordering::Relaxed);
        let packed = u32::from(x) | (u32::from(y) << 16);
        if POSITION.swap(packed, Ordering::Relaxed) != packed {
            COUNTS[2].fetch_add(1, Ordering::Relaxed);
        }
        let previous = LAST_CONTACT.swap(now, Ordering::Relaxed);
        if previous != 0 {
            MAX_GAP.fetch_max(now.wrapping_sub(previous), Ordering::Relaxed);
        }
    } else {
        if report == super::touch_packet::Report::Released {
            LAST_CONTACT.store(0, Ordering::Relaxed);
        }
        let counter = if data[5] & 15 == 0 {
            3
        } else if data[0] & 15 != 6 {
            4
        } else {
            5
        };
        COUNTS[counter].fetch_add(1, Ordering::Relaxed);
    }
    let sequence = SEQUENCE.load(Ordering::Relaxed).wrapping_add(1);
    let entry = &TRACE[sequence as usize % TRACE.len()];
    entry[0].store(0, Ordering::Release);
    entry[1].store(now, Ordering::Relaxed);
    let mut padded = [0; 28];
    padded[..27].copy_from_slice(data);
    for (slot, bytes) in entry[2..].iter().zip(padded.chunks_exact(4)) {
        slot.store(u32::from_le_bytes(bytes.try_into().unwrap()), Ordering::Relaxed);
    }
    entry[0].store(sequence, Ordering::Release);
    SEQUENCE.store(sequence, Ordering::Release);
}

pub fn event(kind: usize) {
    COUNTS[6 + kind].fetch_add(1, Ordering::Relaxed);
}
pub fn error() {
    COUNTS[9].fetch_add(1, Ordering::Relaxed);
}
pub fn snapshot() -> ([u32; 10], u32) {
    (COUNTS.each_ref().map(|v| v.load(Ordering::Relaxed)), MAX_GAP.load(Ordering::Relaxed))
}
pub fn sequence() -> u32 {
    SEQUENCE.load(Ordering::Acquire)
}
pub fn entry(sequence: u32) -> Option<[u32; 8]> {
    let entry = &TRACE[sequence as usize % TRACE.len()];
    if entry[0].load(Ordering::Acquire) != sequence {
        return None;
    }
    let value = core::array::from_fn(|i| entry[i + 1].load(Ordering::Relaxed));
    (entry[0].load(Ordering::Acquire) == sequence).then_some(value)
}
