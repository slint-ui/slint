// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{ImageCacheKey, SharedImageBuffer, SharedPixelBuffer};
use crate::lengths::PhysicalPx;
#[cfg(not(target_arch = "wasm32"))]
use crate::SharedString;
use resvg::{tiny_skia, usvg};

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
        let size = self.svg_tree.size().to_int_size();
        [size.width(), size.height()].into()
    }

    pub fn cache_key(&self) -> ImageCacheKey {
        self.cache_key.clone()
    }

    /// Renders the SVG with the specified size, if no size is specified, get the size from the image
    #[allow(clippy::unnecessary_cast)] // Coord
    pub fn render(
        &self,
        size: Option<euclid::Size2D<u32, PhysicalPx>>,
    ) -> Result<SharedImageBuffer, usvg::Error> {
        let tree = &self.svg_tree;

        let (target_size, transform) = match size {
            Some(size) => {
                let target_size = tiny_skia::IntSize::from_wh(size.width, size.height)
                    .ok_or(usvg::Error::InvalidSize)?;
                let target_size = tree.size().to_int_size().scale_to(target_size);
                let target_size_f = target_size.to_size();

                let transform = tiny_skia::Transform::from_scale(
                    target_size_f.width() as f32 / tree.size().width() as f32,
                    target_size_f.height() as f32 / tree.size().height() as f32,
                );
                (target_size, transform)
            }
            None => (tree.size().to_int_size(), tiny_skia::Transform::default()),
        };

        let mut buffer = SharedPixelBuffer::new(target_size.width(), target_size.height());
        let mut skia_buffer = tiny_skia::PixmapMut::from_bytes(
            buffer.make_mut_bytes(),
            target_size.width(),
            target_size.height(),
        )
        .ok_or(usvg::Error::InvalidSize)?;

        resvg::render(tree, transform, &mut skia_buffer);
        Ok(SharedImageBuffer::RGBA8Premultiplied(buffer))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_from_path(
    path: &SharedString,
    cache_key: ImageCacheKey,
) -> Result<ParsedSVG, std::io::Error> {
    let svg_data = std::fs::read(std::path::Path::new(&path.as_str()))?;

    let option = usvg::Options::default();
    usvg::Tree::from_data(&svg_data, &option)
        .map(|svg| ParsedSVG { svg_tree: svg, cache_key })
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

pub fn load_from_data(slice: &[u8], cache_key: ImageCacheKey) -> Result<ParsedSVG, usvg::Error> {
    let option = usvg::Options::default();
    usvg::Tree::from_data(slice, &option).map(|svg| ParsedSVG { svg_tree: svg, cache_key })
}
