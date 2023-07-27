// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

#[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
use crate::thread_local_ as thread_local;

use super::{PhysicalLength, PhysicalSize};
use crate::graphics::{BitmapFont, FontRequest};
use crate::lengths::{LogicalLength, LogicalSize, ScaleFactor};
use crate::textlayout::TextLayout;
use crate::Coord;

thread_local! {
    static BITMAP_FONTS: RefCell<Vec<&'static BitmapFont>> = RefCell::default()
}

#[derive(derive_more::From, Clone)]
pub enum GlyphAlphaMap {
    Static(&'static [u8]),
    Shared(Rc<[u8]>),
}

#[derive(Clone)]
pub struct RenderableGlyph {
    pub x: PhysicalLength,
    pub y: PhysicalLength,
    pub width: PhysicalLength,
    pub height: PhysicalLength,
    pub alpha_map: GlyphAlphaMap,
}

impl RenderableGlyph {
    pub fn size(&self) -> PhysicalSize {
        PhysicalSize::from_lengths(self.width, self.height)
    }
}

pub trait GlyphRenderer {
    fn render_glyph(&self, glyph_id: core::num::NonZeroU16) -> RenderableGlyph;
}

pub(super) const DEFAULT_FONT_SIZE: LogicalLength = LogicalLength::new(12 as Coord);

mod pixelfont;
#[cfg(all(feature = "software-renderer-systemfonts", not(target_arch = "wasm32")))]
pub mod vectorfont;

#[cfg(all(feature = "software-renderer-systemfonts", not(target_arch = "wasm32")))]
pub mod systemfonts;

#[derive(derive_more::From)]
pub enum Font {
    PixelFont(pixelfont::PixelFont),
    #[cfg(all(feature = "software-renderer-systemfonts", not(target_arch = "wasm32")))]
    VectorFont(vectorfont::VectorFont),
}

pub fn match_font(request: &FontRequest, scale_factor: ScaleFactor) -> Font {
    let requested_weight = request
        .weight
        .and_then(|weight| weight.try_into().ok())
        .unwrap_or(/* CSS normal */ 400);

    let bitmap_font = BITMAP_FONTS.with(|fonts| {
        let fonts = fonts.borrow();

        request.family.as_ref().and_then(|requested_family| {
            fonts
                .iter()
                .filter(|bitmap_font| {
                    core::str::from_utf8(bitmap_font.family_name.as_slice()).unwrap()
                        == requested_family.as_str()
                        && bitmap_font.italic == request.italic
                })
                .min_by_key(|bitmap_font| bitmap_font.weight.abs_diff(requested_weight))
                .copied()
        })
    });

    let font = match bitmap_font {
        Some(bitmap_font) => bitmap_font,
        None => {
            #[cfg(all(feature = "software-renderer-systemfonts", not(target_arch = "wasm32")))]
            if let Some(vectorfont) = systemfonts::match_font(request, scale_factor) {
                return vectorfont.into();
            }
            if let Some(fallback_bitmap_font) = BITMAP_FONTS.with(|fonts| {
                let fonts = fonts.borrow();
                fonts
                    .iter()
                    .cloned()
                    .filter(|bitmap_font| bitmap_font.italic == request.italic)
                    .min_by_key(|bitmap_font| bitmap_font.weight.abs_diff(requested_weight))
                    .or_else(|| fonts.first().cloned())
            }) {
                fallback_bitmap_font
            } else {
                #[cfg(all(feature = "software-renderer-systemfonts", not(target_arch = "wasm32")))]
                return systemfonts::fallbackfont(request, scale_factor).into();
                #[cfg(any(not(feature = "software-renderer-systemfonts"), target_arch = "wasm32"))]
                panic!("No font fallback found. The software renderer requires enabling the `EmbedForSoftwareRenderer` option when compiling slint files.")
            }
        }
    };

    let requested_pixel_size: PhysicalLength =
        (request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE).cast() * scale_factor).cast();

    let nearest_pixel_size = font
        .glyphs
        .partition_point(|glyphs| glyphs.pixel_size() <= requested_pixel_size)
        .saturating_sub(1);

    let matching_glyphs = &font.glyphs[nearest_pixel_size];

    pixelfont::PixelFont { bitmap_font: font, glyphs: matching_glyphs }.into()
}

pub fn text_layout_for_font<'a, Font: crate::textlayout::AbstractFont>(
    font: &'a Font,
    font_request: &FontRequest,
    scale_factor: ScaleFactor,
) -> TextLayout<'a, Font>
where
    Font: crate::textlayout::TextShaper<Length = PhysicalLength>,
{
    let letter_spacing =
        font_request.letter_spacing.map(|spacing| (spacing.cast() * scale_factor).cast());

    TextLayout { font, letter_spacing }
}

pub fn register_bitmap_font(font_data: &'static BitmapFont) {
    BITMAP_FONTS.with(|fonts| fonts.borrow_mut().push(font_data))
}

pub fn text_size(
    font_request: FontRequest,
    text: &str,
    max_width: Option<LogicalLength>,
    scale_factor: ScaleFactor,
) -> LogicalSize {
    let font = match_font(&font_request, scale_factor);
    let (longest_line_width, height) = match font {
        Font::PixelFont(pf) => {
            let layout = text_layout_for_font(&pf, &font_request, scale_factor);
            layout.text_size(
                text,
                max_width.map(|max_width| (max_width.cast() * scale_factor).cast()),
            )
        }
        #[cfg(all(feature = "software-renderer-systemfonts", not(target_arch = "wasm32")))]
        Font::VectorFont(vf) => {
            let layout = text_layout_for_font(&vf, &font_request, scale_factor);
            layout.text_size(
                text,
                max_width.map(|max_width| (max_width.cast() * scale_factor).cast()),
            )
        }
    };

    (PhysicalSize::from_lengths(longest_line_width, height).cast() / scale_factor).cast()
}
