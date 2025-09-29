// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub use parley;

use std::boxed::Box;
use std::cell::RefCell;

use crate::{
    graphics::FontRequest,
    items::TextStrokeStyle,
    lengths::{LogicalLength, ScaleFactor},
    textlayout::{TextHorizontalAlignment, TextOverflow, TextVerticalAlignment, TextWrap},
};
use i_slint_common::sharedfontique;

pub const DEFAULT_FONT_SIZE: LogicalLength = LogicalLength::new(12.);

struct Contexts {
    layout: parley::LayoutContext<Brush>,
    font: parley::FontContext,
}

impl Default for Contexts {
    fn default() -> Self {
        Self {
            font: parley::FontContext {
                collection: sharedfontique::COLLECTION.inner.clone(),
                source_cache: sharedfontique::COLLECTION.source_cache.clone(),
            },
            layout: Default::default(),
        }
    }
}

std::thread_local! {
    static CONTEXTS: RefCell<Box<Contexts>> = Default::default();
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
    let max_physical_height = options.max_height.map(|max_height| max_height * scale_factor);
    let pixel_size = options
        .font_request
        .as_ref()
        .and_then(|font_request| font_request.pixel_size)
        .unwrap_or(DEFAULT_FONT_SIZE);

    let push_to_builder = |builder: &mut parley::RangedBuilder<_>| {
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

        builder.push_default(parley::StyleProperty::Brush(Brush { stroke: options.stroke }));
    };

    let (mut layout, elision_info) = CONTEXTS.with_borrow_mut(move |contexts| {
        let elision_info = if let (TextOverflow::Elide, Some(max_physical_width)) =
            (options.text_overflow, max_physical_width)
        {
            let mut builder =
                contexts.layout.ranged_builder(&mut contexts.font, "…", scale_factor.get(), true);
            push_to_builder(&mut builder);
            let mut layout = builder.build("…");
            layout.break_all_lines(None);
            let line = layout.lines().next().unwrap();
            let item = line.items().next().unwrap();
            let run = match item {
                parley::layout::PositionedLayoutItem::GlyphRun(run) => Some(run),
                _ => None,
            }
            .unwrap();
            let glyph = run.positioned_glyphs().next().unwrap();
            Some(ElisionInfo { elipsis_glyph: glyph, max_physical_width })
        } else {
            None
        };

        let mut builder =
            contexts.layout.ranged_builder(&mut contexts.font, text, scale_factor.get(), true);
        push_to_builder(&mut builder);
        (builder.build(text), elision_info)
    });

    layout.break_all_lines(max_physical_width.filter(|_| options.text_wrap != TextWrap::NoWrap));
    layout.align(
        max_physical_width,
        match options.horizontal_align {
            TextHorizontalAlignment::Left => parley::Alignment::Left,
            TextHorizontalAlignment::Center => parley::Alignment::Middle,
            TextHorizontalAlignment::Right => parley::Alignment::Right,
        },
        parley::AlignmentOptions::default(),
    );

    let y_offset = match (max_physical_height, options.vertical_align) {
        (Some(max_height), TextVerticalAlignment::Center) => {
            (max_height.get() - layout.height()) / 2.0
        }
        (Some(max_height), TextVerticalAlignment::Bottom) => max_height.get() - layout.height(),
        (None, _) | (Some(_), TextVerticalAlignment::Top) => 0.0,
    };

    Layout { inner: layout, y_offset, elision_info }
}

pub struct ElisionInfo {
    pub elipsis_glyph: parley::layout::Glyph,
    pub max_physical_width: f32,
}

pub struct Layout {
    inner: parley::Layout<Brush>,
    pub y_offset: f32,
    pub elision_info: Option<ElisionInfo>,
}

impl Layout {
    /// Returns an iterator over the run's glyphs but with an optional elision
    /// glyph replacing the last line's last glyph that's exceeding the max width - if applicable.
    pub fn glyphs_with_elision<'a>(
        &'a self,
        glyph_run: &'a parley::layout::GlyphRun<Brush>,
        last_line: bool,
    ) -> impl Iterator<Item = parley::layout::Glyph> + Clone + 'a {
        let run_beyond_max_width = self.elision_info.as_ref().map_or(false, |info| {
            let run_end = glyph_run.offset() + glyph_run.advance();

            run_end > info.max_physical_width
        });

        let mut elipsis_emitted = false;
        glyph_run.positioned_glyphs().filter_map(move |mut glyph| {
            if !last_line {
                return Some(glyph);
            }
            if !run_beyond_max_width {
                return Some(glyph);
            }
            let Some(elision_info) = &self.elision_info else {
                return Some(glyph);
            };

            if glyph.x + glyph.advance + elision_info.elipsis_glyph.advance
                > elision_info.max_physical_width
            {
                if elipsis_emitted {
                    None
                } else {
                    elipsis_emitted = true;
                    glyph.advance = elision_info.elipsis_glyph.advance;
                    glyph.id = elision_info.elipsis_glyph.id;
                    Some(glyph)
                }
            } else {
                Some(glyph)
            }
        })
    }

    pub fn draw(
        &self,
        draw_glyphs: &mut dyn FnMut(
            &parley::Font,
            f32,
            &Option<TextStrokeStyle>,
            &mut dyn Iterator<Item = parley::layout::Glyph>,
        ),
    ) {
        for (line_index, line) in self.lines().enumerate() {
            let last_line = line_index == self.len() - 1;
            for item in line.items() {
                match item {
                    parley::PositionedLayoutItem::GlyphRun(glyph_run) => {
                        let run = glyph_run.run();

                        let font = run.font();

                        let brush = glyph_run.style().brush;

                        let mut glyphs = self.glyphs_with_elision(&glyph_run, last_line);

                        draw_glyphs(font, run.font_size(), &brush.stroke, &mut glyphs);
                    }
                    parley::PositionedLayoutItem::InlineBox(_inline_box) => {}
                };
            }
        }
    }
}

impl std::ops::Deref for Layout {
    type Target = parley::Layout<Brush>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
