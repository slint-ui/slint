// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::{cell::RefCell, rc::Rc};

use i_slint_core::Color;

use crate::glwindow::GLWindow;

use self::itemrenderer::CanvasRc;

pub mod fonts;
mod images;
pub mod itemrenderer;

pub struct FemtoVGRenderer {
    canvas: CanvasRc,
    graphics_cache: itemrenderer::ItemGraphicsCache,
    texture_cache: RefCell<images::TextureCache>,
}

impl FemtoVGRenderer {
    pub fn new(canvas: CanvasRc) -> Self {
        Self { canvas, graphics_cache: Default::default(), texture_cache: Default::default() }
    }

    pub fn release_graphics_resources(&self) {
        self.graphics_cache.clear_all();
        self.texture_cache.borrow_mut().clear();
    }

    pub fn component_destroyed(&self, component: i_slint_core::component::ComponentRef) {
        self.graphics_cache.component_destroyed(component)
    }

    pub fn render(
        &self,
        width: u32,
        height: u32,
        scale_factor: f32,
        clear_color: &Color,
        window: &Rc<GLWindow>,
        render_callback: &mut dyn FnMut(&mut dyn i_slint_core::item_rendering::ItemRenderer),
    ) {
        {
            let mut canvas = self.canvas.as_ref().borrow_mut();
            // We pass 1.0 as dpi / device pixel ratio as femtovg only uses this factor to scale
            // text metrics. Since we do the entire translation from logical pixels to physical
            // pixels on our end, we don't need femtovg to scale a second time.
            canvas.set_size(width, height, 1.0);
            canvas.clear_rect(
                0,
                0,
                width,
                height,
                self::itemrenderer::to_femtovg_color(&clear_color),
            );
            // For the BeforeRendering rendering notifier callback it's important that this happens *after* clearing
            // the back buffer, in order to allow the callback to provide its own rendering of the background.
            // femtovg's clear_rect() will merely schedule a clear call, so flush right away to make it immediate.
            canvas.flush();
            canvas.set_size(width, height, 1.0);
        }

        let mut item_renderer =
            self::itemrenderer::GLItemRenderer::new(&self, &window, scale_factor, width, height);

        render_callback(&mut item_renderer);

        self.canvas.borrow_mut().flush();

        // Delete any images and layer images (and their FBOs) before making the context not current anymore, to
        // avoid GPU memory leaks.
        self.texture_cache.borrow_mut().drain();
        drop(item_renderer);
    }
}

impl Drop for FemtoVGRenderer {
    fn drop(&mut self) {
        if Rc::strong_count(&self.canvas) != 1 {
            i_slint_core::debug_log!("internal warning: there are canvas references left when destroying the window. OpenGL resources will be leaked.")
        }
    }
}
