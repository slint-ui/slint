// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use core::cell::RefCell;

use alloc::rc::Rc;
use std::collections::HashMap;

use crate::lengths::{LogicalLength, ScaleFactor};

use super::super::PhysicalLength;
use super::vectorfont::VectorFont;

fn init_fontdb() -> FontDatabase {
    let mut db = fontdb::Database::new();
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
                fontdb::Family::Name("Liberation Sans"),
            ],
            ..Default::default()
        }) {
            if let Some(family_name) =
                db.face(fallback_id).unwrap().families.first().map(|(name, _)| name.clone())
            {
                db.set_sans_serif_family(family_name);
            }
        }
    }

    let fallback_font_id = std::env::var_os("SLINT_DEFAULT_FONT").and_then(|maybe_font_path| {
        let path = std::path::Path::new(&maybe_font_path);
        if path.extension().is_some() {
            let face_count = db.len();
            match db.load_font_file(path) {
                Ok(()) => {
                    db.faces().get(face_count).map(|face_info| face_info.id)
                },
                Err(err) => {
                    eprintln!(
                        "Could not load the font set via `SLINT_DEFAULT_FONT`: {}: {}", path.display(), err,                        
                    );
                    None
                },
            }
        } else {
            eprintln!(
                "The environment variable `SLINT_DEFAULT_FONT` is set, but its value is not referring to a file",

            );
            None
        }
    }).unwrap_or_else(|| {
        let query = fontdb::Query { families: &[fontdb::Family::SansSerif], ..Default::default() };

        db.query(&query).expect("fatal: fontdb could not locate a sans-serif font on the system")
    });

    FontDatabase { db, fallback_font_id }
}

#[derive(derive_more::Deref, derive_more::DerefMut)]
pub struct FontDatabase {
    #[deref]
    #[deref_mut]
    db: fontdb::Database,
    fallback_font_id: fontdb::ID,
}

thread_local! {
    static VECTOR_FONTS: Rc<RefCell<FontDatabase>> = Rc::new(RefCell::new(init_fontdb()))
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
    request.family.as_ref().and_then(|family_str| {
        let family = fontdb::Family::Name(family_str);

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
    })
}

pub fn fallbackfont(pixel_size: Option<LogicalLength>, scale_factor: ScaleFactor) -> VectorFont {
    let requested_pixel_size: PhysicalLength =
        (pixel_size.unwrap_or(super::DEFAULT_FONT_SIZE).cast() * scale_factor).cast();

    VECTOR_FONTS
        .with(|fonts| {
            let fonts_borrowed = fonts.borrow();

            let fontdue_font =
                get_or_create_fontdue_font(&*fonts_borrowed, fonts_borrowed.fallback_font_id);
            VectorFont::new(
                fonts.clone(),
                fonts_borrowed.fallback_font_id,
                fontdue_font,
                requested_pixel_size,
            )
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
