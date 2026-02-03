// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

extern crate alloc;
use alloc::boxed::Box;
use alloc::rc::Rc;
//use alloc::vec::Vec;
//use core::cell::RefCell;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

struct Platform {
    scale_factor: f32,
    window: Rc<slint::platform::software_renderer::MinimalSoftwareWindow>,
    //event_queue: Queue,
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

            //            let events_to_process =
            //                critical_section::with(|cs| self.event_queue.0.borrow(cs).take());
            //            for event in events_to_process.into_iter() {
            //                match event {
            //                    Event::Quit => return Ok(()),
            //                    Event::Event(f) => f(),
            //                }
            //            }

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

    //fn new_event_loop_proxy(&self) -> Option<Box<dyn slint::platform::EventLoopProxy>> {
    //    Some(Box::new(self.event_queue.clone()) as Box<dyn slint::platform::EventLoopProxy>)
    //}

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(unsafe {
            slint_safeui_platform_duration_since_start() as u64
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

    let platform = Platform {
        scale_factor,
        window,
        //event_queue: Queue(critical_section::Mutex::new(RefCell::new(Vec::new())).into()),
    };

    slint::platform::set_platform(Box::new(platform)).unwrap();
}

//enum Event {
//    Quit,
//    Event(Box<dyn FnOnce() + Send>),
//}
//
//#[derive(Clone)]
//struct Queue(alloc::sync::Arc<critical_section::Mutex<RefCell<Vec<Event>>>>);
//
//impl slint::platform::EventLoopProxy for Queue {
//    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
//        critical_section::with(|cs| {
//            self.0.borrow_ref_mut(cs).push(Event::Quit);
//        });
//
//        unsafe { slint_safeui_platform_wake() };
//        Ok(())
//    }
//
//    fn invoke_from_event_loop(
//        &self,
//        event: Box<dyn FnOnce() + Send>,
//    ) -> Result<(), slint::EventLoopError> {
//        critical_section::with(|cs| {
//            self.0.borrow_ref_mut(cs).push(Event::Event(event));
//        });
//        unsafe { slint_safeui_platform_wake() };
//        Ok(())
//    }
//}

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
