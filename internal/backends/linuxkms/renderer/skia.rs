// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::platform::PlatformError;

pub struct SkiaRendererAdapter {
    renderer: i_slint_renderer_skia::SkiaRenderer,
    presenter: Option<Box<dyn crate::display::Presenter>>,
    size: PhysicalWindowSize,
}

impl SkiaRendererAdapter {
    #[cfg(feature = "renderer-linuxkms-skia-vulkan")]
    pub fn new_vulkan() -> Result<Box<dyn crate::fullscreenwindowadapter::Renderer>, PlatformError>
    {
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

    #[cfg(feature = "renderer-linuxkms-skia-opengl")]
    pub fn new_opengl() -> Result<Box<dyn crate::fullscreenwindowadapter::Renderer>, PlatformError>
    {
        let display = crate::display::egldisplay::create_egl_display()?;

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
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::Renderer>, PlatformError> {
        #[allow(unused_assignments)]
        let mut result = Err(format!("No skia renderer available").into());

        #[cfg(feature = "renderer-linuxkms-skia-vulkan")]
        {
            result = Self::new_vulkan();
        }

        #[cfg(feature = "renderer-linuxkms-skia-opengl")]
        if result.is_err() {
            result = Self::new_opengl();
        }

        result
    }
}

impl crate::fullscreenwindowadapter::Renderer for SkiaRendererAdapter {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }
    fn render_and_present(&self, window: &i_slint_core::api::Window) -> Result<(), PlatformError> {
        self.renderer.render(window)?;
        if let Some(presenter) = self.presenter.as_ref() {
            presenter.present()?;
        }
        Ok(())
    }
    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.size
    }
}
