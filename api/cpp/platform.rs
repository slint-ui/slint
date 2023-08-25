// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::ffi::c_void;
use i_slint_core::api::{LogicalSize, PhysicalSize, Window};
use i_slint_core::graphics::IntSize;
use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::ffi::WindowAdapterRcOpaque;
use i_slint_core::window::{WindowAdapter, WindowProperties};
use i_slint_core::SharedString;

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
    set_visible: unsafe extern "C" fn(WindowAdapterUserData, bool),
    request_redraw: unsafe extern "C" fn(WindowAdapterUserData),
    size: unsafe extern "C" fn(WindowAdapterUserData) -> IntSize,
    update_window_properties: unsafe extern "C" fn(WindowAdapterUserData, &WindowProperties),
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

    fn set_visible(&self, visible: bool) -> Result<(), PlatformError> {
        unsafe { (self.set_visible)(self.user_data, visible) };
        Ok(())
    }

    fn size(&self) -> PhysicalSize {
        let s = unsafe { (self.size)(self.user_data) };
        PhysicalSize::new(s.width, s.height)
    }

    fn renderer(&self) -> &dyn Renderer {
        unsafe { core::mem::transmute((self.get_renderer_ref)(self.user_data)) }
    }

    fn request_redraw(&self) {
        unsafe { (self.request_redraw)(self.user_data) }
    }

    fn update_window_properties(&self, properties: WindowProperties<'_>) {
        unsafe { (self.update_window_properties)(self.user_data, &properties) }
    }
}

#[no_mangle]
pub extern "C" fn slint_window_properties_get_title(wp: &WindowProperties, out: &mut SharedString) {
    *out = wp.title();
}

#[no_mangle]
pub extern "C" fn slint_window_properties_get_background(
    wp: &WindowProperties,
    out: &mut i_slint_core::Brush,
) {
    *out = wp.background();
}

#[repr(C)]
#[derive(Clone, Copy)]
/// a Repr(C) variant of slint::platform::LayoutConstraints
pub struct LayoutConstraintsReprC {
    pub min: i_slint_core::graphics::Size,
    pub max: i_slint_core::graphics::Size,
    pub preferred: i_slint_core::graphics::Size,
    pub has_min: bool,
    pub has_max: bool,
}

#[no_mangle]
pub extern "C" fn slint_window_properties_get_layout_constraints(
    wp: &WindowProperties,
) -> LayoutConstraintsReprC {
    let c = wp.layout_constraints();
    LayoutConstraintsReprC {
        min: i_slint_core::lengths::logical_size_from_api(c.min.unwrap_or_default()).to_untyped(),
        max: i_slint_core::lengths::logical_size_from_api(
            c.max.unwrap_or(LogicalSize { width: f32::MAX, height: f32::MAX }),
        )
        .to_untyped(),
        preferred: i_slint_core::lengths::logical_size_from_api(c.preferred).to_untyped(),
        has_min: c.min.is_some(),
        has_max: c.max.is_some(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn slint_window_adapter_new(
    user_data: WindowAdapterUserData,
    drop: unsafe extern "C" fn(WindowAdapterUserData),
    get_renderer_ref: unsafe extern "C" fn(WindowAdapterUserData) -> RendererPtr,
    set_visible: unsafe extern "C" fn(WindowAdapterUserData, bool),
    request_redraw: unsafe extern "C" fn(WindowAdapterUserData),
    size: unsafe extern "C" fn(WindowAdapterUserData) -> IntSize,
    update_window_properties: unsafe extern "C" fn(WindowAdapterUserData, &WindowProperties),
    target: *mut WindowAdapterRcOpaque,
) {
    let window = Rc::<CppWindowAdapter>::new_cyclic(|w| CppWindowAdapter {
        window: Window::new(w.clone()),
        user_data,
        drop,
        get_renderer_ref,
        set_visible,
        request_redraw,
        size,
        update_window_properties,
    });

    core::ptr::write(target as *mut Rc<dyn WindowAdapter>, window);
}

type PlatformUserData = *mut c_void;

struct CppPlatform {
    user_data: PlatformUserData,
    drop: unsafe extern "C" fn(PlatformUserData),
    window_factory: unsafe extern "C" fn(PlatformUserData, *mut WindowAdapterRcOpaque),
    #[cfg(not(feature = "std"))]
    duration_since_start: unsafe extern "C" fn(PlatformUserData) -> u64,
    run_event_loop: unsafe extern "C" fn(PlatformUserData),
    quit_event_loop: unsafe extern "C" fn(PlatformUserData),
    invoke_from_event_loop: unsafe extern "C" fn(PlatformUserData, PlatformTaskOpaque),
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

    #[cfg(not(feature = "std"))]
    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(unsafe { (self.duration_since_start)(self.user_data) })
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        unsafe { (self.run_event_loop)(self.user_data) };
        Ok(())
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn i_slint_core::platform::EventLoopProxy>> {
        Some(Box::new(CppEventLoopProxy {
            user_data: self.user_data,
            quit_event_loop: self.quit_event_loop,
            invoke_from_event_loop: self.invoke_from_event_loop,
        }))
    }
}

struct CppEventLoopProxy {
    user_data: PlatformUserData,
    quit_event_loop: unsafe extern "C" fn(PlatformUserData),
    invoke_from_event_loop: unsafe extern "C" fn(PlatformUserData, PlatformTaskOpaque),
}

impl i_slint_core::platform::EventLoopProxy for CppEventLoopProxy {
    fn quit_event_loop(&self) -> Result<(), i_slint_core::api::EventLoopError> {
        unsafe { (self.quit_event_loop)(self.user_data) };
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), i_slint_core::api::EventLoopError> {
        unsafe {
            (self.invoke_from_event_loop)(
                self.user_data,
                core::mem::transmute::<*mut dyn FnOnce(), PlatformTaskOpaque>(Box::into_raw(event)),
            )
        };
        Ok(())
    }
}

unsafe impl Send for CppEventLoopProxy {}
unsafe impl Sync for CppEventLoopProxy {}

#[no_mangle]
pub unsafe extern "C" fn slint_platform_register(
    user_data: PlatformUserData,
    drop: unsafe extern "C" fn(PlatformUserData),
    window_factory: unsafe extern "C" fn(PlatformUserData, *mut WindowAdapterRcOpaque),
    #[allow(unused)] duration_since_start: unsafe extern "C" fn(PlatformUserData) -> u64,
    run_event_loop: unsafe extern "C" fn(PlatformUserData),
    quit_event_loop: unsafe extern "C" fn(PlatformUserData),
    invoke_from_event_loop: unsafe extern "C" fn(PlatformUserData, PlatformTaskOpaque),
) {
    let p = CppPlatform {
        user_data,
        drop,
        window_factory,
        #[cfg(not(feature = "std"))]
        duration_since_start,
        run_event_loop,
        quit_event_loop,
        invoke_from_event_loop,
    };
    i_slint_core::platform::set_platform(Box::new(p)).unwrap();
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

/// Returns the duration in millisecond until the next timer or `u64::MAX` if there is no pending timers
#[no_mangle]
pub extern "C" fn slint_platform_duration_until_next_timer_update() -> u64 {
    i_slint_core::platform::duration_until_next_timer_update()
        .map_or(u64::MAX, |d| d.as_millis() as u64)
}

#[repr(C)]
pub struct PlatformTaskOpaque(*const c_void, *const c_void);

#[no_mangle]
pub unsafe extern "C" fn slint_platform_task_drop(event: PlatformTaskOpaque) {
    drop(Box::from_raw(core::mem::transmute::<PlatformTaskOpaque, *mut dyn FnOnce()>(event)));
}

#[no_mangle]
pub unsafe extern "C" fn slint_platform_task_run(event: PlatformTaskOpaque) {
    let f = Box::from_raw(core::mem::transmute::<PlatformTaskOpaque, *mut dyn FnOnce()>(event));
    f();
}

#[cfg(feature = "renderer-software")]
mod software_renderer {
    use super::*;
    type SoftwareRendererOpaque = *const c_void;
    use i_slint_core::graphics::{IntRect, Rgb8Pixel};
    use i_slint_core::software_renderer::{RepaintBufferType, Rgb565Pixel, SoftwareRenderer};

    #[no_mangle]
    pub unsafe extern "C" fn slint_software_renderer_new(
        buffer_age: u32,
    ) -> SoftwareRendererOpaque {
        let repaint_buffer_type = match buffer_age {
            0 => RepaintBufferType::NewBuffer,
            1 => RepaintBufferType::ReusedBuffer,
            2 => RepaintBufferType::SwappedBuffers,
            _ => unreachable!(),
        };
        Box::into_raw(Box::new(SoftwareRenderer::new_with_repaint_buffer_type(repaint_buffer_type)))
            as SoftwareRendererOpaque
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_software_renderer_drop(r: SoftwareRendererOpaque) {
        drop(Box::from_raw(r as *mut SoftwareRenderer));
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_software_renderer_render_rgb8(
        r: SoftwareRendererOpaque,
        buffer: *mut Rgb8Pixel,
        buffer_len: usize,
        pixel_stride: usize,
    ) -> IntRect {
        let buffer = core::slice::from_raw_parts_mut(buffer, buffer_len);
        let renderer = &*(r as *const SoftwareRenderer);
        let r = renderer.render(buffer, pixel_stride);
        let (orig, size) = (r.bounding_box_origin(), r.bounding_box_size());
        i_slint_core::graphics::euclid::rect(orig.x, orig.y, size.width as i32, size.height as i32)
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_software_renderer_render_rgb565(
        r: SoftwareRendererOpaque,
        buffer: *mut u16,
        buffer_len: usize,
        pixel_stride: usize,
    ) -> IntRect {
        let buffer = core::slice::from_raw_parts_mut(buffer as *mut Rgb565Pixel, buffer_len);
        let renderer = &*(r as *const SoftwareRenderer);
        let r = renderer.render(buffer, pixel_stride);
        let (orig, size) = (r.bounding_box_origin(), r.bounding_box_size());
        i_slint_core::graphics::euclid::rect(orig.x, orig.y, size.width as i32, size.height as i32)
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_software_renderer_handle(
        r: SoftwareRendererOpaque,
    ) -> RendererPtr {
        let r = (r as *const SoftwareRenderer) as *const dyn Renderer;
        core::mem::transmute(r)
    }
}

#[cfg(all(feature = "i-slint-renderer-skia", feature = "raw-window-handle"))]
pub mod skia {
    use super::*;
    use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

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
            RawWindowHandle::Win32(init_raw!(raw_window_handle::Win32WindowHandle {
                hwnd,
                hinstance
            })),
            RawDisplayHandle::Windows(raw_window_handle::WindowsDisplayHandle::empty()),
        );
        Box::into_raw(Box::new(handle)) as CppRawHandleOpaque
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_new_raw_window_handle_x11_xcb(
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
    pub unsafe extern "C" fn slint_new_raw_window_handle_x11_xlib(
        window: core::ffi::c_ulong,
        visual_id: core::ffi::c_ulong,
        display: *mut c_void,
        screen: core::ffi::c_int,
    ) -> CppRawHandleOpaque {
        use raw_window_handle::{XlibDisplayHandle, XlibWindowHandle};
        let handle = CppRawHandle(
            RawWindowHandle::Xlib(init_raw!(XlibWindowHandle { window, visual_id })),
            RawDisplayHandle::Xlib(init_raw!(XlibDisplayHandle { display, screen })),
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

    type SkiaRendererOpaque = *const c_void;
    type SkiaRenderer = i_slint_renderer_skia::SkiaRenderer;

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
    pub unsafe extern "C" fn slint_skia_renderer_render(r: SkiaRendererOpaque) {
        let r = &*(r as *const SkiaRenderer);
        r.render().unwrap();
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_skia_renderer_handle(r: SkiaRendererOpaque) -> RendererPtr {
        let r = (r as *const SkiaRenderer) as *const dyn Renderer;
        core::mem::transmute(r)
    }
}
