// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Skia on wgpu, presenting through [`crate::display::gbmdmabufdisplay`].

// cSpell: ignore bufs dmabuf

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;
use i_slint_renderer_skia::{SkiaWGPU30Renderer, SkiaWGPU30RendererExt};
use wgpu_30 as wgpu;

use super::dmabuf::{init_wgpu, wait_for_gpu};
use crate::display::RenderingRotation;
use crate::display::gbmdmabufdisplay::{BUFFER_COUNT, GbmDmabufDisplay};
use crate::display::scanout_barriers::ScanoutBarriers;
use crate::drmoutput::DrmOutput;

pub struct SkiaDmabufRendererAdapter {
    renderer: SkiaWGPU30Renderer,
    display: GbmDmabufDisplay,
    barriers: ScanoutBarriers,
    device: wgpu::Device,
    size: PhysicalWindowSize,
}

impl SkiaDmabufRendererAdapter {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        drm_output: DrmOutput,
        requested_graphics_api: Option<&RequestedGraphicsAPI>,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        let (instance, adapter, device, queue) = init_wgpu(requested_graphics_api)?;

        let (width, height) = drm_output.size();
        let size = PhysicalWindowSize::new(width, height);

        let display = GbmDmabufDisplay::new(drm_output, &device)?;
        let barriers = ScanoutBarriers::new(&device, BUFFER_COUNT)?;

        let renderer = SkiaWGPU30Renderer::new(instance, adapter, device.clone(), queue)?;

        eprintln!("Using Skia renderer with wgpu, presenting dma-bufs on a DRM plane");

        Ok(Box::new(Self { renderer, display, barriers, device, size }))
    }
}

impl crate::fullscreenwindowadapter::FullscreenRenderer for SkiaDmabufRendererAdapter {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        draw_mouse_cursor_callback: &dyn Fn(&mut dyn ItemRenderer),
    ) -> Result<DrawOutcome, PlatformError> {
        let (index, texture) = self.display.back_buffer();
        self.barriers.acquire(index, texture)?;
        let drawn = self
            .renderer
            .render_to_texture_transformed(
                texture,
                rotation.degrees(),
                rotation.translation_after_rotation(self.size),
                Some(&|item_renderer| {
                    draw_mouse_cursor_callback(item_renderer);
                }),
            )
            .and_then(|()| self.barriers.release(index, texture));

        // KMS has no way to wait on the render: wgpu-hal exports no sync fd for
        // the plane's IN_FENCE_FD property, and wgpu won't take a signal semaphore
        // for a submission. Block until the GPU is done instead, at the cost of a
        // pipeline stall per frame. A failed frame waits too: the next one
        // re-records the barriers it submitted.
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
