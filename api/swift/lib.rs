// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*! This crate just exposes the functions used by the Swift integration */

#![no_std]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use alloc::rc::Rc;
use core::ffi::c_void;
use i_slint_core::SharedString;
use i_slint_core::window::{WindowAdapter, ffi::WindowAdapterRcOpaque};

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

use alloc::boxed::Box;
use i_slint_core::graphics::Image;

/// Allocates a new default (empty) Image on the heap and returns a pointer.
/// The caller must eventually call `slint_swift_image_drop` to free it.
#[unsafe(no_mangle)]
pub extern "C" fn slint_swift_image_new() -> *mut Image {
    Box::into_raw(Box::new(Image::default()))
}

/// Drops a heap-allocated Image previously created by `slint_swift_image_new`
/// or `slint_swift_image_clone`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_image_drop(image: *mut Image) {
    if !image.is_null() {
        unsafe {
            drop(Box::from_raw(image));
        }
    }
}

/// Clones a heap-allocated Image. Returns a new heap-allocated Image.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_image_clone(image: *const Image) -> *mut Image {
    if image.is_null() {
        return slint_swift_image_new();
    }
    unsafe { Box::into_raw(Box::new((*image).clone())) }
}

/// Loads an image from a file path into a heap-allocated Image.
/// Returns a pointer to the new Image.
#[cfg(feature = "std")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_image_load_from_path(path: &SharedString) -> *mut Image {
    let img = Image::load_from_path(std::path::Path::new(path.as_str())).unwrap_or_default();
    Box::into_raw(Box::new(img))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_windowrc_init(out: *mut WindowAdapterRcOpaque) {
    assert_eq!(
        core::mem::size_of::<Rc<dyn WindowAdapter>>(),
        core::mem::size_of::<WindowAdapterRcOpaque>()
    );
    let win = with_platform(|b| b.create_window_adapter()).unwrap();
    unsafe {
        core::ptr::write(out as *mut Rc<dyn WindowAdapter>, win);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_ensure_backend() {
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
pub extern "C" fn slint_quit_event_loop() {
    i_slint_core::api::quit_event_loop().unwrap();
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

#[cfg(not(feature = "std"))]
mod allocator {
    use core::alloc::Layout;
    use core::ffi::c_void;

    struct CAlloc;
    unsafe impl core::alloc::GlobalAlloc for CAlloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            unsafe extern "C" {
                pub fn malloc(size: usize) -> *mut c_void;
            }
            unsafe {
                let align = layout.align();
                if align <= core::mem::size_of::<usize>() {
                    malloc(layout.size()) as *mut u8
                } else {
                    let ptr = malloc(layout.size() + align) as *mut u8;
                    let shift = align - (ptr as usize % align);
                    let ptr = ptr.add(shift);
                    core::ptr::write(ptr.sub(1), shift as u8);
                    ptr
                }
            }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            let align = layout.align();
            unsafe extern "C" {
                pub fn free(p: *mut c_void);
            }
            unsafe {
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

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
