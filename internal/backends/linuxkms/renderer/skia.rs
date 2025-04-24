// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::sync::Arc;

use crate::display::RenderingRotation;
use crate::drmoutput::DrmOutput;
use i_slint_core::api::{PhysicalSize as PhysicalWindowSize, Window};
use i_slint_core::item_rendering::{DirtyRegion, ItemRenderer};
use i_slint_core::platform::PlatformError;
use i_slint_renderer_skia::skia_safe;
use i_slint_renderer_skia::SkiaRendererExt;

pub struct SkiaRendererAdapter {
    renderer: i_slint_renderer_skia::SkiaRenderer,
    presenter: Arc<dyn crate::display::Presenter>,
    size: PhysicalWindowSize,
}

impl SkiaRendererAdapter {
    #[cfg(feature = "renderer-skia-vulkan")]
    pub fn new_vulkan(
        _device_opener: &crate::DeviceOpener,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        // TODO: figure out how to associate vulkan with an existing drm fd.
        let display = crate::display::vulkandisplay::create_vulkan_display()?;

        let skia_vk_surface = i_slint_renderer_skia::vulkan_surface::VulkanSurface::from_surface(
            display.physical_device,
            display.queue_family_index,
            display.surface,
            display.size,
        )?;

        let renderer = Box::new(Self {
            renderer: i_slint_renderer_skia::SkiaRenderer::new_with_surface(Box::new(
                skia_vk_surface,
            )),
            // TODO: For vulkan we don't have a page flip event handling mechanism yet, so drive it with a timer.
            presenter: display.presenter,
            size: display.size,
        });

        eprintln!("Using Skia Vulkan renderer");

        Ok(renderer)
    }

    #[cfg(feature = "renderer-skia-opengl")]
    pub fn new_opengl(
        device_opener: &crate::DeviceOpener,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        let drm_output = DrmOutput::new(device_opener)?;
        let display = Arc::new(crate::display::gbmdisplay::GbmDisplay::new(drm_output)?);

        let (width, height) = display.drm_output.size();
        let size = i_slint_core::api::PhysicalSize::new(width, height);

        let skia_gl_surface =
            i_slint_renderer_skia::opengl_surface::OpenGLSurface::new_with_config(
                display.clone(),
                display.clone(),
                size,
                None,
                display.config_template_builder(),
                Some(&|config| display.filter_gl_config(config)),
            )?;

        let renderer = Box::new(Self {
            renderer: i_slint_renderer_skia::SkiaRenderer::new_with_surface(Box::new(
                skia_gl_surface,
            )),
            presenter: display.clone(),
            size,
        });

        renderer.renderer.set_pre_present_callback(Some(Box::new({
            move || {
                // Make sure the in-flight font-buffer from the previous swap_buffers call has been
                // posted to the screen.
                display.drm_output.wait_for_page_flip();
            }
        })));

        eprintln!("Using Skia OpenGL renderer");

        Ok(renderer)
    }

    pub fn new_software(
        device_opener: &crate::DeviceOpener,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        let display = crate::display::swdisplay::new(device_opener)?;

        let skia_software_surface: i_slint_renderer_skia::software_surface::SoftwareSurface =
            DrmDumbBufferAccess { display: display.clone() }.into();

        let (width, height) = display.size();
        let size = i_slint_core::api::PhysicalSize::new(width, height);

        let renderer = Box::new(Self {
            renderer: i_slint_renderer_skia::SkiaRenderer::new_with_surface(Box::new(
                skia_software_surface,
            )),
            presenter: display.as_presenter(),
            size,
        });

        eprintln!("Using Skia Software renderer");

        Ok(renderer)
    }

    pub fn new_try_vulkan_then_opengl_then_software(
        device_opener: &crate::DeviceOpener,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
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

        if result.is_err() {
            result = Self::new_software(device_opener);
        }

        result
    }
}

impl crate::fullscreenwindowadapter::FullscreenRenderer for SkiaRendererAdapter {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        draw_mouse_cursor_callback: &dyn Fn(&mut dyn ItemRenderer),
    ) -> Result<(), PlatformError> {
        self.renderer.render_transformed_with_post_callback(
            rotation.degrees(),
            rotation.translation_after_rotation(self.size),
            self.size,
            Some(&|item_renderer| {
                draw_mouse_cursor_callback(item_renderer);
            }),
        )?;
        self.presenter.present()?;
        Ok(())
    }
    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.size
    }
}
struct DrmDumbBufferAccess {
    display: Arc<dyn crate::display::swdisplay::SoftwareBufferDisplay>,
}

impl i_slint_renderer_skia::software_surface::RenderBuffer for DrmDumbBufferAccess {
    fn with_buffer(
        &self,
        _window: &Window,
        size: PhysicalWindowSize,
        render_callback: &mut dyn FnMut(
            std::num::NonZeroU32,
            std::num::NonZeroU32,
            skia_safe::ColorType,
            u8,
            &mut [u8],
        ) -> Result<
            Option<DirtyRegion>,
            i_slint_core::platform::PlatformError,
        >,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        let Some((width, height)) = size.width.try_into().ok().zip(size.height.try_into().ok())
        else {
            // Nothing to render
            return Ok(());
        };

        self.display.map_back_buffer(&mut |pixels, age, format| {
            render_callback(
                width,
                height,
                match format {
                    drm::buffer::DrmFourcc::Xrgb8888 => skia_safe::ColorType::BGRA8888,
                    drm::buffer::DrmFourcc::Rgb565 => skia_safe::ColorType::RGB565,
                    _ => {
                        return Err(format!(
                        "Unsupported frame buffer format {format} used with skia software renderer"
                    )
                        .into())
                    }
                },
                age,
                pixels.as_mut(),
            )?;
            Ok(())
        })
    }
}
