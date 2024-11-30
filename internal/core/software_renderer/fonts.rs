// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use alloc::rc::Rc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::cell::RefCell;

#[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
use crate::thread_local_ as thread_local;

use super::{Fixed, PhysicalLength, PhysicalSize};
use crate::graphics::{BitmapFont, FontRequest};
use crate::items::TextWrap;
use crate::lengths::{LogicalLength, LogicalSize, ScaleFactor};
use crate::textlayout::{FontMetrics, TextLayout};
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
    pub x: Fixed<i32, 8>,
    pub y: Fixed<i32, 8>,
    pub width: PhysicalLength,
    pub height: PhysicalLength,
    pub alpha_map: GlyphAlphaMap,
    pub pixel_stride: u16,
    pub sdf: bool,
}

impl RenderableGlyph {
    pub fn size(&self) -> PhysicalSize {
        PhysicalSize::from_lengths(self.width, self.height)
    }
}

pub trait GlyphRenderer {
    fn render_glyph(&self, glyph_id: core::num::NonZeroU16) -> Option<RenderableGlyph>;
    /// The amount of pixel in the original image that correspond to one pixel in the rendered image
    fn scale_delta(&self) -> Fixed<u16, 8>;
}

pub(super) const DEFAULT_FONT_SIZE: LogicalLength = LogicalLength::new(12 as Coord);

mod pixelfont;
#[cfg(feature = "software-renderer-systemfonts")]
pub mod vectorfont;

#[cfg(feature = "software-renderer-systemfonts")]
pub mod systemfonts;

#[derive(derive_more::From)]
pub enum Font {
    PixelFont(pixelfont::PixelFont),
    #[cfg(feature = "software-renderer-systemfonts")]
    VectorFont(vectorfont::VectorFont),
}

impl crate::textlayout::FontMetrics<PhysicalLength> for Font {
    fn ascent(&self) -> PhysicalLength {
        match self {
            Font::PixelFont(pixel_font) => pixel_font.ascent(),
            #[cfg(feature = "software-renderer-systemfonts")]
            Font::VectorFont(vector_font) => vector_font.ascent(),
        }
    }

    fn height(&self) -> PhysicalLength {
        match self {
            Font::PixelFont(pixel_font) => pixel_font.height(),
            #[cfg(feature = "software-renderer-systemfonts")]
            Font::VectorFont(vector_font) => vector_font.height(),
        }
    }

    fn descent(&self) -> PhysicalLength {
        match self {
            Font::PixelFont(pixel_font) => pixel_font.descent(),
            #[cfg(feature = "software-renderer-systemfonts")]
            Font::VectorFont(vector_font) => vector_font.descent(),
        }
    }

    fn x_height(&self) -> PhysicalLength {
        match self {
            Font::PixelFont(pixel_font) => pixel_font.x_height(),
            #[cfg(feature = "software-renderer-systemfonts")]
            Font::VectorFont(vector_font) => vector_font.x_height(),
        }
    }

    fn cap_height(&self) -> PhysicalLength {
        match self {
            Font::PixelFont(pixel_font) => pixel_font.cap_height(),
            #[cfg(feature = "software-renderer-systemfonts")]
            Font::VectorFont(vector_font) => vector_font.cap_height(),
        }
    }
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
            #[cfg(feature = "software-renderer-systemfonts")]
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
                #[cfg(feature = "software-renderer-systemfonts")]
                return systemfonts::fallbackfont(request, scale_factor).into();
                #[cfg(not(feature = "software-renderer-systemfonts"))]
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

    let pixel_size = if font.sdf { requested_pixel_size } else { matching_glyphs.pixel_size() };

    pixelfont::PixelFont { bitmap_font: font, glyphs: matching_glyphs, pixel_size }.into()
}

pub fn text_layout_for_font<'a, Font>(
    font: &'a Font,
    font_request: &FontRequest,
    scale_factor: ScaleFactor,
) -> TextLayout<'a, Font>
where
    Font: crate::textlayout::AbstractFont + crate::textlayout::TextShaper<Length = PhysicalLength>,
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
    text_wrap: TextWrap,
) -> LogicalSize {
    let font = match_font(&font_request, scale_factor);
    let (longest_line_width, height) = match font {
        Font::PixelFont(pf) => {
            let layout = text_layout_for_font(&pf, &font_request, scale_factor);
            layout.text_size(
                text,
                max_width.map(|max_width| (max_width.cast() * scale_factor).cast()),
                text_wrap,
            )
        }
        #[cfg(feature = "software-renderer-systemfonts")]
        Font::VectorFont(vf) => {
            let layout = text_layout_for_font(&vf, &font_request, scale_factor);
            layout.text_size(
                text,
                max_width.map(|max_width| (max_width.cast() * scale_factor).cast()),
                text_wrap,
            )
        }
    };

    (PhysicalSize::from_lengths(longest_line_width, height).cast() / scale_factor).cast()
}

pub fn font_metrics(
    font_request: FontRequest,
    scale_factor: ScaleFactor,
) -> crate::items::FontMetrics {
    let font = match_font(&font_request, scale_factor);

    let ascent: LogicalLength = (font.ascent().cast() / scale_factor).cast();
    let descent: LogicalLength = (font.descent().cast() / scale_factor).cast();
    let x_height: LogicalLength = (font.x_height().cast() / scale_factor).cast();
    let cap_height: LogicalLength = (font.cap_height().cast() / scale_factor).cast();

    crate::items::FontMetrics {
        ascent: ascent.get() as _,
        descent: descent.get() as _,
        x_height: x_height.get() as _,
        cap_height: cap_height.get() as _,
    }
}
