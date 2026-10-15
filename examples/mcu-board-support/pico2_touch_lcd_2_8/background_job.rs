// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, Ordering};

const FREE: u32 = 0;
const BUILDING: u32 = 1;
const READY: u32 = 2;
const ACTIVE: u32 = 3;
const DONE: u32 = 4;

#[derive(Clone, Copy)]
struct Request {
    data: *mut (),
    step: unsafe fn(*mut ()) -> bool,
    wake: fn(),
}

pub struct Queue {
    state: AtomicU32,
    request: UnsafeCell<Option<Request>>,
}

// A submitted closure is Send and remains exclusively borrowed until its join completes.
unsafe impl Sync for Queue {}

impl Queue {
    pub const fn new() -> Self {
        Self { state: AtomicU32::new(FREE), request: UnsafeCell::new(None) }
    }

    pub fn scope<F: FnMut() -> bool + Send, R>(
        &self,
        job: &mut F,
        local: impl FnOnce() -> R,
        wait: fn(),
        wake: fn(),
    ) -> R {
        unsafe fn step<F: FnMut() -> bool>(data: *mut ()) -> bool {
            unsafe { (&mut *data.cast::<F>())() }
        }
        assert!(
            self.state
                .compare_exchange(FREE, BUILDING, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
        );
        unsafe {
            *self.request.get() =
                Some(Request { data: core::ptr::from_mut(job).cast(), step: step::<F>, wake });
        }
        self.state.store(READY, Ordering::Release);
        let join = Join { queue: self, wait };
        wake();
        let result = local();
        drop(join);
        result
    }

    pub fn step(&self) -> bool {
        if self.state.compare_exchange(READY, ACTIVE, Ordering::Acquire, Ordering::Relaxed).is_err()
        {
            return false;
        }
        let request = unsafe { (*self.request.get()).unwrap() };
        let done = unsafe { (request.step)(request.data) };
        self.state.store(if done { DONE } else { READY }, Ordering::Release);
        if done {
            (request.wake)();
        }
        true
    }
}

struct Join<'a> {
    queue: &'a Queue,
    wait: fn(),
}

impl Drop for Join<'_> {
    fn drop(&mut self) {
        while self.queue.state.load(Ordering::Acquire) != DONE {
            (self.wait)();
        }
        unsafe { *self.queue.request.get() = None };
        self.queue.state.store(FREE, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn joins_borrowed_output_before_reusing_the_queue() {
        let queue = Queue::new();
        let stop = AtomicBool::new(false);
        std::thread::scope(|threads| {
            threads.spawn(|| {
                while !stop.load(Ordering::Acquire) {
                    queue.step();
                    std::thread::yield_now();
                }
            });
            for round in 0..1000 {
                let mut pixels = [0; 128];
                let mut next = 0;
                queue.scope(
                    &mut || {
                        pixels[next] = round + next;
                        next += 1;
                        next == pixels.len()
                    },
                    || std::thread::yield_now(),
                    std::thread::yield_now,
                    || {},
                );
                assert_eq!(pixels, core::array::from_fn(|i| round + i));
            }
            stop.store(true, Ordering::Release);
        });
    }

    #[test]
    fn local_panic_still_joins_before_releasing_borrowed_state() {
        let queue = Queue::new();
        let stop = AtomicBool::new(false);
        let mut calls = 0;
        std::thread::scope(|threads| {
            threads.spawn(|| {
                while !stop.load(Ordering::Acquire) {
                    queue.step();
                    std::thread::yield_now();
                }
            });
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                queue.scope(
                    &mut || {
                        calls += 1;
                        calls == 100
                    },
                    || panic!("Local render failed"),
                    std::thread::yield_now,
                    || {},
                );
            }));
            assert!(panic.is_err());
            assert_eq!(calls, 100);
            queue.scope(&mut || true, || {}, std::thread::yield_now, || {});
            stop.store(true, Ordering::Release);
        });
    }
}
