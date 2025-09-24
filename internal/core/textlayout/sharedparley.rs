// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub use parley;

use std::cell::RefCell;

use crate::{
    graphics::FontRequest,
    items::TextStrokeStyle,
    lengths::{LogicalLength, ScaleFactor},
    textlayout::{TextHorizontalAlignment, TextOverflow, TextVerticalAlignment, TextWrap},
};
use i_slint_common::sharedfontique;

pub const DEFAULT_FONT_SIZE: LogicalLength = LogicalLength::new(12.);

fn font_context() -> parley::FontContext {
    parley::FontContext {
        collection: sharedfontique::COLLECTION.inner.clone(),
        source_cache: sharedfontique::COLLECTION.source_cache.clone(),
    }
}

std::thread_local! {
    static LAYOUT_CONTEXT: RefCell<parley::LayoutContext<Brush>> = Default::default();
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub struct Brush {
    pub stroke: Option<TextStrokeStyle>,
}

pub struct LayoutOptions {
    pub max_width: Option<LogicalLength>,
    pub max_height: Option<LogicalLength>,
    pub horizontal_align: TextHorizontalAlignment,
    pub vertical_align: TextVerticalAlignment,
    pub stroke: Option<TextStrokeStyle>,
    pub font_request: Option<FontRequest>,
    pub text_wrap: TextWrap,
    pub text_overflow: TextOverflow,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            max_width: None,
            max_height: None,
            horizontal_align: TextHorizontalAlignment::Left,
            vertical_align: TextVerticalAlignment::Top,
            stroke: None,
            font_request: None,
            text_wrap: TextWrap::WordWrap,
            text_overflow: TextOverflow::Clip,
        }
    }
}

pub fn layout(text: &str, scale_factor: ScaleFactor, options: LayoutOptions) -> Layout {
    let max_physical_width = options.max_width.map(|max_width| (max_width * scale_factor).get());
    let pixel_size = options
        .font_request
        .as_ref()
        .and_then(|font_request| font_request.pixel_size)
        .unwrap_or(DEFAULT_FONT_SIZE);

    let mut layout = LAYOUT_CONTEXT.with_borrow_mut(move |layout_context| {
        let mut font_context = font_context();
        let mut builder =
            layout_context.ranged_builder(&mut font_context, text, scale_factor.get(), true);
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
        builder.push_default(parley::StyleProperty::FontSize(pixel_size.get()));
        builder.push_default(parley::StyleProperty::WordBreak(match options.text_wrap {
            TextWrap::NoWrap => parley::style::WordBreakStrength::KeepAll,
            TextWrap::WordWrap => parley::style::WordBreakStrength::Normal,
            TextWrap::CharWrap => parley::style::WordBreakStrength::BreakAll,
        }));
        if options.text_overflow == TextOverflow::Elide {
            todo!();
        }

        builder.push_default(parley::StyleProperty::Brush(Brush { stroke: options.stroke }));

        builder.build(text)
    });

    layout.break_all_lines(max_physical_width);
    layout.align(
        max_physical_width,
        match options.horizontal_align {
            TextHorizontalAlignment::Left => parley::Alignment::Left,
            TextHorizontalAlignment::Center => parley::Alignment::Middle,
            TextHorizontalAlignment::Right => parley::Alignment::Right,
        },
        parley::AlignmentOptions::default(),
    );

    let y_offset = match (options.max_height, options.vertical_align) {
        (Some(max_height), TextVerticalAlignment::Center) => {
            (max_height.get() - layout.height()) / 2.0
        }
        (Some(max_height), TextVerticalAlignment::Bottom) => max_height.get() - layout.height(),
        (None, _) | (Some(_), TextVerticalAlignment::Top) => 0.0,
    };

    Layout { inner: layout, y_offset }
}

pub struct Layout {
    inner: parley::Layout<Brush>,
    pub y_offset: f32,
}

impl std::ops::Deref for Layout {
    type Target = parley::Layout<Brush>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
