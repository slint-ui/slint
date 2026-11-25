// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Skia Graphite on wgpu. Experimental.
//!
//! It works like the adapter in `skia_dmabuf.rs`, except that Graphite draws through wgpu,
//! see [`ScanoutBarriers`].
//! Graphite has no surface for a DRM plane yet, so it scans out dma-bufs on every driver.

// cSpell: ignore bufs dmabuf

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;
use i_slint_renderer_skia::SkiaGraphiteWGPU30Renderer;
use wgpu_30 as wgpu;

use super::dmabuf::init_wgpu;
use crate::display::RenderingRotation;
use crate::display::gbmdmabufdisplay::GbmDmabufDisplay;
use crate::display::scanout_barriers::ScanoutBarriers;
use crate::drmoutput::DrmOutput;

/// The scanout buffers [`GbmDmabufDisplay::present_after`] needs.
const BUFFER_COUNT: usize = 4;

pub struct SkiaGraphiteDmabufRendererAdapter {
    /// Dropped first, see `SkiaDmabufRendererAdapter`.
    barriers: ScanoutBarriers,
    renderer: SkiaGraphiteWGPU30Renderer,
    display: GbmDmabufDisplay,
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: PhysicalWindowSize,
}

impl SkiaGraphiteDmabufRendererAdapter {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        drm_output: DrmOutput,
        requested_graphics_api: Option<&RequestedGraphicsAPI>,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        let (instance, _adapter, device, queue) = init_wgpu(requested_graphics_api)?;

        let (width, height) = drm_output.size();
        let size = PhysicalWindowSize::new(width, height);

        let display = GbmDmabufDisplay::new(drm_output, &device, BUFFER_COUNT)?;
        let barriers = ScanoutBarriers::new(&device, BUFFER_COUNT)?;

        let renderer = SkiaGraphiteWGPU30Renderer::new(instance, device.clone(), queue.clone())?;

        eprintln!("Using Skia Graphite renderer with wgpu, presenting dma-bufs on a DRM plane");

        Ok(Box::new(Self { renderer, display, barriers, device, queue, size }))
    }
}

impl crate::fullscreenwindowadapter::FullscreenRenderer for SkiaGraphiteDmabufRendererAdapter {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn render_and_present(
        &self,
        rotation: RenderingRotation,
    ) -> Result<DrawOutcome, PlatformError> {
        let (index, texture) = self.display.back_buffer();
        let drawn = self
            .barriers
            .acquire(index, texture)
            .and_then(|()| {
                self.renderer.render_to_texture_transformed(
                    texture,
                    rotation.degrees(),
                    rotation.translation_after_rotation(self.size),
                )
            })
            .and_then(|()| {
                let export_sync_file = self.barriers.exports_sync_files()
                    && self.display.drm_output.takes_sync_files();
                self.barriers.release(index, texture, export_sync_file)
            });

        let sync_file = match drawn {
            Ok(sync_file) => sync_file,
            Err(err) => {
                if self.barriers.has_stopped() {
                    return Err(err);
                }
                if let Err(wait_err) = self.barriers.wait_after_failure() {
                    return Err(format!("{err}. {wait_err}").into());
                }
                if let Err(e) = self.display.flush(&self.device) {
                    eprintln!("Error presenting the frame before a failed one: {e}");
                }
                return Err(err);
            }
        };

        let submit = || self.queue.submit([]);
        self.display
            .present_after(&self.device, submit, sync_file)
            .map_err(|e| format!("Error presenting dma-buf: {e}"))?;
        Ok(DrawOutcome::Success)
    }

    fn flush_pending_frame(&self) -> Result<(), PlatformError> {
        if self.barriers.has_stopped() {
            return Ok(());
        }
        self.display
            .flush(&self.device)
            .map_err(|e| format!("Error presenting dma-buf: {e}").into())
    }

    fn size(&self) -> PhysicalWindowSize {
        self.size
    }
}
