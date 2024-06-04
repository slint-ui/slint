// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::{marker::PhantomData, ops::Range};

use euclid::num::Zero;

use super::ShapeBuffer;

#[derive(Clone)]
pub struct GlyphCluster<Length: Clone> {
    pub byte_range: Range<usize>,
    pub glyph_range: Range<usize>,
    pub width: Length,
    pub is_whitespace: bool,
    pub is_line_or_paragraph_separator: bool,
}

#[derive(Clone)]
pub struct GlyphClusterIterator<'a, Length> {
    text: &'a str,
    shaped_text: &'a ShapeBuffer<Length>,
    current_run: usize,
    // absolute byte offset in the entire text
    byte_offset: usize,
    glyph_index: usize,
    marker: PhantomData<Length>,
}

impl<'a, Length> GlyphClusterIterator<'a, Length> {
    pub fn new(text: &'a str, shaped_text: &'a ShapeBuffer<Length>) -> Self {
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

impl<'a, Length: Copy + Clone + Zero + core::ops::AddAssign> Iterator
    for GlyphClusterIterator<'a, Length>
{
    type Item = GlyphCluster<Length>;

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

        let mut cluster_width: Length = Length::zero();

        let cluster_start = self.glyph_index;

        let mut cluster_byte_offset;
        loop {
            let glyph = &self.shaped_text.glyphs[self.glyph_index];
            // Rustybuzz uses a relative byte offset as cluster index
            cluster_byte_offset = current_run.byte_range.start + glyph.text_byte_offset;
            if cluster_byte_offset != self.byte_offset {
                break;
            }
            cluster_width += glyph.advance;

            self.glyph_index += 1;

            if self.glyph_index >= current_run.glyph_range.end {
                cluster_byte_offset = current_run.byte_range.end;
                break;
            }
        }
        let byte_range = self.byte_offset..cluster_byte_offset;
        let (is_whitespace, is_line_or_paragraph_separator) = self.text[self.byte_offset..]
            .chars()
            .next()
            .map(|ch| {
                let is_line_or_paragraph_separator =
                    ch == '\n' || ch == '\u{2028}' || ch == '\u{2029}';
                (ch.is_whitespace(), is_line_or_paragraph_separator)
            })
            .unwrap_or_default();
        self.byte_offset = cluster_byte_offset;

        Some(GlyphCluster {
            byte_range,
            glyph_range: Range { start: cluster_start, end: self.glyph_index },
            width: cluster_width,
            is_whitespace,
            is_line_or_paragraph_separator,
        })
    }
}
