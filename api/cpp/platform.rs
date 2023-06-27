// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::ffi::c_void;
use i_slint_core::api::{PhysicalSize, Window};
use i_slint_core::graphics::{IntSize, Rgb8Pixel};
use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::renderer::Renderer;
use i_slint_core::software_renderer::{RepaintBufferType, SoftwareRenderer};
use i_slint_core::window::ffi::WindowAdapterRcOpaque;
use i_slint_core::window::{WindowAdapter, WindowAdapterInternal};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

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
    size: unsafe extern "C" fn(WindowAdapterUserData) -> IntSize,
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

    fn size(&self) -> PhysicalSize {
        let s = unsafe { (self.size)(self.user_data) };
        PhysicalSize::new(s.width, s.height)
    }

    fn renderer(&self) -> &dyn Renderer {
        unsafe { core::mem::transmute((self.get_renderer_ref)(self.user_data)) }
    }

    fn internal(&self, _: i_slint_core::InternalToken) -> Option<&dyn WindowAdapterInternal> {
        Some(self)
    }

    fn request_redraw(&self) {
        unsafe { (self.request_redraw)(self.user_data) }
    }
}

impl WindowAdapterInternal for CppWindowAdapter {
    fn show(&self) -> Result<(), PlatformError> {
        unsafe { (self.show)(self.user_data) };
        Ok(())
    }
    fn hide(&self) -> Result<(), PlatformError> {
        unsafe { (self.hide)(self.user_data) }
        Ok(())
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
    size: unsafe extern "C" fn(WindowAdapterUserData) -> IntSize,
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
        size,
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
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let mut uninit = core::mem::MaybeUninit::<Rc<dyn WindowAdapter>>::uninit();
        unsafe {
            (self.window_factory)(
                self.user_data,
                uninit.as_mut_ptr() as *mut WindowAdapterRcOpaque,
            );
            Ok(uninit.assume_init())
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

#[no_mangle]
pub unsafe extern "C" fn slint_windowrc_has_active_animations(
    handle: *const WindowAdapterRcOpaque,
) -> bool {
    let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
    window_adapter.window().has_active_animations()
}

/// Dispatch resize event
#[no_mangle]
pub unsafe extern "C" fn slint_windowrc_dispatch_resize_event(
    handle: *const WindowAdapterRcOpaque,
    width: f32,
    height: f32,
) {
    let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
    window_adapter.window().dispatch_event(i_slint_core::platform::WindowEvent::Resized {
        size: i_slint_core::api::LogicalSize { width, height },
    });
}

/// Dispatch scale factor change event
#[no_mangle]
pub unsafe extern "C" fn slint_windowrc_dispatch_scale_factor_change_event(
    handle: *const WindowAdapterRcOpaque,
    scale_factor: f32,
) {
    let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
    window_adapter
        .window()
        .dispatch_event(i_slint_core::platform::WindowEvent::ScaleFactorChanged { scale_factor });
}

#[no_mangle]
pub extern "C" fn slint_platform_update_timers_and_animations() {
    i_slint_core::platform::update_timers_and_animations()
}

type SoftwareRendererOpaque = *const c_void;

#[no_mangle]
pub unsafe extern "C" fn slint_software_renderer_new(buffer_age: u32) -> SoftwareRendererOpaque {
    let repaint_buffer_type = match buffer_age {
        0 => RepaintBufferType::NewBuffer,
        1 => RepaintBufferType::ReusedBuffer,
        2 => RepaintBufferType::SwappedBuffers,
        _ => unreachable!(),
    };
    Box::into_raw(Box::new(SoftwareRenderer::new_without_window(repaint_buffer_type)))
        as SoftwareRendererOpaque
}

#[no_mangle]
pub unsafe extern "C" fn slint_software_renderer_drop(r: SoftwareRendererOpaque) {
    drop(Box::from_raw(r as *mut SoftwareRenderer));
}

#[no_mangle]
pub unsafe extern "C" fn slint_software_renderer_render_rgb8(
    r: SoftwareRendererOpaque,
    window_adapter: *const WindowAdapterRcOpaque,
    buffer: *mut Rgb8Pixel,
    buffer_len: usize,
    pixel_stride: usize,
) {
    let buffer = core::slice::from_raw_parts_mut(buffer, buffer_len);
    let renderer = &*(r as *const SoftwareRenderer);
    let window_adapter = &*(window_adapter as *const Rc<dyn WindowAdapter>);
    renderer.set_window(window_adapter.window());
    renderer.render(buffer, pixel_stride);
}

#[no_mangle]
pub unsafe extern "C" fn slint_software_renderer_handle(r: SoftwareRendererOpaque) -> RendererPtr {
    let r = (r as *const SoftwareRenderer) as *const dyn Renderer;
    core::mem::transmute(r)
}

type SkiaRendererOpaque = *const c_void;
type SkiaRenderer = i_slint_renderer_skia::SkiaRenderer;

struct CppRawHandle(RawWindowHandle, RawDisplayHandle);

// the raw handle type are #[non_exhaustive], so they can't be initialize with the convenient syntax. Work that around.
macro_rules! init_raw {
    ($ty:ty { $($var:ident),* }) => {
        {
            let mut h = <$ty>::empty();
            $(h.$var = $var;)*
            h
        }
    };
}

type CppRawHandleOpaque = *const c_void;

#[no_mangle]
pub unsafe extern "C" fn slint_new_raw_window_handle_win32(
    hwnd: *mut c_void,
    hinstance: *mut c_void,
) -> CppRawHandleOpaque {
    let handle = CppRawHandle(
        RawWindowHandle::Win32(init_raw!(raw_window_handle::Win32WindowHandle { hwnd, hinstance })),
        RawDisplayHandle::Windows(raw_window_handle::WindowsDisplayHandle::empty()),
    );
    Box::into_raw(Box::new(handle)) as CppRawHandleOpaque
}

#[no_mangle]
pub unsafe extern "C" fn slint_new_raw_window_handle_x11(
    window: u32,
    visual_id: u32,
    connection: *mut c_void,
    screen: core::ffi::c_int,
) -> CppRawHandleOpaque {
    use raw_window_handle::{XcbDisplayHandle, XcbWindowHandle};
    let handle = CppRawHandle(
        RawWindowHandle::Xcb(init_raw!(XcbWindowHandle { window, visual_id })),
        RawDisplayHandle::Xcb(init_raw!(XcbDisplayHandle { connection, screen })),
    );
    Box::into_raw(Box::new(handle)) as CppRawHandleOpaque
}

#[no_mangle]
pub unsafe extern "C" fn slint_new_raw_window_handle_wayland(
    surface: *mut c_void,
    display: *mut c_void,
) -> CppRawHandleOpaque {
    use raw_window_handle::{WaylandDisplayHandle, WaylandWindowHandle};
    let handle = CppRawHandle(
        RawWindowHandle::Wayland(init_raw!(WaylandWindowHandle { surface })),
        RawDisplayHandle::Wayland(init_raw!(WaylandDisplayHandle { display })),
    );
    Box::into_raw(Box::new(handle)) as CppRawHandleOpaque
}

#[no_mangle]
pub unsafe extern "C" fn slint_new_raw_window_handle_appkit(
    ns_view: *mut c_void,
    ns_window: *mut c_void,
) -> CppRawHandleOpaque {
    use raw_window_handle::{AppKitDisplayHandle, AppKitWindowHandle};
    let handle = CppRawHandle(
        RawWindowHandle::AppKit(init_raw!(AppKitWindowHandle { ns_view, ns_window })),
        RawDisplayHandle::AppKit(AppKitDisplayHandle::empty()),
    );
    Box::into_raw(Box::new(handle)) as CppRawHandleOpaque
}

#[no_mangle]
pub unsafe extern "C" fn slint_raw_window_handle_drop(handle: CppRawHandleOpaque) {
    drop(Box::from_raw(handle as *mut CppRawHandle))
}

#[no_mangle]
pub unsafe extern "C" fn slint_skia_renderer_new(
    handle_opaque: CppRawHandleOpaque,
    size: IntSize,
) -> SkiaRendererOpaque {
    let handle = &*(handle_opaque as *const CppRawHandle);

    // Safety: This is safe because the handle remains valid; the next rwh release provides `new()` without unsafe.
    let active_handle = unsafe { raw_window_handle::ActiveHandle::new_unchecked() };

    // Safety: the C++ code should ensure that the handle is valid
    let (window_handle, display_handle) = unsafe {
        (
            raw_window_handle::WindowHandle::borrow_raw(handle.0, active_handle),
            raw_window_handle::DisplayHandle::borrow_raw(handle.1),
        )
    };

    let boxed_renderer: Box<SkiaRenderer> = Box::new(
        SkiaRenderer::new(
            window_handle,
            display_handle,
            PhysicalSize { width: size.width, height: size.height },
        )
        .unwrap(),
    );
    Box::into_raw(boxed_renderer) as SkiaRendererOpaque
}

#[no_mangle]
pub unsafe extern "C" fn slint_skia_renderer_drop(r: SkiaRendererOpaque) {
    drop(Box::from_raw(r as *mut SkiaRenderer))
}

#[no_mangle]
pub unsafe extern "C" fn slint_skia_renderer_show(r: SkiaRendererOpaque) {
    let r = &*(r as *const SkiaRenderer);
    r.show().unwrap()
}

#[no_mangle]
pub unsafe extern "C" fn slint_skia_renderer_hide(r: SkiaRendererOpaque) {
    let r = &*(r as *const SkiaRenderer);
    r.hide().unwrap()
}

#[no_mangle]
pub unsafe extern "C" fn slint_skia_renderer_resize(r: SkiaRendererOpaque, size: IntSize) {
    let r = &*(r as *const SkiaRenderer);
    r.resize_event(PhysicalSize { width: size.width, height: size.height }).unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn slint_skia_renderer_render(
    r: SkiaRendererOpaque,
    window: *const WindowAdapterRcOpaque,
) {
    let window_adapter = &*(window as *const Rc<dyn WindowAdapter>);
    let r = &*(r as *const SkiaRenderer);
    r.render(window_adapter.window()).unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn slint_skia_renderer_handle(r: SkiaRendererOpaque) -> RendererPtr {
    let r = (r as *const SkiaRenderer) as *const dyn Renderer;
    core::mem::transmute(r)
}
