// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{RepaintBufferType, SoftwareRenderer};
use crate::api::Window;
use crate::platform::Renderer;
use crate::window::WindowAdapter;
use alloc::rc::{Rc, Weak};
use core::cell::Cell;

/// This is a minimal adapter for a Window that doesn't have any other feature than rendering
/// using the software renderer.
pub struct MinimalSoftwareWindow {
    window: Window,
    renderer: SoftwareRenderer,
    needs_redraw: Cell<bool>,
    size: Cell<crate::api::PhysicalSize>,
}

impl MinimalSoftwareWindow {
    /// Instantiate a new MinimalWindowAdaptor
    ///
    /// The `repaint_buffer_type` parameter specify what kind of buffer are passed to the [`SoftwareRenderer`]
    pub fn new(repaint_buffer_type: RepaintBufferType) -> Rc<Self> {
        Rc::new_cyclic(|w: &Weak<Self>| Self {
            window: Window::new(w.clone()),
            renderer: SoftwareRenderer::new_with_repaint_buffer_type(repaint_buffer_type),
            needs_redraw: Default::default(),
            size: Default::default(),
        })
    }
    /// If the window needs to be redrawn, the callback will be called with the
    /// [renderer](SoftwareRenderer) that should be used to do the drawing.
    ///
    /// [`SoftwareRenderer::render()`] or [`SoftwareRenderer::render_by_line()`] should be called
    /// in that callback.
    ///
    /// Return true if something was redrawn.
    pub fn draw_if_needed(&self, render_callback: impl FnOnce(&SoftwareRenderer)) -> bool {
        if self.needs_redraw.replace(false) || self.renderer.rendering_metrics_collector.is_some() {
            render_callback(&self.renderer);
            true
        } else {
            false
        }
    }

    #[cfg(feature = "experimental")]
    /// If the window needs to be redrawn, the callback will be called with the
    /// [renderer](SoftwareRenderer) that should be used to do the drawing.
    ///
    /// [`SoftwareRenderer::render()`] or [`SoftwareRenderer::render_by_line()`] should be called
    /// in that callback.
    ///
    /// Return true if something was redrawn.
    pub async fn draw_async_if_needed(
        &self,
        render_callback: impl AsyncFnOnce(&SoftwareRenderer),
    ) -> bool {
        if self.needs_redraw.replace(false) || self.renderer.rendering_metrics_collector.is_some() {
            render_callback(&self.renderer).await;
            true
        } else {
            false
        }
    }

    #[doc(hidden)]
    /// Forward to the window through Deref
    /// (Before 1.1, WindowAdapter didn't have set_size, so the one from Deref was used.
    /// But in Slint 1.1, if one had imported the WindowAdapter trait, the other one would be found)
    pub fn set_size(&self, size: impl Into<crate::api::WindowSize>) {
        self.window.set_size(size);
    }
}

impl WindowAdapter for MinimalSoftwareWindow {
    fn window(&self) -> &Window {
        &self.window
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn size(&self) -> crate::api::PhysicalSize {
        self.size.get()
    }
    fn set_size(&self, size: crate::api::WindowSize) {
        let sf = self.window.scale_factor();
        self.size.set(size.to_physical(sf));
        self.window
            .dispatch_event(crate::platform::WindowEvent::Resized { size: size.to_logical(sf) })
    }

    fn request_redraw(&self) {
        self.needs_redraw.set(true);
    }
}

impl core::ops::Deref for MinimalSoftwareWindow {
    type Target = Window;
    fn deref(&self) -> &Self::Target {
        &self.window
    }
}
