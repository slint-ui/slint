// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{cell::RefCell, pin::Pin, rc::Rc};

use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::RendererSealed;
use i_slint_core::{api::PhysicalSize as PhysicalWindowSize, graphics::RequestedGraphicsAPI};

use crate::{FemtoVGRenderer, GraphicsBackend, WindowSurface, wgpu::wgpu::Texture};

use wgpu_28 as wgpu;

pub struct WGPUBackend {
    instance: RefCell<Option<wgpu::Instance>>,
    device: RefCell<Option<wgpu::Device>>,
    queue: RefCell<Option<wgpu::Queue>>,
    surface_config: RefCell<Option<wgpu::SurfaceConfiguration>>,
    surface: RefCell<Option<wgpu::Surface<'static>>>,
}

pub struct WGPUWindowSurface {
    surface_texture: wgpu::SurfaceTexture,
}

impl WindowSurface<femtovg::renderer::WGPURenderer> for WGPUWindowSurface {
    fn render_surface(&self) -> &Texture {
        &self.surface_texture.texture
    }
}

impl GraphicsBackend for WGPUBackend {
    type Renderer = femtovg::renderer::WGPURenderer;
    type WindowSurface = WGPUWindowSurface;
    const NAME: &'static str = "WGPU";

    fn new_suspended() -> Self {
        Self {
            instance: Default::default(),
            device: Default::default(),
            queue: Default::default(),
            surface_config: Default::default(),
            surface: Default::default(),
        }
    }

    fn clear_graphics_context(&self) {
        self.surface_config.borrow_mut().take();
        self.surface.borrow_mut().take();
        self.queue.borrow_mut().take();
        self.device.borrow_mut().take();
    }

    fn begin_surface_rendering(
        &self,
    ) -> Result<Self::WindowSurface, Box<dyn std::error::Error + Send + Sync>> {
        let surface = self.surface.borrow();
        let surface = surface.as_ref().unwrap();
        let frame = match surface.get_current_texture() {
            Ok(texture) => texture,
            Err(wgpu::SurfaceError::Timeout) => surface.get_current_texture()?,
            // Outdated or lost: re-configure and try again
            Err(_) => {
                let mut device = self.device.borrow_mut();
                let device = device.as_mut().unwrap();

                surface.configure(device, self.surface_config.borrow().as_ref().unwrap());
                surface.get_current_texture()?
            }
        };
        Ok(WGPUWindowSurface { surface_texture: frame })
    }

    fn submit_commands(&self, commands: <Self::Renderer as femtovg::Renderer>::CommandBuffer) {
        self.queue.borrow().as_ref().unwrap().submit(Some(commands));
    }

    fn present_surface(
        &self,
        surface: Self::WindowSurface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        surface.surface_texture.present();
        Ok(())
    }

    #[cfg(feature = "unstable-wgpu-28")]
    fn with_graphics_api<R>(
        &self,
        callback: impl FnOnce(Option<i_slint_core::api::GraphicsAPI<'_>>) -> R,
    ) -> Result<R, i_slint_core::platform::PlatformError> {
        let instance = self.instance.borrow().clone();
        let device = self.device.borrow().clone();
        let queue = self.queue.borrow().clone();
        if let (Some(instance), Some(device), Some(queue)) = (instance, device, queue) {
            Ok(callback(Some(i_slint_core::graphics::create_graphics_api_wgpu_28(
                instance, device, queue,
            ))))
        } else {
            Ok(callback(None))
        }
    }

    #[cfg(not(feature = "unstable-wgpu-28"))]
    fn with_graphics_api<R>(
        &self,
        callback: impl FnOnce(Option<i_slint_core::api::GraphicsAPI<'_>>) -> R,
    ) -> Result<R, i_slint_core::platform::PlatformError> {
        Ok(callback(None))
    }

    fn resize(
        &self,
        width: std::num::NonZeroU32,
        height: std::num::NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Try to get hold of the wgpu types, but if we receive the resize event while suspended, ignore it.
        let mut surface_config = self.surface_config.borrow_mut();
        let Some(surface_config) = surface_config.as_mut() else { return Ok(()) };
        let mut device = self.device.borrow_mut();
        let Some(device) = device.as_mut() else { return Ok(()) };
        let mut surface = self.surface.borrow_mut();
        let Some(surface) = surface.as_mut() else { return Ok(()) };

        // Prefer FIFO modes over possible Mailbox setting for frame pacing and better energy efficiency.
        surface_config.present_mode = wgpu::PresentMode::AutoVsync;
        surface_config.width = width.get();
        surface_config.height = height.get();

        surface.configure(device, surface_config);
        Ok(())
    }
}

impl FemtoVGRenderer<WGPUBackend> {
    pub fn set_window_handle(
        &self,
        window_handle: Box<dyn wgpu::WindowHandle>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (instance, adapter, device, queue, surface) =
            i_slint_core::graphics::wgpu_28::init_instance_adapter_device_queue_surface(
                window_handle,
                requested_graphics_api,
                /* rendering artifacts :( */
                wgpu::Backends::GL,
            )?;

        let mut surface_config =
            surface.get_default_config(&adapter, size.width, size.height).unwrap();

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities
            .formats
            .iter()
            .find(|f| {
                matches!(f, wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm)
            })
            .copied()
            .unwrap_or_else(|| swapchain_capabilities.formats[0]);
        surface_config.format = swapchain_format;
        surface.configure(&device, &surface_config);

        *self.graphics_backend.instance.borrow_mut() = Some(instance.clone());
        *self.graphics_backend.device.borrow_mut() = Some(device.clone());
        *self.graphics_backend.queue.borrow_mut() = Some(queue.clone());
        *self.graphics_backend.surface_config.borrow_mut() = Some(surface_config);
        *self.graphics_backend.surface.borrow_mut() = Some(surface);

        let wgpu_renderer = femtovg::renderer::WGPURenderer::new(device, queue);
        let femtovg_canvas = femtovg::Canvas::new_with_text_context(
            wgpu_renderer,
            crate::font_cache::FONT_CACHE.with(|cache| cache.borrow().text_context.clone()),
        )
        .unwrap();

        let canvas = Rc::new(RefCell::new(femtovg_canvas));
        self.reset_canvas(canvas);
        Ok(())
    }
}

struct TextureWindowSurface {
    texture: wgpu::Texture,
}

impl WindowSurface<femtovg::renderer::WGPURenderer> for TextureWindowSurface {
    fn render_surface(&self) -> &wgpu::Texture {
        &self.texture
    }
}

struct WgpuTextureBackend {
    queue: wgpu::Queue,
    current_texture: RefCell<Option<wgpu::Texture>>,
}

impl GraphicsBackend for WgpuTextureBackend {
    type Renderer = femtovg::renderer::WGPURenderer;
    type WindowSurface = TextureWindowSurface;
    const NAME: &'static str = "WGPU Texture";

    fn new_suspended() -> Self {
        panic!("Suspended backend not supported for WgpuTextureBackend (requires device/queue)");
    }

    fn clear_graphics_context(&self) {
        // Nothing to clear here, we don't own the device/queue/texture
    }

    fn begin_surface_rendering(
        &self,
    ) -> Result<Self::WindowSurface, Box<dyn std::error::Error + Send + Sync>> {
        let texture =
            self.current_texture.borrow().clone().ok_or("No texture set for rendering")?;
        Ok(TextureWindowSurface { texture })
    }

    fn submit_commands(&self, commands: <Self::Renderer as femtovg::Renderer>::CommandBuffer) {
        self.queue.submit(Some(commands));
    }

    fn present_surface(
        &self,
        _surface: Self::WindowSurface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // No presentation needed - the caller owns the texture and handles presenting it
        Ok(())
    }

    fn with_graphics_api<R>(
        &self,
        callback: impl FnOnce(Option<i_slint_core::api::GraphicsAPI<'_>>) -> R,
    ) -> Result<R, i_slint_core::platform::PlatformError> {
        // Users of FemtoVGWGPURenderer already have direct access to the device/queue
        Ok(callback(None))
    }

    fn resize(
        &self,
        _width: std::num::NonZeroU32,
        _height: std::num::NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // No resize needed - texture size is determined by the texture passed to render_to_texture
        Ok(())
    }
}

/// Use the FemtoVG renderer with WGPU when implementing a custom Slint platform where you want the scene to be rendered
/// into a WGPU texture. The rendering is done using the [FemtoVG](https://github.com/femtovg/femtovg) library.
pub struct FemtoVGWGPURenderer(FemtoVGRenderer<WgpuTextureBackend>);

impl FemtoVGWGPURenderer {
    /// Creates a new FemtoVGWGPURenderer.
    ///
    /// The `device` and `queue` are the WGPU device and queue that will be used for rendering.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Result<Self, PlatformError> {
        let backend =
            WgpuTextureBackend { queue: queue.clone(), current_texture: RefCell::new(None) };
        let renderer = FemtoVGRenderer::new_internal(backend);

        let wgpu_renderer = femtovg::renderer::WGPURenderer::new(device, queue);
        let femtovg_canvas = femtovg::Canvas::new_with_text_context(
            wgpu_renderer,
            crate::font_cache::FONT_CACHE.with(|cache| cache.borrow().text_context.clone()),
        )
        .map_err(|e| format!("Failed to create femtovg canvas: {:?}", e))?;

        let canvas = Rc::new(RefCell::new(femtovg_canvas));
        renderer.reset_canvas(canvas);
        Ok(Self(renderer))
    }

    /// Render the scene to the given texture.
    ///
    /// The texture must be a valid WGPU texture.
    pub fn render_to_texture(&self, texture: &wgpu::Texture) -> Result<(), PlatformError> {
        *self.0.graphics_backend.current_texture.borrow_mut() = Some(texture.clone());
        let result = self.0.render();
        *self.0.graphics_backend.current_texture.borrow_mut() = None;
        result
    }
}

impl RendererSealed for FemtoVGWGPURenderer {
    fn text_size(
        &self,
        text_item: Pin<&dyn i_slint_core::item_rendering::RenderString>,
        item_rc: &i_slint_core::items::ItemRc,
        max_width: Option<i_slint_core::lengths::LogicalLength>,
        text_wrap: i_slint_core::items::TextWrap,
    ) -> i_slint_core::lengths::LogicalSize {
        self.0.text_size(text_item, item_rc, max_width, text_wrap)
    }

    fn char_size(
        &self,
        text_item: Pin<&dyn i_slint_core::item_rendering::HasFont>,
        item_rc: &i_slint_core::items::ItemRc,
        ch: char,
    ) -> i_slint_core::lengths::LogicalSize {
        self.0.char_size(text_item, item_rc, ch)
    }

    fn font_metrics(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
    ) -> i_slint_core::items::FontMetrics {
        self.0.font_metrics(font_request)
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        item_rc: &i_slint_core::items::ItemRc,
        pos: i_slint_core::lengths::LogicalPoint,
    ) -> usize {
        self.0.text_input_byte_offset_for_position(text_input, item_rc, pos)
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        item_rc: &i_slint_core::items::ItemRc,
        byte_offset: usize,
    ) -> i_slint_core::lengths::LogicalRect {
        self.0.text_input_cursor_rect_for_byte_offset(text_input, item_rc, byte_offset)
    }

    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.0.register_font_from_memory(data)
    }

    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.0.register_font_from_path(path)
    }

    fn default_font_size(&self) -> i_slint_core::lengths::LogicalLength {
        self.0.default_font_size()
    }

    fn set_rendering_notifier(
        &self,
        callback: Box<dyn i_slint_core::api::RenderingNotifier>,
    ) -> Result<(), i_slint_core::api::SetRenderingNotifierError> {
        self.0.set_rendering_notifier(callback)
    }

    fn free_graphics_resources(
        &self,
        component: i_slint_core::item_tree::ItemTreeRef,
        items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'_>>>,
    ) -> Result<(), PlatformError> {
        self.0.free_graphics_resources(component, items)
    }

    fn set_window_adapter(&self, window_adapter: &Rc<dyn i_slint_core::window::WindowAdapter>) {
        self.0.set_window_adapter(window_adapter)
    }

    fn window_adapter(&self) -> Option<Rc<dyn i_slint_core::window::WindowAdapter>> {
        RendererSealed::window_adapter(&self.0)
    }

    fn resize(&self, size: i_slint_core::api::PhysicalSize) -> Result<(), PlatformError> {
        self.0.resize(size)
    }

    fn take_snapshot(
        &self,
    ) -> Result<
        i_slint_core::graphics::SharedPixelBuffer<i_slint_core::graphics::Rgba8Pixel>,
        PlatformError,
    > {
        self.0.take_snapshot()
    }

    fn supports_transformations(&self) -> bool {
        self.0.supports_transformations()
    }
}
