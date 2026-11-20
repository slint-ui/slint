// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A [`SlintWindowRenderer`] that rasterizes with vello_cpu into an offscreen buffer.

// cSpell: ignore winsys

use std::sync::Arc;

use anyrender::{ImageRenderer, PaintScene, WindowHandle};
use anyrender_vello_cpu::{VelloCpuImageRenderer, VelloCpuScenePainter};
use i_slint_core::graphics::{Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;

use crate::{AnyrenderSlintRenderer, SlintWindowRenderer};

/// Renders into a CPU buffer with vello_cpu, without a window surface.
/// Read the result with [`Window::take_snapshot`](i_slint_core::api::Window::take_snapshot).
pub struct VelloCpuWindowRenderer {
    renderer: VelloCpuImageRenderer,
    width: u32,
    height: u32,
}

impl VelloCpuWindowRenderer {
    pub fn new() -> Self {
        Self { renderer: VelloCpuImageRenderer::new(1, 1), width: 1, height: 1 }
    }

    fn render_to_buffer<F>(
        &mut self,
        surface_size: i_slint_core::api::PhysicalSize,
        base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
        draw: F,
    ) -> Result<Vec<u8>, PlatformError>
    where
        F: FnOnce(&mut VelloCpuScenePainter) -> Result<(), PlatformError>,
    {
        self.slint_set_size(surface_size.width, surface_size.height)?;
        self.renderer.reset();
        let (width, height) = (self.width, self.height);

        let mut result = Ok(());
        let mut buffer = Vec::new();
        self.renderer.render_to_vec(
            |painter| {
                painter.fill(
                    peniko::Fill::NonZero,
                    kurbo::Affine::IDENTITY,
                    peniko::BrushRef::Solid(base_color),
                    None,
                    &kurbo::Rect::new(0., 0., width as f64, height as f64),
                );
                result = draw(painter);
            },
            &mut buffer,
        );
        result.map(|()| buffer)
    }
}

impl Default for VelloCpuWindowRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl anyrender::RenderContext for VelloCpuWindowRenderer {}

impl anyrender::WindowRenderer for VelloCpuWindowRenderer {
    type ScenePainter<'a>
        = VelloCpuScenePainter
    where
        Self: 'a;

    fn resume<F: FnOnce() + 'static>(
        &mut self,
        _window: Arc<dyn WindowHandle>,
        width: u32,
        height: u32,
        on_ready: F,
    ) {
        anyrender::WindowRenderer::set_size(self, width, height);
        on_ready();
    }

    fn complete_resume(&mut self) -> bool {
        true
    }

    fn suspend(&mut self) {}

    fn is_active(&self) -> bool {
        true
    }

    fn set_size(&mut self, width: u32, height: u32) {
        let _ = self.slint_set_size(width, height);
    }

    fn render<F: FnOnce(&mut Self::ScenePainter<'_>)>(&mut self, draw_fn: F) {
        let size = i_slint_core::api::PhysicalSize::new(self.width, self.height);
        let _ = self.render_to_buffer(size, peniko::color::palette::css::WHITE, |painter| {
            draw_fn(painter);
            Ok(())
        });
    }
}

impl SlintWindowRenderer for VelloCpuWindowRenderer {
    fn slint_render<F>(
        &mut self,
        surface_size: i_slint_core::api::PhysicalSize,
        base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
        _notify: &mut dyn FnMut(i_slint_core::api::RenderingState),
        draw: F,
    ) -> Result<DrawOutcome, PlatformError>
    where
        F: FnOnce(&mut Self::ScenePainter<'_>) -> Result<(), PlatformError>,
    {
        self.render_to_buffer(surface_size, base_color, draw)?;
        Ok(DrawOutcome::Success)
    }

    fn slint_set_size(&mut self, width: u32, height: u32) -> Result<(), PlatformError> {
        // vello_cpu sizes its render targets with u16.
        let width = width.clamp(1, u16::MAX as u32);
        let height = height.clamp(1, u16::MAX as u32);
        if (width, height) != (self.width, self.height) {
            self.renderer.resize(width, height);
            self.width = width;
            self.height = height;
        }
        Ok(())
    }

    fn slint_take_snapshot<F>(
        &mut self,
        surface_size: i_slint_core::api::PhysicalSize,
        base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
        draw: F,
    ) -> Result<SharedPixelBuffer<Rgba8Pixel>, PlatformError>
    where
        F: FnOnce(&mut Self::ScenePainter<'_>) -> Result<(), PlatformError>,
    {
        let mut buffer = self.render_to_buffer(surface_size, base_color, draw)?;
        // vello_cpu writes premultiplied alpha; snapshots are straight alpha.
        for pixel in buffer.as_chunks_mut::<4>().0 {
            let alpha = pixel[3] as u32;
            if alpha > 0 && alpha < 255 {
                for channel in &mut pixel[..3] {
                    *channel = (*channel as u32 * 255 / alpha).min(255) as u8;
                }
            }
        }
        let mut snapshot = SharedPixelBuffer::<Rgba8Pixel>::new(self.width, self.height);
        snapshot.make_mut_bytes().copy_from_slice(&buffer);
        Ok(snapshot)
    }

    fn winsys_info(&self) -> String {
        "vello_cpu renderer".into()
    }
}

/// A Slint renderer rasterizing with vello_cpu into an offscreen buffer.
pub type VelloCpuRenderer = AnyrenderSlintRenderer<VelloCpuWindowRenderer>;

impl VelloCpuRenderer {
    pub fn new_vello_cpu() -> Self {
        Self::with_window_renderer(VelloCpuWindowRenderer::new())
    }
}
