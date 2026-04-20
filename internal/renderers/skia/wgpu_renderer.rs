// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use i_slint_core::api::{GraphicsAPI, PhysicalSize as PhysicalWindowSize};
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::partial_renderer::DirtyRegion;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::RendererSealed;
use i_slint_core::window::WindowAdapter;

use wgpu_28 as wgpu;

use crate::wgpu_28_surface::Backend;
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
    gr_context: RefCell<skia_safe::gpu::DirectContext>,
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    backend: Backend,
    textures_to_transition_for_sampling: RefCell<Vec<wgpu::Texture>>,
}

impl SkiaWGPURenderer {
    /// Creates a new SkiaWGPURenderer.
    ///
    /// The `instance`, `adapter`, `device` and `queue` are the WGPU resources used for rendering.
    /// The `adapter` is needed to determine the GPU backend and create the Skia graphics context.
    ///
    /// An optional `shared_context` can be provided to share resources (e.g., a Vulkan instance)
    /// across multiple renderers. If `None`, a new default context is created.
    ///
    /// The wgpu resources are also provided to rendering notifier callbacks via
    /// [`GraphicsAPI::WGPU28`](i_slint_core::api::GraphicsAPI::WGPU28).
    pub fn new(
        shared_context: Option<&SkiaSharedContext>,
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Result<Self, PlatformError> {
        let backend: Backend = adapter.get_info().backend.try_into()?;

        let gr_context = backend.make_context(&adapter, &device, &queue).ok_or_else(|| {
            PlatformError::from("Failed to create Skia graphics context from WGPU")
        })?;

        let default_context;
        let shared_context = match shared_context {
            Some(ctx) => ctx,
            None => {
                default_context = SkiaSharedContext::default();
                &default_context
            }
        };
        // Use SkiaRenderer::default() to stay resilient to field additions, then disable
        // partial rendering — there is no buffer age tracking for external texture targets.
        let mut renderer = SkiaRenderer::default(shared_context);
        renderer.partial_rendering_state = None;

        Ok(Self {
            renderer,
            gr_context: RefCell::new(gr_context),
            instance,
            device,
            queue,
            backend,
            textures_to_transition_for_sampling: Default::default(),
        })
    }

    /// Render the scene to the given texture.
    ///
    /// The texture must have been created with `RENDER_ATTACHMENT` usage and have a supported
    /// format (`Rgba8Unorm`, `Bgra8Unorm`, or sRGB variants on Vulkan).
    pub fn render_to_texture(&self, texture: &wgpu::Texture) -> Result<(), PlatformError> {
        let size = texture.size();
        self.render_to_texture_impl(texture, size.width as i32, size.height as i32)
    }

    /// Render the scene to a sub-region of the given texture, specified by width and height.
    ///
    /// `width` and `height` must not exceed the texture's dimensions.
    pub fn render_to_texture_view(
        &self,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
    ) -> Result<(), PlatformError> {
        let size = texture.size();
        if width > size.width || height > size.height {
            return Err(format!(
                "render_to_texture_view: requested size {}x{} exceeds texture size {}x{}",
                width, height, size.width, size.height
            )
            .into());
        }
        self.render_to_texture_impl(texture, width as i32, height as i32)
    }

    fn render_to_texture_impl(
        &self,
        texture: &wgpu::Texture,
        width: i32,
        height: i32,
    ) -> Result<(), PlatformError> {
        let gr_context = &mut self.gr_context.borrow_mut();

        let mut skia_surface =
            self.backend.make_surface_from_texture(width, height, gr_context, texture).ok_or_else(
                || PlatformError::from("Failed to wrap WGPU texture as Skia render target"),
            )?;

        let window_adapter = self.renderer.window_adapter()?;
        let window = window_adapter.window();

        let surface_adapter = TextureSurface {
            instance: &self.instance,
            device: &self.device,
            queue: &self.queue,
            backend: &self.backend,
            textures_to_transition: &self.textures_to_transition_for_sampling,
        };

        self.renderer.render_to_canvas(
            skia_surface.canvas(),
            0.,
            (0., 0.),
            Some(gr_context),
            0,
            Some(&surface_adapter),
            window,
            None,
        );

        // Transition any imported wgpu textures to sampling state before submitting.
        let textures_to_transition = self.textures_to_transition_for_sampling.take();
        if !textures_to_transition.is_empty() {
            let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Skia texture transition encoder"),
            });
            encoder.transition_resources(
                std::iter::empty(),
                textures_to_transition.iter().map(|texture| wgpu::TextureTransition {
                    texture,
                    selector: None,
                    state: wgpu::TextureUses::RESOURCE,
                }),
            );
            self.queue.submit(Some(encoder.finish()));
        }

        gr_context.submit(None);

        Ok(())
    }

    /// Returns a reference to the inner [`SkiaRenderer`].
    ///
    /// Use this to return from [`WindowAdapter::renderer()`](i_slint_core::window::WindowAdapter::renderer)
    /// in your platform implementation. The `SkiaRenderer` implements the `Renderer` trait.
    pub fn renderer(&self) -> &SkiaRenderer {
        &self.renderer
    }
}

/// Lightweight adapter that implements the [`Surface`](crate::Surface) trait for
/// render-to-texture, providing `with_graphics_api` and `import_wgpu_texture` support
/// without requiring a window or swapchain.
struct TextureSurface<'a> {
    instance: &'a wgpu::Instance,
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    backend: &'a Backend,
    textures_to_transition: &'a RefCell<Vec<wgpu::Texture>>,
}

impl crate::Surface for TextureSurface<'_> {
    fn new(
        _: &SkiaSharedContext,
        _: Arc<dyn raw_window_handle::HasWindowHandle + Send + Sync>,
        _: Arc<dyn raw_window_handle::HasDisplayHandle + Send + Sync>,
        _: PhysicalWindowSize,
        _: Option<RequestedGraphicsAPI>,
    ) -> Result<Self, PlatformError>
    where
        Self: Sized,
    {
        Err("TextureSurface cannot be created from a window handle".into())
    }

    fn name(&self) -> &'static str {
        "wgpu-texture"
    }

    #[cfg(feature = "unstable-wgpu-28")]
    fn with_graphics_api(&self, callback: &mut dyn FnMut(GraphicsAPI<'_>)) {
        let api = i_slint_core::graphics::create_graphics_api_wgpu_28(
            self.instance.clone(),
            self.device.clone(),
            self.queue.clone(),
        );
        callback(api)
    }

    fn render(
        &self,
        _: &i_slint_core::api::Window,
        _: PhysicalWindowSize,
        _: &dyn Fn(
            &skia_safe::Canvas,
            Option<&mut skia_safe::gpu::DirectContext>,
            u8,
        ) -> Option<DirtyRegion>,
        _: &RefCell<Option<Box<dyn FnMut()>>>,
    ) -> Result<(), PlatformError> {
        Err("TextureSurface does not support render() — use render_to_texture() instead".into())
    }

    fn resize_event(&self, _: PhysicalWindowSize) -> Result<(), PlatformError> {
        Ok(())
    }

    fn bits_per_pixel(&self) -> Result<u8, PlatformError> {
        Ok(32)
    }

    #[cfg(any(feature = "unstable-wgpu-27", feature = "unstable-wgpu-28"))]
    fn import_wgpu_texture(
        &self,
        canvas: &skia_safe::Canvas,
        any_wgpu_texture: &i_slint_core::graphics::WGPUTexture,
    ) -> Option<skia_safe::Image> {
        let texture = match any_wgpu_texture {
            #[cfg(feature = "unstable-wgpu-27")]
            i_slint_core::graphics::WGPUTexture::WGPU27Texture(..) => return None,
            #[cfg(feature = "unstable-wgpu-28")]
            i_slint_core::graphics::WGPUTexture::WGPU28Texture(texture) => texture.clone(),
        };

        self.textures_to_transition.borrow_mut().push(texture.clone());
        self.backend.import_texture(canvas, texture)
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
