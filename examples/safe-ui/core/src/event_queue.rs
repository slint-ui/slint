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
pub const QUEUE_CAPACITY: usize = 32;

/// A callback to be invoked from C
pub struct FfiCallback {
    pub callback: unsafe extern "C" fn(*mut c_void),
    pub user_data: *mut c_void,
    pub drop_user_data: Option<unsafe extern "C" fn(*mut c_void)>,
}

// SAFETY: FfiCallback contains raw pointers which are `!Send` by default.
// This is safe because: producers only push under a critical section, and
// the consumer (take_queue) runs on a single thread (the Slint event loop).
// The pointers are never accessed concurrently.
unsafe impl Send for FfiCallback {}

impl Drop for FfiCallback {
    fn drop(&mut self) {
        if let Some(drop_fn) = self.drop_user_data {
            // SAFETY: Caller guaranteed drop_user_data is safe to call
            // from any context.
            unsafe { drop_fn(self.user_data) };
        }
    }
}

/// A single entry in the unified event queue.
///
/// Both FFI callbacks (from C firmware) and Rust closures (from
/// `EventLoopProxy`) are stored as variants.
pub enum QueueEntry {
    Quit,
    Callback(Box<dyn FnOnce() + Send>),
    FfiCallback(FfiCallback),
}

/// Static unified event queue. FFI producers push via
/// [`slint_safeui_invoke_from_event_loop`], Rust producers via
/// [`SafeUiEventLoopProxy`]. The consumer ([`take_queue`]) runs
/// on the Slint event loop.
static EVENT_QUEUE: Mutex<RefCell<Deque<QueueEntry, QUEUE_CAPACITY>>> =
    Mutex::new(RefCell::new(Deque::new()));

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
/// After the callback executes, `drop_user_data(user_data)` is called
/// (if non-NULL) to release any resources owned by `user_data`. If the
/// queue is full, `drop_user_data` is called immediately before
/// returning `-1`, so the caller never leaks.
///
/// # Safety
/// - `callback` must be a valid function pointer.
/// - `user_data` must remain valid until either `callback` or
///   `drop_user_data` is invoked (or may be null).
/// - `drop_user_data` (if non-null) must be safe to call from any
///   context — it may run in the caller's context on queue-full, or on
///   the Slint event loop thread after normal execution.
#[unsafe(no_mangle)]
pub extern "C" fn slint_safeui_invoke_from_event_loop(
    callback: unsafe extern "C" fn(*mut c_void),
    user_data: *mut c_void,
    drop_user_data: Option<unsafe extern "C" fn(*mut c_void)>,
) -> i32 {
    let ffi_cb = FfiCallback { callback, user_data, drop_user_data };
    let entry = QueueEntry::FfiCallback(ffi_cb);

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
            Err(rejected) => {
                // Queue full — the FfiCallback's Drop impl will run and
                // call drop_user_data automatically.
                drop(rejected);
                -1
            }
        }
    });

    result
}

/// Take all pending entries from the queue under a single short critical
/// section.
///
/// Must be called from the Slint event loop thread. The caller is
/// responsible for iterating the returned deque and handling each
/// [`QueueEntry`] variant.
pub(crate) fn take_queue() -> Deque<QueueEntry, QUEUE_CAPACITY> {
    critical_section::with(|cs| {
        let mut queue = EVENT_QUEUE.borrow_ref_mut(cs);
        core::mem::replace(&mut *queue, Deque::new())
    })
}
