// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

extern crate alloc;
use alloc::boxed::Box;
use alloc::rc::Rc;

use crate::bindings::*;

use crate::event_dispatch;
use event_queue::QueueEntry;
use event_queue::SafeUiEventLoopProxy;

pub use event_queue::push_input_event;
pub use event_queue::wake_event_loop;

struct Platform {
    scale_factor: f32,
    window: Rc<slint::platform::software_renderer::MinimalSoftwareWindow>,
}

impl slint::platform::Platform for Platform {
    fn create_window_adapter(
        &self,
    ) -> Result<alloc::rc::Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        self.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {
            scale_factor: self.scale_factor,
        });

        let mut width: u32 = 0;
        let mut height: u32 = 0;
        unsafe {
            slint_safeui_platform_get_screen_size(&mut width as *mut _, &mut height as *mut _);
        }

        self.window.set_size(slint::WindowSize::Physical(slint::PhysicalSize::new(width, height)));
        self.window.request_redraw();

        loop {
            slint::platform::update_timers_and_animations();

            // Process all pending queue entries (FFI callbacks, Rust
            // closures, input events, quit signals).
            for entry in event_queue::take_queue() {
                match entry {
                    QueueEntry::Quit => return Ok(()),
                    QueueEntry::Callback(f) => f(),
                    QueueEntry::FfiCallback(ffi_cb) => {
                        // SAFETY: The C caller guaranteed that callback is a
                        // valid function pointer and user_data remains valid
                        // until invocation.
                        unsafe { (ffi_cb.callback)(ffi_cb.user_data) };
                    }
                    QueueEntry::InputEvent(ffi_event) => {
                        match event_dispatch::convert_ffi_event(&ffi_event, self.scale_factor) {
                            None => return Ok(()),
                            Some(window_event) => {
                                self.window.dispatch_event(window_event);
                            }
                        }
                    }
                }
            }

            self.window.draw_if_needed(|renderer| {
                render_wrapper::<crate::pixels::PlatformPixel, _>(&|buffer, pixel_stride| {
                    renderer.render(buffer, pixel_stride);
                })
            });

            let mut next_timeout = slint::platform::duration_until_next_timer_update();

            if self.window.has_active_animations() {
                let frame_duration = core::time::Duration::from_millis(16);
                next_timeout = Some(match next_timeout {
                    Some(x) => x.min(frame_duration),
                    None => frame_duration,
                })
            }

            unsafe {
                slint_safeui_platform_wait_for_events(
                    next_timeout.map_or(-1, |dur| dur.as_millis() as i32),
                )
            };
        }
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn slint::platform::EventLoopProxy>> {
        Some(Box::new(SafeUiEventLoopProxy))
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(unsafe {
            slint_safeui_platform_duration_since_start() as u64
        })
    }
}

mod event_queue {
    use core::{cell::RefCell, ffi::c_void};

    use alloc::boxed::Box;
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

    /// A single entry in the unified event queue.
    ///
    /// FFI callbacks (from C firmware), Rust closures (from
    /// `EventLoopProxy`), and input events (from
    /// `slint_safeui_dispatch_event`) are stored as variants.
    pub enum QueueEntry {
        Quit,
        Callback(Box<dyn FnOnce() + Send>),
        FfiCallback(FfiCallback),
        InputEvent(crate::ffi_event::FfiEvent),
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
                wake_event_loop();
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
                wake_event_loop();
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

    /// Push a raw input event into the unified queue.
    ///
    /// Called from [`crate::event_dispatch::slint_safeui_dispatch_event`].
    /// Returns `0` on success, `-1` if the queue is full.
    pub fn push_input_event(event: crate::ffi_event::FfiEvent) -> i32 {
        let result = critical_section::with(|cs| {
            let mut queue = EVENT_QUEUE.borrow_ref_mut(cs);
            match queue.push_back(QueueEntry::InputEvent(event)) {
                Ok(()) => {
                    // Wake the Slint event loop so it drains promptly.
                    wake_event_loop();
                    0
                }
                Err(_) => -1,
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
    pub fn take_queue() -> Deque<QueueEntry, QUEUE_CAPACITY> {
        critical_section::with(|cs| {
            let mut queue = EVENT_QUEUE.borrow_ref_mut(cs);
            core::mem::replace(&mut *queue, Deque::new())
        })
    }
}

fn render_wrapper<P, F>(f: &F)
where
    P: slint::platform::software_renderer::TargetPixel + bytemuck::Pod,
    F: Fn(&mut [P], usize),
{
    let user_data = f as *const _ as *const core::ffi::c_void;

    unsafe extern "C" fn c_render_wrap<P, F>(
        user_data: *const core::ffi::c_void,
        buffer: *mut core::ffi::c_char,
        byte_size: core::ffi::c_uint,
        pixel_stride: core::ffi::c_uint,
    ) where
        P: slint::platform::software_renderer::TargetPixel + bytemuck::Pod,
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

pub fn slint_init_safeui_platform(width: u32, height: u32, scale_factor: f32) {
    let window = slint::platform::software_renderer::MinimalSoftwareWindow::new(
        slint::platform::software_renderer::RepaintBufferType::NewBuffer,
    );

    window.set_size(slint::PhysicalSize { width, height });

    let platform = Platform { scale_factor, window };

    slint::platform::set_platform(Box::new(platform)).unwrap();
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

mod allocator {
    use core::alloc::Layout;
    use core::ffi::c_void;
    unsafe extern "C" {
        pub fn free(p: *mut c_void);
        pub fn malloc(size: usize) -> *mut c_void;
    }

    struct CAlloc;
    unsafe impl core::alloc::GlobalAlloc for CAlloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let align = layout.align();
            if align <= core::mem::size_of::<usize>() {
                unsafe { malloc(layout.size()) as *mut u8 }
            } else {
                // Ideally we'd use aligned_alloc, but that function caused heap corruption with esp-idf
                let ptr = unsafe { malloc(layout.size() + align) as *mut u8 };
                let shift = align - (ptr as usize % align);
                let ptr = unsafe { ptr.add(shift) };
                unsafe { core::ptr::write(ptr.sub(1), shift as u8) };
                ptr
            }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe {
                let align = layout.align();
                if align <= core::mem::size_of::<usize>() {
                    free(ptr as *mut c_void);
                } else {
                    let shift = core::ptr::read(ptr.sub(1)) as usize;
                    free(ptr.sub(shift) as *mut c_void);
                }
            }
        }
    }

    #[global_allocator]
    static ALLOCATOR: CAlloc = CAlloc;
}
