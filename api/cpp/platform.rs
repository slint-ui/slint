// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use alloc::rc::Rc;
use alloc::{boxed::Box, string::String};
use core::ffi::c_void;
use i_slint_core::api::{
    LogicalSize, PhysicalPosition, PhysicalSize, Window, WindowPosition, WindowSize,
};
use i_slint_core::graphics::euclid;
use i_slint_core::graphics::IntSize;
use i_slint_core::platform::{Clipboard, Platform, PlatformError};
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
    set_size: unsafe extern "C" fn(WindowAdapterUserData, IntSize),
    update_window_properties: unsafe extern "C" fn(WindowAdapterUserData, &WindowProperties),
    position:
        unsafe extern "C" fn(WindowAdapterUserData, &mut euclid::default::Point2D<i32>) -> bool,
    set_position: unsafe extern "C" fn(WindowAdapterUserData, euclid::default::Point2D<i32>),
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

    fn position(&self) -> Option<PhysicalPosition> {
        let mut pos = euclid::default::Point2D::<i32>::default();
        if unsafe { (self.position)(self.user_data, &mut pos) } {
            Some(i_slint_core::graphics::ffi::physical_position_to_api(pos))
        } else {
            None
        }
    }

    fn set_position(&self, position: WindowPosition) {
        let physical_position = i_slint_core::graphics::ffi::physical_position_from_api(
            position.to_physical(self.window.scale_factor()),
        );
        unsafe { (self.set_position)(self.user_data, physical_position) }
    }

    fn set_size(&self, size: WindowSize) {
        let physical_size = i_slint_core::graphics::ffi::physical_size_from_api(
            size.to_physical(self.window.scale_factor()),
        );
        unsafe { (self.set_size)(self.user_data, physical_size) }
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

#[no_mangle]
pub extern "C" fn slint_window_properties_get_fullscreen(wp: &WindowProperties) -> bool {
    wp.is_fullscreen()
}

#[no_mangle]
pub extern "C" fn slint_window_properties_get_minimized(wp: &WindowProperties) -> bool {
    wp.is_minimized()
}

#[no_mangle]
pub extern "C" fn slint_window_properties_get_maximized(wp: &WindowProperties) -> bool {
    wp.is_maximized()
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
    set_size: unsafe extern "C" fn(WindowAdapterUserData, IntSize),
    update_window_properties: unsafe extern "C" fn(WindowAdapterUserData, &WindowProperties),
    position: unsafe extern "C" fn(
        WindowAdapterUserData,
        &mut euclid::default::Point2D<i32>,
    ) -> bool,
    set_position: unsafe extern "C" fn(WindowAdapterUserData, euclid::default::Point2D<i32>),
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
        set_size,
        update_window_properties,
        position,
        set_position,
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
    // silent the warning despite `Clipboard` is a `#[non_exhaustive]` enum from another crate.
    #[allow(improper_ctypes_definitions)]
    set_clipboard_text: unsafe extern "C" fn(PlatformUserData, &SharedString, Clipboard),
    #[allow(improper_ctypes_definitions)]
    clipboard_text: unsafe extern "C" fn(PlatformUserData, &mut SharedString, Clipboard) -> bool,
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

    fn set_clipboard_text(&self, text: &str, clipboard: Clipboard) {
        let shared_text = SharedString::from(text);
        unsafe { (self.set_clipboard_text)(self.user_data, &shared_text, clipboard) }
    }

    fn clipboard_text(&self, clipboard: Clipboard) -> Option<String> {
        let mut out_text = SharedString::new();
        let status = unsafe { (self.clipboard_text)(self.user_data, &mut out_text, clipboard) };
        status.then(|| out_text.into())
    }

    #[cfg(feature = "esp-println")]
    fn debug_log(&self, arguments: core::fmt::Arguments) {
        esp_println::println!("{arguments}");
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

// silent the warning depite `Clipboard` is a `#[non_exhaustive]` enum from another crate.
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub unsafe extern "C" fn slint_platform_register(
    user_data: PlatformUserData,
    drop: unsafe extern "C" fn(PlatformUserData),
    window_factory: unsafe extern "C" fn(PlatformUserData, *mut WindowAdapterRcOpaque),
    #[allow(unused)] duration_since_start: unsafe extern "C" fn(PlatformUserData) -> u64,
    set_clipboard_text: unsafe extern "C" fn(PlatformUserData, &SharedString, Clipboard),
    clipboard_text: unsafe extern "C" fn(PlatformUserData, &mut SharedString, Clipboard) -> bool,
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
        set_clipboard_text,
        clipboard_text,
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
    use i_slint_core::software_renderer::{
        PhysicalRegion, RepaintBufferType, Rgb565Pixel, SoftwareRenderer,
    };
    use i_slint_core::SharedVector;

    #[cfg(feature = "experimental")]
    use i_slint_core::software_renderer::{
        CompositionMode, PremultipliedRgbaColor, RenderingRotation, TargetPixelBuffer, Texture,
        TexturePixelFormat,
    };

    #[cfg(feature = "experimental")]
    type CppTargetPixelBufferUserData = *mut c_void;

    #[repr(C)]
    #[cfg(feature = "experimental")]
    pub struct CppInternalTexture {
        pub bytes: *const u8,
        pub bytes_len: usize,
        pub pixel_format: TexturePixelFormat,
        pub pixel_stride: u16,
        pub width: u16,
        pub height: u16,
        pub delta_x: u16,
        pub delta_y: u16,
        pub source_offset_x: u16,
        pub source_offset_y: u16,
    }

    #[repr(C)]
    #[cfg(feature = "experimental")]
    pub struct CppRgb8TargetPixelBuffer {
        user_data: CppTargetPixelBufferUserData,
        line_slice: unsafe extern "C" fn(
            CppTargetPixelBufferUserData,
            usize,
            slice_ptr: &mut *mut Rgb8Pixel,
            slice_len: *mut usize,
        ),
        num_lines: unsafe extern "C" fn(CppTargetPixelBufferUserData) -> usize,
        fill_rectangle: unsafe extern "C" fn(
            CppTargetPixelBufferUserData,
            i16,
            i16,
            i16,
            i16,
            u8,
            u8,
            u8,
            u8,
            CompositionMode,
        ) -> bool,
        draw_texture: unsafe extern "C" fn(
            CppTargetPixelBufferUserData,
            i16,
            i16,
            i16,
            i16,
            &CppInternalTexture,
            u32,
            u8,
            i32,
            CompositionMode,
        ) -> bool,
    }

    #[cfg(feature = "experimental")]
    impl TargetPixelBuffer for CppRgb8TargetPixelBuffer {
        type TargetPixel = Rgb8Pixel;

        fn line_slice(&mut self, line_number: usize) -> &mut [Self::TargetPixel] {
            unsafe {
                let mut data = core::ptr::null_mut();
                let mut len = 0;
                (self.line_slice)(self.user_data, line_number, &mut data, &mut len);
                core::slice::from_raw_parts_mut(data, len)
            }
        }

        fn num_lines(&self) -> usize {
            unsafe { (self.num_lines)(self.user_data) }
        }

        fn fill_rectangle(
            &mut self,
            x: i16,
            y: i16,
            width: i16,
            height: i16,
            color: PremultipliedRgbaColor,
            composition_mode: CompositionMode,
        ) -> bool {
            unsafe {
                (self.fill_rectangle)(
                    self.user_data,
                    x,
                    y,
                    width,
                    height,
                    color.red,
                    color.green,
                    color.blue,
                    color.alpha,
                    composition_mode,
                )
            }
        }

        fn draw_texture(
            &mut self,
            x: i16,
            y: i16,
            width: i16,
            height: i16,
            texture: Texture<'_>,
            colorize: u32,
            alpha: u8,
            rotation: RenderingRotation,
            composition_mode: CompositionMode,
        ) -> bool {
            unsafe {
                (self.draw_texture)(
                    self.user_data,
                    x,
                    y,
                    width,
                    height,
                    &CppInternalTexture {
                        bytes: texture.bytes.as_ptr(),
                        bytes_len: texture.bytes.len(),
                        pixel_format: texture.pixel_format,
                        pixel_stride: texture.pixel_stride,
                        width: texture.width,
                        height: texture.height,
                        delta_x: texture.delta_x,
                        delta_y: texture.delta_y,
                        source_offset_x: texture.source_offset_x,
                        source_offset_y: texture.source_offset_y,
                    },
                    colorize,
                    alpha,
                    rotation.angle() as i32,
                    composition_mode,
                )
            }
        }
    }

    #[cfg(feature = "experimental")]
    #[repr(C)]
    pub struct CppRgb565TargetPixelBuffer {
        user_data: CppTargetPixelBufferUserData,
        line_slice: unsafe extern "C" fn(
            CppTargetPixelBufferUserData,
            usize,
            slice_ptr: &mut *mut u16,
            slice_len: *mut usize,
        ),
        num_lines: unsafe extern "C" fn(CppTargetPixelBufferUserData) -> usize,
        fill_rectangle: unsafe extern "C" fn(
            CppTargetPixelBufferUserData,
            i16,
            i16,
            i16,
            i16,
            u8,
            u8,
            u8,
            u8,
            CompositionMode,
        ) -> bool,
        draw_texture: unsafe extern "C" fn(
            CppTargetPixelBufferUserData,
            i16,
            i16,
            i16,
            i16,
            &CppInternalTexture,
            u32,
            u8,
            i32,
            CompositionMode,
        ) -> bool,
    }

    #[cfg(feature = "experimental")]
    impl TargetPixelBuffer for CppRgb565TargetPixelBuffer {
        type TargetPixel = Rgb565Pixel;

        fn line_slice(&mut self, line_number: usize) -> &mut [Self::TargetPixel] {
            unsafe {
                let mut data = core::ptr::null_mut();
                let mut len = 0;
                (self.line_slice)(self.user_data, line_number, &mut data, &mut len);
                core::slice::from_raw_parts_mut(data as *mut Rgb565Pixel, len)
            }
        }

        fn num_lines(&self) -> usize {
            unsafe { (self.num_lines)(self.user_data) }
        }

        fn fill_rectangle(
            &mut self,
            x: i16,
            y: i16,
            width: i16,
            height: i16,
            color: PremultipliedRgbaColor,
            composition_mode: CompositionMode,
        ) -> bool {
            unsafe {
                (self.fill_rectangle)(
                    self.user_data,
                    x,
                    y,
                    width,
                    height,
                    color.red,
                    color.green,
                    color.blue,
                    color.alpha,
                    composition_mode,
                )
            }
        }

        fn draw_texture(
            &mut self,
            x: i16,
            y: i16,
            width: i16,
            height: i16,
            texture: Texture<'_>,
            colorize: u32,
            alpha: u8,
            rotation: RenderingRotation,
            composition_mode: CompositionMode,
        ) -> bool {
            unsafe {
                (self.draw_texture)(
                    self.user_data,
                    x,
                    y,
                    width,
                    height,
                    &CppInternalTexture {
                        bytes: texture.bytes.as_ptr(),
                        bytes_len: texture.bytes.len(),
                        pixel_format: texture.pixel_format,
                        pixel_stride: texture.pixel_stride,
                        width: texture.width,
                        height: texture.height,
                        delta_x: texture.delta_x,
                        delta_y: texture.delta_y,
                        source_offset_x: texture.source_offset_x,
                        source_offset_y: texture.source_offset_y,
                    },
                    colorize,
                    alpha,
                    rotation.angle() as i32,
                    composition_mode,
                )
            }
        }
    }

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
    ) -> PhysicalRegion {
        let buffer = core::slice::from_raw_parts_mut(buffer, buffer_len);
        let renderer = &*(r as *const SoftwareRenderer);
        renderer.render(buffer, pixel_stride)
    }

    #[cfg(feature = "experimental")]
    #[no_mangle]
    pub unsafe extern "C" fn slint_software_renderer_render_accel_rgb8(
        r: SoftwareRendererOpaque,
        buffer: *mut CppRgb8TargetPixelBuffer,
    ) -> PhysicalRegion {
        let renderer = &*(r as *const SoftwareRenderer);
        unsafe { renderer.render_into_buffer(&mut *buffer) }
    }

    #[cfg(feature = "experimental")]
    #[no_mangle]
    pub unsafe extern "C" fn slint_software_renderer_render_accel_rgb565(
        r: SoftwareRendererOpaque,
        buffer: *mut CppRgb565TargetPixelBuffer,
    ) -> PhysicalRegion {
        let renderer = &*(r as *const SoftwareRenderer);
        unsafe { renderer.render_into_buffer(&mut *buffer) }
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_software_renderer_render_rgb565(
        r: SoftwareRendererOpaque,
        buffer: *mut u16,
        buffer_len: usize,
        pixel_stride: usize,
    ) -> PhysicalRegion {
        let buffer = core::slice::from_raw_parts_mut(buffer as *mut Rgb565Pixel, buffer_len);
        let renderer = &*(r as *const SoftwareRenderer);
        renderer.render(buffer, pixel_stride)
    }

    struct LineByLineProcessor<TargetPixel> {
        process_line_fn: extern "C" fn(
            *mut core::ffi::c_void,
            usize,
            usize,
            usize,
            extern "C" fn(*const core::ffi::c_void, *mut TargetPixel, usize),
            *const core::ffi::c_void,
        ),
        user_data: *mut core::ffi::c_void,
    }

    impl<TargetPixel: i_slint_core::software_renderer::TargetPixel>
        i_slint_core::software_renderer::LineBufferProvider for LineByLineProcessor<TargetPixel>
    {
        type TargetPixel = TargetPixel;
        fn process_line(
            &mut self,
            line: usize,
            range: core::ops::Range<usize>,
            render_fn: impl FnOnce(&mut [TargetPixel]),
        ) {
            self.cpp_process_line(line, range, render_fn);
        }
    }

    impl<TargetPixel> LineByLineProcessor<TargetPixel> {
        fn cpp_process_line<RenderFn: FnOnce(&mut [TargetPixel])>(
            &mut self,
            line: usize,
            range: core::ops::Range<usize>,
            render_fn: RenderFn,
        ) {
            let mut render_fn = Some(render_fn);
            let render_fn_ptr = &mut render_fn as *mut Option<RenderFn> as *const core::ffi::c_void;

            extern "C" fn cpp_render_line_callback<
                TargetPixel,
                RenderFn: FnOnce(&mut [TargetPixel]),
            >(
                render_fn_ptr: *const core::ffi::c_void,
                line_start: *mut TargetPixel,
                len: usize,
            ) {
                let line_slice = unsafe { core::slice::from_raw_parts_mut(line_start, len) };
                let render_fn =
                    unsafe { (*(render_fn_ptr as *mut Option<RenderFn>)).take().unwrap() };
                render_fn(line_slice);
            }

            (self.process_line_fn)(
                self.user_data,
                line,
                range.start,
                range.end,
                cpp_render_line_callback::<TargetPixel, RenderFn>,
                render_fn_ptr,
            );
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_software_renderer_render_by_line_rgb565(
        r: SoftwareRendererOpaque,
        process_line_fn: extern "C" fn(
            *mut core::ffi::c_void,
            usize,
            usize,
            usize,
            extern "C" fn(*const core::ffi::c_void, *mut Rgb565Pixel, usize),
            *const core::ffi::c_void,
        ),
        user_data: *mut core::ffi::c_void,
    ) -> PhysicalRegion {
        let renderer = &*(r as *const SoftwareRenderer);
        let processor = LineByLineProcessor { process_line_fn, user_data };
        renderer.render_by_line(processor)
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_software_renderer_render_by_line_rgb8(
        r: SoftwareRendererOpaque,
        process_line_fn: extern "C" fn(
            *mut core::ffi::c_void,
            usize,
            usize,
            usize,
            extern "C" fn(*const core::ffi::c_void, *mut Rgb8Pixel, usize),
            *const core::ffi::c_void,
        ),
        user_data: *mut core::ffi::c_void,
    ) -> PhysicalRegion {
        let renderer = &*(r as *const SoftwareRenderer);
        let processor = LineByLineProcessor { process_line_fn, user_data };
        renderer.render_by_line(processor)
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_software_renderer_set_rendering_rotation(
        r: SoftwareRendererOpaque,
        rotation: i32,
    ) {
        use i_slint_core::software_renderer::RenderingRotation;
        let renderer = &*(r as *const SoftwareRenderer);
        renderer.set_rendering_rotation(match rotation {
            90 => RenderingRotation::Rotate90,
            180 => RenderingRotation::Rotate180,
            270 => RenderingRotation::Rotate270,
            _ => RenderingRotation::NoRotation,
        });
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_software_renderer_handle(
        r: SoftwareRendererOpaque,
    ) -> RendererPtr {
        let r = (r as *const SoftwareRenderer) as *const dyn Renderer;
        core::mem::transmute(r)
    }

    #[no_mangle]
    pub extern "C" fn slint_software_renderer_region_to_rects(
        region: &PhysicalRegion,
        out: &mut SharedVector<IntRect>,
    ) {
        *out = region
            .iter()
            .map(|r| euclid::rect(r.0.x, r.0.y, r.1.width as i32, r.1.height as i32))
            .collect();
    }
}

#[cfg(all(feature = "i-slint-renderer-skia", feature = "raw-window-handle"))]
pub mod skia {
    use super::*;
    use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
    use std::sync::Arc;

    struct RawHandlePair((RawWindowHandle, RawDisplayHandle));

    impl raw_window_handle::HasDisplayHandle for RawHandlePair {
        fn display_handle(
            &self,
        ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
            // Safety: It is assumed that the C++ side keeps the window/display handles alive.
            Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(self.0 .1) })
        }
    }

    impl raw_window_handle::HasWindowHandle for RawHandlePair {
        fn window_handle(
            &self,
        ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
            // Safety: It is assumed that the C++ side keeps the window/display handles alive.
            Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(self.0 .0) })
        }
    }

    struct CppRawHandle(Arc<RawHandlePair>);

    impl From<(RawWindowHandle, RawDisplayHandle)> for CppRawHandle {
        fn from(pair: (RawWindowHandle, RawDisplayHandle)) -> Self {
            Self(Arc::new(RawHandlePair(pair)))
        }
    }

    type CppRawHandleOpaque = *const c_void;

    #[no_mangle]
    pub unsafe extern "C" fn slint_new_raw_window_handle_win32(
        hwnd: *mut c_void,
        _hinstance: *mut c_void,
    ) -> CppRawHandleOpaque {
        let handle = CppRawHandle::from((
            RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle::new(
                (hwnd as isize).try_into().expect("C++: NativeWindowHandle created with null hwnd"),
            )),
            RawDisplayHandle::Windows(raw_window_handle::WindowsDisplayHandle::new()),
        ));
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
        let handle = CppRawHandle::from((
            RawWindowHandle::Xcb({
                let mut hnd = XcbWindowHandle::new(
                    window
                        .try_into()
                        .expect("C++: NativeWindowHandle created with null xcb window handle"),
                );
                hnd.visual_id = visual_id.try_into().ok();
                hnd
            }),
            RawDisplayHandle::Xcb(XcbDisplayHandle::new(
                Some(core::ptr::NonNull::new_unchecked(connection)),
                screen,
            )),
        ));
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
        let handle = CppRawHandle::from((
            RawWindowHandle::Xlib({
                let mut hnd = XlibWindowHandle::new(window);
                hnd.visual_id = visual_id;
                hnd
            }),
            RawDisplayHandle::Xlib(XlibDisplayHandle::new(
                Some(core::ptr::NonNull::new_unchecked(display)),
                screen,
            )),
        ));
        Box::into_raw(Box::new(handle)) as CppRawHandleOpaque
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_new_raw_window_handle_wayland(
        surface: *mut c_void,
        display: *mut c_void,
    ) -> CppRawHandleOpaque {
        use raw_window_handle::{WaylandDisplayHandle, WaylandWindowHandle};
        let handle = CppRawHandle::from((
            RawWindowHandle::Wayland(WaylandWindowHandle::new(core::ptr::NonNull::new_unchecked(
                surface,
            ))),
            RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
                core::ptr::NonNull::new_unchecked(display),
            )),
        ));
        Box::into_raw(Box::new(handle)) as CppRawHandleOpaque
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_new_raw_window_handle_appkit(
        ns_view: *mut c_void,
        _ns_window: *mut c_void,
    ) -> CppRawHandleOpaque {
        use raw_window_handle::{AppKitDisplayHandle, AppKitWindowHandle};
        let handle = CppRawHandle::from((
            RawWindowHandle::AppKit(AppKitWindowHandle::new(core::ptr::NonNull::new_unchecked(
                ns_view,
            ))),
            RawDisplayHandle::AppKit(AppKitDisplayHandle::new()),
        ));
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

        let boxed_renderer: Box<SkiaRenderer> = Box::new(
            SkiaRenderer::new(
                handle.0.clone(),
                handle.0.clone(),
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
