// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! The C-driven backend: an [`slint_safeui_app::Platform`] implemented on top
//! of the C system interface, the ISR-safe event queue that feeds it, and the
//! freestanding allocator and panic handler.

use core::ffi::c_void;
use core::time::Duration;

use alloc::vec::Vec;

use crate::bindings::*;
use crate::event_dispatch::EventAction;
use crate::pixels::PlatformPixel;

use event_queue::QueueEntry;
use heapless::Deque;

use slint_safeui_app::{AppEvent, Platform};

pub use event_queue::push_input_event;
pub use event_queue::wake_event_loop;

/// Drives the UI through the C system interface.
pub struct FfiPlatform {
    size: slint_sc::Size,
    /// Entries drained from the static queue, not yet handed to the event loop.
    pending: Deque<QueueEntry, { event_queue::QUEUE_CAPACITY }>,
    /// Reused RGB8 scratch the scene renders into before it is converted to the
    /// display's pixel format.
    frame: Vec<u8>,
}

impl Default for FfiPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl FfiPlatform {
    pub fn new() -> Self {
        let mut width: u32 = 0;
        let mut height: u32 = 0;
        // SAFETY: both pointers are valid and only written through.
        unsafe {
            slint_safeui_platform_get_screen_size(&mut width as *mut _, &mut height as *mut _);
        }
        Self { size: slint_sc::Size::new(width, height), pending: Deque::new(), frame: Vec::new() }
    }
}

impl Platform for FfiPlatform {
    fn now(&self) -> Duration {
        // SAFETY: the C function takes no arguments and returns a plain integer.
        Duration::from_millis(unsafe { slint_safeui_platform_duration_since_start() } as u64)
    }

    fn size(&self) -> slint_sc::Size {
        self.size
    }

    fn get_input_event(&mut self) -> Option<AppEvent> {
        loop {
            if self.pending.is_empty() {
                self.pending = event_queue::take_queue();
                if self.pending.is_empty() {
                    return None;
                }
            }
            match self.pending.pop_front().unwrap() {
                QueueEntry::FfiCallback(ffi_cb) => {
                    // SAFETY: The C caller guaranteed that callback is a valid
                    // function pointer and user_data remains valid until invocation.
                    unsafe { (ffi_cb.callback)(ffi_cb.user_data) };
                }
                QueueEntry::InputEvent(event) => {
                    match crate::event_dispatch::convert_ffi_event(&event) {
                        EventAction::Quit => return Some(AppEvent::Quit),
                        EventAction::Touch(touch) => return Some(AppEvent::Touch(touch)),
                        EventAction::Ignore => {}
                    }
                }
            }
        }
    }

    async fn wait_for_more_events(&mut self, timeout: Option<Duration>) {
        let max_wait = timeout.map_or(-1, |d| d.as_millis() as i32);
        // SAFETY: the C function takes a plain integer and blocks the caller.
        unsafe { slint_safeui_platform_wait_for_events(max_wait) };
    }

    fn with_frame_buffer(&mut self, render: impl FnOnce(&mut [u8])) {
        let width = self.size.width as usize;
        let height = self.size.height as usize;
        self.frame.resize(width * height * 3, 0);
        render(&mut self.frame);

        // Convert the RGB8 frame to the display's pixel format and flush it.
        let rgb = &self.frame;
        render_wrapper::<PlatformPixel, _>(|pixels, pixel_stride| {
            for y in 0..height {
                let source = &rgb[y * width * 3..];
                let destination = &mut pixels[y * pixel_stride..];
                for x in 0..width {
                    destination[x] = PlatformPixel::from_rgb8(
                        source[x * 3],
                        source[x * 3 + 1],
                        source[x * 3 + 2],
                    );
                }
            }
        });
    }
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

    /// A single entry in the unified event queue.
    ///
    /// FFI callbacks (from C firmware) and input events (from
    /// `slint_safeui_dispatch_event`) are stored as variants.
    pub enum QueueEntry {
        FfiCallback(FfiCallback),
        InputEvent(crate::ffi_event::FfiEvent),
    }

    /// Static unified event queue. FFI producers push via
    /// [`slint_safeui_invoke_from_event_loop`]. The consumer ([`take_queue`])
    /// runs on the Slint event loop.
    static EVENT_QUEUE: Mutex<RefCell<Deque<QueueEntry, QUEUE_CAPACITY>>> =
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
        let entry = QueueEntry::FfiCallback(ffi_cb);

        critical_section::with(|cs| {
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
        })
    }

    /// Push a raw input event into the unified queue.
    ///
    /// Called from [`crate::event_dispatch::slint_safeui_dispatch_event`].
    /// Returns `0` on success, `-1` if the queue is full.
    pub fn push_input_event(event: crate::ffi_event::FfiEvent) -> i32 {
        critical_section::with(|cs| {
            let mut queue = EVENT_QUEUE.borrow_ref_mut(cs);
            match queue.push_back(QueueEntry::InputEvent(event)) {
                Ok(()) => {
                    // Wake the Slint event loop so it drains promptly.
                    wake_event_loop();
                    0
                }
                Err(_) => -1,
            }
        })
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

fn render_wrapper<P, F>(render: F)
where
    P: bytemuck::Pod,
    F: FnOnce(&mut [P], usize),
{
    let mut render = Some(render);
    let user_data = (&mut render) as *mut Option<F> as *const c_void;

    unsafe extern "C" fn c_render_wrap<P, F>(
        user_data: *const c_void,
        buffer: *mut core::ffi::c_char,
        byte_size: core::ffi::c_uint,
        pixel_stride: core::ffi::c_uint,
    ) where
        P: bytemuck::Pod,
        F: FnOnce(&mut [P], usize),
    {
        let buffer = unsafe {
            core::slice::from_raw_parts_mut(
                buffer as *mut P,
                byte_size as usize / core::mem::size_of::<P>(),
            )
        };
        // SAFETY: user_data points at the `Option<F>` created below, alive for
        // this call and used by no one else.
        let render = unsafe { &mut *(user_data as *mut Option<F>) };
        (render.take().unwrap())(buffer, pixel_stride as usize);
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
