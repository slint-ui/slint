// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

extern crate alloc;
use alloc::boxed::Box;
use core::cell::RefCell;
use core::ffi::c_void;
use critical_section::Mutex;
use heapless::Deque;

unsafe extern "C" {
    /// Wake the Slint event loop task. Provided by the C platform layer.
    fn slint_safeui_platform_wake();
}

/// Maximum number of entries buffered between drain cycles.
const QUEUE_CAPACITY: usize = 32;

/// A single entry in the unified event queue.
///
/// Both FFI callbacks (from C firmware) and Rust closures (from
/// `EventLoopProxy`) are stored as variants. The `FfiCallback` variant
/// is ISR-safe to construct — it's just two pointer-sized fields.
enum QueueEntry {
    Quit,
    Callback(Box<dyn FnOnce() + Send>),
    FfiCallback { callback: unsafe extern "C" fn(*mut c_void), user_data: *mut c_void },
}

// SAFETY: The `FfiCallback` variant contains raw pointers which are `!Send`.
// This is safe because: producers only push under a critical section, and
// the consumer (drain_callbacks) runs on a single thread (the Slint event
// loop). The pointers are never accessed concurrently.
unsafe impl Send for QueueEntry {}

/// Static unified event queue. FFI producers push via
/// [`slint_safeui_invoke_from_event_loop`], Rust producers via
/// [`SafeUiEventLoopProxy`]. The consumer ([`drain_callbacks`]) pops
/// from the Slint event loop.
static EVENT_QUEUE: Mutex<RefCell<Deque<QueueEntry, QUEUE_CAPACITY>>> =
    Mutex::new(RefCell::new(Deque::new()));

/// Event type consumed by the platform event loop.
///
/// This is the Rust-side representation produced by [`drain_callbacks`].
/// It never crosses the FFI boundary.
pub enum Event {
    /// Clean shutdown of the event loop.
    Quit,
    /// A callback to execute on the event loop thread.
    Callback(Box<dyn FnOnce() + Send>),
}

/// Proxy for injecting events from Rust code into the Slint event loop.
///
/// This is returned by `Platform::new_event_loop_proxy()` and enables
/// `slint::invoke_from_event_loop()` and `slint::quit_event_loop()`.
#[derive(Clone)]
pub struct SafeUiEventLoopProxy;

impl slint::platform::EventLoopProxy for SafeUiEventLoopProxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        let result = critical_section::with(|cs| {
            EVENT_QUEUE
                .borrow_ref_mut(cs)
                .push_back(QueueEntry::Quit)
                .map_err(|_| slint::EventLoopError::EventLoopTerminated)
        });
        if result.is_ok() {
            unsafe { slint_safeui_platform_wake() };
        }
        result
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), slint::EventLoopError> {
        let result = critical_section::with(|cs| {
            EVENT_QUEUE
                .borrow_ref_mut(cs)
                .push_back(QueueEntry::Callback(event))
                .map_err(|_| slint::EventLoopError::EventLoopTerminated)
        });
        if result.is_ok() {
            unsafe { slint_safeui_platform_wake() };
        }
        result
    }
}

/// Schedule a callback to run on the Slint event loop thread.
///
/// This function is the **only** FFI entry point for cross-thread
/// invocation. It is ISR-safe: no heap allocation, no blocking, no FPU
/// usage.
///
/// # Safety
/// - `callback` must be a valid function pointer.
/// - `user_data` must remain valid until the callback is invoked on the
///   event loop thread (or may be null).
#[unsafe(no_mangle)]
pub extern "C" fn slint_safeui_invoke_from_event_loop(
    callback: unsafe extern "C" fn(*mut c_void),
    user_data: *mut c_void,
) -> i32 {
    let entry = QueueEntry::FfiCallback { callback, user_data };

    let result = critical_section::with(|cs| {
        let mut queue = EVENT_QUEUE.borrow_ref_mut(cs);
        match queue.push_back(entry) {
            Ok(()) => {
                // Wake the Slint event loop so it drains promptly.
                // SAFETY: slint_safeui_platform_wake is provided by the C platform
                // layer and is documented as callable from ISR context.
                unsafe { slint_safeui_platform_wake() };
                0
            }
            Err(_) => -1,
        }
    });

    result
}

/// Drain all pending entries from the unified queue.
///
/// Entries are popped under a single short critical section. FFI callbacks
/// are executed immediately; Rust closures and quit signals are returned
/// as [`Event`] values for the caller to handle.
///
/// Must be called from the Slint event loop thread.
pub fn drain_callbacks() -> heapless::Vec<Event, QUEUE_CAPACITY> {
    let mut raw: heapless::Vec<QueueEntry, QUEUE_CAPACITY> = heapless::Vec::new();
    critical_section::with(|cs| {
        let mut queue = EVENT_QUEUE.borrow_ref_mut(cs);
        while let Some(entry) = queue.pop_front() {
            let _ = raw.push(entry);
        }
    });

    let mut events: heapless::Vec<Event, QUEUE_CAPACITY> = heapless::Vec::new();
    for entry in raw {
        match entry {
            QueueEntry::Quit => {
                let _ = events.push(Event::Quit);
            }
            QueueEntry::Callback(f) => {
                let _ = events.push(Event::Callback(f));
            }
            QueueEntry::FfiCallback { callback, user_data } => {
                // SAFETY: The C caller guaranteed that callback is a valid
                // function pointer and user_data remains valid until invocation.
                unsafe { (callback)(user_data) };
            }
        }
    }

    events
}
