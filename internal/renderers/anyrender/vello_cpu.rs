// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! [`SlintWindowRenderer`] implementation rasterizing on the CPU with
//! [`vello_cpu`].
//!
//! Unlike the vello and vello_hybrid backends this one does not present
//! anything itself: frames are rasterized into an RGBA8 buffer that the
//! windowing backend copies to the window (softbuffer in the winit backend),
//! the same division of labor as the software renderer.
//!
//! This is also the fallback the vello backend falls back to when no GPU is
//! available.

use anyrender::{PaintScene, WindowRenderer};
use anyrender_vello_cpu::VelloCpuScenePainter;
use i_slint_core::api::PhysicalSize;
use i_slint_core::graphics::{Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::platform::PlatformError;
use vello_cpu::{RenderContext, RenderMode, Resources};

use crate::{AnyrenderSlintRenderer, SlintWindowRenderer};

/// vello_cpu addresses pixels with 16 bit coordinates.
const MAX_DIMENSION: u32 = u16::MAX as u32;

/// Rasterizes Slint's scene on the CPU into an RGBA8 buffer.
pub struct VelloCpuWindowRenderer {
    painter: VelloCpuScenePainter,
    /// The rasterized frame, RGBA8 with premultiplied alpha.
    buffer: Vec<u8>,
    width: u32,
    height: u32,
}

impl VelloCpuWindowRenderer {
    pub fn new() -> Self {
        Self::with_size(1, 1)
    }

    fn with_size(width: u32, height: u32) -> Self {
        let (width, height) = clamp_size(width, height);
        Self {
            painter: VelloCpuScenePainter {
                render_ctx: RenderContext::new(width as u16, height as u16),
                resources: Resources::new(),
            },
            buffer: vec![0; width as usize * height as usize * 4],
            width,
            height,
        }
    }

    /// The last rasterized frame as RGBA8 with premultiplied alpha, together
    /// with its dimensions. Rows are tightly packed.
    pub fn frame_buffer(&self) -> (&[u8], u32, u32) {
        (&self.buffer, self.width, self.height)
    }

    fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = clamp_size(width, height);
        if self.width == width && self.height == height {
            return;
        }
        self.painter.render_ctx = RenderContext::new(width as u16, height as u16);
        self.buffer.resize(width as usize * height as usize * 4, 0);
        self.width = width;
        self.height = height;
    }

    /// Record the scene through `draw` and rasterize it into `buffer`.
    fn render_scene(
        painter: &mut VelloCpuScenePainter,
        buffer: &mut [u8],
        width: u32,
        height: u32,
        base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
        draw: impl FnOnce(&mut VelloCpuScenePainter) -> Result<(), PlatformError>,
    ) -> Result<(), PlatformError> {
        painter.reset();

        // vello_cpu starts from a transparent surface and, unlike vello, takes
        // no base color, so paint the window background as the first command.
        painter.fill(
            peniko::Fill::default(),
            kurbo::Affine::IDENTITY,
            peniko::BrushRef::Solid(base_color),
            None,
            &kurbo::Rect::new(0., 0., width as f64, height as f64),
        );

        draw(painter)?;

        painter.render_ctx.flush();
        painter.render_ctx.render_to_buffer(
            &mut painter.resources,
            buffer,
            width as u16,
            height as u16,
            RenderMode::OptimizeSpeed,
        );
        Ok(())
    }
}

fn clamp_size(width: u32, height: u32) -> (u32, u32) {
    (width.clamp(1, MAX_DIMENSION), height.clamp(1, MAX_DIMENSION))
}

impl Default for VelloCpuWindowRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl anyrender::RenderContext for VelloCpuWindowRenderer {}

impl WindowRenderer for VelloCpuWindowRenderer {
    type ScenePainter<'a>
        = VelloCpuScenePainter
    where
        Self: 'a;

    fn resume<F: FnOnce() + 'static>(
        &mut self,
        _window: std::sync::Arc<dyn anyrender::WindowHandle>,
        width: u32,
        height: u32,
        on_ready: F,
    ) {
        self.resize(width, height);
        on_ready();
    }

    fn complete_resume(&mut self) -> bool {
        true
    }

    fn suspend(&mut self) {}

    /// Rasterization needs no window, so the renderer is always ready.
    fn is_active(&self) -> bool {
        true
    }

    fn set_size(&mut self, width: u32, height: u32) {
        self.resize(width, height);
    }

    fn render<F: FnOnce(&mut Self::ScenePainter<'_>)>(&mut self, draw_fn: F) {
        let _ = self.slint_render(
            PhysicalSize::new(self.width, self.height),
            peniko::color::palette::css::WHITE,
            |scene| {
                draw_fn(scene);
                Ok(())
            },
        );
    }
}

impl SlintWindowRenderer for VelloCpuWindowRenderer {
    fn slint_render<F>(
        &mut self,
        surface_size: PhysicalSize,
        base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
        draw: F,
    ) -> Result<(), PlatformError>
    where
        F: FnOnce(&mut Self::ScenePainter<'_>) -> Result<(), PlatformError>,
    {
        self.resize(surface_size.width, surface_size.height);

        let Self { painter, buffer, width, height } = self;
        Self::render_scene(painter, buffer, *width, *height, base_color, draw)
    }

    fn slint_set_size(&mut self, width: u32, height: u32) -> Result<(), PlatformError> {
        self.resize(width, height);
        Ok(())
    }

    fn slint_take_snapshot<F>(
        &mut self,
        surface_size: PhysicalSize,
        base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
        draw: F,
    ) -> Result<SharedPixelBuffer<Rgba8Pixel>, PlatformError>
    where
        F: FnOnce(&mut Self::ScenePainter<'_>) -> Result<(), PlatformError>,
    {
        let (width, height) = clamp_size(surface_size.width, surface_size.height);

        // A snapshot may have a different size than the window, so rasterize
        // it separately instead of disturbing the window's own context.
        let mut painter = VelloCpuScenePainter {
            render_ctx: RenderContext::new(width as u16, height as u16),
            resources: Resources::new(),
        };
        let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
        Self::render_scene(
            &mut painter,
            pixels.make_mut_bytes(),
            width,
            height,
            base_color,
            draw,
        )?;

        crate::unpremultiply_rgba(pixels.make_mut_bytes());
        Ok(pixels)
    }
}

impl AnyrenderSlintRenderer<VelloCpuWindowRenderer> {
    /// Create a Slint renderer that rasterizes on the CPU with vello_cpu.
    pub fn new_vello_cpu() -> Self {
        Self::with_window_renderer(VelloCpuWindowRenderer::new())
    }

    /// Resize the buffer frames are rasterized into.
    pub fn set_surface_size(&self, width: u32, height: u32) {
        self.window_renderer().resize(width, height);
    }

    /// The last rasterized frame, see
    /// [`VelloCpuWindowRenderer::frame_buffer`]. The closure form keeps the
    /// borrow of the window renderer contained.
    pub fn with_frame_buffer<R>(&self, callback: impl FnOnce(&[u8], u32, u32) -> R) -> R {
        let window_renderer = self.window_renderer();
        let (buffer, width, height) = window_renderer.frame_buffer();
        callback(buffer, width, height)
    }
}
