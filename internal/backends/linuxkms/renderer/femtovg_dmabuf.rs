// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! FemtoVG on wgpu for drivers without `VK_EXT_acquire_drm_display`.
//!
//! Same shape as [`super::skia_dmabuf`], with one difference: FemtoVG draws
//! through wgpu alone, so the handoff of each buffer to the display controller
//! goes through [`ScanoutBarriers`] rather than through the renderer.

// cSpell: ignore dmabuf

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;
use i_slint_renderer_femtovg::FemtoVGWGPURenderer;
use wgpu_30 as wgpu;

use super::dmabuf::{init_wgpu, wait_for_gpu};
use crate::display::RenderingRotation;
use crate::display::gbmdmabufdisplay::GbmDmabufDisplay;
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
        let barriers = ScanoutBarriers::new(&device, &queue)?;

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
        // The buffer to draw into is two flips old, so the frame can be drawn
        // while the previous one is still on its way to the screen.
        // `GbmDmabufDisplay::present` waits for that flip.
        let texture = self.display.back_buffer();

        self.barriers.acquire(texture)?;

        self.renderer.render_to_texture_transformed(
            texture,
            rotation.degrees(),
            rotation.translation_after_rotation(self.size),
            Some(&|item_renderer| {
                draw_mouse_cursor_callback(item_renderer);
            }),
        )?;

        self.barriers.release(texture)?;

        // KMS has no way to wait on the render: wgpu-hal exports no sync fd for
        // the plane's IN_FENCE_FD property, and wgpu won't take a signal semaphore
        // for a submission. Block until the GPU is done instead, at the cost of a
        // pipeline stall per frame.
        wait_for_gpu(&self.device)?;

        self.display.present().map_err(|e| format!("Error presenting dma-buf: {e}"))?;

        Ok(DrawOutcome::Success)
    }

    fn size(&self) -> PhysicalWindowSize {
        self.size
    }
}
