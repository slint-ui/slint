// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::ops::Range;

use euclid::num::Zero;

use super::glyphclusters::GlyphClusterIterator;
use super::{BreakOpportunity, LineBreakIterator, ShapeBuffer};

#[derive(Debug, PartialEq, Eq, Default)]
pub struct TextFragment<Length> {
    pub byte_range: Range<usize>,
    pub glyph_range: Range<usize>,
    pub width: Length,
    pub trailing_whitespace_width: Length,
    pub trailing_whitespace_bytes: usize,
    pub trailing_mandatory_break: bool,
}

#[derive(Clone)]
pub struct TextFragmentIterator<'a, Length> {
    line_breaks: LineBreakIterator<'a>,
    glyph_clusters: GlyphClusterIterator<'a, Length>,
    text_len: usize,
    pub break_anywhere: bool,
}

impl<'a, Length> TextFragmentIterator<'a, Length> {
    pub fn new(text: &'a str, shape_buffer: &'a ShapeBuffer<Length>) -> Self {
        Self {
            line_breaks: LineBreakIterator::new(text),
            glyph_clusters: GlyphClusterIterator::new(text, shape_buffer),
            text_len: text.len(),
            break_anywhere: false,
        }
    }
}

impl<Length: Clone + Default + core::ops::AddAssign + Zero + Copy> Iterator
    for TextFragmentIterator<'_, Length>
{
    type Item = TextFragment<Length>;

    fn next(&mut self) -> Option<Self::Item> {
        let first_glyph_cluster = self.glyph_clusters.next()?;

        let mut fragment = Self::Item::default();

        let next_break_offset = if self.break_anywhere {
            if first_glyph_cluster.is_line_or_paragraph_separator {
                fragment.trailing_mandatory_break = true;
            }
            0
        } else if let Some((next_break_offset, break_type)) = self.line_breaks.next() {
            if matches!(break_type, BreakOpportunity::Mandatory) {
                fragment.trailing_mandatory_break = true;
            }
            next_break_offset
        } else {
            self.text_len
        };

        if first_glyph_cluster.is_whitespace {
            fragment.trailing_whitespace_width = first_glyph_cluster.width;
            fragment.trailing_whitespace_bytes = first_glyph_cluster.byte_range.len();
            fragment.byte_range.start = first_glyph_cluster.byte_range.start;
            fragment.byte_range.end = first_glyph_cluster.byte_range.start;
        } else {
            fragment.width = first_glyph_cluster.width;
            fragment.byte_range = first_glyph_cluster.byte_range.clone();
        }

        let start = first_glyph_cluster.glyph_range.start;
        let mut last_glyph_cluster = first_glyph_cluster;

        while last_glyph_cluster.byte_range.end < next_break_offset {
            let next_glyph_cluster = match self.glyph_clusters.next() {
                Some(cluster) => cluster,
                None => break,
            };

            if next_glyph_cluster.is_line_or_paragraph_separator {
                break;
            }

            if next_glyph_cluster.is_whitespace {
                fragment.trailing_whitespace_width += next_glyph_cluster.width;
                fragment.trailing_whitespace_bytes += next_glyph_cluster.byte_range.len();
            } else {
                // transition from whitespace to characters by treating previous trailing whitespace
                // as regular characters
                if last_glyph_cluster.is_whitespace {
                    fragment.width += core::mem::take(&mut fragment.trailing_whitespace_width);
                    fragment.width += next_glyph_cluster.width;
                    fragment.byte_range.end = next_glyph_cluster.byte_range.end;
                    fragment.trailing_whitespace_bytes = 0;
                } else {
                    fragment.width += next_glyph_cluster.width;
                    fragment.byte_range.end = next_glyph_cluster.byte_range.end;
                }
            }

            last_glyph_cluster = next_glyph_cluster.clone();
        }

        fragment.glyph_range = Range { start, end: last_glyph_cluster.glyph_range.end };

        // Make sure that adjacent fragments are advanced in their byte range:
        // this assertion should hold: fragment.byte_range.end + fragment.trailing_whitespace_bytes == next_fragment.byte_range.start
        // That means characters causing mandatory breaks need to be included.
        if fragment.trailing_mandatory_break && !self.break_anywhere {
            fragment.trailing_whitespace_bytes = next_break_offset - fragment.byte_range.end;
        }

        Some(fragment)
    }
}

#[cfg(test)]
use super::{FixedTestFont, TextLayout};
#[cfg(test)]
use std::{vec, vec::Vec};

#[test]
fn fragment_iterator_simple() {
    let font = FixedTestFont;
    let text = "H WX";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let fragments = TextFragmentIterator::new(text, &shape_buffer).collect::<Vec<_>>();
    let expected = vec![
        TextFragment {
            byte_range: Range { start: 0, end: 1 },
            glyph_range: Range { start: 0, end: 2 },
            width: 10.,
            trailing_whitespace_width: 10.,
            trailing_mandatory_break: false,
            trailing_whitespace_bytes: 1,
        },
        TextFragment {
            byte_range: Range { start: 2, end: text.len() },
            glyph_range: Range { start: 2, end: text.len() },
            width: 20.,
            trailing_whitespace_width: 0.,
            trailing_mandatory_break: false,
            trailing_whitespace_bytes: 0,
        },
    ];
    assert_eq!(fragments, expected);
}

#[test]
fn fragment_iterator_simple_v2() {
    let font = FixedTestFont;
    let text = "Hello World";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let fragments = TextFragmentIterator::new(text, &shape_buffer).collect::<Vec<_>>();
    let expected = vec![
        TextFragment {
            byte_range: Range { start: 0, end: 5 },
            glyph_range: Range { start: 0, end: 6 },
            width: 50.,
            trailing_whitespace_width: 10.,
            trailing_mandatory_break: false,
            trailing_whitespace_bytes: 1,
        },
        TextFragment {
            byte_range: Range { start: 6, end: text.len() },
            glyph_range: Range { start: 6, end: text.len() },
            width: 10. * (text.len() - 6) as f32,
            trailing_whitespace_width: 0.,
            trailing_whitespace_bytes: 0,
            trailing_mandatory_break: false,
        },
    ];
    assert_eq!(fragments, expected);
}

#[test]
fn fragment_iterator_forced_break() {
    let font = FixedTestFont;
    let text = "H\nW";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let fragments = TextFragmentIterator::new(text, &shape_buffer).collect::<Vec<_>>();
    assert_eq!(
        fragments,
        vec![
            TextFragment {
                byte_range: Range { start: 0, end: 1 },
                glyph_range: Range { start: 0, end: 1 },
                width: 10.,
                trailing_whitespace_width: 0.,
                trailing_whitespace_bytes: 1,
                trailing_mandatory_break: true,
            },
            TextFragment {
                byte_range: Range { start: 2, end: 3 },
                glyph_range: Range { start: 2, end: 3 },
                width: 10.,
                trailing_whitespace_width: 0.,
                trailing_whitespace_bytes: 0,
                trailing_mandatory_break: false,
            },
        ]
    );
}

#[test]
fn fragment_iterator_forced_break_multi() {
    let font = FixedTestFont;
    let text = "H\n\n\nW";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let fragments = TextFragmentIterator::new(text, &shape_buffer).collect::<Vec<_>>();
    assert_eq!(
        fragments,
        vec![
            TextFragment {
                byte_range: Range { start: 0, end: 1 },
                glyph_range: Range { start: 0, end: 1 },
                width: 10.,
                trailing_whitespace_width: 0.,
                trailing_whitespace_bytes: 1,
                trailing_mandatory_break: true,
            },
            TextFragment {
                byte_range: Range { start: 2, end: 2 },
                glyph_range: Range { start: 2, end: 3 },
                width: 0.,
                trailing_whitespace_width: 10.,
                trailing_whitespace_bytes: 1,
                trailing_mandatory_break: true,
            },
            TextFragment {
                byte_range: Range { start: 3, end: 3 },
                glyph_range: Range { start: 3, end: 4 },
                width: 0.,
                trailing_whitespace_width: 10.,
                trailing_whitespace_bytes: 1,
                trailing_mandatory_break: true,
            },
            TextFragment {
                byte_range: Range { start: 4, end: 5 },
                glyph_range: Range { start: 4, end: 5 },
                width: 10.,
                trailing_whitespace_width: 0.,
                trailing_whitespace_bytes: 0,
                trailing_mandatory_break: false,
            },
        ]
    );
}

#[test]
fn fragment_iterator_nbsp() {
    let font = FixedTestFont;
    let text = "X H\u{00a0}W";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let fragments = TextFragmentIterator::new(text, &shape_buffer).collect::<Vec<_>>();
    assert_eq!(
        fragments,
        vec![
            TextFragment {
                byte_range: Range { start: 0, end: 1 },
                glyph_range: Range { start: 0, end: 2 },
                width: 10.,
                trailing_whitespace_width: 10.,
                trailing_whitespace_bytes: 1,
                trailing_mandatory_break: false,
            },
            TextFragment {
                byte_range: Range { start: 2, end: 6 },
                glyph_range: Range { start: 2, end: 5 },
                width: 30.,
                trailing_whitespace_width: 0.,
                trailing_whitespace_bytes: 0,
                trailing_mandatory_break: false,
            }
        ]
    );
}

#[test]
fn fragment_iterator_break_anywhere() {
    let font = FixedTestFont;
    let text = "AB\nCD\nEF";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let mut fragments = TextFragmentIterator::new(text, &shape_buffer);
    assert_eq!(
        fragments.next(),
        Some(TextFragment {
            byte_range: Range { start: 0, end: 2 },
            glyph_range: Range { start: 0, end: 2 },
            width: 20.,
            trailing_whitespace_width: 0.,
            trailing_whitespace_bytes: 1,
            trailing_mandatory_break: true,
        })
    );
    assert_eq!(
        fragments.next(),
        Some(TextFragment {
            byte_range: Range { start: 3, end: 5 },
            glyph_range: Range { start: 3, end: 5 },
            width: 20.,
            trailing_whitespace_width: 0.,
            trailing_whitespace_bytes: 1,
            trailing_mandatory_break: true,
        },)
    );
    fragments.break_anywhere = true;
    let last_two = fragments.by_ref().take(2).collect::<Vec<_>>();
    assert_eq!(
        last_two,
        vec![
            TextFragment {
                byte_range: Range { start: 6, end: 7 },
                glyph_range: Range { start: 6, end: 7 },
                width: 10.,
                trailing_whitespace_width: 0.,
                trailing_whitespace_bytes: 0,
                trailing_mandatory_break: false,
            },
            TextFragment {
                byte_range: Range { start: 7, end: 8 },
                glyph_range: Range { start: 7, end: 8 },
                width: 10.,
                trailing_whitespace_width: 0.,
                trailing_whitespace_bytes: 0,
                trailing_mandatory_break: false,
            },
        ]
    );
}

#[test]
fn fragment_iterator_leading_nbsp() {
    let font = FixedTestFont;
    let text = "A\n\u{00a0}\u{00a0}AB";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let fragments = TextFragmentIterator::new(text, &shape_buffer).collect::<Vec<_>>();
    assert_eq!(
        fragments,
        vec![
            TextFragment {
                byte_range: Range { start: 0, end: 1 },
                glyph_range: Range { start: 0, end: 1 },
                width: 10.,
                trailing_whitespace_width: 0.,
                trailing_whitespace_bytes: 1,
                trailing_mandatory_break: true,
            },
            TextFragment {
                byte_range: Range { start: 2, end: 8 },
                glyph_range: Range { start: 2, end: 6 },
                width: 40.,
                trailing_whitespace_width: 0.,
                trailing_whitespace_bytes: 0,
                trailing_mandatory_break: false,
            }
        ]
    );
}
