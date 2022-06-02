// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![cfg(feature = "svg")]

use alloc::rc::Rc;

use super::SharedPixelBuffer;

#[derive(Clone)]
pub struct ParsedSVG(Rc<usvg::Tree>);

impl core::fmt::Debug for ParsedSVG {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("ParsedSVG").finish()
    }
}

impl PartialEq for ParsedSVG {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl ParsedSVG {
    pub fn size(&self) -> crate::graphics::IntSize {
        let size = self.0.svg_node().size.to_screen_size();
        [size.width(), size.height()].into()
    }

    /// Renders the SVG with the specified size.
    ///
    /// NOTE: returned Rgba8Pixel buffer has alpha pre-multiplied
    pub fn render(
        &self,
        size: euclid::default::Size2D<u32>,
    ) -> Result<SharedPixelBuffer<super::Rgba8Pixel>, usvg::Error> {
        let tree = &*self.0;
        // resvg doesn't support scaling to width/height, just fit to width.
        // FIXME: the fit should actually depends on the image-fit property
        let fit = usvg::FitTo::Width(size.width);
        let size =
            fit.fit_to(tree.svg_node().size.to_screen_size()).ok_or(usvg::Error::InvalidSize)?;
        let mut buffer = SharedPixelBuffer::new(size.width(), size.height());
        let skia_buffer =
            tiny_skia::PixmapMut::from_bytes(buffer.make_mut_bytes(), size.width(), size.height())
                .ok_or(usvg::Error::InvalidSize)?;
        resvg::render(tree, fit, Default::default(), skia_buffer)
            .ok_or(usvg::Error::InvalidSize)?;
        Ok(buffer)
    }
}

fn with_svg_options<T>(callback: impl FnOnce(usvg::OptionsRef<'_>) -> T) -> T {
    // TODO: When the font db cache is a feature in corelib, use it:
    /*
    crate::fonts::FONT_CACHE.with(|cache| {
        let options = usvg::Options::default();
        let mut options_ref = options.to_ref();
        let cache = cache.borrow();
        options_ref.fontdb = &cache.available_fonts;
        callback(options_ref)
    })
    */

    let options = usvg::Options::default();
    let options_ref = options.to_ref();
    callback(options_ref)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_from_path(path: &std::path::Path) -> Result<ParsedSVG, std::io::Error> {
    let svg_data = std::fs::read(path)?;

    with_svg_options(|options| {
        usvg::Tree::from_data(&svg_data, &options)
            .map(|svg| ParsedSVG(svg.into()))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    })
}

pub fn load_from_data(slice: &[u8]) -> Result<ParsedSVG, usvg::Error> {
    with_svg_options(|options| {
        usvg::Tree::from_data(slice, &options).map(|svg| ParsedSVG(svg.into()))
    })
}
