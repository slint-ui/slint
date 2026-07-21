// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A [`SlintWindowRenderer`](crate::SlintWindowRenderer) that records all
//! drawing commands into an [`anyrender::recording::Scene`] without
//! rasterizing.
//!
//! Used by `test-driver-screenshots` to capture the command stream
//! produced by [`AnyrenderItemRenderer`](crate::AnyrenderItemRenderer)
//! for golden snapshot tests.

use std::sync::Arc;

use anyrender::WindowHandle;
use anyrender::recording::Scene;
use i_slint_core::platform::PlatformError;

use crate::{AnyrenderSlintRenderer, SlintWindowRenderer};

/// Records every drawing command emitted by the renderer into an
/// [`anyrender::recording::Scene`]. Designed for tests; does no
/// rasterization and has no platform dependencies.
pub struct RecordingWindowRenderer {
    scene: Scene,
    width: u32,
    height: u32,
}

impl RecordingWindowRenderer {
    pub fn new() -> Self {
        Self { scene: Scene::default(), width: 0, height: 0 }
    }

    /// Take ownership of the recorded scene, leaving an empty one in place.
    pub fn take_scene(&mut self) -> Scene {
        std::mem::take(&mut self.scene)
    }

    /// Borrow the recorded scene without consuming it.
    pub fn scene(&self) -> &Scene {
        &self.scene
    }
}

impl Default for RecordingWindowRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl anyrender::RenderContext for RecordingWindowRenderer {}

impl anyrender::WindowRenderer for RecordingWindowRenderer {
    type ScenePainter<'a>
        = Scene
    where
        Self: 'a;

    fn resume<F: FnOnce() + 'static>(
        &mut self,
        _window: Arc<dyn WindowHandle>,
        width: u32,
        height: u32,
        on_ready: F,
    ) {
        self.width = width;
        self.height = height;
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
        self.width = width;
        self.height = height;
    }

    fn render<F: FnOnce(&mut Self::ScenePainter<'_>)>(&mut self, draw_fn: F) {
        self.scene = Scene::default();
        draw_fn(&mut self.scene);
    }
}

impl SlintWindowRenderer for RecordingWindowRenderer {
    fn slint_render<F>(
        &mut self,
        _surface_size: i_slint_core::api::PhysicalSize,
        _base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
        draw: F,
    ) -> Result<(), PlatformError>
    where
        F: FnOnce(&mut Self::ScenePainter<'_>) -> Result<(), PlatformError>,
    {
        // Reset so each frame's recording is independent of the previous one.
        self.scene = Scene::default();
        draw(&mut self.scene)
    }

    fn slint_set_size(&mut self, width: u32, height: u32) -> Result<(), PlatformError> {
        self.width = width;
        self.height = height;
        Ok(())
    }
}

impl AnyrenderSlintRenderer<RecordingWindowRenderer> {
    pub fn new_recording() -> Self {
        Self::with_window_renderer(RecordingWindowRenderer::new())
    }

    /// Drive a single render pass and return the recorded scene.
    pub fn record(&self) -> Result<Scene, PlatformError> {
        self.render()?;
        Ok(self.window_renderer().take_scene())
    }
}
