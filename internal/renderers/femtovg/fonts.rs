// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cspell:ignore Noto fontconfig

use crate::PhysicalLength;
use core::num::NonZeroUsize;
use femtovg::TextContext;
use i_slint_common::sharedfontique::{self, parley};
use i_slint_core::{
    graphics::FontRequest,
    items::{TextHorizontalAlignment, TextVerticalAlignment, TextWrap},
    lengths::LogicalLength,
};
use std::cell::RefCell;
use std::collections::HashMap;

pub const DEFAULT_FONT_SIZE: LogicalLength = LogicalLength::new(12.);

pub struct FontCache {
    pub(crate) text_context: femtovg::TextContext,
    fonts: HashMap<(u64, u32), femtovg::FontId>,
}

impl Default for FontCache {
    fn default() -> Self {
        let text_context = TextContext::default();
        text_context.resize_shaped_words_cache(NonZeroUsize::new(10_000_000).unwrap());
        text_context.resize_shaping_run_cache(NonZeroUsize::new(1_000_000).unwrap());

        Self { text_context, fonts: Default::default() }
    }
}

impl FontCache {
    pub fn font(&mut self, font: &parley::Font) -> femtovg::FontId {
        let text_context = self.text_context.clone();

        *self.fonts.entry((font.data.id(), font.index)).or_insert_with(move || {
            text_context.add_shared_font_with_index(font.data.clone(), font.index).unwrap()
        })
    }
}

thread_local! {
    pub static FONT_CACHE: RefCell<FontCache> = RefCell::new(Default::default())
}

pub struct LayoutOptions {
    pub max_width: Option<PhysicalLength>,
    pub horizontal_align: TextHorizontalAlignment,
    pub stroke: Option<sharedfontique::BrushTextStrokeStyle>,
    pub selection: Option<std::ops::Range<usize>>,
    pub font_request: Option<FontRequest>,
    pub text_wrap: TextWrap,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            max_width: None,
            horizontal_align: TextHorizontalAlignment::Left,
            stroke: None,
            selection: None,
            font_request: None,
            text_wrap: TextWrap::WordWrap,
        }
    }
}

pub fn layout(text: &str, options: LayoutOptions) -> parley::Layout<sharedfontique::Brush> {
    let mut font_context = sharedfontique::font_context();
    let mut layout_context = sharedfontique::layout_context();

    let mut builder = layout_context.ranged_builder(&mut font_context, text, 1.0, true);
    if let Some(ref font_request) = options.font_request {
        if let Some(family) = &font_request.family {
            builder.push_default(parley::StyleProperty::FontStack(
                parley::style::FontStack::Single(parley::style::FontFamily::Named(
                    family.as_str().into(),
                )),
            ));
        }
        if let Some(weight) = font_request.weight {
            builder.push_default(parley::StyleProperty::FontWeight(
                parley::style::FontWeight::new(weight as f32),
            ));
        }
        if let Some(letter_spacing) = font_request.letter_spacing {
            builder.push_default(parley::StyleProperty::LetterSpacing(letter_spacing.get()));
        }
        builder.push_default(parley::StyleProperty::FontStyle(if font_request.italic {
            parley::style::FontStyle::Italic
        } else {
            parley::style::FontStyle::Normal
        }));
    }
    let pixel_size = options
        .font_request
        .and_then(|font_request| font_request.pixel_size)
        .unwrap_or(DEFAULT_FONT_SIZE);
    builder.push_default(parley::StyleProperty::FontSize(pixel_size.get()));
    builder.push_default(parley::StyleProperty::WordBreak(match options.text_wrap {
        TextWrap::NoWrap => parley::style::WordBreakStrength::KeepAll,
        TextWrap::WordWrap => parley::style::WordBreakStrength::Normal,
        TextWrap::CharWrap => parley::style::WordBreakStrength::BreakAll,
    }));

    builder.push_default(parley::StyleProperty::Brush(sharedfontique::Brush {
        stroke: options.stroke,
        ..Default::default()
    }));
    if let Some(selection) = options.selection {
        builder.push(
            parley::StyleProperty::Brush(sharedfontique::Brush {
                stroke: options.stroke,
                is_selected: true,
            }),
            selection,
        );
    }

    let mut layout: parley::Layout<sharedfontique::Brush> = builder.build(text);
    layout.break_all_lines(options.max_width.map(|max_width| max_width.get()));
    layout.align(
        options.max_width.map(|max_width| max_width.get()),
        match options.horizontal_align {
            TextHorizontalAlignment::Left => parley::Alignment::Left,
            TextHorizontalAlignment::Center => parley::Alignment::Middle,
            TextHorizontalAlignment::Right => parley::Alignment::Right,
        },
        parley::AlignmentOptions::default(),
    );
    layout
}

pub fn get_offset(
    vertical_align: TextVerticalAlignment,
    max_height: PhysicalLength,
    layout: &parley::Layout<sharedfontique::Brush>,
) -> f32 {
    match vertical_align {
        TextVerticalAlignment::Top => 0.0,
        TextVerticalAlignment::Center => (max_height.get() - layout.height()) / 2.0,
        TextVerticalAlignment::Bottom => max_height.get() - layout.height(),
    }
}
