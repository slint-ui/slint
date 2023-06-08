// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! This module contains the window adapter implementation to communicate between Slint and Vulkan + libinput

use std::cell::Cell;
use std::rc::Rc;

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::{platform::PlatformError, window::WindowAdapter};

pub trait Renderer {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer;
    fn render_and_present(&self, window: &i_slint_core::api::Window) -> Result<(), PlatformError>;
    fn size(&self) -> PhysicalWindowSize;
}

pub struct FullscreenWindowAdapter {
    window: i_slint_core::api::Window,
    renderer: Box<dyn Renderer>,
    needs_redraw: Cell<bool>,
}

impl WindowAdapter for FullscreenWindowAdapter {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }
}

impl i_slint_core::window::WindowAdapterSealed for FullscreenWindowAdapter {
    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.renderer.size()
    }

    fn renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        self.renderer.as_core_renderer()
    }

    fn request_redraw(&self) {
        self.needs_redraw.set(true)
    }
}

impl FullscreenWindowAdapter {
    pub fn new(
        renderer: Box<dyn Renderer>,
    ) -> Result<Rc<Self>, PlatformError> {        
        Ok(Rc::<FullscreenWindowAdapter>::new_cyclic(|self_weak| FullscreenWindowAdapter {
            window: i_slint_core::api::Window::new(self_weak.clone()),
            renderer,
            needs_redraw: Cell::new(true),
        }))
    }

    pub fn render_if_needed(&self) -> Result<(), PlatformError> {
        if self.needs_redraw.replace(false) {
            self.renderer.render_and_present(&self.window)?;
        }
        Ok(())
    }
}
