// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::pin::Pin;
use std::rc::Rc;

use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::RendererSealed;
use i_slint_core::window::WindowAdapter;

use wgpu_28 as wgpu;

use crate::wgpu_28_surface::{Backend, WGPUSurface};
use crate::{SkiaRenderer, SkiaSharedContext};

/// Use the Skia renderer with WGPU when implementing a custom Slint platform where you want the
/// scene to be rendered into a WGPU texture. The rendering is done using the
/// [Skia](https://skia.org/) library with platform-native GPU acceleration.
///
/// This is the Skia equivalent of `FemtoVGWGPURenderer`, offering superior font rendering
/// quality through platform-native text rasterizers.
///
/// Rendering notifier callbacks registered via
/// [`Window::set_rendering_notifier()`](i_slint_core::api::Window::set_rendering_notifier)
/// will receive [`GraphicsAPI::WGPU28`](i_slint_core::api::GraphicsAPI::WGPU28) with the
/// renderer's instance, device, and queue.
pub struct SkiaWGPURenderer {
    renderer: SkiaRenderer,
    surface: WGPUSurface,
}

impl SkiaWGPURenderer {
    /// Creates a new SkiaWGPURenderer.
    ///
    /// The `instance`, `adapter`, `device` and `queue` are the WGPU resources used for rendering.
    /// The `adapter` is needed to determine the GPU backend and create the Skia graphics context.
    ///
    /// The wgpu resources are also provided to rendering notifier callbacks via
    /// [`GraphicsAPI::WGPU28`](i_slint_core::api::GraphicsAPI::WGPU28).
    pub fn new(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Result<Self, PlatformError> {
        let backend: Backend = adapter.get_info().backend.try_into()?;

        let gr_context = backend.make_context(&adapter, &device, &queue).ok_or_else(|| {
            PlatformError::from("Failed to create Skia graphics context from WGPU")
        })?;

        let surface = WGPUSurface::new_offscreen(instance, device, queue, backend, gr_context);

        let shared_context = SkiaSharedContext::default();
        // Use SkiaRenderer::default() to stay resilient to field additions, then disable
        // partial rendering — there is no buffer age tracking for external texture targets.
        let mut renderer = SkiaRenderer::default(&shared_context);
        renderer.partial_rendering_state = None;

        Ok(Self { renderer, surface })
    }

    /// Render the scene to the given texture.
    ///
    /// The texture must have been created with `RENDER_ATTACHMENT` usage and have a supported
    /// format. Supported formats depend on the GPU backend: `Rgba8Unorm` and `Rgba8UnormSrgb`
    /// are supported on all backends; `Bgra8Unorm` is additionally supported on Metal and Vulkan.
    pub fn render_to_texture(&self, texture: &wgpu::Texture) -> Result<(), PlatformError> {
        self.renderer.invoke_rendering_notifier_setup(&self.surface)?;

        let gr_context = &mut self.surface.gr_context.borrow_mut();

        let mut skia_surface =
            self.surface.backend.make_surface(gr_context, texture).ok_or_else(|| {
                PlatformError::from("Failed to wrap WGPU texture as Skia render target")
            })?;

        let window_adapter = self.renderer.window_adapter()?;
        let window = window_adapter.window();

        self.renderer.render_to_canvas(
            skia_surface.canvas(),
            0.,
            (0., 0.),
            Some(gr_context),
            0,
            Some(&self.surface),
            window,
            None,
        );

        self.surface.flush_and_submit(gr_context);

        Ok(())
    }
}

#[doc(hidden)]
impl RendererSealed for SkiaWGPURenderer {
    fn text_size(
        &self,
        text_item: Pin<&dyn i_slint_core::item_rendering::RenderString>,
        item_rc: &i_slint_core::items::ItemRc,
        max_width: Option<i_slint_core::lengths::LogicalLength>,
        text_wrap: i_slint_core::items::TextWrap,
    ) -> i_slint_core::lengths::LogicalSize {
        self.renderer.text_size(text_item, item_rc, max_width, text_wrap)
    }

    fn char_size(
        &self,
        text_item: Pin<&dyn i_slint_core::item_rendering::HasFont>,
        item_rc: &i_slint_core::items::ItemRc,
        ch: char,
    ) -> i_slint_core::lengths::LogicalSize {
        self.renderer.char_size(text_item, item_rc, ch)
    }

    fn font_metrics(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
    ) -> i_slint_core::items::FontMetrics {
        self.renderer.font_metrics(font_request)
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        item_rc: &i_slint_core::items::ItemRc,
        pos: i_slint_core::lengths::LogicalPoint,
    ) -> usize {
        self.renderer.text_input_byte_offset_for_position(text_input, item_rc, pos)
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        item_rc: &i_slint_core::items::ItemRc,
        byte_offset: usize,
    ) -> i_slint_core::lengths::LogicalRect {
        self.renderer.text_input_cursor_rect_for_byte_offset(text_input, item_rc, byte_offset)
    }

    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.renderer.register_font_from_memory(data)
    }

    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.renderer.register_font_from_path(path)
    }

    fn default_font_size(&self) -> i_slint_core::lengths::LogicalLength {
        self.renderer.default_font_size()
    }

    fn set_rendering_notifier(
        &self,
        callback: Box<dyn i_slint_core::api::RenderingNotifier>,
    ) -> Result<(), i_slint_core::api::SetRenderingNotifierError> {
        self.renderer.set_rendering_notifier(callback)
    }

    fn free_graphics_resources(
        &self,
        component: i_slint_core::item_tree::ItemTreeRef,
        items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'_>>>,
    ) -> Result<(), PlatformError> {
        self.renderer.free_graphics_resources(component, items)
    }

    fn set_window_adapter(&self, window_adapter: &Rc<dyn WindowAdapter>) {
        self.renderer.set_window_adapter(window_adapter)
    }

    fn window_adapter(&self) -> Option<Rc<dyn WindowAdapter>> {
        RendererSealed::window_adapter(&self.renderer)
    }

    fn resize(&self, size: i_slint_core::api::PhysicalSize) -> Result<(), PlatformError> {
        self.renderer.resize(size)
    }

    fn take_snapshot(
        &self,
    ) -> Result<
        i_slint_core::graphics::SharedPixelBuffer<i_slint_core::graphics::Rgba8Pixel>,
        PlatformError,
    > {
        self.renderer.take_snapshot()
    }

    fn supports_transformations(&self) -> bool {
        self.renderer.supports_transformations()
    }
}
