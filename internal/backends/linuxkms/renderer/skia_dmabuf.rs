// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Skia on wgpu for drivers without `VK_EXT_acquire_drm_display`.
//!
//! The wgpu DRM surface target hands the display to Vulkan, which needs
//! `VK_EXT_acquire_drm_display`; few drivers have it. This path renders into a
//! GBM buffer imported as a wgpu texture instead, and posts it with a DRM page
//! flip. See [`crate::display::gbmdmabufdisplay`].

// cSpell: ignore dmabuf

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;
use i_slint_renderer_skia::SkiaWGPU30Renderer;
use wgpu_30 as wgpu;

use super::dmabuf::{init_wgpu, wait_for_gpu};
use crate::display::RenderingRotation;
use crate::display::gbmdmabufdisplay::GbmDmabufDisplay;
use crate::drmoutput::DrmOutput;

pub struct SkiaDmabufRendererAdapter {
    renderer: SkiaWGPU30Renderer,
    display: GbmDmabufDisplay,
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

        let renderer = SkiaWGPU30Renderer::new(instance, adapter, device.clone(), queue)?;

        eprintln!("Using Skia renderer with wgpu, presenting dma-bufs on a DRM plane");

        Ok(Box::new(Self { renderer, display, device, size }))
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
        // The buffer to draw into is two flips old, so the frame can be drawn
        // while the previous one is still on its way to the screen.
        // `GbmDmabufDisplay::present` waits for that flip.
        self.renderer.render_to_scanout_texture(
            self.display.back_buffer(),
            rotation.degrees(),
            rotation.translation_after_rotation(self.size),
            Some(&|item_renderer| {
                draw_mouse_cursor_callback(item_renderer);
            }),
        )?;

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
