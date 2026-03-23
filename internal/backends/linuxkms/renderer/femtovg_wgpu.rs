// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::os::fd::{AsFd, AsRawFd};

use i_slint_core::graphics::wgpu_28::wgpu;
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::platform::PlatformError;
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

        let plane = drm_output.find_compatible_plane()?;
        let (width, height) = drm_output.size();
        let refresh_rate_mhz = drm_output.refresh_rate_millihertz();

        let surface_target =
            i_slint_core::graphics::wgpu_28::SurfaceTarget::Drm(wgpu::SurfaceTargetUnsafe::Drm {
                fd: drm_output.drm_device.as_fd().as_raw_fd(),
                plane: plane.handle().into(),
                connector_id: drm_output.connector.handle().into(),
                width,
                height,
                refresh_rate: refresh_rate_mhz,
            });

        let size = i_slint_core::api::PhysicalSize::new(width, height);

        let renderer = i_slint_renderer_femtovg::FemtoVGRenderer::new_suspended();
        renderer
            .set_surface(surface_target, size, requested_graphics_api.cloned())
            .map_err(|e| format!("Error initializing FemtoVG wgpu surface: {e}"))?;

        let renderer = Box::new(Self { renderer, size, _drm_output: drm_output });

        eprintln!("Using FemtoVG wgpu renderer");

        Ok(renderer)
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
    ) -> Result<(), PlatformError> {
        self.renderer.render_transformed_with_post_callback(
            rotation.degrees(),
            rotation.translation_after_rotation(self.size),
            self.size,
            Some(&|item_renderer| {
                draw_mouse_cursor_callback(item_renderer);
            }),
        )?;
        Ok(())
    }

    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.size
    }
}
