// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::ops::Range;

use euclid::num::Zero;

use crate::items::TextWrap;

use super::fragments::{TextFragment, TextFragmentIterator};
use super::{ShapeBuffer, TextShaper};

#[derive(Clone, Default, Debug)]
pub struct TextLine<Length: Default + Clone> {
    // The range excludes trailing whitespace
    pub byte_range: Range<usize>,
    // number of bytes in text after byte_range occupied by trailing whitespace
    pub trailing_whitespace_bytes: usize,
    pub(crate) glyph_range: Range<usize>,
    trailing_whitespace: Length,
    pub(crate) text_width: Length, // with as occupied by the glyphs
}

impl<
        Length: Default + Copy + Clone + Zero + core::ops::Add<Output = Length> + core::cmp::PartialOrd,
    > TextLine<Length>
{
    pub fn line_text<'a>(&self, paragraph: &'a str) -> &'a str {
        &paragraph[self.byte_range.clone()]
    }

    pub fn width_including_trailing_whitespace(&self) -> Length {
        if self.text_width > Length::zero() {
            self.text_width + self.trailing_whitespace
        } else {
            Length::zero()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.byte_range.is_empty()
    }
}

impl<Length: Clone + Copy + Default + core::ops::AddAssign> TextLine<Length> {
    pub fn add_fragment(&mut self, fragment: &TextFragment<Length>) {
        if self.byte_range.is_empty() {
            self.byte_range = fragment.byte_range.clone();
        } else if !fragment.byte_range.is_empty() {
            self.byte_range.end = fragment.byte_range.end;
        }
        if self.glyph_range.is_empty() {
            self.glyph_range = fragment.glyph_range.clone();
        } else {
            self.glyph_range.end = fragment.glyph_range.end;
        }
        if !fragment.byte_range.is_empty() {
            self.text_width += self.trailing_whitespace;
            self.trailing_whitespace = Length::default();
            self.trailing_whitespace_bytes = 0;
        }
        self.text_width += fragment.width;
        self.trailing_whitespace += fragment.trailing_whitespace_width;
        self.trailing_whitespace_bytes += fragment.trailing_whitespace_bytes;
    }
}

pub struct TextLineBreaker<'a, Font: TextShaper> {
    fragments: TextFragmentIterator<'a, Font::Length>,
    available_width: Option<Font::Length>,
    current_line: TextLine<Font::Length>,
    num_emitted_lines: usize,
    mandatory_line_break_on_next_iteration: bool,
    max_lines: Option<usize>,
    text_wrap: TextWrap,
}

impl<'a, Font: TextShaper> TextLineBreaker<'a, Font> {
    pub fn new(
        text: &'a str,
        shape_buffer: &'a ShapeBuffer<Font::Length>,
        available_width: Option<Font::Length>,
        max_lines: Option<usize>,
        text_wrap: TextWrap,
    ) -> Self {
        Self {
            fragments: TextFragmentIterator::new(text, shape_buffer),
            available_width,
            current_line: Default::default(),
            num_emitted_lines: 0,
            mandatory_line_break_on_next_iteration: false,
            max_lines,
            text_wrap,
        }
    }
}

impl<Font: TextShaper> Iterator for TextLineBreaker<'_, Font> {
    type Item = TextLine<Font::Length>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(max_lines) = self.max_lines {
            if self.num_emitted_lines >= max_lines {
                return None;
            }
        }

        if core::mem::take(&mut self.mandatory_line_break_on_next_iteration) {
            self.num_emitted_lines += 1;
            return Some(core::mem::take(&mut self.current_line));
        }

        self.fragments.break_anywhere = false;

        let mut next_line = loop {
            // Clone the fragment iterator so that we can roll back in case we must break down the first
            // word with `break_anywhere = true`.
            let mut fragments = self.fragments.clone();

            let fragment = match fragments.next() {
                Some(fragment) => fragment,
                None => {
                    break None;
                }
            };

            // As trailing_mandatory_break is only set if break_anywhere is false, the fragment must
            // be first processed with break_anywhere = false and if no mandatory break is found, the
            // loop is re-run with break_anywhere = true when using CharWrap.
            if self.text_wrap == TextWrap::CharWrap
                && !fragment.trailing_mandatory_break
                && !self.fragments.break_anywhere
            {
                self.fragments.break_anywhere = true;
                continue;
            }

            if let Some(available_width) = self.available_width {
                if self.current_line.width_including_trailing_whitespace() + fragment.width
                    > available_width
                {
                    if self.current_line.is_empty() {
                        if !self.fragments.break_anywhere {
                            // Try again but break anywhere this time. self.fragments is cloned at the beginning
                            // of the loop.
                            self.fragments.break_anywhere = true;
                            continue;
                        } else {
                            // Even if we allow to break anywhere, there is still no room for the next fragment.
                            // Just use it anywhere otherwise we would return many empty lines
                            self.fragments = fragments;
                            self.current_line.add_fragment(&fragment);
                            break Some(core::mem::take(&mut self.current_line));
                        }
                    }

                    let next_line = core::mem::take(&mut self.current_line);
                    self.mandatory_line_break_on_next_iteration = fragment.trailing_mandatory_break;

                    if self.text_wrap != TextWrap::CharWrap
                        && !fragments.break_anywhere
                        && fragment.width < available_width
                    {
                        self.current_line.add_fragment(&fragment);
                        self.fragments = fragments;
                    }

                    break Some(next_line);
                };
            }

            self.fragments = fragments;
            self.current_line.add_fragment(&fragment);

            if fragment.trailing_mandatory_break {
                break Some(core::mem::take(&mut self.current_line));
            }
        };

        // Emit at least one single line
        if next_line.is_none()
            && (!self.current_line.byte_range.is_empty() || self.num_emitted_lines == 0)
        {
            next_line = Some(core::mem::take(&mut self.current_line));
        }

        if next_line.is_some() {
            self.num_emitted_lines += 1;
        }

        next_line
    }
}

#[cfg(test)]
use super::{FixedTestFont, TextLayout};

#[test]
fn test_empty_line_break() {
    let font = FixedTestFont;
    let text = "";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(50.),
        None,
        TextWrap::WordWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].line_text(text), "");
}

#[test]
fn test_basic_line_break_char_wrap() {
    // The available width is half-way into the next word
    let font = FixedTestFont;
    let text = "Hello World";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(80.),
        None,
        TextWrap::CharWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].line_text(text), "Hello Wo");
    assert_eq!(lines[1].line_text(text), "rld");
}

#[test]
fn test_basic_line_break() {
    let font = FixedTestFont;
    let text = "Hello World";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(50.),
        None,
        TextWrap::WordWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].line_text(text), "Hello");
    assert_eq!(lines[1].line_text(text), "World");
}

#[test]
fn test_basic_line_break_max_lines() {
    let font = FixedTestFont;
    let text = "Hello World";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(50.),
        Some(1),
        TextWrap::WordWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].line_text(text), "Hello");
}

#[test]
fn test_linebreak_trailing_space() {
    let font = FixedTestFont;
    let text = "Hello              ";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(50.),
        None,
        TextWrap::WordWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].line_text(text), "Hello");
}

#[test]
fn test_forced_break() {
    let font = FixedTestFont;
    let text = "Hello\nWorld";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines =
        TextLineBreaker::<FixedTestFont>::new(text, &shape_buffer, None, None, TextWrap::WordWrap)
            .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].line_text(text), "Hello");
    assert_eq!(lines[1].line_text(text), "World");
}

#[test]
fn test_forced_break_multi() {
    let font = FixedTestFont;
    let text = "Hello\n\n\nWorld";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines =
        TextLineBreaker::<FixedTestFont>::new(text, &shape_buffer, None, None, TextWrap::WordWrap)
            .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].line_text(text), "Hello");
    assert_eq!(lines[1].line_text(text), "");
    assert_eq!(lines[2].line_text(text), "");
    assert_eq!(lines[3].line_text(text), "World");
}

#[test]
fn test_forced_break_multi_char_wrap() {
    let font = FixedTestFont;
    let text = "Hello\n\n\nWorld";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(30.),
        None,
        TextWrap::CharWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 6);
    assert_eq!(lines[0].line_text(text), "Hel");
    assert_eq!(lines[1].line_text(text), "lo");
    assert_eq!(lines[2].line_text(text), "");
    assert_eq!(lines[3].line_text(text), "");
    assert_eq!(lines[4].line_text(text), "Wor");
    assert_eq!(lines[5].line_text(text), "ld");
}

#[test]
fn test_forced_break_max_lines() {
    let font = FixedTestFont;
    let text = "Hello\n\n\nWorld";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        None,
        Some(2),
        TextWrap::WordWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].line_text(text), "Hello");
    assert_eq!(lines[1].line_text(text), "");
}

#[test]
fn test_nbsp_break() {
    let font = FixedTestFont;
    let text = "Ok Hello\u{00a0}World";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(110.),
        None,
        TextWrap::WordWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].line_text(text), "Ok");
    assert_eq!(lines[1].line_text(text), "Hello\u{00a0}World");
}

#[test]
fn test_single_line_multi_break_opportunity() {
    let font = FixedTestFont;
    let text = "a b c";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines =
        TextLineBreaker::<FixedTestFont>::new(text, &shape_buffer, None, None, TextWrap::WordWrap)
            .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].line_text(text), "a b c");
}

#[test]
fn test_basic_line_break_anywhere_fallback() {
    let font = FixedTestFont;
    let text = "HelloWorld";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(50.),
        None,
        TextWrap::WordWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].line_text(text), "Hello");
    assert_eq!(lines[1].line_text(text), "World");
}

#[test]
fn test_basic_line_break_anywhere_fallback_multi_line() {
    let font = FixedTestFont;
    let text = "HelloWorld\nHelloWorld";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(50.),
        None,
        TextWrap::WordWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].line_text(text), "Hello");
    assert_eq!(lines[1].line_text(text), "World");
    assert_eq!(lines[2].line_text(text), "Hello");
    assert_eq!(lines[3].line_text(text), "World");
}

#[test]
fn test_basic_line_break_anywhere_fallback_multi_line_char_wrap() {
    let font = FixedTestFont;
    let text = "HelloWorld\nHelloWorld";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(50.),
        None,
        TextWrap::CharWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].line_text(text), "Hello");
    assert_eq!(lines[1].line_text(text), "World");
    assert_eq!(lines[2].line_text(text), "Hello");
    assert_eq!(lines[3].line_text(text), "World");
}

#[test]
fn test_basic_line_break_anywhere_fallback_multi_line_v2() {
    let font = FixedTestFont;
    let text = "HelloW orldHellow";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(50.),
        None,
        TextWrap::WordWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].line_text(text), "Hello");
    assert_eq!(lines[1].line_text(text), "W");
    assert_eq!(lines[2].line_text(text), "orldH");
    assert_eq!(lines[3].line_text(text), "ellow");
}

#[test]
fn test_basic_line_break_anywhere_fallback_max_lines() {
    let font = FixedTestFont;
    let text = "HelloW orldHellow";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(50.),
        Some(3),
        TextWrap::WordWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].line_text(text), "Hello");
    assert_eq!(lines[1].line_text(text), "W");
    assert_eq!(lines[2].line_text(text), "orldH");
}

#[test]
fn test_basic_line_break_space() {
    // The available width is half-way into the trailing "W"
    let font = FixedTestFont;
    let text = "H W";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(25.),
        None,
        TextWrap::WordWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].line_text(text), "H");
    assert_eq!(lines[1].line_text(text), "W");
}

#[test]
fn test_basic_line_break_space_char_wrap() {
    // The available width is half-way into the trailing "W"
    let font = FixedTestFont;
    let text = "H W";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(25.),
        None,
        TextWrap::CharWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].line_text(text), "H");
    assert_eq!(lines[1].line_text(text), "W");
}

#[test]
fn test_basic_line_break_space_v2() {
    // The available width is half-way into the trailing "W"
    let font = FixedTestFont;
    let text = "B B W";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(45.),
        None,
        TextWrap::WordWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].line_text(text), "B B");
    assert_eq!(lines[1].line_text(text), "W");
}

#[test]
fn test_basic_line_break_space_v3() {
    // The available width is half-way into the trailing "W"
    let font = FixedTestFont;
    let text = "H   W";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(15.),
        None,
        TextWrap::WordWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].line_text(text), "H");
    assert_eq!(lines[1].line_text(text), "W");
}

#[test]
fn test_basic_line_break_space_v4() {
    // The available width is half-way into the trailing space
    let font = FixedTestFont;
    let text = "H W  H  ";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(65.),
        None,
        TextWrap::WordWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].line_text(text), "H W  H");
}

#[test]
fn test_line_width_with_whitespace() {
    let font = FixedTestFont;
    let text = "Hello World";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(200.),
        None,
        TextWrap::WordWrap,
    )
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text_width, text.len() as f32 * 10.);
}

#[test]
fn zero_width() {
    let font = FixedTestFont;
    let text = "He\nHe o";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(0.0001),
        None,
        TextWrap::WordWrap,
    )
    .map(|t| t.line_text(text))
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines, ["H", "e", "", "H", "e", "o"]);
}

#[test]
fn zero_width_char_wrap() {
    let font = FixedTestFont;
    let text = "He\nHe o";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(0.0001),
        None,
        TextWrap::CharWrap,
    )
    .map(|t| t.line_text(text))
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines, ["H", "e", "", "H", "e", "o"]);
}

#[test]
fn char_wrap_sentences() {
    let font = FixedTestFont;
    let text = "Hello world\nHow are you?";
    let shape_buffer = ShapeBuffer::new(&TextLayout { font: &font, letter_spacing: None }, text);
    let lines = TextLineBreaker::<FixedTestFont>::new(
        text,
        &shape_buffer,
        Some(80.),
        None,
        TextWrap::CharWrap,
    )
    .map(|t| t.line_text(text))
    .collect::<std::vec::Vec<_>>();
    assert_eq!(lines, ["Hello wo", "rld", "How are", "you?"]);
}
