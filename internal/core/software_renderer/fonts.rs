// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use alloc::vec::Vec;
use core::cell::RefCell;

#[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
use crate::thread_local_ as thread_local;

use super::{PhysicalLength, PhysicalSize};
use crate::graphics::{BitmapFont, BitmapGlyphs, FontRequest};
use crate::lengths::{LogicalLength, LogicalSize, ScaleFactor};
use crate::textlayout::{Glyph, TextLayout, TextShaper};
use crate::Coord;

thread_local! {
    static FONTS: RefCell<Vec<&'static BitmapFont>> = RefCell::default()
}

trait FontMetrics {
    fn ascent(&self, font: &BitmapFont) -> PhysicalLength;
    fn descent(&self, font: &BitmapFont) -> PhysicalLength;
    fn height(&self, font: &BitmapFont) -> PhysicalLength;
    fn pixel_size(&self) -> PhysicalLength;
}

impl FontMetrics for BitmapGlyphs {
    fn ascent(&self, font: &BitmapFont) -> PhysicalLength {
        (PhysicalLength::new(self.pixel_size).cast() * font.ascent / font.units_per_em).cast()
    }
    fn descent(&self, font: &BitmapFont) -> PhysicalLength {
        (PhysicalLength::new(self.pixel_size).cast() * font.descent / font.units_per_em).cast()
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

pub(super) const DEFAULT_FONT_SIZE: LogicalLength = LogicalLength::new(12 as Coord);

// A font that is resolved to a specific pixel size.
pub struct PixelFont {
    bitmap_font: &'static BitmapFont,
    pub glyphs: &'static BitmapGlyphs,
}

impl PixelFont {
    pub fn pixel_size(&self) -> PhysicalLength {
        self.glyphs.pixel_size()
    }
    pub fn glyph_index_to_glyph_id(index: usize) -> core::num::NonZeroU16 {
        core::num::NonZeroU16::new(index as u16 + 1).unwrap()
    }
    pub fn glyph_id_to_glyph_index(id: core::num::NonZeroU16) -> usize {
        id.get() as usize - 1
    }
}

impl TextShaper for PixelFont {
    type LengthPrimitive = i16;
    type Length = PhysicalLength;
    fn shape_text<GlyphStorage: core::iter::Extend<Glyph<PhysicalLength>>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    ) {
        let glyphs_iter = text.char_indices().map(|(byte_offset, char)| {
            let glyph_index = self
                .bitmap_font
                .character_map
                .binary_search_by_key(&char, |char_map_entry| char_map_entry.code_point)
                .ok()
                .map(|char_map_index| {
                    self.bitmap_font.character_map[char_map_index].glyph_index as usize
                });
            let x_advance = glyph_index.map_or_else(
                || self.pixel_size(),
                |glyph_index| PhysicalLength::new(self.glyphs.glyph_data[glyph_index].x_advance),
            );
            Glyph {
                glyph_id: glyph_index.map(|index| Self::glyph_index_to_glyph_id(index)),
                advance: x_advance,
                text_byte_offset: byte_offset,
                ..Default::default()
            }
        });
        glyphs.extend(glyphs_iter);
    }

    fn glyph_for_char(&self, ch: char) -> Option<Glyph<PhysicalLength>> {
        self.bitmap_font
            .character_map
            .binary_search_by_key(&ch, |char_map_entry| char_map_entry.code_point)
            .ok()
            .map(|char_map_index| {
                let glyph_index =
                    self.bitmap_font.character_map[char_map_index].glyph_index as usize;
                let bitmap_glyph = &self.glyphs.glyph_data[glyph_index as usize];
                let x_advance = PhysicalLength::new(bitmap_glyph.x_advance);
                Glyph {
                    glyph_id: Some(Self::glyph_index_to_glyph_id(glyph_index)),
                    advance: x_advance,
                    text_byte_offset: 0,
                    ..Default::default()
                }
            })
    }
}

impl crate::textlayout::FontMetrics<PhysicalLength> for PixelFont {
    fn ascent(&self) -> PhysicalLength {
        self.glyphs.ascent(self.bitmap_font)
    }

    fn height(&self) -> PhysicalLength {
        self.glyphs.height(self.bitmap_font)
    }

    fn descent(&self) -> PhysicalLength {
        self.glyphs.descent(self.bitmap_font)
    }
}

pub fn match_font(request: &FontRequest, scale_factor: ScaleFactor) -> PixelFont {
    let font = FONTS.with(|fonts| {
        let fonts = fonts.borrow();
        let fallback_font = *fonts
            .first()
            .expect("The software renderer requires enabling the `EmbedForSoftwareRenderer` option when compiling slint files.");

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
        (request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE).cast() * scale_factor).cast();

    let nearest_pixel_size = font
        .glyphs
        .partition_point(|glyphs| glyphs.pixel_size() <= requested_pixel_size)
        .saturating_sub(1);

    let matching_glyphs = &font.glyphs[nearest_pixel_size];

    PixelFont { bitmap_font: font, glyphs: matching_glyphs }
}

pub fn text_layout_for_font<'a>(
    font: &'a PixelFont,
    font_request: &FontRequest,
    scale_factor: ScaleFactor,
) -> TextLayout<'a, PixelFont> {
    let letter_spacing =
        font_request.letter_spacing.map(|spacing| (spacing.cast() * scale_factor).cast());

    TextLayout { font, letter_spacing }
}

pub fn register_bitmap_font(font_data: &'static BitmapFont) {
    FONTS.with(|fonts| fonts.borrow_mut().push(font_data))
}

pub fn text_size(
    font_request: FontRequest,
    text: &str,
    max_width: Option<LogicalLength>,
    scale_factor: ScaleFactor,
) -> LogicalSize {
    let font = match_font(&font_request, scale_factor);
    let layout = text_layout_for_font(&font, &font_request, scale_factor);

    let (longest_line_width, height) =
        layout.text_size(text, max_width.map(|max_width| (max_width.cast() * scale_factor).cast()));

    (PhysicalSize::from_lengths(longest_line_width, height).cast() / scale_factor).cast()
}
