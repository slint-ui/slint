// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![cfg(feature = "svg")]

use super::{ImageCacheKey, SharedImageBuffer, SharedPixelBuffer};
use crate::lengths::PhysicalPx;
#[cfg(not(target_arch = "wasm32"))]
use crate::SharedString;
use resvg::{
    tiny_skia,
    usvg::{self, TreeTextToPath},
};
use usvg::TreeParsing;

pub struct ParsedSVG {
    svg_tree: usvg::Tree,
    cache_key: ImageCacheKey,
}

impl super::OpaqueImage for ParsedSVG {
    fn size(&self) -> crate::graphics::IntSize {
        self.size()
    }
    fn cache_key(&self) -> ImageCacheKey {
        self.cache_key.clone()
    }
}

impl core::fmt::Debug for ParsedSVG {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("ParsedSVG").finish()
    }
}

impl ParsedSVG {
    pub fn size(&self) -> crate::graphics::IntSize {
        let size = self.svg_tree.size.to_screen_size();
        [size.width(), size.height()].into()
    }

    pub fn cache_key(&self) -> ImageCacheKey {
        self.cache_key.clone()
    }

    /// Renders the SVG with the specified size.
    pub fn render(
        &self,
        size: euclid::Size2D<u32, PhysicalPx>,
    ) -> Result<SharedImageBuffer, usvg::Error> {
        let tree = &self.svg_tree;
        let fit = resvg::FitTo::Size(size.width, size.height);
        let size = fit.fit_to(tree.size.to_screen_size()).ok_or(usvg::Error::InvalidSize)?;
        let mut buffer = SharedPixelBuffer::new(size.width(), size.height());
        let skia_buffer =
            tiny_skia::PixmapMut::from_bytes(buffer.make_mut_bytes(), size.width(), size.height())
                .ok_or(usvg::Error::InvalidSize)?;
        resvg::render(tree, fit, Default::default(), skia_buffer)
            .ok_or(usvg::Error::InvalidSize)?;
        Ok(SharedImageBuffer::RGBA8Premultiplied(buffer))
    }
}

fn with_svg_options<T>(callback: impl FnOnce(&usvg::Options) -> T) -> T {
    let options = usvg::Options::default();
    callback(&options)
}

fn fixup_text(mut tree: usvg::Tree) -> usvg::Tree {
    if tree.has_text_nodes() {
        crate::sharedfontdb::FONT_DB.with(|db| {
            tree.convert_text(&*db.borrow());
        })
    }
    tree
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_from_path(
    path: &SharedString,
    cache_key: ImageCacheKey,
) -> Result<ParsedSVG, std::io::Error> {
    let svg_data = std::fs::read(std::path::Path::new(&path.as_str()))?;

    with_svg_options(|options| {
        usvg::Tree::from_data(&svg_data, options)
            .map(fixup_text)
            .map(|svg| ParsedSVG { svg_tree: svg, cache_key })
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    })
}

pub fn load_from_data(slice: &[u8], cache_key: ImageCacheKey) -> Result<ParsedSVG, usvg::Error> {
    with_svg_options(|options| {
        usvg::Tree::from_data(slice, options)
            .map(fixup_text)
            .map(|svg| ParsedSVG { svg_tree: svg, cache_key })
    })
}
