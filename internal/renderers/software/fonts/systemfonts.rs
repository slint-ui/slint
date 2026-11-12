// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore fallbackfont
use core::cell::RefCell;

use std::collections::HashMap;

use i_slint_common::sharedfontique::{HashedBlob, fontique};
use i_slint_core::lengths::ScaleFactor;
use i_slint_core::textlayout::sharedparley::FontContext;

use super::super::PhysicalLength;
use super::vectorfont::VectorFont;

struct CachedFontInfo {
    swash_key: swash::CacheKey,
    swash_offset: u32,
}

i_slint_core::thread_local! {
    // swash font info cached and indexed by fontique blob id (unique incremental) and true type collection index
    static SWASH_FONTS: RefCell<HashMap<(HashedBlob, u32), CachedFontInfo>> = Default::default();
}

pub fn get_swash_font_info(blob: &fontique::Blob<u8>, index: u32) -> (swash::CacheKey, u32) {
    SWASH_FONTS.with(|font_cache| {
        let mut cache = font_cache.borrow_mut();
        let info = cache.entry((blob.clone().into(), index)).or_insert_with(move || {
            let font_ref = swash::FontRef::from_index(blob.data(), index as usize)
                .expect("fatal: swash is unable to parse truetype font");
            CachedFontInfo { swash_key: font_ref.key, swash_offset: font_ref.offset }
        });
        (info.swash_key, info.swash_offset)
    })
}

fn get_swash_font_info_for_query_font(font: &fontique::QueryFont) -> (swash::CacheKey, u32) {
    get_swash_font_info(&font.blob, font.index)
}

pub fn match_font(
    request: &super::FontRequest,
    scale_factor: super::ScaleFactor,
    font_context: &mut FontContext,
) -> Option<VectorFont> {
    if request.family.is_some() {
        let requested_pixel_size: PhysicalLength =
            (request.pixel_size.unwrap_or(super::DEFAULT_FONT_SIZE).cast() * scale_factor).cast();

        if let Some(font) = font_context.query_font(request) {
            let font = font.font.clone();
            let (swash_key, swash_offset) = get_swash_font_info_for_query_font(&font);
            Some(VectorFont::new(font, swash_key, swash_offset, requested_pixel_size))
        } else {
            None
        }
    } else {
        None
    }
}

pub fn fallbackfont(
    font_request: &super::FontRequest,
    scale_factor: ScaleFactor,
    font_context: &mut FontContext,
) -> VectorFont {
    let requested_pixel_size: PhysicalLength =
        (font_request.pixel_size.unwrap_or(super::DEFAULT_FONT_SIZE).cast() * scale_factor).cast();

    let font = font_context.query_font(font_request).unwrap().font.clone();
    let (swash_key, swash_offset) = get_swash_font_info_for_query_font(&font);
    VectorFont::new(font, swash_key, swash_offset, requested_pixel_size)
}

#[cfg(not(target_family = "wasm"))]
pub fn register_font_from_path(
    font_context: &mut FontContext,
    path: &std::path::Path,
) -> Result<(), alloc::boxed::Box<dyn std::error::Error>> {
    let requested_path = path.canonicalize().unwrap_or_else(|_| path.into());
    let contents = std::fs::read(requested_path)?;
    font_context.register_fonts(contents.into());
    Ok(())
}
