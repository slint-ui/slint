// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use alloc::vec::Vec;
use core::cell::RefCell;
use core::convert::TryFrom;

#[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
use i_slint_core::thread_local_ as thread_local;

use i_slint_core::graphics::{BitmapFont, BitmapGlyph, BitmapGlyphs, FontRequest, Size};

thread_local! {
    static FONTS: RefCell<Vec<&'static BitmapFont>> = RefCell::default()
}

pub const DEFAULT_FONT_SIZE: u16 = 12;

pub fn match_font(request: &FontRequest) -> (&'static BitmapFont, &'static BitmapGlyphs) {
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

    let requested_pixel_size = request
        .pixel_size
        .and_then(|size| u16::try_from(size as i64).ok())
        .unwrap_or(DEFAULT_FONT_SIZE);

    let nearest_pixel_size = font
        .glyphs
        .partition_point(|glyphs| glyphs.pixel_size <= requested_pixel_size)
        .saturating_sub(1);

    let matching_glyphs = &font.glyphs[nearest_pixel_size];

    (font, matching_glyphs)
}

pub fn register_bitmap_font(font_data: &'static BitmapFont) {
    FONTS.with(|fonts| fonts.borrow_mut().push(font_data))
}

pub fn glyphs_for_text<'a>(
    font: &'static BitmapFont,
    glyphs: &'static BitmapGlyphs,
    text: &'a str,
) -> impl Iterator<Item = (f32, &'static BitmapGlyph)> + 'a {
    let mut x: f32 = 0.;
    text.chars().filter_map(move |char| {
        if let Some(glyph_index) = font
            .character_map
            .binary_search_by_key(&char, |char_map_entry| char_map_entry.code_point)
            .ok()
            .map(|char_map_index| font.character_map[char_map_index].glyph_index)
        {
            let glyph = &glyphs.glyph_data[glyph_index as usize];
            let glyph_x = x;
            x += glyph.x_advance as f32;
            Some((glyph_x, glyph))
        } else {
            x += glyphs.pixel_size as f32;
            None
        }
    })
}

pub fn text_size(font_request: FontRequest, text: &str, _max_width: Option<f32>) -> Size {
    let (font, glyphs) = match_font(&font_request);

    let width = glyphs_for_text(font, glyphs, text)
        .last()
        .map_or(0., |(last_x, last_glyph)| last_x + last_glyph.x_advance as f32);

    let height = font.ascent * (glyphs.pixel_size as f32) / font.units_per_em;

    Size::new(width, height)
}
