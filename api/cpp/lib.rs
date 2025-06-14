// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*! This crate just expose the function used by the C++ integration */

#![no_std]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use alloc::rc::Rc;
use core::ffi::c_void;
use i_slint_core::items::OperatingSystemType;
use i_slint_core::window::{ffi::WindowAdapterRcOpaque, WindowAdapter};
use i_slint_core::SharedString;

pub mod platform;

#[cfg(feature = "i-slint-backend-selector")]
use i_slint_backend_selector::with_platform;

#[cfg(not(feature = "i-slint-backend-selector"))]
pub fn with_platform<R>(
    f: impl FnOnce(
        &dyn i_slint_core::platform::Platform,
    ) -> Result<R, i_slint_core::platform::PlatformError>,
) -> Result<R, i_slint_core::platform::PlatformError> {
    i_slint_core::with_platform(|| Err(i_slint_core::platform::PlatformError::NoPlatform), f)
}

// One need to make sure something from the crate is exported,
// otherwise its symbols are not going to be in the final binary
#[cfg(feature = "testing")]
pub use i_slint_backend_testing;
#[cfg(feature = "slint-interpreter")]
pub use slint_interpreter;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_windowrc_init(out: *mut WindowAdapterRcOpaque) {
    assert_eq!(
        core::mem::size_of::<Rc<dyn WindowAdapter>>(),
        core::mem::size_of::<WindowAdapterRcOpaque>()
    );
    let win = with_platform(|b| b.create_window_adapter()).unwrap();
    core::ptr::write(out as *mut Rc<dyn WindowAdapter>, win);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_ensure_backend() {
    with_platform(|_b| {
        // Nothing to do, just make sure a backend was created
        Ok(())
    })
    .unwrap()
}

#[unsafe(no_mangle)]
/// Enters the main event loop.
pub extern "C" fn slint_run_event_loop(quit_on_last_window_closed: bool) {
    with_platform(|b| {
        if !quit_on_last_window_closed {
            #[allow(deprecated)]
            b.set_event_loop_quit_on_last_window_closed(false);
        }
        b.run_event_loop()
    })
    .unwrap();
}

/// Will execute the given functor in the main thread
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_post_event(
    event: extern "C" fn(user_data: *mut c_void),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
) {
    struct UserData {
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    }
    impl Drop for UserData {
        fn drop(&mut self) {
            if let Some(x) = self.drop_user_data {
                x(self.user_data)
            }
        }
    }
    unsafe impl Send for UserData {}
    let ud = UserData { user_data, drop_user_data };

    i_slint_core::api::invoke_from_event_loop(move || {
        let ud = &ud;
        event(ud.user_data);
    })
    .unwrap();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_quit_event_loop() {
    i_slint_core::api::quit_event_loop().unwrap();
}

#[cfg(feature = "std")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_register_font_from_path(
    win: *const WindowAdapterRcOpaque,
    path: &SharedString,
    error_str: &mut SharedString,
) {
    let window_adapter = &*(win as *const Rc<dyn WindowAdapter>);
    *error_str = match window_adapter
        .renderer()
        .register_font_from_path(std::path::Path::new(path.as_str()))
    {
        Ok(()) => Default::default(),
        Err(err) => i_slint_core::string::ToSharedString::to_shared_string(&err),
    };
}

#[cfg(feature = "std")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_register_font_from_data(
    win: *const WindowAdapterRcOpaque,
    data: i_slint_core::slice::Slice<'static, u8>,
    error_str: &mut SharedString,
) {
    let window_adapter = &*(win as *const Rc<dyn WindowAdapter>);
    *error_str = match window_adapter.renderer().register_font_from_memory(data.as_slice()) {
        Ok(()) => Default::default(),
        Err(err) => i_slint_core::string::ToSharedString::to_shared_string(&err),
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_register_bitmap_font(
    win: *const WindowAdapterRcOpaque,
    font_data: &'static i_slint_core::graphics::BitmapFont,
) {
    let window_adapter = &*(win as *const Rc<dyn WindowAdapter>);
    window_adapter.renderer().register_bitmap_font(font_data);
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_string_to_float(string: &SharedString, value: &mut f32) -> bool {
    match string.as_str().parse::<f32>() {
        Ok(v) => {
            *value = v;
            true
        }
        Err(_) => false,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_string_character_count(string: &SharedString) -> usize {
    unicode_segmentation::UnicodeSegmentation::graphemes(string.as_str(), true).count()
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_string_to_usize(string: &SharedString, value: &mut usize) -> bool {
    match string.as_str().parse::<usize>() {
        Ok(v) => {
            *value = v;
            true
        }
        Err(_) => false,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_debug(string: &SharedString) {
    i_slint_core::debug_log!("{string}");
}

#[cfg(not(feature = "std"))]
mod allocator {
    use core::alloc::Layout;
    use core::ffi::c_void;
    extern "C" {
        pub fn free(p: *mut c_void);
        pub fn malloc(size: usize) -> *mut c_void;
    }

    struct CAlloc;
    unsafe impl core::alloc::GlobalAlloc for CAlloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let align = layout.align();
            if align <= core::mem::size_of::<usize>() {
                malloc(layout.size()) as *mut u8
            } else {
                // Ideally we'd use aligned_alloc, but that function caused heap corruption with esp-idf
                let ptr = malloc(layout.size() + align) as *mut u8;
                let shift = align - (ptr as usize % align);
                let ptr = ptr.add(shift);
                core::ptr::write(ptr.sub(1), shift as u8);
                ptr
            }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            let align = layout.align();
            if align <= core::mem::size_of::<usize>() {
                free(ptr as *mut c_void);
            } else {
                let shift = core::ptr::read(ptr.sub(1)) as usize;
                free(ptr.sub(shift) as *mut c_void);
            }
        }
    }

    #[global_allocator]
    static ALLOCATOR: CAlloc = CAlloc;
}

#[cfg(all(not(feature = "std"), not(feature = "esp-backtrace")))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
#[cfg(feature = "esp-backtrace")]
use esp_backtrace as _;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_set_xdg_app_id(_app_id: &SharedString) {
    #[cfg(feature = "i-slint-backend-selector")]
    i_slint_backend_selector::with_global_context(|ctx| ctx.set_xdg_app_id(_app_id.clone()))
        .unwrap();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_detect_operating_system() -> OperatingSystemType {
    i_slint_core::detect_operating_system()
}
