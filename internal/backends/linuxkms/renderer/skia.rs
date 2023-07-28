// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::platform::PlatformError;
use i_slint_renderer_skia::SkiaRendererExtension;

pub struct SkiaRendererAdapter {
    renderer: i_slint_renderer_skia::SkiaRenderer,
    presenter: Option<Box<dyn crate::display::Presenter>>,
    size: PhysicalWindowSize,
}

impl SkiaRendererAdapter {
    #[cfg(feature = "renderer-skia-vulkan")]
    pub fn new_vulkan(
        _device_opener: &crate::DeviceOpener,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::Renderer>, PlatformError> {
        // TODO: figure out how to associate vulkan with an existing drm fd.
        let display = crate::display::vulkandisplay::create_vulkan_display()?;

        let skia_vk_surface = i_slint_renderer_skia::vulkan_surface::VulkanSurface::from_surface(
            display.physical_device,
            display.queue_family_index,
            display.surface,
            display.size,
        )?;

        let renderer = Box::new(Self {
            renderer: i_slint_renderer_skia::SkiaRenderer::new_with_surface(skia_vk_surface),
            presenter: None,
            size: display.size,
        });

        eprintln!("Using Skia Vulkan renderer");

        Ok(renderer)
    }

    #[cfg(feature = "renderer-skia-opengl")]
    pub fn new_opengl(
        device_opener: &crate::DeviceOpener,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::Renderer>, PlatformError> {
        let display = crate::display::egldisplay::create_egl_display(device_opener)?;

        use i_slint_renderer_skia::Surface;
        use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
        let skia_gl_surface = i_slint_renderer_skia::opengl_surface::OpenGLSurface::new(
            display.window_handle().unwrap(),
            display.display_handle().unwrap(),
            display.size,
        )?;

        let size = display.size;

        let renderer = Box::new(Self {
            renderer: i_slint_renderer_skia::SkiaRenderer::new_with_surface(skia_gl_surface),
            presenter: Some(Box::new(display)),
            size,
        });

        eprintln!("Using Skia OpenGL renderer");

        Ok(renderer)
    }

    pub fn new_try_vulkan_then_opengl(
        device_opener: &crate::DeviceOpener,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::Renderer>, PlatformError> {
        #[allow(unused_assignments)]
        let mut result = Err(format!("No skia renderer available").into());

        #[cfg(feature = "renderer-skia-vulkan")]
        {
            result = Self::new_vulkan(device_opener);
        }

        #[cfg(feature = "renderer-skia-opengl")]
        if result.is_err() {
            result = Self::new_opengl(device_opener);
        }

        result
    }
}

impl crate::fullscreenwindowadapter::Renderer for SkiaRendererAdapter {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }
    fn render_and_present(
        &self,
        draw_mouse_cursor_callback: &dyn Fn(&mut dyn ItemRenderer),
    ) -> Result<(), PlatformError> {
        self.renderer.render_with_post_callback(Some(&|item_renderer| {
            draw_mouse_cursor_callback(item_renderer);
        }))?;
        if let Some(presenter) = self.presenter.as_ref() {
            presenter.present()?;
        }
        Ok(())
    }
    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.size
    }
}
