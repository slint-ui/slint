// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::cell::RefCell;
use std::collections::HashMap;

use i_slint_core::graphics::euclid::num::Zero;
use i_slint_core::graphics::FontRequest;
use i_slint_core::items::TextVerticalAlignment;
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
    font_collection: skia_safe::textlayout::FontCollection,
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
        FontCache { font_collection, font_mgr, type_face_font_provider: RefCell::new(type_face_font_provider), custom_fonts: Default::default() }
    }
}

pub struct Selection {
    pub range: std::ops::Range<usize>,
    pub background: Color,
    pub foreground: Color,
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
    overflow: items::TextOverflow,
    selection: Option<&Selection>,
) -> (skia_safe::textlayout::Paragraph, PhysicalPoint) {
    let mut text_style = text_style.unwrap_or_default();

    if let Some(family_name) = font_request.family {
        text_style.set_font_families(&[family_name.as_str()]);
    }

    let pixel_size = font_request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE) * scale_factor;

    if let Some(letter_spacing) = font_request.letter_spacing {
        text_style.set_letter_spacing((letter_spacing * scale_factor).get());
    }
    text_style.set_font_size(pixel_size.get());
    text_style.set_font_style(skia_safe::FontStyle::new(
        font_request.weight.map_or(skia_safe::font_style::Weight::NORMAL, |w| w.into()),
        skia_safe::font_style::Width::NORMAL,
        skia_safe::font_style::Slant::Upright,
    ));

    let mut style = skia_safe::textlayout::ParagraphStyle::new();

    if overflow == items::TextOverflow::Elide {
        style.set_ellipsis("…");
    }

    style.set_text_align(match h_align {
        items::TextHorizontalAlignment::Left => skia_safe::textlayout::TextAlign::Left,
        items::TextHorizontalAlignment::Center => skia_safe::textlayout::TextAlign::Center,
        items::TextHorizontalAlignment::Right => skia_safe::textlayout::TextAlign::Right,
    });

    style.set_text_style(&text_style);

    let mut builder = FONT_CACHE.with(|font_cache| {
        skia_safe::textlayout::ParagraphBuilder::new(&style, font_cache.font_collection.clone())
    });

    if let Some(selection) = selection {
        let before_selection = &text[..selection.range.start];
        builder.add_text(before_selection);

        let mut selection_background_paint = skia_safe::Paint::default();
        selection_background_paint.set_color(to_skia_color(&selection.background));
        let mut selection_foreground_paint = skia_safe::Paint::default();
        selection_foreground_paint.set_color(to_skia_color(&selection.foreground));

        let mut selection_style = text_style.clone();
        selection_style.set_background_color(selection_background_paint);
        selection_style.set_foreground_color(selection_foreground_paint);

        builder.push_style(&selection_style);
        let selected_text = &text[selection.range.clone()];
        builder.add_text(selected_text);
        builder.pop();

        let after_selection = &text[selection.range.end..];
        builder.add_text(after_selection);
    } else {
        builder.add_text(text);
    }

    let mut paragraph = builder.build();
    paragraph.layout(max_width.map_or(core::f32::MAX, |physical_width| physical_width.get()));

    let layout_height = PhysicalLength::new(paragraph.height());

    let layout_top_y = match v_align {
        i_slint_core::items::TextVerticalAlignment::Top => PhysicalLength::zero(),
        i_slint_core::items::TextVerticalAlignment::Center => (max_height - layout_height) / 2.,
        i_slint_core::items::TextVerticalAlignment::Bottom => max_height - layout_height,
    };

    (paragraph, PhysicalPoint::from_lengths(Default::default(), layout_top_y))
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
) -> PhysicalRect {
    if string.is_empty() {
        return PhysicalRect::new(
            PhysicalPoint::default(),
            PhysicalSize::from_lengths(cursor_width, PhysicalLength::new(layout.height())),
        );
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
