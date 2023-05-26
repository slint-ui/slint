// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! This module contains the window adapter implementation to communicate between Slint and Vulkan + libinput

use std::cell::Cell;
use std::rc::Rc;

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::{platform::PlatformError, window::WindowAdapter};
use i_slint_renderer_skia::SkiaRenderer;

mod egldisplay;
mod vulkandisplay;

type PresentFn = Box<dyn Fn() -> Result<(), PlatformError>>;

pub struct SkiaWindowAdapter {
    window: i_slint_core::api::Window,
    renderer: SkiaRenderer,
    needs_redraw: Cell<bool>,
    size: PhysicalWindowSize,
    present_fn: PresentFn, // last field that keeps the gbm surface alive, that needs to outlive the renderer.
}

impl WindowAdapter for SkiaWindowAdapter {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }
}

impl i_slint_core::window::WindowAdapterSealed for SkiaWindowAdapter {
    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.size
    }

    fn renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn request_redraw(&self) {
        self.needs_redraw.set(true)
    }
}

impl SkiaWindowAdapter {
    pub fn new() -> Result<Rc<Self>, PlatformError> {
        let (renderer, size, present_fn) = vulkandisplay::create_skia_renderer_with_vulkan()
            .or_else(|_| egldisplay::create_skia_renderer_with_egl())?;

        Ok(Rc::<SkiaWindowAdapter>::new_cyclic(|self_weak| SkiaWindowAdapter {
            window: i_slint_core::api::Window::new(self_weak.clone()),
            renderer,
            present_fn,
            needs_redraw: Cell::new(true),
            size,
        }))
    }

    pub fn render_if_needed(&self) -> Result<(), PlatformError> {
        if self.needs_redraw.replace(false) {
            self.renderer.render(&self.window)?;
            (self.present_fn)()?;
        }
        Ok(())
    }
}
