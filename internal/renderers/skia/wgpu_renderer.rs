// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore rasterizers
use std::pin::Pin;
use std::rc::Rc;

use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::RendererSealed;
use i_slint_core::window::WindowAdapter;

use crate::{SkiaRenderer, SkiaSharedContext};

/// Use the Skia renderer with WGPU when implementing a custom Slint platform where you want the
/// scene to be rendered into a WGPU texture. The rendering is done using the
/// [Skia](https://skia.org/) library with platform-native GPU acceleration.
///
/// This is the Skia equivalent of `FemtoVGWGPURenderer`, offering superior font rendering
/// quality through platform-native text rasterizers.
///
/// This type is generic over the wgpu version, one instantiation per enabled
/// `unstable-wgpu-*` feature. Use it through the version-specific aliases,
/// `SkiaWGPU29Renderer` for wgpu 29 and `SkiaWGPU30Renderer` for wgpu 30 —
/// they stay unambiguous when several wgpu versions are enabled at once.
///
/// Rendering notifier callbacks registered via
/// [`Window::set_rendering_notifier()`](i_slint_core::api::Window::set_rendering_notifier)
/// will receive [`GraphicsAPI::WGPU29`](i_slint_core::api::GraphicsAPI::WGPU29) (respectively
/// [`GraphicsAPI::WGPU30`](i_slint_core::api::GraphicsAPI::WGPU30)) with the
/// renderer's instance, device, and queue.
pub struct SkiaWGPURendererGeneric<Surface> {
    renderer: SkiaRenderer,
    surface: Surface,
}

/// Renders into wgpu 29 textures. Available with the `unstable-wgpu-29` feature;
/// the matching wgpu API types are re-exported in the `slint::wgpu_29` module.
#[cfg(feature = "wgpu-29")]
pub type SkiaWGPU29Renderer = SkiaWGPURendererGeneric<crate::wgpu_29_surface::WGPUSurface>;

/// Renders into wgpu 30 textures. Available with the `unstable-wgpu-30` feature;
/// the matching wgpu API types are re-exported in the `slint::wgpu_30` module.
#[cfg(feature = "wgpu-30")]
pub type SkiaWGPU30Renderer = SkiaWGPURendererGeneric<crate::wgpu_30_surface::WGPUSurface>;

/// Compatibility alias for the newest enabled wgpu version: wgpu 30 when the
/// `unstable-wgpu-30` feature is enabled, wgpu 29 otherwise.
/// Prefer the versioned names, `SkiaWGPU29Renderer` or `SkiaWGPU30Renderer`, in new code;
/// they don't change meaning when another `unstable-wgpu-*` feature gets enabled elsewhere
/// in the dependency graph.
#[cfg(feature = "wgpu-30")]
pub type SkiaWGPURenderer = SkiaWGPU30Renderer;
/// Compatibility alias for the newest enabled wgpu version: wgpu 30 when the
/// `unstable-wgpu-30` feature is enabled, wgpu 29 otherwise.
/// Prefer the versioned names, `SkiaWGPU29Renderer` or `SkiaWGPU30Renderer`, in new code;
/// they don't change meaning when another `unstable-wgpu-*` feature gets enabled elsewhere
/// in the dependency graph.
#[cfg(all(feature = "wgpu-29", not(feature = "wgpu-30")))]
pub type SkiaWGPURenderer = SkiaWGPU29Renderer;

impl<Surface> SkiaWGPURendererGeneric<Surface> {
    fn new_with_surface(surface: Surface) -> Self {
        let shared_context = SkiaSharedContext::default();
        // Use SkiaRenderer::default() to stay resilient to field additions, then disable
        // partial rendering — there is no buffer age tracking for external texture targets.
        let mut renderer = SkiaRenderer::default(&shared_context);
        renderer.partial_rendering_state = None;
        Self { renderer, surface }
    }
}

// The version-specific constructors and render entry points below are kept in sync manually,
// like the wgpu_29_surface/wgpu_30_surface modules they build on.

#[cfg(feature = "wgpu-29")]
impl SkiaWGPU29Renderer {
    /// Creates a new SkiaWGPU29Renderer.
    ///
    /// The `instance`, `adapter`, `device` and `queue` are the WGPU resources used for rendering.
    /// The `adapter` is needed to determine the GPU backend and create the Skia graphics context.
    ///
    /// The wgpu resources are also provided to rendering notifier callbacks via
    /// [`GraphicsAPI::WGPU29`](i_slint_core::api::GraphicsAPI::WGPU29).
    pub fn new(
        instance: wgpu_29::Instance,
        adapter: wgpu_29::Adapter,
        device: wgpu_29::Device,
        queue: wgpu_29::Queue,
    ) -> Result<Self, PlatformError> {
        use crate::wgpu_29_surface::{Backend, WGPUSurface};

        let backend: Backend = adapter.get_info().backend.try_into()?;

        let gr_context = backend.make_context(&adapter, &device, &queue).ok_or_else(|| {
            PlatformError::from("Failed to create Skia graphics context from WGPU")
        })?;

        Ok(Self::new_with_surface(WGPUSurface::new_offscreen(
            instance, device, queue, backend, gr_context,
        )))
    }

    /// Render the scene to the given texture.
    ///
    /// The texture must have been created with `RENDER_ATTACHMENT` usage and have a supported
    /// format. Supported formats depend on the GPU backend: `Rgba8Unorm` and `Rgba8UnormSrgb`
    /// are supported on all backends; `Bgra8Unorm` is additionally supported on Metal and Vulkan.
    pub fn render_to_texture(&self, texture: &wgpu_29::Texture) -> Result<(), PlatformError> {
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

#[cfg(feature = "wgpu-30")]
impl SkiaWGPU30Renderer {
    /// Creates a new SkiaWGPU30Renderer.
    ///
    /// The `instance`, `adapter`, `device` and `queue` are the WGPU resources used for rendering.
    /// The `adapter` is needed to determine the GPU backend and create the Skia graphics context.
    ///
    /// The wgpu resources are also provided to rendering notifier callbacks via
    /// [`GraphicsAPI::WGPU30`](i_slint_core::api::GraphicsAPI::WGPU30).
    pub fn new(
        instance: wgpu_30::Instance,
        adapter: wgpu_30::Adapter,
        device: wgpu_30::Device,
        queue: wgpu_30::Queue,
    ) -> Result<Self, PlatformError> {
        use crate::wgpu_30_surface::{Backend, WGPUSurface};

        let backend: Backend = adapter.get_info().backend.try_into()?;

        let gr_context = backend.make_context(&adapter, &device, &queue).ok_or_else(|| {
            PlatformError::from("Failed to create Skia graphics context from WGPU")
        })?;

        Ok(Self::new_with_surface(WGPUSurface::new_offscreen(
            instance, device, queue, backend, gr_context,
        )))
    }

    /// Render the scene to the given texture.
    ///
    /// The texture must have been created with `RENDER_ATTACHMENT` usage and have a supported
    /// format. Supported formats depend on the GPU backend: `Rgba8Unorm` and `Rgba8UnormSrgb`
    /// are supported on all backends; `Bgra8Unorm` is additionally supported on Metal and Vulkan.
    pub fn render_to_texture(&self, texture: &wgpu_30::Texture) -> Result<(), PlatformError> {
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
impl<Surface> RendererSealed for SkiaWGPURendererGeneric<Surface> {
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

// Assert the consumer-visible signatures of every renderer name. Building the crate only
// compiles the definitions; these coercions prove the intended calls resolve, in particular
// that `SkiaWGPURenderer::new` stays unambiguous with both wgpu features enabled.
#[cfg(test)]
mod signature_tests {
    use super::*;

    #[cfg(feature = "wgpu-29")]
    const _: fn(
        wgpu_29::Instance,
        wgpu_29::Adapter,
        wgpu_29::Device,
        wgpu_29::Queue,
    ) -> Result<SkiaWGPU29Renderer, PlatformError> = SkiaWGPU29Renderer::new;
    #[cfg(feature = "wgpu-29")]
    const _: fn(&SkiaWGPU29Renderer, &wgpu_29::Texture) -> Result<(), PlatformError> =
        SkiaWGPU29Renderer::render_to_texture;

    #[cfg(feature = "wgpu-30")]
    const _: fn(
        wgpu_30::Instance,
        wgpu_30::Adapter,
        wgpu_30::Device,
        wgpu_30::Queue,
    ) -> Result<SkiaWGPU30Renderer, PlatformError> = SkiaWGPU30Renderer::new;
    #[cfg(feature = "wgpu-30")]
    const _: fn(&SkiaWGPU30Renderer, &wgpu_30::Texture) -> Result<(), PlatformError> =
        SkiaWGPU30Renderer::render_to_texture;

    #[cfg(feature = "wgpu-30")]
    const _: fn(
        wgpu_30::Instance,
        wgpu_30::Adapter,
        wgpu_30::Device,
        wgpu_30::Queue,
    ) -> Result<SkiaWGPURenderer, PlatformError> = SkiaWGPURenderer::new;
    #[cfg(all(feature = "wgpu-29", not(feature = "wgpu-30")))]
    const _: fn(
        wgpu_29::Instance,
        wgpu_29::Adapter,
        wgpu_29::Device,
        wgpu_29::Queue,
    ) -> Result<SkiaWGPURenderer, PlatformError> = SkiaWGPURenderer::new;
}
