// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! FemtoVG on wgpu for drivers without `VK_EXT_acquire_drm_display`.
//!
//! It works like the Skia adapter in `skia_dmabuf.rs`,
//! except that FemtoVG draws through wgpu, see [`ScanoutBarriers`].

// cSpell: ignore bufs dmabuf

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;
use i_slint_renderer_femtovg::{FemtoVGWGPURenderer, FemtoVGWGPURendererExt};
use wgpu_30 as wgpu;

use super::dmabuf::{init_wgpu, wait_for_gpu};
use crate::display::RenderingRotation;
use crate::display::gbmdmabufdisplay::{BUFFER_COUNT, GbmDmabufDisplay};
use crate::display::scanout_barriers::ScanoutBarriers;
use crate::drmoutput::DrmOutput;

pub struct FemtoVGDmabufRendererAdapter {
    renderer: FemtoVGWGPURenderer,
    display: GbmDmabufDisplay,
    barriers: ScanoutBarriers,
    device: wgpu::Device,
    size: PhysicalWindowSize,
}

impl FemtoVGDmabufRendererAdapter {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        drm_output: DrmOutput,
        requested_graphics_api: Option<&RequestedGraphicsAPI>,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        let (instance, _adapter, device, queue) = init_wgpu(requested_graphics_api)?;

        let (width, height) = drm_output.size();
        let size = PhysicalWindowSize::new(width, height);

        let display = GbmDmabufDisplay::new(drm_output, &device)?;
        let barriers = ScanoutBarriers::new(&device, BUFFER_COUNT)?;

        let renderer = FemtoVGWGPURenderer::new(instance, device.clone(), queue)?;

        eprintln!("Using FemtoVG wgpu renderer, presenting dma-bufs on a DRM plane");

        Ok(Box::new(Self { renderer, display, barriers, device, size }))
    }
}

impl crate::fullscreenwindowadapter::FullscreenRenderer for FemtoVGDmabufRendererAdapter {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        draw_mouse_cursor_callback: &dyn Fn(&mut dyn ItemRenderer),
    ) -> Result<DrawOutcome, PlatformError> {
        // `Skipped` would ask for the next frame right away, which would draw nothing either.
        if !self.renderer.can_draw()? {
            return Ok(DrawOutcome::Success);
        }

        // Drawing starts before the last flip finished, see `BUFFER_COUNT`.
        let (index, texture) = self.display.back_buffer();

        self.barriers.acquire(index, texture)?;

        let drawn = self
            .renderer
            .render_to_texture_transformed(
                texture,
                rotation.degrees(),
                rotation.translation_after_rotation(self.size),
                Some(draw_mouse_cursor_callback),
            )
            .and_then(|()| self.barriers.release(index, texture));

        // wgpu-hal exports no sync fd for KMS to wait on, so wait for the GPU here.
        // A failed frame waits too, see `ScanoutBarriers::release`.
        let waited = wait_for_gpu(&self.device);
        drawn?;
        waited?;

        self.display.present().map_err(|e| format!("Error presenting dma-buf: {e}"))?;
        Ok(DrawOutcome::Success)
    }

    fn size(&self) -> PhysicalWindowSize {
        self.size
    }
}
