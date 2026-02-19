// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::CompilerConfiguration;
use crate::diagnostics::BuildDiagnostics;
#[cfg(not(target_arch = "wasm32"))]
use crate::embedded_resources::{BitmapFont, BitmapGlyph, BitmapGlyphs, CharacterMapEntry};
#[cfg(not(target_arch = "wasm32"))]
use crate::expression_tree::BuiltinFunction;
use crate::expression_tree::{Expression, Unit};
use crate::object_tree::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;

use i_slint_common::sharedfontique::{self, fontique, ttf_parser};

#[derive(Clone)]
struct Font {
    font: fontique::QueryFont,
}

fn swash_font_ref(font: &Font) -> swash::FontRef<'_> {
    swash::FontRef::from_index(font.font.blob.data(), font.font.index as usize).unwrap()
}

#[cfg(target_arch = "wasm32")]
pub fn embed_glyphs<'a>(
    _component: &Document,
    _compiler_config: &CompilerConfiguration,
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
    doc: &Document,
    compiler_config: &CompilerConfiguration,
    mut pixel_sizes: Vec<i16>,
    mut characters_seen: HashSet<char>,
    all_docs: impl Iterator<Item = &'a crate::object_tree::Document> + 'a,
    diag: &mut BuildDiagnostics,
) {
    use crate::diagnostics::Spanned;

    let generic_diag_location = doc.node.as_ref().map(|n| n.to_source_location());
    let scale_factor = compiler_config.const_scale_factor.unwrap_or(1.);

    characters_seen.extend(
        ('a'..='z')
            .chain('A'..='Z')
            .chain('0'..='9')
            .chain(" '!\"#$%&()*+,-./:;<=>?@\\[]{}^_|~".chars())
            .chain(std::iter::once('●'))
            .chain(std::iter::once('…')),
    );

    if let Ok(sizes_str) = std::env::var("SLINT_FONT_SIZES") {
        for custom_size_str in sizes_str.split(',') {
            let custom_size = if let Ok(custom_size) = custom_size_str
                .parse::<f32>()
                .map(|size_as_float| (size_as_float * scale_factor) as i16)
            {
                custom_size
            } else {
                diag.push_error(
                    format!(
                        "Invalid font size '{custom_size_str}' specified in `SLINT_FONT_SIZES`"
                    ),
                    &generic_diag_location,
                );
                return;
            };

            if let Err(pos) = pixel_sizes.binary_search(&custom_size) {
                pixel_sizes.insert(pos, custom_size)
            }
        }
    }

    let fallback_fonts = get_fallback_fonts();

    let mut custom_fonts: HashMap<std::path::PathBuf, fontique::QueryFont> = Default::default();
    let mut font_paths: HashMap<fontique::FamilyId, std::path::PathBuf> = Default::default();

    let mut collection = sharedfontique::create_collection(false);

    for doc in all_docs {
        for (font_path, import_token) in doc.custom_fonts.iter() {
            match std::fs::read(&font_path) {
                Err(e) => {
                    diag.push_error(format!("Error loading font: {e}"), import_token);
                }
                Ok(bytes) => {
                    if let Some(font) = collection
                        .register_fonts(bytes.into(), None)
                        .first()
                        .and_then(|(id, infos)| {
                            let info = infos.first()?;
                            collection.get_font_for_info(*id, info)
                        })
                    {
                        font_paths.insert(font.family.0, font_path.into());
                        custom_fonts.insert(font_path.into(), font);
                    }
                }
            }
        }
    }

    let mut custom_face_error = false;

    let default_fonts = if !collection.default_fonts.is_empty() {
        collection.default_fonts.as_ref().clone()
    } else {
        let mut default_fonts: HashMap<std::path::PathBuf, fontique::QueryFont> =
            Default::default();

        for c in doc.exported_roots() {
            let (family, source_location) = c
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

            let font = {
                let mut query = collection.query();

                query.set_families(
                    family
                        .as_ref()
                        .map(|family| fontique::QueryFamily::from(family.as_str()))
                        .into_iter()
                        .chain(
                            sharedfontique::FALLBACK_FAMILIES
                                .into_iter()
                                .map(fontique::QueryFamily::Generic),
                        ),
                );

                let mut font = None;

                query.matches_with(|queried_font| {
                    font = Some(queried_font.clone());
                    fontique::QueryStatus::Stop
                });
                font
            };

            match font {
                None => {
                    if let Some(source_location) = source_location {
                        diag.push_error_with_span("could not find font that provides specified family, falling back to Sans-Serif".to_string(), source_location);
                    } else {
                        diag.push_error(
                            "internal error: could not determine a default font for sans-serif"
                                .to_string(),
                            &generic_diag_location,
                        );
                    };
                }
                Some(query_font) => {
                    if let Some(font_info) = collection
                        .family(query_font.family.0)
                        .and_then(|family_info| family_info.fonts().first().cloned())
                    {
                        let path = if let Some(path) = font_paths.get(&query_font.family.0) {
                            path.clone()
                        } else {
                            match &font_info.source().kind {
                                fontique::SourceKind::Path(path) => path.to_path_buf(),
                                fontique::SourceKind::Memory(_) => {
                                    diag.push_error(
                                    "internal error: memory fonts are not supported in the compiler"
                                        .to_string(),
                                    &generic_diag_location,
                                );
                                    custom_face_error = true;
                                    continue;
                                }
                            }
                        };
                        font_paths.insert(query_font.family.0, path.clone());
                        default_fonts.insert(path.clone(), query_font);
                    }
                }
            }
        }

        default_fonts
    };

    if custom_face_error {
        return;
    }

    let mut embed_font_by_path = |path: &std::path::Path, font: &fontique::QueryFont| {
        let Some(family_name) = collection.family_name(font.family.0).to_owned() else {
            diag.push_error(
                format!(
                    "internal error: TrueType font without family name encountered: {}",
                    path.display()
                ),
                &generic_diag_location,
            );
            return;
        };

        let embedded_bitmap_font = embed_font(
            family_name.to_owned(),
            Font { font: font.clone() },
            &pixel_sizes,
            characters_seen.iter().cloned(),
            &fallback_fonts,
            compiler_config,
        );

        let resource_id = doc.embedded_file_resources.borrow().len();
        doc.embedded_file_resources.borrow_mut().insert(
            path.to_string_lossy().into(),
            crate::embedded_resources::EmbeddedResources {
                id: resource_id,
                kind: crate::embedded_resources::EmbeddedResourcesKind::BitmapFontData(
                    embedded_bitmap_font,
                ),
            },
        );

        for c in doc.exported_roots() {
            c.init_code.borrow_mut().font_registration_code.push(Expression::FunctionCall {
                function: BuiltinFunction::RegisterBitmapFont.into(),
                arguments: vec![Expression::NumberLiteral(resource_id as _, Unit::None)],
                source_location: None,
            });
        }
    };

    for (path, font) in default_fonts.iter() {
        custom_fonts.remove(&path.to_owned());
        embed_font_by_path(&path, &font);
    }

    for (path, font) in custom_fonts {
        embed_font_by_path(&path, &font);
    }
}

#[inline(never)] // workaround https://github.com/rust-lang/rust/issues/104099
fn get_fallback_fonts() -> Vec<Font> {
    let mut fallback_fonts = Vec::new();

    let mut collection = sharedfontique::create_collection(false);
    let mut query = collection.query();
    query.set_families(
        sharedfontique::FALLBACK_FAMILIES.into_iter().map(fontique::QueryFamily::Generic).chain(
            core::iter::once(fontique::QueryFamily::Generic(fontique::GenericFamily::Emoji)),
        ),
    );

    query.matches_with(|query_font| {
        fallback_fonts.push(Font { font: query_font.clone() });
        fontique::QueryStatus::Continue
    });

    fallback_fonts
}

#[cfg(not(target_arch = "wasm32"))]
fn embed_font(
    family_name: String,
    font: Font,
    pixel_sizes: &[i16],
    character_coverage: impl Iterator<Item = char>,
    fallback_fonts: &[Font],
    _compiler_config: &CompilerConfiguration,
) -> BitmapFont {
    let mut character_map: Vec<CharacterMapEntry> = character_coverage
        .filter(|code_point| {
            core::iter::once(&font)
                .chain(fallback_fonts.iter())
                .any(|font| swash_font_ref(font).charmap().map(*code_point) != 0)
        })
        .enumerate()
        .map(|(glyph_index, code_point)| CharacterMapEntry {
            code_point,
            glyph_index: u16::try_from(glyph_index)
                .expect("more than 65535 glyphs are not supported"),
        })
        .collect();

    #[cfg(feature = "sdf-fonts")]
    let glyphs = if _compiler_config.use_sdf_fonts {
        embed_sdf_glyphs(pixel_sizes, &character_map, &font, fallback_fonts)
    } else {
        embed_alpha_map_glyphs(pixel_sizes, &character_map, &font, fallback_fonts)
    };
    #[cfg(not(feature = "sdf-fonts"))]
    let glyphs = embed_alpha_map_glyphs(pixel_sizes, &character_map, &font, fallback_fonts);

    character_map.sort_by_key(|entry| entry.code_point);

    let face_info = ttf_parser::Face::parse(font.font.blob.data(), font.font.index).unwrap();

    let metrics = sharedfontique::DesignFontMetrics::new_from_face(&face_info);

    BitmapFont {
        family_name,
        character_map,
        units_per_em: metrics.units_per_em,
        ascent: metrics.ascent,
        descent: metrics.descent,
        x_height: metrics.x_height,
        cap_height: metrics.cap_height,
        glyphs,
        weight: face_info.weight().to_number(),
        italic: face_info.style() != ttf_parser::Style::Normal,
        #[cfg(feature = "sdf-fonts")]
        sdf: _compiler_config.use_sdf_fonts,
        #[cfg(not(feature = "sdf-fonts"))]
        sdf: false,
    }
}

#[cfg(all(not(target_arch = "wasm32")))]
fn embed_alpha_map_glyphs(
    pixel_sizes: &[i16],
    character_map: &Vec<CharacterMapEntry>,
    font: &Font,
    fallback_fonts: &[Font],
) -> Vec<BitmapGlyphs> {
    use rayon::prelude::*;
    use std::cell::RefCell;

    thread_local! {
        static SCALE_CONTEXT: RefCell<swash::scale::ScaleContext> =
            RefCell::new(swash::scale::ScaleContext::new());
    }

    let glyphs = pixel_sizes
        .par_iter()
        .map(|pixel_size| {
            let glyph_data = character_map
                .par_iter()
                .map(|CharacterMapEntry { code_point, .. }| {
                    let font_to_use = core::iter::once(font)
                        .chain(fallback_fonts.iter())
                        .find(|f| swash_font_ref(f).charmap().map(*code_point) != 0)
                        .unwrap_or(font);

                    let font_ref = swash_font_ref(font_to_use);
                    let glyph_id = font_ref.charmap().map(*code_point);
                    let gm = font_ref.glyph_metrics(&[]);
                    let fm = font_ref.metrics(&[]);
                    let scale = *pixel_size as f32 / fm.units_per_em as f32;
                    let advance_width = gm.advance_width(glyph_id) * scale;

                    SCALE_CONTEXT.with(|ctx| {
                        let font_ref = swash_font_ref(font_to_use);
                        let mut ctx = ctx.borrow_mut();
                        let mut scaler = ctx.builder(font_ref).size(*pixel_size as f32).build();
                        let image = swash::scale::Render::new(&[swash::scale::Source::Outline])
                            .format(swash::zeno::Format::Alpha)
                            .render(&mut scaler, glyph_id);

                        match image {
                            Some(image) => {
                                let p = image.placement;
                                BitmapGlyph {
                                    x: i16::try_from(p.left * 64)
                                        .expect("large glyph x coordinate"),
                                    y: i16::try_from((p.top - p.height as i32) * 64)
                                        .expect("large glyph y coordinate"),
                                    width: i16::try_from(p.width).expect("large width"),
                                    height: i16::try_from(p.height).expect("large height"),
                                    x_advance: i16::try_from((advance_width * 64.) as i64)
                                        .expect("large advance width"),
                                    data: image.data,
                                }
                            }
                            None => BitmapGlyph {
                                x: 0,
                                y: 0,
                                width: 0,
                                height: 0,
                                x_advance: i16::try_from((advance_width * 64.) as i64)
                                    .expect("large advance width"),
                                data: vec![],
                            },
                        }
                    })
                })
                .collect();

            BitmapGlyphs { pixel_size: *pixel_size, glyph_data }
        })
        .collect();
    glyphs
}

#[cfg(all(not(target_arch = "wasm32"), feature = "sdf-fonts"))]
fn embed_sdf_glyphs(
    pixel_sizes: &[i16],
    character_map: &Vec<CharacterMapEntry>,
    font: &Font,
    fallback_fonts: &[Font],
) -> Vec<BitmapGlyphs> {
    use rayon::prelude::*;

    const RANGE: f64 = 6.;

    let Some(max_size) = pixel_sizes.iter().max() else {
        return Vec::new();
    };
    let min_size = pixel_sizes.iter().min().expect("we have a 'max' so the vector is not empty");
    let target_pixel_size = (max_size * 2 / 3).max(16).min(RANGE as i16 * min_size);

    let glyph_data = character_map
        .par_iter()
        .map(|CharacterMapEntry { code_point, .. }| {
            core::iter::once(font)
                .chain(fallback_fonts.iter())
                .find_map(|font| {
                    (swash_font_ref(font).charmap().map(*code_point) != 0).then(|| {
                        generate_sdf_for_glyph(font, *code_point, target_pixel_size, RANGE)
                    })
                })
                .unwrap_or_else(|| {
                    generate_sdf_for_glyph(font, *code_point, target_pixel_size, RANGE)
                })
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();

    vec![BitmapGlyphs { pixel_size: target_pixel_size, glyph_data }]
}

#[cfg(all(not(target_arch = "wasm32"), feature = "sdf-fonts"))]
fn generate_sdf_for_glyph(
    font: &Font,
    code_point: char,
    target_pixel_size: i16,
    range: f64,
) -> Option<BitmapGlyph> {
    use fdsm::transform::Transform;
    use nalgebra::{Affine2, Similarity2, Vector2};

    let face =
        fdsm_ttf_parser::ttf_parser::Face::parse(font.font.blob.data(), font.font.index).unwrap();
    let glyph_id = face.glyph_index(code_point).unwrap_or_default();
    let mut shape = fdsm_ttf_parser::load_shape_from_face(&face, glyph_id)?;

    let metrics = sharedfontique::DesignFontMetrics::new(&font.font);
    let target_pixel_size = target_pixel_size as f64;
    let scale = target_pixel_size / metrics.units_per_em as f64;

    // TODO: handle bitmap glyphs (emojis)
    let Some(bbox) = face.glyph_bounding_box(glyph_id) else {
        // For example, for space
        return Some(BitmapGlyph {
            x_advance: (face.glyph_hor_advance(glyph_id).unwrap_or(0) as f64 * scale * 64.) as i16,
            ..Default::default()
        });
    };

    let width = ((bbox.x_max as f64 - bbox.x_min as f64) * scale + 2.).ceil() as u32;
    let height = ((bbox.y_max as f64 - bbox.y_min as f64) * scale + 2.).ceil() as u32;
    let transformation = nalgebra::convert::<_, Affine2<f64>>(Similarity2::new(
        Vector2::new(1. - bbox.x_min as f64 * scale, 1. - bbox.y_min as f64 * scale),
        0.,
        scale,
    ));

    // Unlike msdfgen, the transformation is not passed into the
    // `generate_msdf` function – the coordinates of the control points
    // must be expressed in terms of pixels on the distance field. To get
    // the correct units, we pre-transform the shape:

    shape.transform(&transformation);

    let prepared_shape = shape.prepare();

    // Set up the resulting image and generate the distance field:

    let mut sdf = image::GrayImage::new(width, height);
    fdsm::generate::generate_sdf(&prepared_shape, range, &mut sdf);
    fdsm::render::correct_sign_sdf(
        &mut sdf,
        &prepared_shape,
        fdsm::bezier::scanline::FillRule::Nonzero,
    );

    let mut glyph_data = sdf.into_raw();

    // normalize around 0
    for x in &mut glyph_data {
        *x = x.wrapping_sub(128);
    }

    // invert the y coordinate (as the fsdm crate has the y axis inverted)
    let (w, h) = (width as usize, height as usize);
    for idx in 0..glyph_data.len() / 2 {
        glyph_data.swap(idx, (h - idx / w - 1) * w + idx % w);
    }

    // Add a "0" so that we can always access pos+1 without going out of bound
    // (so that the last row will look like `data[len-1]*1 + data[len]*0`)
    glyph_data.push(0);

    let bg = BitmapGlyph {
        x: i16::try_from((-(1. - bbox.x_min as f64 * scale) * 64.).ceil() as i32)
            .expect("large glyph x coordinate"),
        y: i16::try_from((-(1. - bbox.y_min as f64 * scale) * 64.).ceil() as i32)
            .expect("large glyph y coordinate"),
        width: i16::try_from(width).expect("large width"),
        height: i16::try_from(height).expect("large height"),
        x_advance: i16::try_from(
            (face.glyph_hor_advance(glyph_id).unwrap() as f64 * scale * 64.).round() as i32,
        )
        .expect("large advance width"),
        data: glyph_data,
    };

    Some(bg)
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
        "TextInput" | "Text" | "SimpleText" | "ComplexText" | "StyledTextItem" => {
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
