// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;
use i_slint_renderer_femtovg::FemtoVGRendererExt;

use crate::display::RenderingRotation;
use crate::drmoutput::DrmOutput;

pub struct FemtoVGWgpuRendererAdapter {
    renderer:
        i_slint_renderer_femtovg::FemtoVGRenderer<i_slint_renderer_femtovg::wgpu::WGPUBackend>,
    size: i_slint_core::api::PhysicalSize,
    /// Keep the DRM output alive — vkAcquireDrmDisplayEXT needs the fd open.
    _drm_output: DrmOutput,
}

impl FemtoVGWgpuRendererAdapter {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        device_opener: &crate::DeviceOpener,
        requested_graphics_api: Option<&i_slint_core::graphics::RequestedGraphicsAPI>,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        let drm_output = DrmOutput::new(device_opener)?;

        // Letting Vulkan drive the display needs VK_EXT_acquire_drm_display, which
        // few drivers offer. Without it, render into a dma-buf and page-flip that.
        if std::env::var_os("SLINT_KMS_WGPU_DMABUF").is_some()
            || !super::dmabuf::acquire_drm_display_available()
        {
            return super::femtovg_dmabuf::FemtoVGDmabufRendererAdapter::new(
                drm_output,
                requested_graphics_api,
            );
        }

        let (renderer, size) = match Self::new_wgpu_surface(&drm_output, requested_graphics_api) {
            Ok(surface) => surface,
            // The extension is there but unusable for this device: no Vulkan
            // physical device matching the DRM fd, or no matching display mode.
            Err(err) => {
                eprintln!("Falling back to dma-buf presentation: {err}");
                return super::femtovg_dmabuf::FemtoVGDmabufRendererAdapter::new(
                    drm_output,
                    requested_graphics_api,
                );
            }
        };

        let renderer = Box::new(Self { renderer, size, _drm_output: drm_output });

        eprintln!("Using FemtoVG wgpu renderer");

        Ok(renderer)
    }
}

impl FemtoVGWgpuRendererAdapter {
    /// Creates the renderer that draws straight onto a DRM plane, which requires
    /// `VK_EXT_acquire_drm_display`.
    fn new_wgpu_surface(
        drm_output: &DrmOutput,
        requested_graphics_api: Option<&i_slint_core::graphics::RequestedGraphicsAPI>,
    ) -> Result<
        (
            i_slint_renderer_femtovg::FemtoVGRenderer<i_slint_renderer_femtovg::wgpu::WGPUBackend>,
            i_slint_core::api::PhysicalSize,
        ),
        PlatformError,
    > {
        let (surface_target, size) = drm_output.wgpu_30_surface_target()?;

        let renderer = i_slint_renderer_femtovg::FemtoVGRenderer::new_suspended();
        renderer
            .set_surface(surface_target, size, requested_graphics_api.cloned(), false)
            .map_err(|e| format!("Error initializing FemtoVG wgpu surface: {e}"))?;

        Ok((renderer, size))
    }
}

impl crate::fullscreenwindowadapter::FullscreenRenderer for FemtoVGWgpuRendererAdapter {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        draw_mouse_cursor_callback: &dyn Fn(&mut dyn ItemRenderer),
    ) -> Result<DrawOutcome, PlatformError> {
        self.renderer.render_transformed_with_post_callback(
            rotation.degrees(),
            rotation.translation_after_rotation(self.size),
            self.size,
            Some(&|item_renderer| {
                draw_mouse_cursor_callback(item_renderer);
            }),
        )
    }

    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.size
    }
}
