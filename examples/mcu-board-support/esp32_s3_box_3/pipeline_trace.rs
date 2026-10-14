// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use alloc::boxed::Box;
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

const CAPACITY: usize = 2048;
type Row = [u32; 13];

pub(super) struct Trace {
    rows: Box<[UnsafeCell<Row>]>,
    published: AtomicU32,
    truncated: AtomicU32,
    claimed: AtomicBool,
}

// The unique recorder writes each slot once. Readers only access the prefix
// whose count was released after its final write. Slots are never reset.
unsafe impl Sync for Trace {}

impl Trace {
    pub(super) fn new() -> Self {
        #[cfg(test)]
        let rows = (0..CAPACITY)
            .map(|_| UnsafeCell::new([0; 13]))
            .collect::<alloc::vec::Vec<_>>()
            .into_boxed_slice();
        #[cfg(not(test))]
        let rows = {
            let layout = core::alloc::Layout::array::<UnsafeCell<Row>>(CAPACITY).unwrap();
            let pointer = unsafe {
                esp_alloc::HEAP.alloc_caps(esp_alloc::MemoryCapability::External.into(), layout)
            }
            .cast::<UnsafeCell<Row>>();
            assert!(!pointer.is_null(), "pipeline trace PSRAM allocation");
            for index in 0..CAPACITY {
                unsafe {
                    pointer.add(index).write(UnsafeCell::new([0; 13]));
                }
            }
            unsafe { Box::from_raw(core::ptr::slice_from_raw_parts_mut(pointer, CAPACITY)) }
        };
        Self {
            rows,
            published: AtomicU32::new(0),
            truncated: AtomicU32::new(0),
            claimed: AtomicBool::new(false),
        }
    }
    pub(super) fn recorder(&self) -> Recorder<'_> {
        assert!(
            self.claimed
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
        );
        Recorder { trace: self }
    }
    #[cfg(not(test))]
    pub(super) fn dump(&self) {
        let count = self.published.load(Ordering::Acquire) as usize;
        esp_println::println!(
            "PRINTER_PIPELINE_BEGIN,samples={},truncated={},columns=mode;source_tick;target_tick;display_tick;acquire_blank_us;acquire_us;publish_begin_us;reserve_us;ready_us;display_blank_us;tx_start_us;tx_end_us;bytes",
            count,
            self.truncated.load(Ordering::Relaxed)
        );
        for cell in &self.rows[..count] {
            let row = unsafe { cell.get().read() };
            esp_println::println!(
                "PRINTER_PIPELINE,{};{};{};{};{};{};{};{};{};{};{};{};{}",
                row[0],
                row[1],
                row[2],
                row[3],
                row[4],
                row[5],
                row[6],
                row[7],
                row[8],
                row[9],
                row[10],
                row[11],
                row[12]
            );
        }
        esp_println::println!("PRINTER_PIPELINE_END");
    }
}

pub(super) struct Recorder<'a> {
    trace: &'a Trace,
}
impl Recorder<'_> {
    pub(super) fn push(&mut self, row: Row) {
        let index = self.trace.published.load(Ordering::Relaxed) as usize;
        if index == CAPACITY {
            self.trace.truncated.fetch_add(1, Ordering::Relaxed);
            return;
        }
        unsafe {
            self.trace.rows[index].get().write(row);
        }
        self.trace.published.store(index as u32 + 1, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prefix_has_complete_rows_and_overflow_is_bounded() {
        let trace = Trace::new();
        let mut recorder = trace.recorder();
        for index in 0..CAPACITY + 3 {
            recorder.push([index as u32; 13]);
        }
        assert_eq!(trace.published.load(Ordering::Acquire) as usize, CAPACITY);
        assert_eq!(trace.truncated.load(Ordering::Relaxed), 3);
        for (index, cell) in trace.rows.iter().enumerate() {
            assert_eq!(unsafe { cell.get().read() }, [index as u32; 13]);
        }
    }
    #[test]
    #[should_panic]
    fn a_second_recorder_cannot_be_claimed() {
        let trace = Trace::new();
        let _first = trace.recorder();
        let _second = trace.recorder();
    }
    #[test]
    fn release_count_protects_a_concurrent_reader() {
        let trace = std::sync::Arc::new(Trace::new());
        let writer = trace.clone();
        let thread = std::thread::spawn(move || {
            let mut recorder = writer.recorder();
            for index in 0..CAPACITY {
                recorder.push([index as u32; 13]);
            }
        });
        let mut checked = 0;
        while checked < CAPACITY {
            let count = trace.published.load(Ordering::Acquire) as usize;
            for index in checked..count {
                assert_eq!(unsafe { trace.rows[index].get().read() }, [index as u32; 13]);
            }
            checked = count;
        }
        thread.join().unwrap();
    }
}
