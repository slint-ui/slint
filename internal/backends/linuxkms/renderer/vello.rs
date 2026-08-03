// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Render with vello on WGPU, onto a DRM plane.

use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;
use i_slint_renderer_anyrender::VelloRenderer;

use crate::display::RenderingRotation;
use crate::drmoutput::DrmOutput;

pub struct VelloRendererAdapter {
    renderer: VelloRenderer,
    size: i_slint_core::api::PhysicalSize,
    /// Keep the DRM output alive - the surface refers to its file descriptor.
    _drm_output: DrmOutput,
}

impl VelloRendererAdapter {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        device_opener: &crate::DeviceOpener,
        requested_graphics_api: Option<&i_slint_core::graphics::RequestedGraphicsAPI>,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        let drm_output = DrmOutput::new(device_opener)?;
        let (surface_target, size) = drm_output.wgpu_29_surface_target()?;

        let renderer = VelloRenderer::new_vello();
        renderer.set_surface(
            surface_target,
            size.width,
            size.height,
            requested_graphics_api.cloned(),
        )?;

        eprintln!("Using vello renderer");

        Ok(Box::new(Self { renderer, size, _drm_output: drm_output }))
    }
}

impl crate::fullscreenwindowadapter::FullscreenRenderer for VelloRendererAdapter {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        draw_mouse_cursor_callback: &dyn Fn(&mut dyn ItemRenderer),
    ) -> Result<DrawOutcome, PlatformError> {
        self.renderer.render_with_options(
            rotation.degrees(),
            rotation.translation_after_rotation(self.size),
            Some(&|item_renderer| {
                draw_mouse_cursor_callback(item_renderer);
            }),
        )
    }

    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.size
    }
}
