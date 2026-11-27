// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{SharedImageBuffer, SharedPixelBuffer};
use crate::lengths::PhysicalPx;
use resvg::{tiny_skia, usvg};

/// A parsed SVG document.
pub struct ParsedSVG {
    svg_tree: usvg::Tree,
    weight_in_bytes: usize,
}

impl core::fmt::Debug for ParsedSVG {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("ParsedSVG").finish()
    }
}

impl ParsedSVG {
    fn new(svg_tree: usvg::Tree, source_size: usize) -> Self {
        // The parsed tree typically costs a few times the source size, and outlining
        // a short `<text>` element expands it far more; charge a generous multiple of
        // the source size, plus a constant for the fixed cost of even a tiny tree,
        // so that the cache stays bounded without measuring the tree.
        let weight_in_bytes = source_size.saturating_mul(16).saturating_add(8192);
        Self { svg_tree, weight_in_bytes }
    }

    /// Returns the size of the document in pixels.
    pub fn size(&self) -> crate::graphics::IntSize {
        let size = self.svg_tree.size().to_int_size();
        [size.width(), size.height()].into()
    }

    /// Approximate number of bytes the parsed tree keeps alive, for cache accounting.
    pub fn weight_in_bytes(&self) -> usize {
        self.weight_in_bytes
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
                let source_size = tree.size().to_int_size();
                let target_size = super::aspect_preserving_render_size(
                    [source_size.width(), source_size.height()].into(),
                    size,
                );
                let target_size =
                    tiny_skia::IntSize::from_wh(target_size.width, target_size.height)
                        .ok_or(usvg::Error::InvalidSize)?;
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

/// Resolves SVG `<text>` fonts through the shared [`sharedfontique::svg`] bridge
/// against the running `SlintContext`'s collection, so SVG text uses the same fonts
/// as the rest of the UI.
/// Without a context the text stays unresolved, which never happens for a displayable
/// image since the platform is up by then.
///
/// Gated on `shared-parley`: without Slint's text engine there is no text layout, so
/// SVG text resolution would be moot.
#[cfg(feature = "shared-parley")]
fn svg_options() -> usvg::Options<'static> {
    use i_slint_common::sharedfontique::{self, fontique};

    fn find_font(
        families: &[fontique::QueryFamily],
        attributes: fontique::Attributes,
        require_char: Option<char>,
    ) -> Option<fontique::QueryFont> {
        crate::context::GLOBAL_CONTEXT.with(|p| {
            let ctx = p.get()?;
            let mut font_context = ctx.font_context().try_borrow_mut().ok()?;
            let parley::FontContext { collection, source_cache } = &mut font_context.inner;
            sharedfontique::svg::query_font(
                collection,
                source_cache,
                families,
                attributes,
                require_char,
            )
        })
    }

    sharedfontique::svg::options(find_font)
}

#[cfg(not(feature = "shared-parley"))]
fn svg_options() -> usvg::Options<'static> {
    usvg::Options::default()
}

pub fn load_from_data(slice: &[u8]) -> Result<ParsedSVG, usvg::Error> {
    usvg::Tree::from_data(slice, &svg_options()).map(|svg| ParsedSVG::new(svg, slice.len()))
}
