// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use alloc::vec::Vec;
use core::cell::RefCell;

#[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
use i_slint_core::thread_local_ as thread_local;

use crate::{LogicalLength, LogicalSize, PhysicalLength, PhysicalSize, ScaleFactor};
use i_slint_core::{
    graphics::{BitmapFont, BitmapGlyph, BitmapGlyphs, FontRequest},
    slice::Slice,
    textlayout::TextShaper,
    Coord,
};

thread_local! {
    static FONTS: RefCell<Vec<&'static BitmapFont>> = RefCell::default()
}

#[derive(Copy, Clone)]
pub struct Glyph {
    pub(crate) bitmap_glyph: Option<&'static BitmapGlyph>,
    x_advance: PhysicalLength,
}

impl Glyph {
    pub fn x(&self) -> PhysicalLength {
        self.bitmap_glyph.map(|g| PhysicalLength::new(g.x)).unwrap_or_default()
    }
    pub fn y(&self) -> PhysicalLength {
        self.bitmap_glyph.map(|g| PhysicalLength::new(g.y)).unwrap_or_default()
    }
    pub fn width(&self) -> PhysicalLength {
        self.bitmap_glyph.map(|g| PhysicalLength::new(g.width)).unwrap_or_default()
    }
    pub fn height(&self) -> PhysicalLength {
        self.bitmap_glyph.map(|g| PhysicalLength::new(g.height)).unwrap_or_default()
    }
    pub fn size(&self) -> PhysicalSize {
        PhysicalSize::from_lengths(self.width(), self.height())
    }
    pub fn x_advance(&self) -> PhysicalLength {
        self.x_advance
    }
    pub fn data(&self) -> &Slice<'static, u8> {
        &self.bitmap_glyph.expect("invalid error: Glyph::data called on null").data
    }
}

trait FontMetrics {
    fn ascent(&self, font: &BitmapFont) -> PhysicalLength;
    fn height(&self, font: &BitmapFont) -> PhysicalLength;
    fn pixel_size(&self) -> PhysicalLength;
}

impl FontMetrics for BitmapGlyphs {
    fn ascent(&self, font: &BitmapFont) -> PhysicalLength {
        (PhysicalLength::new(self.pixel_size).cast() * font.ascent / font.units_per_em).cast()
    }
    fn height(&self, font: &BitmapFont) -> PhysicalLength {
        // The descent is negative (relative to the baseline)
        (PhysicalLength::new(self.pixel_size).cast() * (font.ascent - font.descent)
            / font.units_per_em)
            .cast()
    }
    fn pixel_size(&self) -> PhysicalLength {
        PhysicalLength::new(self.pixel_size)
    }
}

pub const DEFAULT_FONT_SIZE: Coord = 12 as Coord;

// A font that is resolved to a specific pixel size.
pub struct PixelFont {
    bitmap_font: &'static BitmapFont,
    glyphs: &'static BitmapGlyphs,
    //letter_spacing: PhysicalLength,
}

impl PixelFont {
    pub fn ascent(&self) -> PhysicalLength {
        self.glyphs.ascent(self.bitmap_font)
    }

    pub fn height(&self) -> PhysicalLength {
        self.glyphs.height(self.bitmap_font)
    }

    pub fn pixel_size(&self) -> PhysicalLength {
        self.glyphs.pixel_size()
    }
}

impl TextShaper for PixelFont {
    type LengthPrimitive = i16;
    type Length = PhysicalLength;
    type Glyph = self::Glyph;
    fn shape_text<GlyphStorage: core::iter::Extend<(Glyph, usize)>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    ) {
        let glyphs_iter = text.char_indices().map(|(byte_offset, char)| {
            let bitmap_glyph = self
                .bitmap_font
                .character_map
                .binary_search_by_key(&char, |char_map_entry| char_map_entry.code_point)
                .ok()
                .map_or(None, |char_map_index| {
                    let glyph_index = self.bitmap_font.character_map[char_map_index].glyph_index;
                    Some(&self.glyphs.glyph_data[glyph_index as usize])
                });
            let x_advance = bitmap_glyph
                .map_or_else(|| self.pixel_size(), |g| PhysicalLength::new(g.x_advance));
            (Glyph { bitmap_glyph, x_advance }, byte_offset)
        });
        glyphs.extend(glyphs_iter);
    }

    fn glyph_for_char(&self, ch: char) -> Option<Self::Glyph> {
        self.bitmap_font
            .character_map
            .binary_search_by_key(&ch, |char_map_entry| char_map_entry.code_point)
            .ok()
            .map(|char_map_index| {
                let glyph_index = self.bitmap_font.character_map[char_map_index].glyph_index;
                let bitmap_glyph = &self.glyphs.glyph_data[glyph_index as usize];
                let x_advance = PhysicalLength::new(bitmap_glyph.x_advance);
                Glyph { bitmap_glyph: Some(bitmap_glyph), x_advance }
            })
    }
    fn glyph_advance_x(&self, glyph: &Glyph) -> PhysicalLength {
        glyph.x_advance
    }
}

pub fn match_font(request: &FontRequest, scale_factor: ScaleFactor) -> PixelFont {
    let font = FONTS.with(|fonts| {
        let fonts = fonts.borrow();
        let fallback_font =
            *fonts.first().expect("internal error: cannot render text without fonts");

        request.family.as_ref().map_or(fallback_font, |requested_family| {
            fonts
                .iter()
                .find(|bitmap_font| {
                    core::str::from_utf8(bitmap_font.family_name.as_slice()).unwrap()
                        == requested_family.as_str()
                })
                .unwrap_or(&fallback_font)
        })
    });

    let requested_pixel_size: PhysicalLength =
        (LogicalLength::new(request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE)).cast() * scale_factor)
            .cast();

    let nearest_pixel_size = font
        .glyphs
        .partition_point(|glyphs| glyphs.pixel_size() <= requested_pixel_size)
        .saturating_sub(1);

    let matching_glyphs = &font.glyphs[nearest_pixel_size];

    PixelFont {
        bitmap_font: font,
        glyphs: matching_glyphs,
        /*letter_spacing: (LogicalLength::new(request.letter_spacing.unwrap_or_default())
        * scale_factor)
        .cast(),
        */
    }
}

pub fn register_bitmap_font(font_data: &'static BitmapFont) {
    FONTS.with(|fonts| fonts.borrow_mut().push(font_data))
}

pub fn text_size(
    font_request: FontRequest,
    text: &str,
    max_width: Option<Coord>,
    scale_factor: ScaleFactor,
) -> LogicalSize {
    let font = match_font(&font_request, scale_factor);

    let (longest_line_width, num_lines) = i_slint_core::textlayout::text_size(
        &font,
        text,
        max_width.map(|max_width| (LogicalLength::new(max_width).cast() * scale_factor).cast()),
    );

    (PhysicalSize::from_lengths(longest_line_width, font.height() * (num_lines as i16)).cast()
        / scale_factor)
        .cast()
}
