// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::cell::RefCell;

use alloc::boxed::Box;
use alloc::rc::Rc;
use std::collections::HashMap;

use crate::lengths::ScaleFactor;
use i_slint_common::sharedfontdb::{self, fontdb};

use super::super::PhysicalLength;
use super::vectorfont::VectorFont;

crate::thread_local! {
    static FONTDUE_FONTS: RefCell<HashMap<fontdb::ID, Rc<fontdue::Font>>> = Default::default();
}

fn get_or_create_fontdue_font(fontdb: &fontdb::Database, id: fontdb::ID) -> Rc<fontdue::Font> {
    FONTDUE_FONTS.with(|font_cache| {
        font_cache
            .borrow_mut()
            .entry(id)
            .or_insert_with(|| {
                fontdb
                    .with_face_data(id, |face_data, font_index| {
                        fontdue::Font::from_bytes(
                            face_data,
                            fontdue::FontSettings {
                                collection_index: font_index,
                                scale: 40.,
                                ..Default::default()
                            },
                        )
                        .expect("fatal: fontdue is unable to parse truetype font")
                        .into()
                    })
                    .unwrap()
            })
            .clone()
    })
}

pub fn match_font(
    request: &super::FontRequest,
    scale_factor: super::ScaleFactor,
) -> Option<VectorFont> {
    request.family.as_ref().and_then(|family_str| {
        let query = request.to_fontdb_query();

        let requested_pixel_size: PhysicalLength =
            (request.pixel_size.unwrap_or(super::DEFAULT_FONT_SIZE).cast() * scale_factor).cast();

        sharedfontdb::FONT_DB.with(|fonts| {
            let borrowed_fontdb = fonts.borrow();
            borrowed_fontdb.query_with_family(query, Some(family_str)).map(|font_id| {
                let fontdue_font = get_or_create_fontdue_font(&borrowed_fontdb, font_id);
                VectorFont::new(font_id, fontdue_font.clone(), requested_pixel_size)
            })
        })
    })
}

pub fn fallbackfont(font_request: &super::FontRequest, scale_factor: ScaleFactor) -> VectorFont {
    let requested_pixel_size: PhysicalLength =
        (font_request.pixel_size.unwrap_or(super::DEFAULT_FONT_SIZE).cast() * scale_factor).cast();

    sharedfontdb::FONT_DB.with_borrow(|fonts| {
        let query = font_request.to_fontdb_query();

        let fallback_font_id = fonts
            .query_with_family(query, None)
            .expect("fatal: query for fallback font returned empty font list");

        let fontdue_font = get_or_create_fontdue_font(fonts, fallback_font_id);
        VectorFont::new(fallback_font_id, fontdue_font, requested_pixel_size)
    })
}

pub fn register_font_from_memory(data: &'static [u8]) -> Result<(), Box<dyn std::error::Error>> {
    sharedfontdb::FONT_DB.with_borrow_mut(|fonts| {
        fonts.make_mut().load_font_source(fontdb::Source::Binary(std::sync::Arc::new(data)))
    });
    Ok(())
}

#[cfg(not(target_family = "wasm"))]
pub fn register_font_from_path(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let requested_path = path.canonicalize().unwrap_or_else(|_| path.into());
    sharedfontdb::FONT_DB.with_borrow_mut(|fonts| {
        for face_info in fonts.faces() {
            match &face_info.source {
                fontdb::Source::Binary(_) => {}
                fontdb::Source::File(loaded_path) | fontdb::Source::SharedFile(loaded_path, ..) => {
                    if *loaded_path == requested_path {
                        return Ok(());
                    }
                }
            }
        }

        fonts.make_mut().load_font_file(requested_path).map_err(|e| e.into())
    })
}
