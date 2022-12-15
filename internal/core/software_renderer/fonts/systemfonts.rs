// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use core::cell::RefCell;

use alloc::rc::Rc;
use std::collections::HashMap;

use crate::lengths::{LogicalLength, ScaleFactor};

use super::super::PhysicalLength;
use super::vectorfont::VectorFont;

fn init_fontdb(db: &mut fontdb::Database) {
    db.load_system_fonts();

    if db
        .query(&fontdb::Query {
            families: &[fontdb::Family::Name(db.family_name(&fontdb::Family::SansSerif))],
            ..Default::default()
        })
        .is_none()
    {
        if let Some(fallback_id) = db.query(&fontdb::Query {
            families: &[
                fontdb::Family::Name("Noto Sans"),
                fontdb::Family::Name("DejaVu Sans"),
                fontdb::Family::Name("FreeSans"),
            ],
            ..Default::default()
        }) {
            db.set_sans_serif_family(db.face(fallback_id).unwrap().family.clone());
        }
    }
}

thread_local! {
    static VECTOR_FONTS: Rc<RefCell<fontdb::Database>> = Rc::new(RefCell::new({
        let mut db = fontdb::Database::new();
        init_fontdb(&mut db);
        db
    }))
}

thread_local! {
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
                            fontdue::FontSettings { collection_index: font_index, scale: 40. },
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
    let family = request
        .family
        .as_ref()
        .map_or(fontdb::Family::SansSerif, |family| fontdb::Family::Name(family));

    let query = fontdb::Query { families: &[family], ..Default::default() };

    let requested_pixel_size: PhysicalLength =
        (request.pixel_size.unwrap_or(super::DEFAULT_FONT_SIZE).cast() * scale_factor).cast();

    VECTOR_FONTS.with(|fonts| {
        let borrowed_fontdb = fonts.borrow();
        borrowed_fontdb.query(&query).map(|font_id| {
            let fontdue_font = get_or_create_fontdue_font(&*borrowed_fontdb, font_id);
            VectorFont::new(fonts.clone(), font_id, fontdue_font.clone(), requested_pixel_size)
        })
    })
}

pub fn fallbackfont(pixel_size: Option<LogicalLength>, scale_factor: ScaleFactor) -> VectorFont {
    let query = fontdb::Query { families: &[fontdb::Family::SansSerif], ..Default::default() };

    let requested_pixel_size: PhysicalLength =
        (pixel_size.unwrap_or(super::DEFAULT_FONT_SIZE).cast() * scale_factor).cast();

    VECTOR_FONTS
        .with(|fonts| {
            fonts
                .borrow()
                .query(&query)
                .map(|font_id| {
                    let fontdue_font = get_or_create_fontdue_font(&*fonts.borrow(), font_id);
                    VectorFont::new(fonts.clone(), font_id, fontdue_font, requested_pixel_size)
                })
                .expect("fatal: fontdb could not locate a sans-serif font on the system")
        })
        .into()
}

pub fn register_font_from_memory(data: &'static [u8]) -> Result<(), Box<dyn std::error::Error>> {
    VECTOR_FONTS.with(|fonts| {
        fonts.borrow_mut().load_font_source(fontdb::Source::Binary(std::sync::Arc::new(data)))
    });
    Ok(())
}

pub fn register_font_from_path(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let requested_path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    VECTOR_FONTS.with(|fonts| {
        for face_info in fonts.borrow().faces() {
            match &face_info.source {
                fontdb::Source::Binary(_) => {}
                fontdb::Source::File(loaded_path) | fontdb::Source::SharedFile(loaded_path, ..) => {
                    if *loaded_path == requested_path {
                        return Ok(());
                    }
                }
            }
        }

        fonts.borrow_mut().load_font_file(requested_path).map_err(|e| e.into())
    })
}
