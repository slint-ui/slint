// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
use std::collections::HashMap;

use i_slint_core::graphics::euclid::num::Zero;
use i_slint_core::graphics::FontRequest;
use i_slint_core::items::{TextHorizontalAlignment, TextVerticalAlignment};
use i_slint_core::lengths::{LogicalLength, ScaleFactor};
use i_slint_core::{items, Color};

use super::itemrenderer::to_skia_color;
use super::{PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalSize};

pub const DEFAULT_FONT_SIZE: LogicalLength = LogicalLength::new(12.);

#[derive(PartialEq, Eq)]
enum CustomFontSource {
    ByData(&'static [u8]),
    ByPath(std::path::PathBuf),
}

struct FontCache {
    font_collection: RefCell<skia_safe::textlayout::FontCollection>,
    font_mgr: skia_safe::FontMgr,
    type_face_font_provider: RefCell<skia_safe::textlayout::TypefaceFontProvider>,
    custom_fonts: RefCell<HashMap<String, CustomFontSource>>,
}

thread_local! {
    static FONT_CACHE: FontCache = {
        let font_mgr = skia_safe::FontMgr::new();
        let type_face_font_provider = skia_safe::textlayout::TypefaceFontProvider::new();
        let mut font_collection = skia_safe::textlayout::FontCollection::new();
        // FontCollection first looks up in the dynamic font manager and then the asset font manager. If the
        // family is empty, the default font manager will match against the system default. We want that behavior,
        // and only if the family is not present in the system, then we want to fall back to the assert font manager
        // to pick up the custom font.
        font_collection.set_asset_font_manager(Some(type_face_font_provider.clone().into()));
        font_collection.set_dynamic_font_manager(font_mgr.clone());
        FontCache { font_collection: RefCell::new(font_collection), font_mgr, type_face_font_provider: RefCell::new(type_face_font_provider), custom_fonts: Default::default() }
    }
}

pub fn default_font(scale_factor: f32) -> Option<skia_safe::Font> {
    FONT_CACHE.with(|font_cache| {
        font_cache.font_mgr.legacy_make_typeface(None, skia_safe::FontStyle::default()).map(
            |type_face| skia_safe::Font::new(type_face, DEFAULT_FONT_SIZE.get() * scale_factor),
        )
    })
}

pub struct Selection {
    pub range: std::ops::Range<usize>,
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub underline: bool,
}

fn font_style_for_request(font_request: &FontRequest) -> skia_safe::FontStyle {
    skia_safe::FontStyle::new(
        font_request.weight.map_or(skia_safe::font_style::Weight::NORMAL, |w| w.into()),
        skia_safe::font_style::Width::NORMAL,
        if font_request.italic {
            skia_safe::font_style::Slant::Italic
        } else {
            skia_safe::font_style::Slant::Upright
        },
    )
}

pub fn create_layout(
    font_request: FontRequest,
    scale_factor: ScaleFactor,
    text: &str,
    text_style: Option<skia_safe::textlayout::TextStyle>,
    max_width: Option<PhysicalLength>,
    max_height: PhysicalLength,
    h_align: items::TextHorizontalAlignment,
    v_align: TextVerticalAlignment,
    wrap: items::TextWrap,
    overflow: items::TextOverflow,
    selection: Option<&Selection>,
) -> (skia_safe::textlayout::Paragraph, PhysicalPoint) {
    let mut text_style = text_style.unwrap_or_default();

    if let Some(family_name) = font_request.family.as_ref() {
        text_style.set_font_families(&[family_name.as_str()]);
    }

    let pixel_size = font_request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE) * scale_factor;

    if let Some(letter_spacing) = font_request.letter_spacing {
        text_style.set_letter_spacing((letter_spacing * scale_factor).get());
    }
    text_style.set_font_size(pixel_size.get());
    text_style.set_font_style(font_style_for_request(&font_request));

    let mut style = skia_safe::textlayout::ParagraphStyle::new();

    if overflow == items::TextOverflow::Elide {
        style.set_ellipsis("…");
        if wrap != items::TextWrap::NoWrap {
            let metrics = text_style.font_metrics();
            let line_height = metrics.descent - metrics.ascent + metrics.leading;
            style.set_max_lines((max_height.get() / line_height).floor() as usize);
        }
    }

    style.set_text_align(match h_align {
        items::TextHorizontalAlignment::Left => skia_safe::textlayout::TextAlign::Left,
        items::TextHorizontalAlignment::Center => skia_safe::textlayout::TextAlign::Center,
        items::TextHorizontalAlignment::Right => skia_safe::textlayout::TextAlign::Right,
    });

    style.set_text_style(&text_style);

    let mut builder = FONT_CACHE.with(|font_cache| {
        skia_safe::textlayout::ParagraphBuilder::new(
            &style,
            font_cache.font_collection.borrow().clone(),
        )
    });

    if let Some(selection) = selection {
        let before_selection = &text[..selection.range.start];
        builder.add_text(before_selection);

        let mut selection_style = text_style.clone();

        if let Some(selection_background) = selection.background {
            let mut selection_background_paint = skia_safe::Paint::default();
            selection_background_paint.set_color(to_skia_color(&selection_background));
            selection_style.set_background_paint(&selection_background_paint);
        }

        if let Some(selection_foreground) = selection.foreground {
            let mut selection_foreground_paint = skia_safe::Paint::default();
            selection_foreground_paint.set_color(to_skia_color(&selection_foreground));
            selection_style.set_foreground_paint(&selection_foreground_paint);
        }

        if selection.underline {
            let mut decoration = skia_safe::textlayout::Decoration::default();
            decoration.ty = skia_safe::textlayout::TextDecoration::UNDERLINE;
            decoration.color = text_style.foreground().color();
            selection_style.set_decoration(&decoration);
        }

        builder.push_style(&selection_style);
        let selected_text = &text[selection.range.clone()];
        builder.add_text(selected_text);
        builder.pop();

        let after_selection = &text[selection.range.end..];
        builder.add_text(after_selection);
    } else {
        builder.add_text(text);
    }

    let no_wrap = wrap == items::TextWrap::NoWrap || overflow == items::TextOverflow::Elide;

    let mut paragraph = builder.build();
    paragraph.layout(
        max_width.filter(|_| !no_wrap).map_or(f32::MAX, |physical_width| physical_width.get()),
    );

    // Layouting out with f32::max when wrapping is disabled, causes an overflow and breaks alignment compensation. Lay out again just wide enough
    // to fit the largest unwrapped line.
    if no_wrap
        && matches!(h_align, TextHorizontalAlignment::Right | TextHorizontalAlignment::Center)
    {
        paragraph.layout(paragraph.longest_line() + 1.);
    }

    let layout_height = PhysicalLength::new(paragraph.height());

    let layout_top_y = match v_align {
        i_slint_core::items::TextVerticalAlignment::Top => PhysicalLength::zero(),
        i_slint_core::items::TextVerticalAlignment::Center => (max_height - layout_height) / 2.,
        i_slint_core::items::TextVerticalAlignment::Bottom => max_height - layout_height,
    };

    let layout_top_x = if no_wrap {
        // With no wrapping, the alignment is done against the layout width that's larger than the available width. Compensate for that
        // by shifting rendering.
        match h_align {
            TextHorizontalAlignment::Left => PhysicalLength::zero(),
            TextHorizontalAlignment::Center => {
                let available_width = max_width.unwrap_or(PhysicalLength::new(f32::MAX));
                (PhysicalLength::new(-paragraph.max_width()) + available_width) / 2.
            }
            TextHorizontalAlignment::Right => {
                let available_width = max_width.unwrap_or(PhysicalLength::new(f32::MAX));
                PhysicalLength::new(-paragraph.max_width()) + available_width
            }
        }
    } else {
        PhysicalLength::zero()
    };

    (paragraph, PhysicalPoint::from_lengths(layout_top_x, layout_top_y))
}

pub fn font_metrics(
    font_request: i_slint_core::graphics::FontRequest,
    scale_factor: ScaleFactor,
) -> i_slint_core::items::FontMetrics {
    let (layout, _) = create_layout(
        font_request,
        scale_factor,
        " ",
        None,
        None,
        PhysicalLength::new(f32::MAX),
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
        None,
    );

    let fonts = layout.get_fonts();

    let Some(font_info) = fonts.first() else {
        return Default::default();
    };

    let metrics = font_info.font.metrics().1;

    i_slint_core::items::FontMetrics {
        ascent: -metrics.ascent / scale_factor.get(),
        descent: -metrics.descent / scale_factor.get(),
        x_height: metrics.x_height / scale_factor.get(),
        cap_height: metrics.cap_height / scale_factor.get(),
    }
}

fn register_font(source: CustomFontSource) -> Result<(), Box<dyn std::error::Error>> {
    FONT_CACHE.with(|font_cache| {
        if font_cache
            .custom_fonts
            .borrow()
            .values()
            .position(|registered_font| *registered_font == source)
            .is_some()
        {
            return Ok(());
        }

        let data: std::borrow::Cow<[u8]> = match &source {
            CustomFontSource::ByData(data) => std::borrow::Cow::Borrowed(data),
            CustomFontSource::ByPath(path) => std::borrow::Cow::Owned(std::fs::read(path)?),
        };

        let type_face =
            font_cache.font_mgr.new_from_data(data.as_ref(), None).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "error parsing TrueType font".to_string(),
                )
            })?;

        drop(data);

        let family_name = type_face.family_name();
        let no_alias: Option<&str> = None;
        font_cache.type_face_font_provider.borrow_mut().register_typeface(type_face, no_alias);
        font_cache.custom_fonts.borrow_mut().insert(family_name, source);
        Ok(())
    })
}

pub fn register_font_from_memory(data: &'static [u8]) -> Result<(), Box<dyn std::error::Error>> {
    register_font(CustomFontSource::ByData(data))
}

pub fn register_font_from_path(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    register_font(CustomFontSource::ByPath(path.into()))
}

pub fn cursor_rect(
    string: &str,
    cursor_pos: usize,
    layout: skia_safe::textlayout::Paragraph,
    cursor_width: PhysicalLength,
    h_align: TextHorizontalAlignment,
) -> PhysicalRect {
    if string.is_empty() {
        let x = match h_align {
            TextHorizontalAlignment::Left => PhysicalLength::default(),
            TextHorizontalAlignment::Center => PhysicalLength::new(layout.max_width() / 2.),
            TextHorizontalAlignment::Right => PhysicalLength::new(layout.max_width()),
        };
        return PhysicalRect::new(
            PhysicalPoint::from_lengths(x, PhysicalLength::default()),
            PhysicalSize::from_lengths(cursor_width, PhysicalLength::new(layout.height())),
        );
    }

    // This is needed in case of the cursor is moving to the end of the text (#7203).
    let cursor_pos = cursor_pos.min(string.len());
    // Not doing this check may cause crashing with non-ASCII text.
    if !string.is_char_boundary(cursor_pos) {
        return Default::default();
    }

    // SkParagraph::getRectsForRange() does not report the text box of a trailing newline
    // correctly. Use the last line's metrics to get the correct coordinates (#3590).
    if cursor_pos == string.len()
        && string.ends_with(|ch| ch == '\n' || ch == '\u{2028}' || ch == '\u{2029}')
    {
        if let Some(metrics) = layout.get_line_metrics_at(layout.line_number() - 1) {
            return PhysicalRect::new(
                PhysicalPoint::new(
                    (metrics.left + metrics.width) as f32,
                    (metrics.baseline - metrics.ascent) as f32,
                ),
                PhysicalSize::from_lengths(
                    cursor_width,
                    PhysicalLength::new(metrics.height as f32),
                ),
            );
        }
    }

    // The cursor is visually between characters, but the logical cursor_pos refers to the
    // index in the string that is the start of a glyph cluster. The cursor is to be drawn
    // at the left edge of that glyph cluster.
    // When the cursor is at the end of the text, there's no glyph cluster to the right.
    // Instead we pick the previous glyph cluster and select the right edge of it.

    let select_glyph_box_edge_x = if cursor_pos == string.len() {
        |rect: &skia_safe::Rect| rect.right
    } else {
        |rect: &skia_safe::Rect| rect.left
    };

    let mut grapheme_cursor =
        unicode_segmentation::GraphemeCursor::new(cursor_pos, string.len(), true);
    let adjacent_grapheme_byte_range = if cursor_pos == string.len() {
        let prev_grapheme = match grapheme_cursor.prev_boundary(string, 0) {
            Ok(byte_offset) => byte_offset.unwrap_or(0),
            Err(_) => return Default::default(),
        };

        prev_grapheme..cursor_pos
    } else {
        let next_grapheme = match grapheme_cursor.next_boundary(string, 0) {
            Ok(byte_offset) => byte_offset.unwrap_or_else(|| string.len()),
            Err(_) => return Default::default(),
        };

        cursor_pos..next_grapheme
    };

    let adjacent_grapheme_utf16_start =
        string[..adjacent_grapheme_byte_range.start].chars().map(char::len_utf16).sum();
    let adjacent_grapheme_utf16_next: usize =
        string[adjacent_grapheme_byte_range].chars().map(char::len_utf16).sum();

    let boxes = layout.get_rects_for_range(
        adjacent_grapheme_utf16_start..adjacent_grapheme_utf16_start + adjacent_grapheme_utf16_next,
        skia_safe::textlayout::RectHeightStyle::Max,
        skia_safe::textlayout::RectWidthStyle::Max,
    );
    boxes
        .into_iter()
        .next()
        .map(|textbox| {
            let x = select_glyph_box_edge_x(&textbox.rect);
            PhysicalRect::new(
                PhysicalPoint::new(x, textbox.rect.y()),
                PhysicalSize::from_lengths(
                    cursor_width,
                    PhysicalLength::new(textbox.rect.height()),
                ),
            )
        })
        .unwrap_or_default()
}
