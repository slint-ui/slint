// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use core::ops::Range;

use euclid::num::Zero;

use super::graphemes::GraphemeIterator;
use super::{BreakOpportunity, LineBreakIterator, ShapeBuffer};

#[derive(Debug, PartialEq, Eq, Default)]
pub struct TextFragment<Length> {
    pub byte_range: Range<usize>,
    pub glyph_range: Range<usize>,
    pub width: Length,
    pub trailing_whitespace_width: Length,
    pub trailing_mandatory_break: bool,
}

#[derive(Clone)]
pub struct TextFragmentIterator<'a, Length, PlatformGlyph> {
    line_breaks: LineBreakIterator<'a>,
    grapheme_cursor: GraphemeIterator<'a, Length, PlatformGlyph>,
    text_len: usize,
    pub break_anywhere: bool,
}

impl<'a, Length, PlatformGlyph> TextFragmentIterator<'a, Length, PlatformGlyph> {
    pub fn new(text: &'a str, shape_buffer: &'a ShapeBuffer<Length, PlatformGlyph>) -> Self {
        Self {
            line_breaks: LineBreakIterator::new(text),
            grapheme_cursor: GraphemeIterator::new(text, shape_buffer),
            text_len: text.len(),
            break_anywhere: false,
        }
    }
}

impl<'a, Length: Clone + Default + core::ops::AddAssign + Zero + Copy, PlatformGlyph> Iterator
    for TextFragmentIterator<'a, Length, PlatformGlyph>
{
    type Item = TextFragment<Length>;

    fn next(&mut self) -> Option<Self::Item> {
        let first_grapheme = self.grapheme_cursor.next()?;

        let mut fragment = Self::Item::default();

        let next_break_offset = if self.break_anywhere {
            0
        } else if let Some((next_break_offset, break_type)) = self.line_breaks.next() {
            if matches!(break_type, BreakOpportunity::Mandatory) {
                fragment.trailing_mandatory_break = true;
            }
            next_break_offset
        } else {
            self.text_len
        };

        if first_grapheme.is_whitespace {
            fragment.trailing_whitespace_width = first_grapheme.width;
        } else {
            fragment.width = first_grapheme.width;
            fragment.byte_range = first_grapheme.byte_range.clone();
        }

        let mut last_grapheme = first_grapheme.clone();

        while last_grapheme.byte_range.end < next_break_offset {
            let next_grapheme = match self.grapheme_cursor.next() {
                Some(grapheme) => grapheme,
                None => break,
            };

            if next_grapheme.is_whitespace {
                fragment.trailing_whitespace_width += next_grapheme.width;
            } else {
                // transition from whitespace to characters by treating previous trailing whitespace
                // as regular characters
                if last_grapheme.is_whitespace {
                    fragment.width += core::mem::take(&mut fragment.trailing_whitespace_width);
                    fragment.width += next_grapheme.width;
                    fragment.byte_range.end = next_grapheme.byte_range.end;
                } else {
                    fragment.width += next_grapheme.width;
                    fragment.byte_range.end = next_grapheme.byte_range.end;
                }
            }

            last_grapheme = next_grapheme.clone();
        }

        fragment.glyph_range =
            Range { start: first_grapheme.glyph_range.start, end: last_grapheme.glyph_range.end };

        Some(fragment)
    }
}

#[cfg(test)]
use super::FixedTestFont;

#[test]
fn fragment_iterator_simple() {
    let font = FixedTestFont;
    let text = "H WX";
    let shape_buffer = ShapeBuffer::new(&font, text);
    let fragments = TextFragmentIterator::new(text, &shape_buffer).collect::<Vec<_>>();
    let expected = vec![
        TextFragment {
            byte_range: Range { start: 0, end: 1 },
            glyph_range: Range { start: 0, end: 2 },
            width: 10.,
            trailing_whitespace_width: 10.,
            trailing_mandatory_break: false,
        },
        TextFragment {
            byte_range: Range { start: 2, end: text.len() },
            glyph_range: Range { start: 2, end: text.len() },
            width: 20.,
            trailing_whitespace_width: 0.,
            trailing_mandatory_break: false,
        },
    ];
    assert_eq!(fragments, expected);
}

#[test]
fn fragment_iterator_simple_v2() {
    let font = FixedTestFont;
    let text = "Hello World";
    let shape_buffer = ShapeBuffer::new(&font, text);
    let fragments = TextFragmentIterator::new(text, &shape_buffer).collect::<Vec<_>>();
    let expected = vec![
        TextFragment {
            byte_range: Range { start: 0, end: 5 },
            glyph_range: Range { start: 0, end: 6 },
            width: 50.,
            trailing_whitespace_width: 10.,
            trailing_mandatory_break: false,
        },
        TextFragment {
            byte_range: Range { start: 6, end: text.len() },
            glyph_range: Range { start: 6, end: text.len() },
            width: 10. * (text.len() - 6) as f32,
            trailing_whitespace_width: 0.,
            trailing_mandatory_break: false,
        },
    ];
    assert_eq!(fragments, expected);
}

#[test]
fn fragment_iterator_forced_break() {
    let font = FixedTestFont;
    let text = "H\nW";
    let shape_buffer = ShapeBuffer::new(&font, text);
    let fragments = TextFragmentIterator::new(text, &shape_buffer).collect::<Vec<_>>();
    assert_eq!(
        fragments,
        vec![
            TextFragment {
                byte_range: Range { start: 0, end: 1 },
                glyph_range: Range { start: 0, end: 2 },
                width: 10.,
                trailing_whitespace_width: 10.,
                trailing_mandatory_break: true,
            },
            TextFragment {
                byte_range: Range { start: 2, end: 3 },
                glyph_range: Range { start: 2, end: 3 },
                width: 10.,
                trailing_whitespace_width: 0.,
                trailing_mandatory_break: false,
            },
        ]
    );
}

#[test]
fn fragment_iterator_forced_break_multi() {
    let font = FixedTestFont;
    let text = "H\n\n\nW";
    let shape_buffer = ShapeBuffer::new(&font, text);
    let fragments = TextFragmentIterator::new(text, &shape_buffer).collect::<Vec<_>>();
    assert_eq!(
        fragments,
        vec![
            TextFragment {
                byte_range: Range { start: 0, end: 1 },
                glyph_range: Range { start: 0, end: 2 },
                width: 10.,
                trailing_whitespace_width: 10.,
                trailing_mandatory_break: true,
            },
            TextFragment {
                byte_range: Range { start: 0, end: 0 },
                glyph_range: Range { start: 2, end: 3 },
                width: 0.,
                trailing_whitespace_width: 10.,
                trailing_mandatory_break: true,
            },
            TextFragment {
                byte_range: Range { start: 0, end: 0 },
                glyph_range: Range { start: 3, end: 4 },
                width: 0.,
                trailing_whitespace_width: 10.,
                trailing_mandatory_break: true,
            },
            TextFragment {
                byte_range: Range { start: 4, end: 5 },
                glyph_range: Range { start: 4, end: 5 },
                width: 10.,
                trailing_whitespace_width: 0.,
                trailing_mandatory_break: false,
            },
        ]
    );
}

#[test]
fn fragment_iterator_nbsp() {
    let font = FixedTestFont;
    let text = "X H\u{00a0}W";
    let shape_buffer = ShapeBuffer::new(&font, text);
    let fragments = TextFragmentIterator::new(text, &shape_buffer).collect::<Vec<_>>();
    assert_eq!(
        fragments,
        vec![
            TextFragment {
                byte_range: Range { start: 0, end: 1 },
                glyph_range: Range { start: 0, end: 2 },
                width: 10.,
                trailing_whitespace_width: 10.,
                trailing_mandatory_break: false,
            },
            TextFragment {
                byte_range: Range { start: 2, end: 6 },
                glyph_range: Range { start: 2, end: 5 },
                width: 30.,
                trailing_whitespace_width: 0.,
                trailing_mandatory_break: false,
            }
        ]
    );
}

#[test]
fn fragment_iterator_break_anywhere() {
    let font = FixedTestFont;
    let text = "AB\nCD\nEF";
    let shape_buffer = ShapeBuffer::new(&font, text);
    let mut fragments = TextFragmentIterator::new(text, &shape_buffer);
    assert_eq!(
        fragments.next(),
        Some(TextFragment {
            byte_range: Range { start: 0, end: 2 },
            glyph_range: Range { start: 0, end: 3 },
            width: 20.,
            trailing_whitespace_width: 10.,
            trailing_mandatory_break: true,
        })
    );
    assert_eq!(
        fragments.next(),
        Some(TextFragment {
            byte_range: Range { start: 3, end: 5 },
            glyph_range: Range { start: 3, end: 6 },
            width: 20.,
            trailing_whitespace_width: 10.,
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
                trailing_mandatory_break: false,
            },
            TextFragment {
                byte_range: Range { start: 7, end: 8 },
                glyph_range: Range { start: 7, end: 8 },
                width: 10.,
                trailing_whitespace_width: 0.,
                trailing_mandatory_break: false,
            },
        ]
    );
}
