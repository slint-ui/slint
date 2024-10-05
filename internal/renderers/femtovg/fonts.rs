// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cspell:ignore Noto fontconfig

use core::num::NonZeroUsize;
use femtovg::TextContext;
use i_slint_common::sharedfontdb::{self, fontdb};
use i_slint_core::graphics::euclid;
use i_slint_core::graphics::FontRequest;
use i_slint_core::items::{TextHorizontalAlignment, TextOverflow, TextVerticalAlignment, TextWrap};
use i_slint_core::lengths::PointLengths;
use i_slint_core::lengths::{LogicalLength, LogicalSize, ScaleFactor, SizeLengths};
use i_slint_core::{SharedString, SharedVector};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use super::{PhysicalLength, PhysicalPoint, PhysicalSize};

pub const DEFAULT_FONT_SIZE: LogicalLength = LogicalLength::new(12.);

#[derive(Clone, PartialEq, Eq, Hash)]
struct FontCacheKey {
    family: SharedString,
    weight: fontdb::Weight,
    style: fontdb::Style,
    stretch: fontdb::Stretch,
}

#[derive(Clone)]
pub struct Font {
    fonts: SharedVector<femtovg::FontId>,
    pixel_size: PhysicalLength,
    text_context: TextContext,
}

impl Font {
    pub fn init_paint(
        &self,
        letter_spacing: PhysicalLength,
        mut paint: femtovg::Paint,
    ) -> femtovg::Paint {
        paint.set_font(&self.fonts);
        paint.set_font_size(self.pixel_size.get());
        paint.set_text_baseline(femtovg::Baseline::Top);
        paint.set_letter_spacing(letter_spacing.get());
        paint
    }

    pub fn text_size(
        &self,
        letter_spacing: PhysicalLength,
        text: &str,
        max_width: Option<PhysicalLength>,
    ) -> PhysicalSize {
        let paint = self.init_paint(letter_spacing, femtovg::Paint::default());
        let font_metrics = self.text_context.measure_font(&paint).unwrap();
        let mut lines = 0;
        let mut width = 0.;
        let mut start = 0;
        if let Some(max_width) = max_width {
            while start < text.len() {
                let max_line_index = text[start..].find('\n').map_or(text.len(), |i| i + 1 + start);
                let index = self
                    .text_context
                    .break_text(max_width.get(), &text[start..max_line_index], &paint)
                    .unwrap();
                if index == 0 {
                    break;
                }
                let index = start + index;
                let measure =
                    self.text_context.measure_text(0., 0., &text[start..index], &paint).unwrap();
                start = index;
                lines += 1;
                width = measure.width().max(width);
            }
        } else {
            for line in text.lines() {
                let measure = self.text_context.measure_text(0., 0., line, &paint).unwrap();
                lines += 1;
                width = measure.width().max(width);
            }
        }
        euclid::size2(width, lines as f32 * font_metrics.height())
    }

    pub fn height(&self) -> PhysicalLength {
        let mut paint = femtovg::Paint::default();
        // These are the only two properties measure_font() needs
        paint.set_font(&self.fonts);
        paint.set_font_size(self.pixel_size.get());
        PhysicalLength::new(self.text_context.measure_font(&paint).unwrap().height())
    }
}

pub(crate) fn text_size(
    font_request: &i_slint_core::graphics::FontRequest,
    scale_factor: ScaleFactor,
    text: &str,
    max_width: Option<LogicalLength>,
) -> LogicalSize {
    let font =
        FONT_CACHE.with(|cache| cache.borrow_mut().font(font_request.clone(), scale_factor, text));
    let letter_spacing = font_request.letter_spacing.unwrap_or_default();
    font.text_size(letter_spacing * scale_factor, text, max_width.map(|x| x * scale_factor))
        / scale_factor
}

pub(crate) fn font_metrics(
    font_request: i_slint_core::graphics::FontRequest,
) -> i_slint_core::items::FontMetrics {
    let primary_font = FONT_CACHE.with(|cache| {
        let query = font_request.to_fontdb_query();

        cache.borrow_mut().load_single_font(font_request.family.as_ref(), query)
    });

    let logical_pixel_size = (font_request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE)).get();

    let units_per_em = primary_font.design_font_metrics.units_per_em;

    i_slint_core::items::FontMetrics {
        ascent: primary_font.design_font_metrics.ascent * logical_pixel_size / units_per_em,
        descent: primary_font.design_font_metrics.descent * logical_pixel_size / units_per_em,
        x_height: primary_font.design_font_metrics.x_height * logical_pixel_size / units_per_em,
        cap_height: primary_font.design_font_metrics.cap_height * logical_pixel_size / units_per_em,
    }
}

#[derive(Clone)]
struct LoadedFont {
    femtovg_font_id: femtovg::FontId,
    fontdb_face_id: fontdb::ID,
    design_font_metrics: i_slint_common::sharedfontdb::DesignFontMetrics,
}

struct SharedFontData(std::sync::Arc<dyn AsRef<[u8]>>);
impl AsRef<[u8]> for SharedFontData {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref().as_ref()
    }
}

#[derive(Default)]
struct GlyphCoverage {
    // Used to express script support for all scripts except Unknown, Common and Inherited
    // For those the detailed glyph_coverage is used instead
    supported_scripts: HashMap<unicode_script::Script, bool>,
    // Especially in characters mapped to the common script, the support varies. For example
    // '✓' and the digit '1' map to Common, but not all fonts providing digits also support the
    // check mark glyph.
    exact_glyph_coverage: HashMap<char, bool>,
}

enum GlyphCoverageCheckResult {
    Incomplete,
    Improved,
    Complete,
}

pub struct FontCache {
    loaded_fonts: HashMap<FontCacheKey, LoadedFont>,
    // for a given fontdb face id, this tells us what we've learned about the script
    // coverage of the font.
    loaded_font_coverage: HashMap<fontdb::ID, GlyphCoverage>,
    pub(crate) text_context: TextContext,
    available_families: HashSet<SharedString>,
}

impl Default for FontCache {
    fn default() -> Self {
        let available_families = sharedfontdb::FONT_DB.with(|db| {
            db.borrow()
                .faces()
                .filter_map(|face_info| {
                    face_info.families.first().map(|(family_name, _)| family_name.as_str().into())
                })
                .collect()
        });

        let text_context = TextContext::default();
        text_context.resize_shaped_words_cache(NonZeroUsize::new(10_000_000).unwrap());
        text_context.resize_shaping_run_cache(NonZeroUsize::new(1_000_000).unwrap());

        Self {
            loaded_fonts: HashMap::new(),
            loaded_font_coverage: HashMap::new(),
            text_context,
            available_families,
        }
    }
}

thread_local! {
    pub static FONT_CACHE: RefCell<FontCache> = RefCell::new(Default::default())
}

impl FontCache {
    fn load_single_font(
        &mut self,
        family: Option<&SharedString>,
        query: fontdb::Query<'_>,
    ) -> LoadedFont {
        let text_context = self.text_context.clone();
        let cache_key = FontCacheKey {
            family: family.cloned().unwrap_or_default(),
            weight: query.weight,
            style: query.style,
            stretch: query.stretch,
        };

        if let Some(loaded_font) = self.loaded_fonts.get(&cache_key) {
            return loaded_font.clone();
        }

        //let now = std::time::Instant::now();

        let fontdb_face_id = sharedfontdb::FONT_DB.with_borrow(|db| {
            db.query_with_family(query, family.map(|s| s.as_str()))
                .or_else(|| {
                    // If the requested family could not be found, fall back to *some* family that must exist
                    db.query_with_family(query, None)
                })
                .expect("there must be a sans-serif font face registered")
        });

        // Safety: We map font files into memory that - while we never unmap them - may
        // theoretically get corrupted/truncated by another process and then we'll crash
        // and burn. In practice that should not happen though, font files are - at worst -
        // removed by a package manager and they unlink instead of truncate, even when
        // replacing files. Unlinking OTOH is safe and doesn't destroy the file mapping,
        // the backing file becomes an orphan in a special area of the file system. That works
        // on Unixy platforms and on Windows the default file flags prevent the deletion.
        #[cfg(not(target_arch = "wasm32"))]
        let (shared_data, face_index) = unsafe {
            sharedfontdb::FONT_DB.with_borrow_mut(|db| {
                db.make_mut().make_shared_face_data(fontdb_face_id).expect("unable to mmap font")
            })
        };
        #[cfg(target_arch = "wasm32")]
        let (shared_data, face_index) = crate::sharedfontdb::FONT_DB.with_borrow(|db| {
            db.face_source(fontdb_face_id)
                .map(|(source, face_index)| {
                    (
                        match source {
                            fontdb::Source::Binary(data) => data.clone(),
                            // We feed only Source::Binary into fontdb on wasm
                            #[allow(unreachable_patterns)]
                            _ => unreachable!(),
                        },
                        face_index,
                    )
                })
                .expect("invalid fontdb face id")
        });

        let design_font_metrics = {
            let face = ttf_parser::Face::parse(shared_data.as_ref().as_ref(), face_index).unwrap();
            i_slint_common::sharedfontdb::DesignFontMetrics::new(face)
        };

        let femtovg_font_id = text_context
            .add_shared_font_with_index(SharedFontData(shared_data), face_index)
            .unwrap();

        //println!("Loaded {:#?} in {}ms.", request, now.elapsed().as_millis());
        let new_font = LoadedFont { femtovg_font_id, fontdb_face_id, design_font_metrics };
        self.loaded_fonts.insert(cache_key, new_font.clone());
        new_font
    }

    pub fn font(
        &mut self,
        font_request: FontRequest,
        scale_factor: ScaleFactor,
        reference_text: &str,
    ) -> Font {
        let pixel_size = font_request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE) * scale_factor;

        let query = font_request.to_fontdb_query();

        let primary_font = self.load_single_font(font_request.family.as_ref(), query);

        use unicode_script::{Script, UnicodeScript};
        // map from required script to sample character
        let mut scripts_required: HashMap<unicode_script::Script, char> = Default::default();
        let mut chars_required: HashSet<char> = Default::default();
        for ch in reference_text.chars() {
            if ch.is_control() || ch.is_whitespace() {
                continue;
            }
            let script = ch.script();
            if script == Script::Common || script == Script::Inherited || script == Script::Unknown
            {
                chars_required.insert(ch);
            } else {
                scripts_required.insert(script, ch);
            }
        }

        let mut coverage_result = self.check_and_update_script_coverage(
            &mut scripts_required,
            &mut chars_required,
            primary_font.fontdb_face_id,
        );

        //eprintln!(
        //    "coverage for {} after checking primary font: {:#?}",
        //    reference_text, scripts_required
        //);

        let fallbacks = if !matches!(coverage_result, GlyphCoverageCheckResult::Complete) {
            self.font_fallbacks_for_request(
                font_request.family.as_ref(),
                pixel_size,
                &primary_font,
                reference_text,
            )
        } else {
            Vec::new()
        };

        let fonts = core::iter::once(primary_font.femtovg_font_id)
            .chain(fallbacks.iter().filter_map(|fallback_family| {
                if matches!(coverage_result, GlyphCoverageCheckResult::Complete) {
                    return None;
                }

                let fallback_font = self.load_single_font(Some(fallback_family), query);

                coverage_result = self.check_and_update_script_coverage(
                    &mut scripts_required,
                    &mut chars_required,
                    fallback_font.fontdb_face_id,
                );

                if matches!(
                    coverage_result,
                    GlyphCoverageCheckResult::Improved | GlyphCoverageCheckResult::Complete
                ) {
                    Some(fallback_font.femtovg_font_id)
                } else {
                    None
                }
            }))
            .collect::<SharedVector<_>>();

        Font { fonts, text_context: self.text_context.clone(), pixel_size }
    }

    #[cfg(target_os = "macos")]
    fn font_fallbacks_for_request(
        &self,
        _family: Option<&SharedString>,
        _pixel_size: PhysicalLength,
        _primary_font: &LoadedFont,
        _reference_text: &str,
    ) -> Vec<SharedString> {
        let requested_font = match core_text::font::new_from_name(
            &_family.as_ref().map_or_else(|| "", |s| s.as_str()),
            _pixel_size.get() as f64,
        ) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        core_text::font::cascade_list_for_languages(
            &requested_font,
            &core_foundation::array::CFArray::from_CFTypes(&[]),
        )
        .iter()
        .map(|fallback_descriptor| SharedString::from(fallback_descriptor.family_name()))
        .filter(|family| self.is_known_family(&family))
        .collect::<Vec<_>>()
    }

    #[cfg(target_os = "windows")]
    fn font_fallbacks_for_request(
        &self,
        _family: Option<&SharedString>,
        _pixel_size: PhysicalLength,
        _primary_font: &LoadedFont,
        reference_text: &str,
    ) -> Vec<SharedString> {
        let system_font_fallback = match dwrote::FontFallback::get_system_fallback() {
            Some(fallback) => fallback,
            None => return Vec::new(),
        };
        let font_collection = dwrote::FontCollection::get_system(false);
        let base_family = Some(_family.as_ref().map_or_else(|| "", |s| s.as_str()));

        let reference_text_utf16: Vec<u16> = reference_text.encode_utf16().collect();

        // Hack to implement the minimum interface for direct write. We have yet to provide the correct
        // locale (but return an empty string for now). This struct stores the number of utf-16 characters
        // so that in get_locale_name it can return that the (empty) locale applies all the characters after
        // `text_position`, by returning the count.
        struct TextAnalysisHack(u32);
        impl dwrote::TextAnalysisSourceMethods for TextAnalysisHack {
            fn get_locale_name(&self, text_position: u32) -> (std::borrow::Cow<'_, str>, u32) {
                ("".into(), self.0 - text_position)
            }

            // We should do better on this one, too...
            fn get_paragraph_reading_direction(
                &self,
            ) -> winapi::um::dwrite::DWRITE_READING_DIRECTION {
                winapi::um::dwrite::DWRITE_READING_DIRECTION_LEFT_TO_RIGHT
            }
        }

        let text_analysis_source = dwrote::TextAnalysisSource::from_text_and_number_subst(
            Box::new(TextAnalysisHack(reference_text_utf16.len() as u32)),
            std::borrow::Cow::Borrowed(&reference_text_utf16),
            dwrote::NumberSubstitution::new(
                winapi::um::dwrite::DWRITE_NUMBER_SUBSTITUTION_METHOD_NONE,
                "",
                true,
            ),
        );

        let mut fallback_fonts = Vec::new();

        let mut utf16_pos = 0;

        while utf16_pos < reference_text_utf16.len() {
            let fallback_result = system_font_fallback.map_characters(
                &text_analysis_source,
                utf16_pos as u32,
                (reference_text_utf16.len() - utf16_pos) as u32,
                &font_collection,
                base_family,
                dwrote::FontWeight::Regular,
                dwrote::FontStyle::Normal,
                dwrote::FontStretch::Normal,
            );

            if let Some(fallback_font) = fallback_result.mapped_font {
                let family: SharedString = fallback_font.family_name().into();
                if self.is_known_family(&family) {
                    fallback_fonts.push(family)
                }
            } else {
                break;
            }

            utf16_pos += fallback_result.mapped_length;
        }

        fallback_fonts
    }

    #[cfg(not(any(
        target_family = "windows",
        target_os = "macos",
        target_os = "ios",
        target_arch = "wasm32",
        target_os = "android",
    )))]
    fn font_fallbacks_for_request(
        &self,
        _family: Option<&SharedString>,
        _pixel_size: PhysicalLength,
        _primary_font: &LoadedFont,
        _reference_text: &str,
    ) -> Vec<SharedString> {
        sharedfontdb::FONT_DB.with(|db| {
            db.borrow()
                .fontconfig_fallback_families
                .iter()
                .filter(|&family_name| self.is_known_family(family_name))
                .map(|family_name| family_name.into())
                .collect()
        })
    }

    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    fn font_fallbacks_for_request(
        &self,
        _family: Option<&SharedString>,
        _pixel_size: PhysicalLength,
        _primary_font: &LoadedFont,
        _reference_text: &str,
    ) -> Vec<SharedString> {
        [SharedString::from("DejaVu Sans")]
            .iter()
            .filter(|family_name| self.is_known_family(&family_name))
            .cloned()
            .collect()
    }

    fn is_known_family(&self, family: &str) -> bool {
        self.available_families.contains(family)
    }

    // From the set of script without coverage, remove all entries that are known to be covered by
    // the given face_id. Any yet unknown script coverage for the face_id is updated (hence
    // mutable self).
    fn check_and_update_script_coverage(
        &mut self,
        scripts_without_coverage: &mut HashMap<unicode_script::Script, char>,
        chars_without_coverage: &mut HashSet<char>,
        face_id: fontdb::ID,
    ) -> GlyphCoverageCheckResult {
        //eprintln!("required scripts {:#?}", required_scripts);
        let coverage = self.loaded_font_coverage.entry(face_id).or_default();

        let mut scripts_that_need_checking = Vec::new();
        let mut chars_that_need_checking = Vec::new();

        let old_uncovered_scripts_count = scripts_without_coverage.len();
        let old_uncovered_chars_count = chars_without_coverage.len();

        scripts_without_coverage.retain(|script, sample| {
            coverage.supported_scripts.get(script).map_or_else(
                || {
                    scripts_that_need_checking.push((*script, *sample));
                    true // this may or may not be supported, so keep it in scripts_without_coverage
                },
                |has_coverage| !has_coverage,
            )
        });

        chars_without_coverage.retain(|ch| {
            coverage.exact_glyph_coverage.get(ch).map_or_else(
                || {
                    chars_that_need_checking.push(*ch);
                    true // this may or may not be supported, so keep it in chars_without_coverage
                },
                |has_coverage| !has_coverage,
            )
        });

        if !scripts_that_need_checking.is_empty() || !chars_that_need_checking.is_empty() {
            sharedfontdb::FONT_DB.with(|db| {
                db.borrow().with_face_data(face_id, |face_data, face_index| {
                    let face = ttf_parser::Face::parse(face_data, face_index).unwrap();

                    for (unchecked_script, sample_char) in scripts_that_need_checking {
                        let glyph_coverage = face.glyph_index(sample_char).is_some();
                        coverage.supported_scripts.insert(unchecked_script, glyph_coverage);

                        if glyph_coverage {
                            scripts_without_coverage.remove(&unchecked_script);
                        }
                    }

                    for unchecked_char in chars_that_need_checking {
                        let glyph_coverage = face.glyph_index(unchecked_char).is_some();
                        coverage.exact_glyph_coverage.insert(unchecked_char, glyph_coverage);

                        if glyph_coverage {
                            chars_without_coverage.remove(&unchecked_char);
                        }
                    }
                });
            })
        }

        let remaining_required_script_coverage = scripts_without_coverage.len();
        let remaining_required_char_coverage = chars_without_coverage.len();

        if scripts_without_coverage.is_empty() && chars_without_coverage.is_empty() {
            GlyphCoverageCheckResult::Complete
        } else if remaining_required_script_coverage < old_uncovered_scripts_count
            || remaining_required_char_coverage < old_uncovered_chars_count
        {
            GlyphCoverageCheckResult::Improved
        } else {
            GlyphCoverageCheckResult::Incomplete
        }
    }
}

/// Layout the given string in lines, and call the `layout_line` callback with the line to draw at position y.
/// The signature of the `layout_line` function is: `(text, pos, start_index, line_metrics)`.
/// start index is the starting byte of the text in the string.
/// Returns the coordinates of the cursor, if a cursor byte offset was provided.
pub(crate) fn layout_text_lines(
    string: &str,
    font: &Font,
    max_size: PhysicalSize,
    (horizontal_alignment, vertical_alignment): (TextHorizontalAlignment, TextVerticalAlignment),
    wrap: TextWrap,
    overflow: TextOverflow,
    single_line: bool,
    cursor_byte_offset: Option<usize>,
    paint: &femtovg::Paint,
    mut layout_line: impl FnMut(&str, PhysicalPoint, usize, &femtovg::TextMetrics),
) -> Option<PhysicalPoint> {
    let wrap = wrap != TextWrap::NoWrap;
    let elide = overflow == TextOverflow::Elide;

    let max_width = max_size.width_length();
    let max_height = max_size.height_length();

    let text_context = FONT_CACHE.with(|cache| cache.borrow().text_context.clone());
    let font_metrics = text_context.measure_font(paint).unwrap();
    let font_height = PhysicalLength::new(font_metrics.height());

    let mut cursor_point: Option<PhysicalPoint> = None;

    let text_height = || {
        if single_line {
            font_height
        } else {
            // Note: this is kind of doing twice the layout because text_size also does it
            let text_height = font
                .text_size(
                    PhysicalLength::new(paint.letter_spacing()),
                    string,
                    if wrap { Some(max_width) } else { None },
                )
                .height_length();
            if elide && text_height > max_height {
                // The height of the text is used for vertical alignment below.
                // If the full text doesn't fit into max_height and eliding is
                // enabled, calculate the height of the max number of lines that
                // fit to ensure correct vertical alignment when elided.
                let max_lines = (max_height.get() / font_height.get()).floor();
                font_height * max_lines
            } else {
                text_height
            }
        }
    };

    let mut process_line =
        |text_span: &str, y: PhysicalLength, start: usize, line_metrics: &femtovg::TextMetrics| {
            let x = match horizontal_alignment {
                TextHorizontalAlignment::Left => PhysicalLength::default(),
                TextHorizontalAlignment::Center => {
                    max_width / 2. - max_width.min(PhysicalLength::new(line_metrics.width())) / 2.
                }
                TextHorizontalAlignment::Right => {
                    max_width - max_width.min(PhysicalLength::new(line_metrics.width()))
                }
            };
            let line_pos = PhysicalPoint::from_lengths(x, y);
            layout_line(text_span, line_pos, start, line_metrics);

            if let Some(cursor_byte_offset) = cursor_byte_offset {
                let text_span_range = start..(start + text_span.len());

                if text_span_range.contains(&cursor_byte_offset)
                    || (cursor_byte_offset == text_span_range.end
                        && cursor_byte_offset == string.len()
                        && !string.ends_with('\n'))
                {
                    let cursor_x = PhysicalLength::new(
                        line_metrics
                            .glyphs
                            .iter()
                            .find_map(|glyph| {
                                if glyph.byte_index == (cursor_byte_offset - start) {
                                    Some(glyph.x)
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| line_metrics.width()),
                    );
                    cursor_point = Some(PhysicalPoint::from_lengths(
                        line_pos.x_length() + cursor_x,
                        line_pos.y_length(),
                    ));
                }
            }
        };

    let baseline_y = match vertical_alignment {
        TextVerticalAlignment::Top => PhysicalLength::default(),
        TextVerticalAlignment::Center => max_height / 2. - text_height() / 2.,
        TextVerticalAlignment::Bottom => max_height - text_height(),
    };
    let mut y = baseline_y;
    let mut start = 0;
    'lines: while start < string.len() && y + font_height <= max_height {
        if wrap && (!elide || y + font_height * 2. <= max_height) {
            let max_line_index = string[start..].find('\n').map_or(string.len(), |i| i + 1 + start);
            let index = text_context
                .break_text(max_width.get(), &string[start..max_line_index], paint)
                .unwrap();
            if index == 0 {
                // FIXME the word is too big to be shown, but we should still break, ideally
                break;
            }
            let index = start + index;
            let line = &string[start..index];
            let text_metrics = text_context.measure_text(0., 0., line, paint).unwrap();
            process_line(line, y, start, &text_metrics);
            y += font_height;
            start = index;
        } else {
            let index = if single_line {
                string.len()
            } else {
                string[start..].find('\n').map_or(string.len(), |i| start + i + 1)
            };
            let line = &string[start..index];
            let text_metrics = text_context.measure_text(0., 0., line, paint).unwrap();
            let elide_last_line =
                elide && index < string.len() && y + font_height * 2. > max_height;
            if text_metrics.width() > max_width.get() || elide_last_line {
                let w = max_width
                    - if elide {
                        PhysicalLength::new(
                            text_context.measure_text(0., 0., "…", paint).unwrap().width(),
                        )
                    } else {
                        PhysicalLength::default()
                    };
                let mut current_x = 0.;
                for glyph in &text_metrics.glyphs {
                    current_x += glyph.advance_x;
                    if current_x >= w.get() {
                        let txt = &line[..glyph.byte_index];
                        if elide {
                            let elided = format!("{}…", txt);
                            process_line(&elided, y, start, &text_metrics);
                        } else {
                            process_line(txt, y, start, &text_metrics);
                        }
                        y += font_height;
                        start = index;
                        continue 'lines;
                    }
                }
                if elide_last_line {
                    let elided = format!("{}…", line.strip_suffix('\n').unwrap_or(line));
                    process_line(&elided, y, start, &text_metrics);
                    y += font_height;
                    start = index;
                    continue 'lines;
                }
            }
            process_line(line, y, start, &text_metrics);
            y += font_height;
            start = index;
        }
    }

    cursor_point.or_else(|| {
        cursor_byte_offset.map(|_| {
            let x = match horizontal_alignment {
                TextHorizontalAlignment::Left => PhysicalLength::default(),
                TextHorizontalAlignment::Center => max_size.width_length() / 2.,
                TextHorizontalAlignment::Right => max_size.width_length(),
            };
            PhysicalPoint::from_lengths(x, y)
        })
    })
}
