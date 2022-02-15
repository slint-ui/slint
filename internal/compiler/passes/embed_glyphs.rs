// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::diagnostics::BuildDiagnostics;
use crate::embedded_resources::{BitmapFont, BitmapGlyph, BitmapGlyphs, CharacterMapEntry};
use crate::expression_tree::{BuiltinFunction, Expression, Unit};
use crate::object_tree::*;
use std::convert::TryFrom;
use std::rc::Rc;

pub fn embed_glyphs<'a>(
    component: &Rc<Component>,
    all_docs: impl Iterator<Item = &'a crate::object_tree::Document> + 'a,
    diag: &mut BuildDiagnostics,
) -> bool {
    if std::env::var("SLINT_EMBED_GLYPHS").is_err() {
        return false;
    }

    let mut fontdb = fontdb::Database::new();
    fontdb.load_system_fonts();

    let fallback_font = fontdb
        .query(&fontdb::Query { families: &[fontdb::Family::SansSerif], ..Default::default() });

    // add custom fonts
    for doc in all_docs {
        for (font_path, import_token) in doc.custom_fonts.iter() {
            if let Err(e) = fontdb.load_font_file(&font_path) {
                diag.push_error(format!("Error loading font: {}", e), import_token);
            }
        }
    }

    // TODO: improve heuristics in choice of which fonts to embed. use default-font-family, etc.
    let (family, source_location) = component
        .root_element
        .borrow()
        .bindings
        .get("default-font-family")
        .and_then(|binding| match &binding.borrow().expression {
            Expression::StringLiteral(family) => {
                Some((Some(family.clone()), binding.borrow().span.clone()))
            }
            _ => None,
        })
        .unwrap_or_default();

    let query = fontdb::Query {
        families: &[family
            .as_ref()
            .map_or(fontdb::Family::SansSerif, |name| fontdb::Family::Name(name))],
        ..Default::default()
    };
    let face_id = fontdb
        .query(&query)
        .or_else(|| {
            if let Some(source_location) = source_location {
                diag.push_warning_with_span(format!("could not find font that provides specified family, falling back to Sans-Serif"), source_location);
            }
            fallback_font
        })
        .expect("internal error: Unable to match any font for embedding");

    let (family_name, path) = {
        let face_info = fontdb
            .face(face_id)
            .expect("internal error: fontdb query returned a font that does not exist");
        (
            face_info.family.clone(),
            match &face_info.source {
                fontdb::Source::File(path) => path.clone(),
                _ => panic!("internal errormemory fonts are not supported in the compiler"),
            },
        )
    };

    let font =
        fontdb
            .with_face_data(face_id, |font_data, face_index| {
                let font = fontdue::Font::from_bytes(
            font_data,
            fontdue::FontSettings { collection_index: face_index, scale: 40. },
        )
        .expect("internal error: fontdb returned a font that ttf-parser/fontdue could not parse");
                embed_font(family_name, font)
            })
            .unwrap();

    let resource_id = component.embedded_file_resources.borrow().len();
    component.embedded_file_resources.borrow_mut().insert(
        path.to_string_lossy().to_string(),
        crate::embedded_resources::EmbeddedResources {
            id: resource_id,
            kind: crate::embedded_resources::EmbeddedResourcesKind::BitmapFontData(font),
        },
    );

    component.setup_code.borrow_mut().push(Expression::FunctionCall {
        function: Box::new(Expression::BuiltinFunctionReference(
            BuiltinFunction::RegisterBitmapFont,
            None,
        )),
        arguments: vec![Expression::NumberLiteral(resource_id as _, Unit::None)],
        source_location: None,
    });

    true
}

fn embed_font(family_name: String, font: fontdue::Font) -> BitmapFont {
    let mut pixel_sizes = std::env::var("SLINT_FONT_SIZES")
        .map(|sizes_str| {
            sizes_str
                .split(',')
                .map(|size_str| size_str.parse::<u16>().expect("invalid font size"))
                .collect::<Vec<_>>()
        })
        .expect("please specify SLINT_FONT_SIZES");
    pixel_sizes.sort();

    // TODO: configure coverage
    let coverage =
        ('a'..='z').chain('A'..='Z').chain('0'..='9').chain([' ', '.', '+', '-', '!', '%']);

    let mut character_map: Vec<CharacterMapEntry> = coverage
        .enumerate()
        .map(|(glyph_index, code_point)| CharacterMapEntry {
            code_point,
            glyph_index: u16::try_from(glyph_index)
                .expect("more than 65535 glyphs are not supported"),
        })
        .collect();
    character_map.sort_by_key(|entry| entry.code_point);

    let glyphs = pixel_sizes
        .iter()
        .map(|pixel_size| {
            let mut glyph_data = Vec::new();
            glyph_data.resize(character_map.len(), Default::default());

            for CharacterMapEntry { code_point, glyph_index } in &character_map {
                let (metrics, bitmap) = font.rasterize(*code_point, *pixel_size as _);
                let glyph = BitmapGlyph {
                    x: i16::try_from(metrics.xmin).expect("large glyph x coordinate"),
                    y: i16::try_from(metrics.ymin).expect("large glyph y coordinate"),
                    width: u16::try_from(metrics.width).expect("large width"),
                    height: u16::try_from(metrics.height).expect("large height"),
                    x_advance: i16::try_from(metrics.advance_width as i64)
                        .expect("large advance width"),
                    data: bitmap,
                };
                glyph_data[*glyph_index as usize] = glyph;
            }

            BitmapGlyphs { pixel_size: *pixel_size, glyph_data }
        })
        .collect();

    // Get the basic metrics in design coordinates
    let metrics = font
        .horizontal_line_metrics(font.units_per_em())
        .expect("encountered font without hmtx table");

    BitmapFont {
        family_name,
        character_map,
        units_per_em: font.units_per_em(),
        ascent: metrics.ascent,
        descent: metrics.descent,
        glyphs,
    }
}
