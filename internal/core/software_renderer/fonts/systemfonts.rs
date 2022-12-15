// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use core::cell::RefCell;

use alloc::rc::Rc;

use crate::lengths::{LogicalLength, ScaleFactor};

use super::super::PhysicalLength;
use super::vectorfont::VectorFont;

thread_local! {
    pub static VECTOR_FONTS: Rc<RefCell<fontdb::Database>> = Rc::new(RefCell::new({
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    }))
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
        fonts
            .borrow()
            .query(&query)
            .map(|font_id| VectorFont::new(fonts.clone(), font_id, requested_pixel_size))
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
                .map(|font_id| VectorFont::new(fonts.clone(), font_id, requested_pixel_size))
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
