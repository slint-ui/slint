// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use std::rc::Rc;

use crate::display::RenderingRotation;
use crate::drmoutput::DrmOutput;
use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::platform::PlatformError;
use i_slint_renderer_skia::skia_safe;
use i_slint_renderer_skia::SkiaRendererExt;

pub struct SkiaRendererAdapter {
    renderer: i_slint_renderer_skia::SkiaRenderer,
    presenter: Rc<dyn crate::display::Presenter>,
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
        let display = crate::display::gbmdisplay::GbmDisplay::new(drm_output)?;

        use i_slint_renderer_skia::Surface;
        use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

        let (width, height) = display.drm_output.size();
        let size = i_slint_core::api::PhysicalSize::new(width, height);

        let skia_gl_surface = i_slint_renderer_skia::opengl_surface::OpenGLSurface::new(
            display.window_handle().unwrap(),
            display.display_handle().unwrap(),
            size,
        )?;

        let renderer = Box::new(Self {
            renderer: i_slint_renderer_skia::SkiaRenderer::new_with_surface(Box::new(
                skia_gl_surface,
            )),
            presenter: Rc::new(display),
            size,
        });

        eprintln!("Using Skia OpenGL renderer");

        Ok(renderer)
    }

    pub fn new_software(
        device_opener: &crate::DeviceOpener,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        let drm_output = DrmOutput::new(device_opener)?;
        let display = Rc::new(crate::display::swdisplay::SoftwareBufferDisplay::new(drm_output)?);

        let skia_software_surface: i_slint_renderer_skia::software_surface::SoftwareSurface =
            DrmDumbBufferAccess { display: display.clone() }.into();

        let (width, height) = display.drm_output.size();
        let size = i_slint_core::api::PhysicalSize::new(width, height);

        let renderer = Box::new(Self {
            renderer: i_slint_renderer_skia::SkiaRenderer::new_with_surface(Box::new(
                skia_software_surface,
            )),
            presenter: display,
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

    fn is_ready_to_present(&self) -> bool {
        self.presenter.is_ready_to_present()
    }

    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        draw_mouse_cursor_callback: &dyn Fn(&mut dyn ItemRenderer),
        ready_for_next_animation_frame: Box<dyn FnOnce()>,
    ) -> Result<(), PlatformError> {
        self.renderer.render_transformed_with_post_callback(
            rotation.degrees(),
            rotation.translation_after_rotation(self.size),
            self.size,
            Some(&|item_renderer| {
                draw_mouse_cursor_callback(item_renderer);
            }),
        )?;
        self.presenter.present_with_next_frame_callback(ready_for_next_animation_frame)?;
        Ok(())
    }
    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.size
    }

    fn register_page_flip_handler(
        &self,
        event_loop_handle: crate::calloop_backend::EventLoopHandle,
    ) -> Result<(), PlatformError> {
        self.presenter.clone().register_page_flip_handler(event_loop_handle)
    }
}
struct DrmDumbBufferAccess {
    display: Rc<crate::display::swdisplay::SoftwareBufferDisplay>,
}

impl i_slint_renderer_skia::software_surface::RenderBuffer for DrmDumbBufferAccess {
    fn with_buffer(
        &self,
        size: PhysicalWindowSize,
        render_callback: &mut dyn FnMut(
            std::num::NonZeroU32,
            std::num::NonZeroU32,
            skia_safe::ColorType,
            &mut [u8],
        )
            -> Result<(), i_slint_core::platform::PlatformError>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        let Some((width, height)) = size.width.try_into().ok().zip(size.height.try_into().ok())
        else {
            // Nothing to render
            return Ok(());
        };

        self.display.map_back_buffer(&mut |mut pixels| {
            render_callback(width, height, skia_safe::ColorType::BGRA8888, pixels.as_mut())
        })
    }
}
