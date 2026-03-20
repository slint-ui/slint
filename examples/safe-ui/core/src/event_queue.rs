// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::ffi::c_void;
use critical_section::Mutex;
use heapless::Deque;

unsafe extern "C" {
    /// Wake the Slint event loop task. Provided by the C platform layer.
    fn slint_safeui_platform_wake();
}

/// A callback + user_data pair from C, stored in the static queue.
#[derive(Clone, Copy)]
struct FfiCallback {
    callback: unsafe extern "C" fn(*mut c_void),
    user_data: *mut c_void,
}

// SAFETY: FfiCallback is only raw pointers/fn pointers. The queue is
// drained on a single thread (the Slint event loop), and producers only
// push under a critical section.
unsafe impl Send for FfiCallback {}

/// Maximum number of FFI callbacks buffered between drain cycles.
const QUEUE_CAPACITY: usize = 32;

/// Static FFI callback queue. Producers (C firmware) push via
/// [`slint_safeui_invoke_from_event_loop`]. The consumer
/// ([`drain_callbacks`]) pops from the Slint event loop.
static FFI_CALLBACK_QUEUE: Mutex<RefCell<Deque<FfiCallback, QUEUE_CAPACITY>>> =
    Mutex::new(RefCell::new(Deque::new()));

/// Internal event queue for Rust-side event injection via
/// [`EventLoopProxy`](slint::platform::EventLoopProxy).
///
/// This uses a heap-allocated `Vec` behind a `Mutex` because:
/// - `Box<dyn FnOnce() + Send>` is not `Copy`, so it can't live in a
///   `heapless::Deque`.
/// - Internal events are rare (UI thread callbacks), so `Vec` overhead
///   is negligible.
/// - The `Vec` itself is only allocated once (empty vec = no heap alloc)
///   and grows on demand.
static INTERNAL_EVENT_QUEUE: Mutex<RefCell<Vec<InternalEvent>>> =
    Mutex::new(RefCell::new(Vec::new()));

/// Rust-internal events that never cross FFI.
enum InternalEvent {
    Quit,
    Callback(Box<dyn FnOnce() + Send>),
}

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
///
/// Events are pushed into the [`INTERNAL_EVENT_QUEUE`] (not the FFI queue)
/// since they carry heap-allocated closures.
#[derive(Clone)]
pub struct SafeUiEventLoopProxy;

impl slint::platform::EventLoopProxy for SafeUiEventLoopProxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        critical_section::with(|cs| {
            INTERNAL_EVENT_QUEUE.borrow_ref_mut(cs).push(InternalEvent::Quit);
        });
        unsafe { slint_safeui_platform_wake() };
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), slint::EventLoopError> {
        critical_section::with(|cs| {
            INTERNAL_EVENT_QUEUE.borrow_ref_mut(cs).push(InternalEvent::Callback(event));
        });
        unsafe { slint_safeui_platform_wake() };
        Ok(())
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
    let entry = FfiCallback { callback, user_data };

    let result = critical_section::with(|cs| {
        let mut queue = FFI_CALLBACK_QUEUE.borrow_ref_mut(cs);
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

/// Drain and execute all pending callbacks, then return internal events.
///
/// FFI callbacks are drained from the static `heapless::Deque` under a short
/// critical section, then **called directly** outside the critical section.
///
/// Internal events (from [`SafeUiEventLoopProxy`]) are drained under a
/// separate short critical section and returned for the caller to handle.
///
/// Must be called from the Slint event loop thread.
pub fn drain_callbacks() -> Vec<Event> {
    // Phase 1: drain FFI callbacks under critical section.
    let mut raw_callbacks: heapless::Vec<FfiCallback, QUEUE_CAPACITY> = heapless::Vec::new();
    critical_section::with(|cs| {
        let mut queue = FFI_CALLBACK_QUEUE.borrow_ref_mut(cs);
        while let Some(cb) = queue.pop_front() {
            let _ = raw_callbacks.push(cb);
        }
    });

    // Phase 2: execute FFI callbacks outside the critical section.
    for cb in &raw_callbacks {
        // SAFETY: The C caller guaranteed that callback is a valid function
        // pointer and user_data remains valid until invocation.
        unsafe { (cb.callback)(cb.user_data) };
    }

    // Phase 3: drain internal events under a separate critical section.
    let internal_events: Vec<InternalEvent> =
        critical_section::with(|cs| core::mem::take(&mut *INTERNAL_EVENT_QUEUE.borrow_ref_mut(cs)));

    let mut events = Vec::with_capacity(internal_events.len());
    for internal in internal_events {
        match internal {
            InternalEvent::Quit => events.push(Event::Quit),
            InternalEvent::Callback(f) => events.push(Event::Callback(f)),
        }
    }

    events
}
