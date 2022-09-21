// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::cell::RefCell;
use std::collections::HashMap;

use i_slint_core::items;
use i_slint_core::{graphics::FontRequest, Coord};

pub const DEFAULT_FONT_SIZE: f32 = 12.;

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

pub fn create_layout(
    font_request: FontRequest,
    scale_factor: f32,
    text: &str,
    text_style: Option<skia_safe::textlayout::TextStyle>,
    max_width: Option<Coord>,
    h_align: items::TextHorizontalAlignment,
    overflow: items::TextOverflow,
) -> skia_safe::textlayout::Paragraph {
    let mut text_style = text_style.unwrap_or_default();

    if let Some(family_name) = font_request.family {
        text_style.set_font_families(&[family_name.as_str()]);
    }

    let pixel_size = font_request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE) * scale_factor;

    if let Some(letter_spacing) = font_request.letter_spacing {
        text_style.set_letter_spacing(letter_spacing * scale_factor);
    }
    text_style.set_font_size(pixel_size);
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

    let mut builder = FONT_CACHE.with(|font_cache| {
        skia_safe::textlayout::ParagraphBuilder::new(&style, font_cache.font_collection.clone())
    });
    builder.push_style(&text_style);
    builder.add_text(text);
    let mut paragraph = builder.build();
    paragraph.layout(max_width.unwrap_or(core::f32::MAX));
    paragraph
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
    cursor_width: f32,
) -> skia_safe::Rect {
    if string.is_empty() {
        return skia_safe::Rect::from_xywh(0., 0., cursor_width, layout.height());
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
            skia_safe::Rect::from_xywh(x, textbox.rect.y(), cursor_width, textbox.rect.height())
        })
        .unwrap_or_default()
}
