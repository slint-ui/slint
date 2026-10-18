// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering};

static COUNTS: [AtomicU32; 10] = [const { AtomicU32::new(0) }; 10];
static POSITION: AtomicU32 = AtomicU32::new(u32::MAX);
static LAST_CONTACT: AtomicU32 = AtomicU32::new(0);
static MAX_GAP: AtomicU32 = AtomicU32::new(0);
static SEQUENCE: AtomicU32 = AtomicU32::new(0);
static TRACE: [[AtomicU32; 4]; 64] = [const { [const { AtomicU32::new(0) }; 4] }; 64];

pub fn report(data: &[u8; 6]) {
    let now = super::now_us() as u32;
    COUNTS[0].fetch_add(1, Ordering::Relaxed);
    let point = super::touch_packet::point(data);
    if let Some((x, y)) = point {
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
        LAST_CONTACT.store(0, Ordering::Relaxed);
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
    entry[2].store(u32::from_le_bytes(data[..4].try_into().unwrap()), Ordering::Relaxed);
    entry[3].store(u32::from(u16::from_le_bytes(data[4..].try_into().unwrap())), Ordering::Relaxed);
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
pub fn entry(sequence: u32) -> Option<[u32; 3]> {
    let entry = &TRACE[sequence as usize % TRACE.len()];
    if entry[0].load(Ordering::Acquire) != sequence {
        return None;
    }
    let value = core::array::from_fn(|i| entry[i + 1].load(Ordering::Relaxed));
    (entry[0].load(Ordering::Acquire) == sequence).then_some(value)
}
