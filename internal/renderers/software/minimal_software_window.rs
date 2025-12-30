// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{RepaintBufferType, SoftwareRenderer};
use alloc::rc::{Rc, Weak};
use core::cell::Cell;
use i_slint_core::api::Window;
use i_slint_core::platform::Renderer;
use i_slint_core::window::WindowAdapter;

/// This is a minimal adapter for a Window that doesn't have any other feature than rendering
/// using the software renderer.
pub struct MinimalSoftwareWindow {
    window: Window,
    renderer: SoftwareRenderer,
    needs_redraw: Cell<bool>,
    size: Cell<i_slint_core::api::PhysicalSize>,
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
        if self.needs_redraw.replace(false)
            || self.renderer.rendering_metrics_collector.as_ref().is_some_and(|m| m.refresh_mode() == i_slint_core::graphics::rendering_metrics_collector::RefreshMode::FullSpeed)
        {
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
    pub fn set_size(&self, size: impl Into<i_slint_core::api::WindowSize>) {
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

    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.size.get()
    }
    fn set_size(&self, size: i_slint_core::api::WindowSize) {
        let sf = self.window.scale_factor();
        self.size.set(size.to_physical(sf));
        let logical_size = size.to_logical(sf);
        self.window
            .dispatch_event(i_slint_core::platform::WindowEvent::Resized { size: logical_size });
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

#[test]
fn test_empty_window() {
    // Test that when creating an empty window without a component, we don't panic when render() is called.
    // This isn't typically done intentionally, but for example if we receive a paint event in Qt before a component
    // is set, this may happen. Concretely as per #2799 this could happen with popups where the call to
    // QWidget::show() with egl delivers an immediate paint event, before we've had a chance to call set_component.
    // Let's emulate this scenario here using public platform API.

    let msw = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    msw.window().request_redraw();
    let mut region = None;
    let render_called = msw.draw_if_needed(|renderer| {
        let mut buffer = i_slint_core::graphics::SharedPixelBuffer::<
            i_slint_core::graphics::Rgb8Pixel,
        >::new(100, 100);
        let stride = buffer.width() as usize;
        region = Some(renderer.render(buffer.make_mut_slice(), stride));
    });
    assert!(render_called);
    let region = region.unwrap();
    assert_eq!(region.bounding_box_size(), i_slint_core::api::PhysicalSize::default());
    assert_eq!(region.bounding_box_origin(), i_slint_core::api::PhysicalPosition::default());
}
