// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::diagnostics::BuildDiagnostics;
#[cfg(not(target_arch = "wasm32"))]
use crate::embedded_resources::{BitmapFont, BitmapGlyph, BitmapGlyphs, CharacterMapEntry};
#[cfg(not(target_arch = "wasm32"))]
use crate::expression_tree::BuiltinFunction;
use crate::expression_tree::{Expression, Unit};
use crate::object_tree::*;
use std::collections::HashSet;
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
pub fn embed_glyphs<'a>(
    _component: &Rc<Component>,
    _scale_factor: f64,
    _pixel_sizes: Vec<i16>,
    _characters_seen: HashSet<char>,
    _all_docs: impl Iterator<Item = &'a crate::object_tree::Document> + 'a,
    _diag: &mut BuildDiagnostics,
) -> bool {
    false
}

#[cfg(not(target_arch = "wasm32"))]
pub fn embed_glyphs<'a>(
    component: &Rc<Component>,
    scale_factor: f64,
    mut pixel_sizes: Vec<i16>,
    mut characters_seen: HashSet<char>,
    all_docs: impl Iterator<Item = &'a crate::object_tree::Document> + 'a,
    diag: &mut BuildDiagnostics,
) {
    characters_seen.extend(
        ('a'..='z')
            .chain('A'..='Z')
            .chain('0'..='9')
            .chain(" !\"#$%&'()*+,-./:;<=>?@\\]^_|~".chars())
            .chain(std::iter::once('…')),
    );

    if let Ok(sizes_str) = std::env::var("SLINT_FONT_SIZES") {
        for custom_size in sizes_str.split(',').map(|size_str| {
            (size_str.parse::<f64>().expect("invalid font size") * scale_factor) as i16
        }) {
            if let Err(pos) = pixel_sizes.binary_search(&custom_size) {
                pixel_sizes.insert(pos, custom_size)
            }
        }
    }

    let mut fontdb = fontdb::Database::new();
    fontdb.load_system_fonts();

    #[cfg(not(any(
        target_family = "windows",
        target_os = "macos",
        target_os = "ios",
        target_arch = "wasm32"
    )))]
    {
        let default_sans_serif_family = {
            let mut fontconfig_fallback_families = fontconfig::find_families("sans-serif");
            if fontconfig_fallback_families.len() == 0 {
                panic!("internal error: unable to resolve 'sans-serif' with fontconfig");
            }
            fontconfig_fallback_families.remove(0)
        };
        fontdb.set_sans_serif_family(default_sans_serif_family);
    }

    let (fallback_fonts, fallback_font) = get_fallback_fonts(&fontdb);

    let mut custom_fonts = Vec::new();

    // add custom fonts
    for doc in all_docs {
        for (font_path, import_token) in doc.custom_fonts.iter() {
            let face_count = fontdb.faces().len();
            if let Err(e) = fontdb.load_font_file(&font_path) {
                diag.push_error(format!("Error loading font: {}", e), import_token);
            } else {
                custom_fonts.extend(fontdb.faces()[face_count..].iter().map(|info| info.id))
            }
        }
    }

    let (default_font_face_id, default_font_path) = {
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
        let face_id = fontdb.query(&query).unwrap_or_else(|| {
            if let Some(source_location) = source_location {
                diag.push_warning_with_span(format!("could not find font that provides specified family, falling back to Sans-Serif"), source_location);
            }
            fallback_font
        });

        let face_info = fontdb
            .face(face_id)
            .expect("internal error: fontdb query returned a font that does not exist");
        (
            face_id,
            match &face_info.source {
                fontdb::Source::File(path) => path.to_string_lossy().to_string(),
                _ => panic!("internal errormemory fonts are not supported in the compiler"),
            },
        )
    };

    // Map from path to family name
    let mut fonts = std::collections::BTreeMap::<String, fontdb::ID>::new();
    fonts.insert(default_font_path.clone(), default_font_face_id);

    // add custom fonts
    fonts.extend(custom_fonts.iter().filter_map(|face_id| {
        fontdb.face(*face_id).map(|face_info| {
            (
                match &face_info.source {
                    fontdb::Source::File(path) => path.to_string_lossy().to_string(),
                    _ => panic!("internal errormemory fonts are not supported in the compiler"),
                },
                *face_id,
            )
        })
    }));

    let embed_font_by_path_and_face_id = |path, face_id| {
        let font = fontdb
            .with_face_data(face_id, |font_data, face_index| {
                let font = fontdue::Font::from_bytes(
        font_data,
        fontdue::FontSettings { collection_index: face_index, scale: 40. },
    )
    .expect("internal error: fontdb returned a font that ttf-parser/fontdue could not parse");
                embed_font(
                    fontdb.face(face_id).unwrap().family.clone(),
                    font,
                    &pixel_sizes,
                    characters_seen.iter().cloned(),
                    &fallback_fonts,
                )
            })
            .unwrap();

        let resource_id = component.embedded_file_resources.borrow().len();
        component.embedded_file_resources.borrow_mut().insert(
            path,
            crate::embedded_resources::EmbeddedResources {
                id: resource_id,
                kind: crate::embedded_resources::EmbeddedResourcesKind::BitmapFontData(font),
            },
        );

        component.root_element.borrow().init_code.borrow_mut().push(Expression::FunctionCall {
            function: Box::new(Expression::BuiltinFunctionReference(
                BuiltinFunction::RegisterBitmapFont,
                None,
            )),
            arguments: vec![Expression::NumberLiteral(resource_id as _, Unit::None)],
            source_location: None,
        });
    };

    // Make sure to embed the default font first, because that becomes the default at run-time.
    embed_font_by_path_and_face_id(
        default_font_path.clone(),
        fonts.remove(&default_font_path).unwrap(),
    );

    for (path, face_id) in fonts {
        embed_font_by_path_and_face_id(path, face_id);
    }
}

#[inline(never)] // workaround https://github.com/rust-lang/rust/issues/104099
fn get_fallback_fonts(fontdb: &fontdb::Database) -> (Vec<fontdue::Font>, fontdb::ID) {
    let fallback_families = if cfg!(target_os = "macos") {
        ["Menlo", "Apple Symbols", "Apple Color Emoji"].iter()
    } else if cfg!(not(any(
        target_family = "windows",
        target_os = "macos",
        target_os = "ios",
        target_arch = "wasm32"
    ))) {
        ["Noto Sans Symbols", "Noto Sans Symbols2", "DejaVu Sans"].iter()
    } else {
        [].iter()
    };
    let fallback_fonts = fallback_families
        .filter_map(|fallback_family| {
            fontdb
                .query(&fontdb::Query {
                    families: &[fontdb::Family::Name(*fallback_family)],
                    ..Default::default()
                })
                .and_then(|face_id| {
                    fontdb
                        .with_face_data(face_id, |face_data, face_index| {
                            fontdue::Font::from_bytes(
                                face_data,
                                fontdue::FontSettings { collection_index: face_index, scale: 40. },
                            )
                            .ok()
                        })
                        .flatten()
                })
        })
        .collect::<Vec<_>>();
    let fallback_font = fontdb
        .query(&fontdb::Query { families: &[fontdb::Family::SansSerif], ..Default::default() })
        .expect("internal error: Failed to locate default system font");
    (fallback_fonts, fallback_font)
}

#[cfg(not(target_arch = "wasm32"))]
fn embed_font(
    family_name: String,
    font: fontdue::Font,
    pixel_sizes: &[i16],
    character_coverage: impl Iterator<Item = char>,
    fallback_fonts: &[fontdue::Font],
) -> BitmapFont {
    let mut character_map: Vec<CharacterMapEntry> = character_coverage
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
                let (metrics, bitmap) = core::iter::once(&font)
                    .chain(fallback_fonts.iter())
                    .find_map(|font| {
                        font.chars()
                            .contains_key(code_point)
                            .then(|| font.rasterize(*code_point, *pixel_size as _))
                    })
                    .unwrap_or_else(|| font.rasterize(*code_point, *pixel_size as _));

                let glyph = BitmapGlyph {
                    x: i16::try_from(metrics.xmin).expect("large glyph x coordinate"),
                    y: i16::try_from(metrics.ymin).expect("large glyph y coordinate"),
                    width: i16::try_from(metrics.width).expect("large width"),
                    height: i16::try_from(metrics.height).expect("large height"),
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

fn try_extract_font_size_from_element(elem: &ElementRc, property_name: &str) -> Option<f64> {
    elem.borrow().bindings.get(property_name).and_then(|expression| {
        match &expression.borrow().expression {
            Expression::NumberLiteral(value, Unit::Px) => Some(*value),
            _ => None,
        }
    })
}

pub fn collect_font_sizes_used(
    component: &Rc<Component>,
    scale_factor: f64,
    sizes_seen: &mut Vec<i16>,
) {
    let mut add_font_size = |logical_size: f64| {
        let pixel_size = (logical_size * scale_factor) as i16;
        match sizes_seen.binary_search(&pixel_size) {
            Ok(_) => {}
            Err(pos) => sizes_seen.insert(pos, pixel_size),
        }
    };

    recurse_elem_including_sub_components(component, &(), &mut |elem, _| match elem
        .borrow()
        .base_type
        .to_string()
        .as_str()
    {
        "TextInput" | "Text" => {
            if let Some(font_size) = try_extract_font_size_from_element(elem, "font-size") {
                add_font_size(font_size)
            }
        }
        "Dialog" | "Window" | "WindowItem" => {
            if let Some(font_size) = try_extract_font_size_from_element(elem, "default-font-size") {
                add_font_size(font_size)
            }
        }
        _ => {}
    });
}

pub fn scan_string_literals(component: &Rc<Component>, characters_seen: &mut HashSet<char>) {
    visit_all_expressions(component, |expr, _| {
        expr.visit_recursive(&mut |expr| {
            if let Expression::StringLiteral(string) = expr {
                characters_seen.extend(string.chars());
            }
        })
    })
}

#[cfg(not(any(
    target_family = "windows",
    target_os = "macos",
    target_os = "ios",
    target_arch = "wasm32"
)))]
mod fontconfig;
