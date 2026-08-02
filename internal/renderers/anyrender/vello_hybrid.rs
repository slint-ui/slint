// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! [`SlintWindowRenderer`] implementation rendering through [`vello_hybrid`]
//! on WebGL2, for the browser.
//!
//! vello_hybrid prepares the geometry on the CPU and leaves the GPU only
//! rendering and compositing through ordinary vertex and fragment shaders.
//! That fits WebGL2, which classic vello - built on compute shaders - cannot
//! use, and it means no WGPU is involved on this path at all: the renderer
//! draws into the WebGL2 context of the window's canvas.

use anyrender::{PaintScene, WindowRenderer};
use anyrender_vello_hybrid::{WebGlImageManager, WebGlScenePainter};
use i_slint_core::api::PhysicalSize;
use i_slint_core::graphics::{Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::platform::PlatformError;
use rustc_hash::FxHashMap;
use web_sys::HtmlCanvasElement;

use crate::{AnyrenderSlintRenderer, SlintWindowRenderer};

/// vello_hybrid addresses pixels with 16 bit coordinates.
const MAX_DIMENSION: u32 = u16::MAX as u32;

/// The WebGL2 renderer of a window that has a canvas. Dropped when the window
/// is suspended.
struct ActiveState {
    renderer: vello_hybrid::WebGlRenderer,
    resources: vello_hybrid::Resources,
    /// Maps the id of a [`peniko::Blob`] to the image uploaded to the atlas,
    /// so images are uploaded once instead of per frame.
    image_cache: FxHashMap<u64, vello_common::paint::ImageId>,
    canvas: HtmlCanvasElement,
}

/// Renders Slint's scene with vello_hybrid into a canvas' WebGL2 context.
pub struct VelloHybridWindowRenderer {
    state: Option<ActiveState>,
    scene: vello_hybrid::Scene,
    width: u32,
    height: u32,
}

impl VelloHybridWindowRenderer {
    pub fn new() -> Self {
        let (width, height) = clamp_size(1, 1);
        Self { state: None, scene: vello_hybrid::Scene::new(width as u16, height as u16), width, height }
    }

    /// Attach the renderer to `canvas` and render frames of `width` x `height`
    /// physical pixels into its WebGL2 context.
    pub fn set_canvas(
        &mut self,
        canvas: HtmlCanvasElement,
        width: u32,
        height: u32,
    ) -> Result<(), PlatformError> {
        let renderer = vello_hybrid::WebGlRenderer::new(&canvas);
        self.state = Some(ActiveState {
            renderer,
            resources: vello_hybrid::Resources::new(),
            image_cache: FxHashMap::default(),
            canvas,
        });
        self.resize(width, height);
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = clamp_size(width, height);
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        self.scene = vello_hybrid::Scene::new(width as u16, height as u16);

        // The renderer asserts that the render size matches the drawing
        // buffer, which follows the canvas attributes rather than its CSS size.
        if let Some(state) = &self.state {
            state.canvas.set_width(width);
            state.canvas.set_height(height);
        }
    }
}

fn clamp_size(width: u32, height: u32) -> (u32, u32) {
    (width.clamp(1, MAX_DIMENSION), height.clamp(1, MAX_DIMENSION))
}

impl Default for VelloHybridWindowRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl anyrender::RenderContext for VelloHybridWindowRenderer {}

impl WindowRenderer for VelloHybridWindowRenderer {
    type ScenePainter<'a>
        = WebGlScenePainter<'a>
    where
        Self: 'a;

    /// The canvas is not reachable through the raw window handle, so the
    /// windowing backend attaches it with [`Self::set_canvas`] instead.
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
        self.state.is_some()
    }

    fn suspend(&mut self) {
        self.state = None;
    }

    fn is_active(&self) -> bool {
        self.state.is_some()
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

impl SlintWindowRenderer for VelloHybridWindowRenderer {
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

        let Self { state, scene, width, height } = self;
        let Some(ActiveState { renderer, resources, image_cache, .. }) = state else {
            return Ok(());
        };

        scene.reset();

        {
            let mut painter = WebGlScenePainter::new(
                scene,
                WebGlImageManager::new(renderer, resources, image_cache),
            );

            // vello_hybrid starts from a transparent surface and takes no base
            // color, so paint the window background as the first command.
            painter.fill(
                peniko::Fill::default(),
                kurbo::Affine::IDENTITY,
                peniko::BrushRef::Solid(base_color),
                None,
                &kurbo::Rect::new(0., 0., *width as f64, *height as f64),
            );

            draw(&mut painter)?;
        }

        renderer
            .render(
                scene,
                resources,
                &vello_hybrid::RenderSize { width: *width, height: *height },
            )
            .map_err(|e| PlatformError::from(format!("Error rendering with vello_hybrid: {e:?}")))
    }

    fn slint_set_size(&mut self, width: u32, height: u32) -> Result<(), PlatformError> {
        self.resize(width, height);
        Ok(())
    }

    fn slint_take_snapshot<F>(
        &mut self,
        _surface_size: PhysicalSize,
        _base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
        _draw: F,
    ) -> Result<SharedPixelBuffer<Rgba8Pixel>, PlatformError>
    where
        F: FnOnce(&mut Self::ScenePainter<'_>) -> Result<(), PlatformError>,
    {
        Err("take_snapshot is not supported by the vello_hybrid renderer".into())
    }
}

impl AnyrenderSlintRenderer<VelloHybridWindowRenderer> {
    /// Create a Slint renderer that renders through vello_hybrid on WebGL2.
    /// It starts out without a canvas; call [`Self::set_canvas`] to attach one.
    pub fn new_vello_hybrid() -> Self {
        Self::with_window_renderer(VelloHybridWindowRenderer::new())
    }

    /// Render into the WebGL2 context of `canvas`, see
    /// [`VelloHybridWindowRenderer::set_canvas`].
    pub fn set_canvas(
        &self,
        canvas: HtmlCanvasElement,
        width: u32,
        height: u32,
    ) -> Result<(), PlatformError> {
        self.window_renderer().set_canvas(canvas, width, height)
    }

    /// Detach the canvas, for example when the window goes away.
    pub fn clear_canvas(&self) {
        self.window_renderer().suspend();
    }
}
