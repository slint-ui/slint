// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::cell::RefCell;

use alloc::boxed::Box;
use alloc::rc::Rc;
use std::collections::HashMap;

use crate::lengths::ScaleFactor;
use i_slint_common::sharedfontique::{self, fontique};

use super::super::PhysicalLength;
use super::vectorfont::VectorFont;

struct CachedFont {
    fontdue_font: Rc<fontdue::Font>,
    // Keep a strong reference to the blob, otherwise it disappears. The fontique collection
    // is temporary, and the shared cache keeps only weak references.
    _fontique_blob: fontique::Blob<u8>,
}

crate::thread_local! {
    // fontdue fonts cached and indexed by fontique blob id (unique incremental) and true type collection index
    static FONTDUE_FONTS: RefCell<HashMap<(u64, u32), CachedFont>> = Default::default();
}

pub fn get_or_create_fontdue_font_from_blob_and_index(
    blob: &fontique::Blob<u8>,
    index: u32,
) -> Rc<fontdue::Font> {
    FONTDUE_FONTS.with(|font_cache| {
        font_cache
            .borrow_mut()
            .entry((blob.id(), index))
            .or_insert_with(move || CachedFont {
                fontdue_font: fontdue::Font::from_bytes(
                    blob.data(),
                    fontdue::FontSettings {
                        collection_index: index,
                        scale: 40.,
                        ..Default::default()
                    },
                )
                .expect("fatal: fontdue is unable to parse truetype font")
                .into(),
                _fontique_blob: blob.clone(),
            })
            .fontdue_font
            .clone()
    })
}

fn get_or_create_fontdue_font(font: &fontique::QueryFont) -> Rc<fontdue::Font> {
    get_or_create_fontdue_font_from_blob_and_index(&font.blob, font.index)
}

pub fn match_font(
    request: &super::FontRequest,
    scale_factor: super::ScaleFactor,
) -> Option<VectorFont> {
    if request.family.is_some() {
        let requested_pixel_size: PhysicalLength =
            (request.pixel_size.unwrap_or(super::DEFAULT_FONT_SIZE).cast() * scale_factor).cast();

        if let Some(font) = request.query_fontique() {
            let fontdue_font = get_or_create_fontdue_font(&font);
            Some(VectorFont::new(font, fontdue_font, requested_pixel_size))
        } else {
            None
        }
    } else {
        None
    }
}

pub fn fallbackfont(font_request: &super::FontRequest, scale_factor: ScaleFactor) -> VectorFont {
    let requested_pixel_size: PhysicalLength =
        (font_request.pixel_size.unwrap_or(super::DEFAULT_FONT_SIZE).cast() * scale_factor).cast();

    let font = font_request.query_fontique().unwrap();
    let fontdue_font = get_or_create_fontdue_font(&font);
    VectorFont::new(font, fontdue_font, requested_pixel_size)
}

pub fn register_font_from_memory(data: &'static [u8]) -> Result<(), Box<dyn std::error::Error>> {
    sharedfontique::get_collection().register_fonts(data.to_vec().into(), None);
    Ok(())
}

#[cfg(not(target_family = "wasm"))]
pub fn register_font_from_path(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let requested_path = path.canonicalize().unwrap_or_else(|_| path.into());
    let contents = std::fs::read(requested_path)?;
    sharedfontique::get_collection().register_fonts(contents.into(), None);
    Ok(())
}
