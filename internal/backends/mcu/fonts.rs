// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use alloc::vec::Vec;
use core::cell::RefCell;

#[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
use i_slint_core::thread_local_ as thread_local;

use crate::{LogicalLength, LogicalSize, PhysicalLength, PhysicalSize, ScaleFactor};
use euclid::num::Zero;
use i_slint_core::{
    graphics::{BitmapFont, BitmapGlyph, BitmapGlyphs, FontRequest},
    slice::Slice,
};

thread_local! {
    static FONTS: RefCell<Vec<&'static BitmapFont>> = RefCell::default()
}

#[derive(Copy, Clone)]
pub struct Glyph(&'static BitmapGlyph);

impl Glyph {
    pub fn x(&self) -> PhysicalLength {
        PhysicalLength::new(self.0.x)
    }
    pub fn y(&self) -> PhysicalLength {
        PhysicalLength::new(self.0.y)
    }
    pub fn width(&self) -> PhysicalLength {
        PhysicalLength::new(self.0.width)
    }
    pub fn height(&self) -> PhysicalLength {
        PhysicalLength::new(self.0.height)
    }
    pub fn size(&self) -> PhysicalSize {
        PhysicalSize::from_lengths(self.width(), self.height())
    }
    pub fn x_advance(&self) -> PhysicalLength {
        PhysicalLength::new(self.0.x_advance)
    }
    pub fn data(&self) -> &Slice<'static, u8> {
        &self.0.data
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

pub const DEFAULT_FONT_SIZE: f32 = 12.0;

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

    pub fn glyphs_for_text<'a>(
        &'a self,
        text: &'a str,
    ) -> impl Iterator<Item = (PhysicalLength, Glyph)> + 'a {
        let mut x: PhysicalLength = PhysicalLength::zero();
        text.chars().filter_map(move |char| {
            if let Some(glyph_index) = self
                .bitmap_font
                .character_map
                .binary_search_by_key(&char, |char_map_entry| char_map_entry.code_point)
                .ok()
                .map(|char_map_index| self.bitmap_font.character_map[char_map_index].glyph_index)
            {
                let glyph = Glyph(&self.glyphs.glyph_data[glyph_index as usize]);
                let glyph_x = x;
                x += glyph.x_advance();
                Some((glyph_x, glyph))
            } else {
                x += self.pixel_size();
                None
            }
        })
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
        (LogicalLength::new(request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE)) * scale_factor).cast();

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
    _max_width: Option<f32>,
    scale_factor: ScaleFactor,
) -> LogicalSize {
    let font = match_font(&font_request, scale_factor);

    let width = font
        .glyphs_for_text(text)
        .last()
        .map_or(PhysicalLength::zero(), |(last_x, last_glyph)| last_x + last_glyph.x_advance());

    PhysicalSize::from_lengths(width, font.height()).cast() / scale_factor
}
