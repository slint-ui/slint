// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use slint_sc::{SliceBuffer, TargetPixel};

/// Monotonic milliseconds since `slint_app_main` was called.
pub fn duration_since_start_ms() -> u32 {
    (unsafe { slint_safeui_platform_duration_since_start() }) as u32
}

/// Blocks the current task for at most `max_wait_ms` milliseconds, or until
/// [`slint_safeui_platform_wake`] is called from another context.
pub fn wait_for_events_ms(max_wait_ms: i32) {
    unsafe { slint_safeui_platform_wait_for_events(max_wait_ms) };
}

/// Dispatch every callback posted to the static queue since the last call.
pub fn drain_events() {
    for cb in event_queue::take_queue() {
        // SAFETY: the producer of the callback guaranteed the pointer is
        // valid until it runs.
        unsafe { (cb.callback)(cb.user_data) };
    }
}

/// Borrow the framebuffer from firmware and call `draw` with a
/// [`SliceBuffer`] spanning it so the application can render into it.
pub fn render_frame(draw: impl Fn(&mut SliceBuffer<'_, crate::pixels::PlatformPixel>)) {
    render_wrapper::<crate::pixels::PlatformPixel, _>(&|pixels, pixel_stride| {
        let height = pixels.len() / pixel_stride;
        let mut buffer = SliceBuffer::new(pixels, pixel_stride, height);
        draw(&mut buffer);
    });
}

mod event_queue {
    use core::{cell::RefCell, ffi::c_void};

    use critical_section::Mutex;
    use heapless::Deque;

    /// Wake the Slint event loop from Rust code (e.g. after pushing to an
    /// event queue). This is a thin wrapper around the C platform function.
    pub fn wake_event_loop() {
        // SAFETY: slint_safeui_platform_wake is provided by the C platform
        // layer and is documented as callable from any context.
        unsafe { crate::platform::slint_safeui_platform_wake() };
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

    /// Static FFI event queue. Producers push via
    /// [`slint_safeui_invoke_from_event_loop`]; the consumer
    /// ([`take_queue`]) runs on the Slint event loop.
    static EVENT_QUEUE: Mutex<RefCell<Deque<FfiCallback, QUEUE_CAPACITY>>> =
        Mutex::new(RefCell::new(Deque::new()));

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

        let result = critical_section::with(|cs| {
            let mut queue = EVENT_QUEUE.borrow_ref_mut(cs);
            match queue.push_back(ffi_cb) {
                Ok(()) => {
                    // Wake the Slint event loop so it drains promptly.
                    wake_event_loop();
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
    /// Must be called from the Slint event loop thread.
    pub fn take_queue() -> Deque<FfiCallback, QUEUE_CAPACITY> {
        critical_section::with(|cs| {
            let mut queue = EVENT_QUEUE.borrow_ref_mut(cs);
            core::mem::replace(&mut *queue, Deque::new())
        })
    }
}

fn render_wrapper<P, F>(f: &F)
where
    P: TargetPixel,
    F: Fn(&mut [P], usize),
{
    let user_data = f as *const _ as *const core::ffi::c_void;

    unsafe extern "C" fn c_render_wrap<P, F>(
        user_data: *const core::ffi::c_void,
        buffer: *mut core::ffi::c_char,
        byte_size: core::ffi::c_uint,
        pixel_stride: core::ffi::c_uint,
    ) where
        P: TargetPixel,
        F: Fn(&mut [P], usize),
    {
        let buffer = unsafe {
            core::slice::from_raw_parts_mut(
                buffer as *mut P,
                byte_size as usize / core::mem::size_of::<P>(),
            )
        };
        let f = unsafe { &*(user_data as *const F) };
        f(buffer, pixel_stride as usize)
    }

    unsafe { slint_safeui_platform_render(user_data, Some(c_render_wrap::<P, F>)) }
}

#[cfg(feature = "panic-handler")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use core::ffi::CStr;
    use core::fmt::{self, Write};

    pub struct FixedBuf<'a> {
        buf: &'a mut [u8],
        pos: usize,
    }

    impl<'a> FixedBuf<'a> {
        pub fn new(storage: &'a mut [u8]) -> Self {
            Self { buf: storage, pos: 0 }
        }

        pub fn as_cstr(&mut self) -> &CStr {
            let cap = self.buf.len();
            let end = core::cmp::min(self.pos, cap.saturating_sub(1));
            self.buf[end] = 0;
            unsafe { CStr::from_bytes_with_nul_unchecked(&self.buf[..=end]) }
        }
    }

    impl Write for FixedBuf<'_> {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            let bytes = s.as_bytes();
            let cap = self.buf.len();

            if self.pos >= cap {
                return Ok(());
            }

            // Leave room for terminating null
            let remaining = cap - self.pos - 1;
            let to_copy = remaining.min(bytes.len());

            let dst = &mut self.buf[self.pos..self.pos + to_copy];
            dst.copy_from_slice(&bytes[..to_copy]);

            self.pos += to_copy;
            Ok(())
        }
    }

    unsafe extern "C" {
        pub fn slint_log_error(msg: *const core::ffi::c_char);
    }

    let mut storage: [u8; 256] = [0; 256];

    unsafe {
        let mut w = FixedBuf::new(&mut storage);
        write!(&mut w, "Rust PANIC: {:?}", info).ok();
        slint_log_error(w.as_cstr().as_ptr());
    };

    loop {}
}
