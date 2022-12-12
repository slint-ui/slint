// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use core::ffi::c_void;
use i_slint_core::api::Window;
use i_slint_core::graphics::Rgb8Pixel;
use i_slint_core::platform::Platform;
use i_slint_core::renderer::Renderer;
use i_slint_core::software_renderer::SoftwareRenderer;
use i_slint_core::window::ffi::WindowAdapterRcOpaque;
use i_slint_core::window::{WindowAdapter, WindowAdapterSealed};
use std::rc::Rc;

type WindowAdapterUserData = *mut c_void;

// FIXME wrapper over &dyn Renderer
#[repr(C)]
pub struct RendererPtr {
    _a: *const c_void,
    _b: *const c_void,
}

pub struct CppWindowAdapter {
    window: Window,
    user_data: WindowAdapterUserData,
    drop: unsafe extern "C" fn(WindowAdapterUserData),
    /// Safety: the returned pointer must live for the lifetime of self
    get_renderer_ref: unsafe extern "C" fn(WindowAdapterUserData) -> RendererPtr,
    show: unsafe extern "C" fn(WindowAdapterUserData),
    hide: unsafe extern "C" fn(WindowAdapterUserData),
    request_redraw: unsafe extern "C" fn(WindowAdapterUserData),
}

impl Drop for CppWindowAdapter {
    fn drop(&mut self) {
        unsafe { (self.drop)(self.user_data) };
    }
}

impl WindowAdapter for CppWindowAdapter {
    fn window(&self) -> &Window {
        &self.window
    }
}

impl WindowAdapterSealed for CppWindowAdapter {
    fn renderer(&self) -> &dyn Renderer {
        unsafe { core::mem::transmute((self.get_renderer_ref)(self.user_data)) }
    }

    fn show(&self) {
        unsafe { (self.show)(self.user_data) }
    }
    fn hide(&self) {
        unsafe { (self.hide)(self.user_data) }
    }

    fn request_redraw(&self) {
        unsafe { (self.request_redraw)(self.user_data) }
    }
}

#[no_mangle]
pub unsafe extern "C" fn slint_window_adapter_new(
    user_data: WindowAdapterUserData,
    drop: unsafe extern "C" fn(WindowAdapterUserData),
    get_renderer_ref: unsafe extern "C" fn(WindowAdapterUserData) -> RendererPtr,
    show: unsafe extern "C" fn(WindowAdapterUserData),
    hide: unsafe extern "C" fn(WindowAdapterUserData),
    request_redraw: unsafe extern "C" fn(WindowAdapterUserData),
    target: *mut WindowAdapterRcOpaque,
) {
    let window = Rc::<CppWindowAdapter>::new_cyclic(|w| CppWindowAdapter {
        window: Window::new(w.clone()),
        user_data,
        drop,
        get_renderer_ref,
        show,
        request_redraw,
        hide,
    });

    core::ptr::write(target as *mut Rc<dyn WindowAdapter>, window);
}

type PlatformUserData = *mut c_void;

struct CppPlatform {
    user_data: PlatformUserData,
    drop: unsafe extern "C" fn(PlatformUserData),
    window_factory: unsafe extern "C" fn(PlatformUserData, *mut WindowAdapterRcOpaque),
}

impl Drop for CppPlatform {
    fn drop(&mut self) {
        unsafe { (self.drop)(self.user_data) };
    }
}

impl Platform for CppPlatform {
    fn create_window_adapter(&self) -> Rc<dyn WindowAdapter> {
        let mut uninit = core::mem::MaybeUninit::<Rc<dyn WindowAdapter>>::uninit();
        unsafe {
            (self.window_factory)(
                self.user_data,
                uninit.as_mut_ptr() as *mut WindowAdapterRcOpaque,
            );
            uninit.assume_init()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn slint_platform_register(
    user_data: PlatformUserData,
    drop: unsafe extern "C" fn(PlatformUserData),
    window_factory: unsafe extern "C" fn(PlatformUserData, *mut WindowAdapterRcOpaque),
) {
    let p = CppPlatform { user_data, drop, window_factory };
    i_slint_core::platform::set_platform(Box::new(p)).unwrap();
}

type SoftwareRendererOpaque = *const c_void;

#[no_mangle]
pub unsafe extern "C" fn slint_software_renderer_new(
    buffer_age: u32,
    window: &WindowAdapterRcOpaque,
) -> SoftwareRendererOpaque {
    let window = core::mem::transmute::<&WindowAdapterRcOpaque, &Rc<dyn WindowAdapter>>(window);
    let weak = Rc::downgrade(window);
    match buffer_age {
        0 => Box::into_raw(Box::new(SoftwareRenderer::<0>::new(weak))) as SoftwareRendererOpaque,
        1 => Box::into_raw(Box::new(SoftwareRenderer::<1>::new(weak))) as SoftwareRendererOpaque,
        2 => Box::into_raw(Box::new(SoftwareRenderer::<2>::new(weak))) as SoftwareRendererOpaque,
        _ => unreachable!(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn slint_software_renderer_drop(buffer_age: u32, r: SoftwareRendererOpaque) {
    match buffer_age {
        0 => drop(Box::from_raw(r as *mut SoftwareRenderer<0>)),
        1 => drop(Box::from_raw(r as *mut SoftwareRenderer<1>)),
        2 => drop(Box::from_raw(r as *mut SoftwareRenderer<2>)),
        _ => unreachable!(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn slint_software_renderer_render_rgb8(
    buffer_age: u32,
    r: SoftwareRendererOpaque,
    buffer: *mut Rgb8Pixel,
    buffer_len: usize,
    buffer_stride: usize,
) {
    let buffer = core::slice::from_raw_parts_mut(buffer, buffer_len);
    match buffer_age {
        0 => (*(r as *const SoftwareRenderer<0>)).render(buffer, buffer_stride),
        1 => (*(r as *const SoftwareRenderer<1>)).render(buffer, buffer_stride),
        2 => (*(r as *const SoftwareRenderer<2>)).render(buffer, buffer_stride),
        _ => unreachable!(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn slint_software_renderer_handle(
    buffer_age: u32,
    r: SoftwareRendererOpaque,
) -> RendererPtr {
    let r = match buffer_age {
        0 => (r as *const SoftwareRenderer<0>) as *const dyn Renderer,
        1 => (r as *const SoftwareRenderer<1>) as *const dyn Renderer,
        2 => (r as *const SoftwareRenderer<2>) as *const dyn Renderer,
        _ => unreachable!(),
    };
    core::mem::transmute(r)
}

#[no_mangle]
pub unsafe extern "C" fn slint_windowrc_has_active_animations(
    handle: *const WindowAdapterRcOpaque,
) -> bool {
    let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
    window_adapter.window().has_active_animations()
}

#[no_mangle]
pub extern "C" fn slint_platform_update_timers_and_animations() {
    i_slint_core::platform::update_timers_and_animations()
}
