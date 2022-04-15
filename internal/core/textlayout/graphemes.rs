// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use core::ops::Range;

use euclid::num::Zero;

use super::{ShapeBuffer, TextShaper};

#[derive(Clone)]
pub struct Grapheme<Length> {
    pub byte_range: Range<usize>,
    pub glyph_range: Range<usize>,
    pub width: Length,
    pub is_whitespace: bool,
}

pub struct GraphemeIterator<'a, Font: TextShaper> {
    text: &'a str,
    font: &'a Font,
    shaped_text: &'a ShapeBuffer<Font>,
    current_run: usize,
    // absolute byte offset in the entire text
    byte_offset: usize,
    glyph_index: usize,
}

impl<'a, Font: TextShaper> Clone for GraphemeIterator<'a, Font> {
    fn clone(&self) -> Self {
        Self {
            text: self.text.clone(),
            font: self.font.clone(),
            shaped_text: self.shaped_text.clone(),
            current_run: self.current_run.clone(),
            byte_offset: self.byte_offset.clone(),
            glyph_index: self.glyph_index.clone(),
        }
    }
}

impl<'a, Font: TextShaper> GraphemeIterator<'a, Font> {
    pub fn new(text: &'a str, font: &'a Font, shaped_text: &'a ShapeBuffer<Font>) -> Self {
        Self { text, font, shaped_text, current_run: 0, byte_offset: 0, glyph_index: 0 }
    }
}

impl<'a, Font: TextShaper> Iterator for GraphemeIterator<'a, Font> {
    type Item = Grapheme<Font::Length>;

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

        let mut grapheme_width: Font::Length = Font::Length::zero();

        let grapheme_glyph_start = self.glyph_index;

        let mut cluster_byte_offset;
        loop {
            let (glyph, glyph_byte_offset) = &self.shaped_text.glyphs[self.glyph_index];
            // Rustybuzz uses a relative byte offset as cluster index
            cluster_byte_offset = current_run.byte_range.start + glyph_byte_offset;
            if cluster_byte_offset != self.byte_offset {
                break;
            }
            grapheme_width += self.font.glyph_advance_x(glyph);

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
