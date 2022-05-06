// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use core::{marker::PhantomData, ops::Range};

use euclid::num::Zero;

use super::{GlyphProperties, ShapeBuffer};

#[derive(Clone)]
pub struct Grapheme<Length: Clone> {
    pub byte_range: Range<usize>,
    pub glyph_range: Range<usize>,
    pub width: Length,
    pub is_whitespace: bool,
}

#[derive(Clone)]
pub struct GraphemeIterator<'a, Length, Glyph> {
    text: &'a str,
    shaped_text: &'a ShapeBuffer<Glyph>,
    current_run: usize,
    // absolute byte offset in the entire text
    byte_offset: usize,
    glyph_index: usize,
    marker: PhantomData<Length>,
}

impl<'a, Length, Glyph> GraphemeIterator<'a, Length, Glyph> {
    pub fn new(text: &'a str, shaped_text: &'a ShapeBuffer<Glyph>) -> Self {
        Self {
            text,
            shaped_text,
            current_run: 0,
            byte_offset: 0,
            glyph_index: 0,
            marker: Default::default(),
        }
    }
}

impl<'a, Length: Clone + Zero + core::ops::AddAssign, Glyph: GlyphProperties<Length>> Iterator
    for GraphemeIterator<'a, Length, Glyph>
{
    type Item = Grapheme<Length>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_run >= self.shaped_text.text_runs.len() {
            return None;
        }

        let current_run =
            if self.byte_offset < self.shaped_text.text_runs[self.current_run].byte_range.end {
                &self.shaped_text.text_runs[self.current_run]
            } else {
                self.current_run += 1;
                self.shaped_text.text_runs.get(self.current_run)?
            };

        let mut grapheme_width: Length = Length::zero();

        let grapheme_glyph_start = self.glyph_index;

        let mut cluster_byte_offset;
        loop {
            let glyph = &self.shaped_text.glyphs[self.glyph_index];
            // Rustybuzz uses a relative byte offset as cluster index
            cluster_byte_offset = current_run.byte_range.start + glyph.byte_offset();
            if cluster_byte_offset != self.byte_offset {
                break;
            }
            grapheme_width += glyph.advance();

            self.glyph_index += 1;

            if self.glyph_index >= self.shaped_text.glyphs.len() {
                cluster_byte_offset = current_run.byte_range.end;
                break;
            }
        }
        let grapheme_byte_offset = self.byte_offset;
        let grapheme_byte_len = cluster_byte_offset - self.byte_offset;
        let is_whitespace = self.text[self.byte_offset..]
            .chars()
            .next()
            .map(|ch| ch.is_whitespace())
            .unwrap_or_default();
        self.byte_offset = cluster_byte_offset;

        Some(Grapheme {
            byte_range: Range {
                start: grapheme_byte_offset,
                end: grapheme_byte_offset + grapheme_byte_len,
            },
            glyph_range: Range { start: grapheme_glyph_start, end: self.glyph_index },
            width: grapheme_width,
            is_whitespace,
        })
    }
}
