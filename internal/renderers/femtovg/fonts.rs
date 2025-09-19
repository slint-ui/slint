// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cspell:ignore Noto fontconfig

use crate::{PhysicalLength, PhysicalPoint};
use core::num::NonZeroUsize;
use femtovg::TextContext;
use i_slint_common::sharedfontique::{self, parley};
use i_slint_core::{items::TextHorizontalAlignment, lengths::LogicalLength};
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

pub fn layout(
    text: &str,
    max_width: Option<PhysicalLength>,
    horizontal_align: TextHorizontalAlignment,
) -> parley::Layout<()> {
    let mut font_context = sharedfontique::font_context();
    let mut layout_context = sharedfontique::layout_context();

    let mut builder = layout_context.ranged_builder(&mut font_context, text, 1.0, true);
    builder.push_default(parley::StyleProperty::FontSize(16.0));
    let mut layout: parley::Layout<()> = builder.build(text);
    layout.break_all_lines(max_width.map(|max_width| max_width.get()));
    layout.align(
        max_width.map(|max_width| max_width.get()),
        match horizontal_align {
            TextHorizontalAlignment::Left => parley::Alignment::Left,
            TextHorizontalAlignment::Center => parley::Alignment::Middle,
            TextHorizontalAlignment::Right => parley::Alignment::Right,
        },
        parley::AlignmentOptions::default(),
    );
    layout
}

pub fn get_cursor_location(
    layout: &parley::Layout<()>,
    cursor_byte_offset: usize,
    offset: f32,
) -> Option<PhysicalPoint> {
    let mut cursor_point = None;

    for line in layout.lines() {
        for item in line.items() {
            match item {
                parley::PositionedLayoutItem::GlyphRun(glyph_run) => {
                    let range = glyph_run.run().text_range();
                    if range.contains(&cursor_byte_offset) {
                        cursor_point = glyph_run
                            .positioned_glyphs()
                            .nth(cursor_byte_offset - range.start)
                            .map(|glyph| PhysicalPoint::new(glyph.x, glyph.y + offset));
                    } else if cursor_byte_offset == range.end {
                        cursor_point = glyph_run.positioned_glyphs().last().map(|glyph| {
                            PhysicalPoint::new(glyph.x + glyph.advance, glyph.y + offset)
                        });
                    }
                }
                parley::PositionedLayoutItem::InlineBox(_inline_box) => {}
            };
        }
    }

    cursor_point
}
