// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::cell::RefCell;

pub mod fonts;
mod images;
pub mod itemrenderer;

#[derive(Default)]
pub struct FemtoVGRenderer {
    texture_cache: RefCell<images::TextureCache>,
}

impl FemtoVGRenderer {
    pub fn release_graphics_resources(&self) {
        self.texture_cache.borrow_mut().clear();
    }

    pub fn finish(&self, item_renderer: itemrenderer::GLItemRenderer) {
        item_renderer.canvas.borrow_mut().flush();

        // Delete any images and layer images (and their FBOs) before making the context not current anymore, to
        // avoid GPU memory leaks.
        self.texture_cache.borrow_mut().drain();
        drop(item_renderer);
    }
}
