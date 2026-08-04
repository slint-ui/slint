// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore rasterizers
use std::pin::Pin;
use std::rc::Rc;

use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::RendererSealed;
use i_slint_core::window::WindowAdapter;

// When multiple wgpu versions are enabled, the renderer is compiled against the newest one,
// consistent with the wgpu version selection in the winit and linuxkms backends.
#[cfg(feature = "wgpu-30")]
use wgpu_30 as wgpu;

#[cfg(all(feature = "wgpu-29", not(feature = "wgpu-30")))]
use wgpu_29 as wgpu;

#[cfg(feature = "wgpu-30")]
use crate::wgpu_30_surface::{Backend, WGPUSurface};

#[cfg(all(feature = "wgpu-29", not(feature = "wgpu-30")))]
use crate::wgpu_29_surface::{Backend, WGPUSurface};

use crate::{SkiaRenderer, SkiaSharedContext};

/// Use the Skia renderer with WGPU when implementing a custom Slint platform where you want the
/// scene to be rendered into a WGPU texture. The rendering is done using the
/// [Skia](https://skia.org/) library with platform-native GPU acceleration.
///
/// This is the Skia equivalent of `FemtoVGWGPURenderer`, offering superior font rendering
/// quality through platform-native text rasterizers.
///
/// The wgpu types used by this renderer follow the newest enabled `unstable-wgpu-*` feature:
/// wgpu 30 when `unstable-wgpu-30` is enabled, wgpu 29 otherwise.
///
/// Rendering notifier callbacks registered via
/// [`Window::set_rendering_notifier()`](i_slint_core::api::Window::set_rendering_notifier)
/// will receive [`GraphicsAPI::WGPU30`](i_slint_core::api::GraphicsAPI::WGPU30) (respectively
/// [`GraphicsAPI::WGPU29`](i_slint_core::api::GraphicsAPI::WGPU29)) with the
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
    /// [`GraphicsAPI::WGPU30`](i_slint_core::api::GraphicsAPI::WGPU30) (respectively
    /// [`GraphicsAPI::WGPU29`](i_slint_core::api::GraphicsAPI::WGPU29)).
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
        // The renderer draws into caller-provided textures: there is no surface to
        // (re-)create, and partial rendering stays off because there is no buffer age
        // tracking for external texture targets.
        let renderer = SkiaRenderer::new_with_surface_factory(
            &shared_context,
            |_, _, _, _, _| Err("SkiaWGPURenderer does not support dynamic surface creation".into()),
            None,
        );

        Ok(Self { renderer, surface })
    }

    /// Render the scene to the given texture.
    ///
    /// The texture must have been created with `RENDER_ATTACHMENT` usage and have a supported
    /// format.
    /// Supported formats depend on the GPU backend: `Rgba8Unorm` and `Rgba8UnormSrgb` are
    /// supported on all backends; `Bgra8Unorm` is additionally supported on Metal and Vulkan.
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
    // The text and font registration functions use their default implementations, which
    // reach the inner renderer's state through this accessor and window_adapter().
    fn text_layout_cache(
        &self,
    ) -> Option<&i_slint_core::textlayout::sharedparley::TextLayoutCache> {
        RendererSealed::text_layout_cache(&self.renderer)
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
