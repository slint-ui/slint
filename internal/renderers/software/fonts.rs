// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore pixelfont vectorfont
use alloc::rc::{Rc, Weak};
use alloc::vec::Vec;
use core::cell::RefCell;

use super::{Fixed, PhysicalLength, PhysicalSize};
use i_slint_core::graphics::{BitmapFont, FontRequest, OwnedStorage, Storage};
#[cfg(feature = "systemfonts")]
use i_slint_core::graphics::{Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::lengths::ScaleFactor;
use i_slint_core::textlayout::TextLayout;
use pixelfont::PixelFont;

i_slint_core::thread_local! {
    static BITMAP_FONTS: RefCell<Vec<RegisteredFont>> = RefCell::default()
}

enum RegisteredFont {
    Static(&'static BitmapFont),
    Shared(Weak<BitmapFont<OwnedStorage>>),
}

impl PartialEq for RegisteredFont {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Static(a), Self::Static(b)) => core::ptr::eq(*a, *b),
            (Self::Shared(a), Self::Shared(b)) => a.ptr_eq(b),
            _ => false,
        }
    }
}

impl RegisteredFont {
    /// How far the weight of the font is from `weight`, when the font is still alive,
    /// of `family`, and `italic` (any, when `None`).
    fn weight_distance(
        &self,
        family: Option<&str>,
        italic: Option<bool>,
        weight: u16,
    ) -> Option<u16> {
        fn distance<S: Storage>(
            font: &BitmapFont<S>,
            family: Option<&str>,
            italic: Option<bool>,
            weight: u16,
        ) -> Option<u16> {
            let matches = italic.is_none_or(|italic| font.italic == italic)
                && family.is_none_or(|family| &*font.family_name == family.as_bytes());
            matches.then(|| font.weight.abs_diff(weight))
        }
        match self {
            Self::Static(font) => distance(font, family, italic, weight),
            Self::Shared(font) => distance(&*font.upgrade()?, family, italic, weight),
        }
    }

    fn at_pixel_size(&self, pixel_size: PhysicalLength) -> Option<Font> {
        Some(match self {
            Self::Static(font) => Font::Pixel(PixelFont::new(*font, pixel_size)),
            Self::Shared(font) => Font::SharedPixel(PixelFont::new(font.upgrade()?, pixel_size)),
        })
    }
}

#[derive(derive_more::From, Clone)]
pub enum GlyphAlphaMap {
    /// A glyph of a font that generated code keeps in a static
    Static(&'static [u8]),
    /// A glyph of a font built at run time, which it keeps alive while the glyph is drawn
    #[from(skip)]
    Shared { font: Rc<BitmapFont<OwnedStorage>>, glyph_set: u16, glyph: u16 },
    /// A glyph rendered at run time, from a vector font
    Rendered(Rc<[u8]>),
}

impl GlyphAlphaMap {
    pub fn as_slice(&self) -> &[u8] {
        match self {
            Self::Static(data) => data,
            Self::Shared { font, glyph_set, glyph } => {
                &font.glyphs[*glyph_set as usize].glyph_data[*glyph as usize].data
            }
            Self::Rendered(data) => data,
        }
    }
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

/// The pixels of a glyph rendered from a vector font.
#[cfg(feature = "systemfonts")]
#[derive(Clone)]
pub enum VectorGlyphPixels {
    /// One coverage value per pixel, drawn in the text's color.
    AlphaMap(Rc<[u8]>),
    /// Premultiplied RGBA pixels of a color glyph, such as an emoji, drawn as they are.
    Color(SharedPixelBuffer<Rgba8Pixel>),
}

// Subset of `RenderableGlyph`, specifically for VectorFonts.
#[cfg(feature = "systemfonts")]
#[derive(Clone)]
pub struct RenderableVectorGlyph {
    pub x: Fixed<i32, 8>,
    pub y: Fixed<i32, 8>,
    pub width: PhysicalLength,
    pub height: PhysicalLength,
    pub pixels: VectorGlyphPixels,
    pub pixel_stride: u16,
    pub glyph_origin_x: f32,
}

#[cfg(feature = "systemfonts")]
impl RenderableVectorGlyph {
    pub fn size(&self) -> PhysicalSize {
        PhysicalSize::from_lengths(self.width, self.height)
    }

    /// The number of bytes the glyph's pixels take, for the glyph cache.
    pub fn byte_size(&self) -> usize {
        match &self.pixels {
            VectorGlyphPixels::AlphaMap(alpha_map) => alpha_map.len(),
            VectorGlyphPixels::Color(buffer) => buffer.as_bytes().len(),
        }
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
    Pixel(PixelFont),
    SharedPixel(PixelFont<OwnedStorage>),
    #[cfg(feature = "systemfonts")]
    Vector(vectorfont::VectorFont),
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
            $crate::fonts::Font::Pixel($bound) => $body,
            $crate::fonts::Font::SharedPixel($bound) => $body,
            #[cfg(feature = "systemfonts")]
            $crate::fonts::Font::Vector($bound) => $body,
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
    let requested_pixel_size: PhysicalLength =
        (request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE).cast() * scale_factor).cast();
    let nearest_weight = |family: Option<&str>, italic: Option<bool>| {
        BITMAP_FONTS.with(|fonts| {
            fonts
                .borrow()
                .iter()
                .filter_map(|font| {
                    Some((font.weight_distance(family, italic, requested_weight)?, font))
                })
                .min_by_key(|(distance, _)| *distance)
                .and_then(|(_, font)| font.at_pixel_size(requested_pixel_size))
        })
    };

    if let Some(font) = request
        .family
        .as_ref()
        .and_then(|family| nearest_weight(Some(family.as_str()), Some(request.italic)))
    {
        return font;
    }
    #[cfg(feature = "systemfonts")]
    if let Some(vectorfont) = systemfonts::match_font(request, scale_factor, font_context) {
        return vectorfont.into();
    }
    if let Some(font) =
        nearest_weight(None, Some(request.italic)).or_else(|| nearest_weight(None, None))
    {
        return font;
    }
    #[cfg(feature = "systemfonts")]
    return systemfonts::fallbackfont(request, scale_factor, font_context).into();
    #[cfg(not(feature = "systemfonts"))]
    panic!(
        "No font fallback found. The software renderer requires enabling the `EmbedForSoftwareRenderer` option when compiling slint files."
    )
}

pub fn text_layout_for_font<'a, Font>(
    font: &'a Font,
    font_request: &FontRequest,
    line_height_factor: Option<f32>,
    scale_factor: ScaleFactor,
) -> TextLayout<'a, Font>
where
    Font: i_slint_core::textlayout::AbstractFont
        + i_slint_core::textlayout::TextShaper<Length = PhysicalLength>,
{
    let letter_spacing =
        font_request.letter_spacing.map(|spacing| (spacing.cast() * scale_factor).cast());
    let line_height = line_height_factor.map(|factor| {
        let line_height = font.height().get() as f32 * factor;
        PhysicalLength::new(num_traits::Float::round(line_height).max(0.) as i16)
    });

    TextLayout { font, letter_spacing, line_height }
}

pub fn register_bitmap_font(font_data: &'static BitmapFont) {
    register(RegisteredFont::Static(font_data));
}

pub fn register_shared_bitmap_font(font_data: &Rc<BitmapFont<OwnedStorage>>) {
    register(RegisteredFont::Shared(Rc::downgrade(font_data)));
}

fn register(font: RegisteredFont) {
    BITMAP_FONTS.with(|fonts| {
        let mut fonts = fonts.borrow_mut();
        fonts.retain(
            |font| !matches!(font, RegisteredFont::Shared(font) if font.strong_count() == 0),
        );
        if !fonts.contains(&font) {
            fonts.push(font);
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
        let font = Rc::new(BitmapFont::<OwnedStorage> {
            family_name: b"Shared".to_vec(),
            character_map: Vec::new(),
            units_per_em: 1.,
            ascent: 0.,
            descent: 0.,
            x_height: 0.,
            cap_height: 0.,
            glyphs: alloc::vec![i_slint_core::graphics::BitmapGlyphs::<OwnedStorage> {
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
                    .filter(|font| font.weight_distance(Some("Shared"), None, 400).is_some())
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
        let font_request =
            FontRequest { pixel_size: Some(LogicalLength::new(20.)), ..Default::default() };

        let layout =
            text_layout_for_font(&TestFont, &font_request, Some(1.5), ScaleFactor::new(1.));

        assert_eq!(TestFont.height(), PhysicalLength::new(24));
        assert_eq!(layout.line_height, Some(PhysicalLength::new(36)));
    }

    #[test]
    fn line_height_factor_zero_collapses_lines() {
        let font_request =
            FontRequest { pixel_size: Some(LogicalLength::new(20.)), ..Default::default() };

        let layout = text_layout_for_font(&TestFont, &font_request, Some(0.), ScaleFactor::new(1.));

        assert_eq!(layout.line_height, Some(PhysicalLength::new(0)));
    }
}
