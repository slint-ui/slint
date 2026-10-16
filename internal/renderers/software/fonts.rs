// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore pixelfont vectorfont
use alloc::rc::{Rc, Weak};
use alloc::vec::Vec;
use core::cell::RefCell;

use super::{Fixed, PhysicalLength, PhysicalSize};
use i_slint_core::graphics::{BitmapFont, FontRequest, SharedBitmapFont};
use i_slint_core::lengths::ScaleFactor;
use i_slint_core::textlayout::TextLayout;
use pixelfont::BitmapFontRef;

i_slint_core::thread_local! {
    static BITMAP_FONTS: RefCell<Vec<RegisteredFont>> = RefCell::default()
}

/// A font in [`BITMAP_FONTS`]. A shared font stays only as long as its owner keeps it.
enum RegisteredFont {
    Static(&'static BitmapFont),
    Shared(Weak<SharedBitmapFont>),
}

impl RegisteredFont {
    fn get(&self) -> Option<BitmapFontRef> {
        match self {
            Self::Static(font) => Some(BitmapFontRef::Static(font)),
            Self::Shared(font) => font.upgrade().map(BitmapFontRef::Shared),
        }
    }
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

// Subset of `RenderableGlyph`, specifically for VectorFonts.
#[cfg(feature = "systemfonts")]
#[derive(Clone)]
pub struct RenderableVectorGlyph {
    pub x: Fixed<i32, 8>,
    pub y: Fixed<i32, 8>,
    pub width: PhysicalLength,
    pub height: PhysicalLength,
    pub alpha_map: Rc<[u8]>,
    pub pixel_stride: u16,
    pub glyph_origin_x: f32,
}

#[cfg(feature = "systemfonts")]
impl RenderableVectorGlyph {
    pub fn size(&self) -> PhysicalSize {
        PhysicalSize::from_lengths(self.width, self.height)
    }
}

pub trait GlyphRenderer {
    fn render_glyph(
        &self,
        glyph_id: core::num::NonZeroU16,
        slint_context: &i_slint_core::SlintContext,
    ) -> Option<RenderableGlyph>;
    /// The amount of pixel in the original image that correspond to one pixel in the rendered image
    fn scale_delta(&self) -> Fixed<u16, 8>;
}

pub(super) use i_slint_core::textlayout::DEFAULT_FONT_SIZE;

mod pixelfont;
#[cfg(feature = "systemfonts")]
pub mod vectorfont;

#[cfg(feature = "systemfonts")]
pub mod systemfonts;

#[derive(derive_more::From)]
pub enum Font {
    PixelFont(pixelfont::PixelFont),
    #[cfg(feature = "systemfonts")]
    VectorFont(vectorfont::VectorFont),
}

/// Runs `$body` with `$bound` bound to the concrete font held by `$font`.
///
/// The bitmap and vector paths through the text code are the same code; they need two
/// match arms only because `PixelFont` and `VectorFont` are distinct types. The body is
/// monomorphized per variant, the way a generic function would be, so that it can be
/// written once.
///
/// Keep only font-dependent work in the body: it is emitted once per variant.
///
/// Callers that hand off to parley for vector fonts must do so before calling this, as
/// `sharedparley::` needs the font context rather than a laid-out font.
macro_rules! with_font {
    ($font:expr, |$bound:ident| $body:block) => {
        match $font {
            $crate::fonts::Font::PixelFont($bound) => $body,
            #[cfg(feature = "systemfonts")]
            $crate::fonts::Font::VectorFont($bound) => $body,
        }
    };
}
pub(crate) use with_font;

impl i_slint_core::textlayout::FontMetrics<PhysicalLength> for Font {
    fn ascent(&self) -> PhysicalLength {
        with_font!(self, |font| { font.ascent() })
    }

    fn height(&self) -> PhysicalLength {
        with_font!(self, |font| { font.height() })
    }

    fn descent(&self) -> PhysicalLength {
        with_font!(self, |font| { font.descent() })
    }

    fn x_height(&self) -> PhysicalLength {
        with_font!(self, |font| { font.x_height() })
    }

    fn cap_height(&self) -> PhysicalLength {
        with_font!(self, |font| { font.cap_height() })
    }
}

pub fn match_font(
    request: &FontRequest,
    scale_factor: ScaleFactor,
    #[cfg(feature = "systemfonts")]
    font_context: &mut i_slint_core::textlayout::sharedparley::FontContext,
) -> Font {
    let requested_weight = request
        .weight
        .and_then(|weight| weight.try_into().ok())
        .unwrap_or(/* CSS normal */ 400);

    let bitmap_font = BITMAP_FONTS.with(|fonts| {
        let fonts = fonts.borrow();

        request.family.as_ref().and_then(|requested_family| {
            fonts
                .iter()
                .filter_map(RegisteredFont::get)
                .filter(|bitmap_font| {
                    bitmap_font.family_name() == requested_family.as_str()
                        && bitmap_font.italic() == request.italic
                })
                .min_by_key(|bitmap_font| bitmap_font.weight().abs_diff(requested_weight))
        })
    });

    let font = match bitmap_font {
        Some(bitmap_font) => bitmap_font,
        None => {
            #[cfg(feature = "systemfonts")]
            if let Some(vectorfont) = systemfonts::match_font(request, scale_factor, font_context) {
                return vectorfont.into();
            }
            if let Some(fallback_bitmap_font) = BITMAP_FONTS.with(|fonts| {
                let fonts = fonts.borrow();
                let available = || fonts.iter().filter_map(RegisteredFont::get);
                available()
                    .filter(|bitmap_font| bitmap_font.italic() == request.italic)
                    .min_by_key(|bitmap_font| bitmap_font.weight().abs_diff(requested_weight))
                    .or_else(|| available().next())
            }) {
                fallback_bitmap_font
            } else {
                #[cfg(feature = "systemfonts")]
                return systemfonts::fallbackfont(request, scale_factor, font_context).into();
                #[cfg(not(feature = "systemfonts"))]
                panic!(
                    "No font fallback found. The software renderer requires enabling the `EmbedForSoftwareRenderer` option when compiling slint files."
                )
            }
        }
    };

    let requested_pixel_size: PhysicalLength =
        (request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE).cast() * scale_factor).cast();

    let glyphs = font.glyphs_for_pixel_size(requested_pixel_size);
    let pixel_size = if font.sdf() { requested_pixel_size } else { font.glyphs_pixel_size(glyphs) };

    pixelfont::PixelFont { font, glyphs, pixel_size }.into()
}

pub fn text_layout_for_font<'a, Font>(
    font: &'a Font,
    font_request: &FontRequest,
    scale_factor: ScaleFactor,
) -> TextLayout<'a, Font>
where
    Font: i_slint_core::textlayout::AbstractFont
        + i_slint_core::textlayout::TextShaper<Length = PhysicalLength>,
{
    let letter_spacing =
        font_request.letter_spacing.map(|spacing| (spacing.cast() * scale_factor).cast());
    let line_height = font_request.line_height_for_natural_height(font.height().get() as f32).map(
        |line_height| PhysicalLength::new(num_traits::Float::round(line_height).max(0.) as i16),
    );

    TextLayout { font, letter_spacing, line_height }
}

pub fn register_bitmap_font(font_data: &'static BitmapFont) {
    BITMAP_FONTS.with(|fonts| {
        let mut fonts = fonts.borrow_mut();
        if !fonts.iter().any(
            |font| matches!(font, RegisteredFont::Static(font) if core::ptr::eq(*font, font_data)),
        ) {
            fonts.push(RegisteredFont::Static(font_data));
        }
    })
}

pub fn register_shared_bitmap_font(font_data: &Rc<SharedBitmapFont>) {
    BITMAP_FONTS.with(|fonts| {
        let mut fonts = fonts.borrow_mut();
        fonts.retain(|font| !matches!(font, RegisteredFont::Shared(font) if font.strong_count() == 0));
        if !fonts.iter().any(|font| {
            matches!(font, RegisteredFont::Shared(font) if core::ptr::eq(font.as_ptr(), Rc::as_ptr(font_data)))
        }) {
            fonts.push(RegisteredFont::Shared(Rc::downgrade(font_data)));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_core::lengths::LogicalLength;
    use i_slint_core::slice::Slice;
    use i_slint_core::textlayout::{FontMetrics, Glyph, TextShaper};

    struct TestFont;

    impl FontMetrics<PhysicalLength> for TestFont {
        fn ascent(&self) -> PhysicalLength {
            PhysicalLength::new(18)
        }

        fn descent(&self) -> PhysicalLength {
            PhysicalLength::new(-6)
        }

        fn x_height(&self) -> PhysicalLength {
            PhysicalLength::new(10)
        }

        fn cap_height(&self) -> PhysicalLength {
            PhysicalLength::new(14)
        }
    }

    impl TextShaper for TestFont {
        type LengthPrimitive = i16;
        type Length = PhysicalLength;

        fn shape_text<GlyphStorage: core::iter::Extend<Glyph<Self::Length>>>(
            &self,
            _text: &str,
            _glyphs: &mut GlyphStorage,
        ) {
        }

        fn glyph_for_char(&self, _ch: char) -> Option<Glyph<Self::Length>> {
            None
        }
    }

    #[test]
    fn registering_a_font_again_keeps_one_entry() {
        static FONT: BitmapFont = BitmapFont {
            family_name: Slice::from_slice(b"Test"),
            character_map: Slice::from_slice(&[]),
            units_per_em: 1.,
            ascent: 0.,
            descent: 0.,
            x_height: 0.,
            cap_height: 0.,
            glyphs: Slice::from_slice(&[]),
            weight: 400,
            italic: false,
            sdf: false,
        };
        register_bitmap_font(&FONT);
        register_bitmap_font(&FONT);
        assert_eq!(BITMAP_FONTS.with(|fonts| fonts.borrow().len()), 1);
    }

    #[test]
    fn a_shared_font_goes_away_with_its_owner() {
        let font = Rc::new(SharedBitmapFont {
            family_name: "Shared".into(),
            character_map: Vec::new(),
            units_per_em: 1.,
            ascent: 0.,
            descent: 0.,
            x_height: 0.,
            cap_height: 0.,
            glyphs: alloc::vec![i_slint_core::graphics::SharedBitmapGlyphs {
                pixel_size: 12,
                glyph_data: Vec::new(),
            }],
            weight: 400,
            italic: false,
            sdf: false,
        });
        let registered = || {
            BITMAP_FONTS.with(|fonts| {
                fonts
                    .borrow()
                    .iter()
                    .filter_map(RegisteredFont::get)
                    .filter(|font| font.family_name() == "Shared")
                    .count()
            })
        };
        register_shared_bitmap_font(&font);
        register_shared_bitmap_font(&font);
        assert_eq!(registered(), 1);
        drop(font);
        assert_eq!(registered(), 0);
    }

    #[test]
    fn line_height_factor_scales_natural_height() {
        let font_request = FontRequest {
            pixel_size: Some(LogicalLength::new(20.)),
            line_height_factor: Some(1.5),
            ..Default::default()
        };

        let layout = text_layout_for_font(&TestFont, &font_request, ScaleFactor::new(1.));

        assert_eq!(TestFont.height(), PhysicalLength::new(24));
        assert_eq!(layout.line_height, Some(PhysicalLength::new(36)));
    }

    #[test]
    fn line_height_factor_zero_collapses_lines() {
        let font_request = FontRequest {
            pixel_size: Some(LogicalLength::new(20.)),
            line_height_factor: Some(0.),
            ..Default::default()
        };

        let layout = text_layout_for_font(&TestFont, &font_request, ScaleFactor::new(1.));

        assert_eq!(layout.line_height, Some(PhysicalLength::new(0)));
    }
}
