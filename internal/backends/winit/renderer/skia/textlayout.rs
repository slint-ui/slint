// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_core::items;
use i_slint_core::{graphics::FontRequest, Coord};

pub const DEFAULT_FONT_SIZE: f32 = 12.;

thread_local! {
    pub static FONT_COLLECTION: skia_safe::textlayout::FontCollection = {
        let mut font_collection = skia_safe::textlayout::FontCollection::new();
        font_collection.set_default_font_manager(skia_safe::FontMgr::new(), None);
        font_collection
    }
}

pub fn create_layout(
    font_request: FontRequest,
    scale_factor: f32,
    text: &str,
    text_style: Option<skia_safe::textlayout::TextStyle>,
    (max_width, max_height): (Option<Coord>, Option<Coord>),
    (h_align, v_align): (items::TextHorizontalAlignment, items::TextVerticalAlignment),
    overflow: items::TextOverflow,
    wrap: items::TextWrap,
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
    if let Some(h) = max_height {
        style.set_height(h);
    }
    if overflow == items::TextOverflow::Elide {
        style.set_ellipsis("…");
    }

    style.set_text_align(match h_align {
        items::TextHorizontalAlignment::Left => skia_safe::textlayout::TextAlign::Left,
        items::TextHorizontalAlignment::Center => skia_safe::textlayout::TextAlign::Center,
        items::TextHorizontalAlignment::Right => skia_safe::textlayout::TextAlign::Right,
    });

    let mut builder = FONT_COLLECTION.with(|font_collection| {
        skia_safe::textlayout::ParagraphBuilder::new(&style, font_collection)
    });
    builder.push_style(&text_style);
    builder.add_text(text);
    let mut paragraph = builder.build();
    paragraph.layout(
        max_width
            .filter(|_| overflow == items::TextOverflow::Elide || wrap != items::TextWrap::NoWrap)
            .unwrap_or(core::f32::MAX),
    );
    paragraph
}
