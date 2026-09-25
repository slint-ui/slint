// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore bidi lineseparator
use alloc::vec::Vec;
use core::ops::Range;

use super::TextLayout;

/// This struct describes a glyph from shaping to rendering. This includes the relative shaping
/// offsets, advance (in abstract lengths) and platform specific glyph data.
#[derive(Clone, Default, Debug)]
pub struct Glyph<Length> {
    pub advance: Length,
    pub offset_x: Length,
    pub offset_y: Length,
    /// Glyph IDs are font specific identifiers. In TrueType fonts zero indicates the missing glyph, which
    /// is mapped to an Option here.
    pub glyph_id: Option<core::num::NonZeroU16>,
    /// The byte offset back in the original (Rust) string to the character that
    /// "produced" this glyph. When one character produces multiple glyphs (for example
    /// decomposed ligature), then all glyphs have the same offset.
    pub text_byte_offset: usize,
}

/// Adds two widths, returning `None` when the result would not fit the coordinate type. A line
/// wider than that cannot be positioned or displayed anyway, so the layout stops there instead of
/// overflowing (which would panic in a debug build). Floats never overflow, so they always add.
pub trait CheckedAdd: Copy {
    /// Adds, returning `None` if the result would not fit the coordinate type.
    fn checked_add(self, other: Self) -> Option<Self>;

    /// Adds, clamping to the coordinate type's maximum instead of overflowing.
    fn saturating_add(self, other: Self) -> Self;
}

impl CheckedAdd for f32 {
    fn checked_add(self, other: Self) -> Option<Self> {
        Some(self + other)
    }

    fn saturating_add(self, other: Self) -> Self {
        self + other
    }
}

impl<U> CheckedAdd for euclid::Length<i16, U> {
    fn checked_add(self, other: Self) -> Option<Self> {
        self.get().checked_add(other.get()).map(euclid::Length::new)
    }

    fn saturating_add(self, other: Self) -> Self {
        euclid::Length::new(self.get().saturating_add(other.get()))
    }
}

/// This trait defines the interface between the text layout and the platform specific
/// mapping of text to glyphs. An implementation of the TextShaper trait must provide
/// metric types (Length, LengthPrimitive), which is used for the line breaking calculation
/// and glyph positioning, as well as an opaque platform specific glyph data type.
///
/// Functionality wise it provides the ability to convert a string into a set of glyphs,
/// each of which has basic metric fields as well as an offset back into the original string.
/// Typically this is implemented by using a general text shaper, which performs an M:N mapping
/// from unicode characters to glyphs, via glyph substitutions and script specific rules. In addition
/// the glyphs may be positioned for the required appearance (such as stacked diacritics).
///
/// Finally, for convenience the TextShaper also provides a single glyph_for_char function, for example
/// used to lookup single glyphs (such as the elision character) as well as additional metrics
/// used for text paragraph layout.
pub trait TextShaper {
    type LengthPrimitive: core::ops::Mul
        + core::ops::Div
        + core::ops::Add<Output = Self::LengthPrimitive>
        + core::ops::AddAssign
        + euclid::num::Zero
        + euclid::num::One
        + core::convert::From<i16>
        + Copy
        + core::fmt::Debug;
    type Length: euclid::num::Zero
        + CheckedAdd
        + core::ops::AddAssign
        + core::ops::Add<Output = Self::Length>
        + core::ops::Sub<Output = Self::Length>
        + Default
        + Clone
        + Copy
        + core::cmp::PartialOrd
        + core::ops::Mul<Self::LengthPrimitive, Output = Self::Length>
        + core::ops::Div<Self::LengthPrimitive, Output = Self::Length>
        + DivCount
        + core::fmt::Debug;
    // Shapes the given string and emits the result into the given glyphs buffer.
    fn shape_text<GlyphStorage: core::iter::Extend<Glyph<Self::Length>>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    );
    fn glyph_for_char(&self, ch: char) -> Option<Glyph<Self::Length>>;
}

/// How many times one length fits into another of the same unit.
/// euclid's `Length / Length` produces a `Scale` rather than a plain number, so the division the
/// paragraph layout needs is spelled out here instead of as a `core::ops::Div` bound.
pub trait DivCount {
    /// The number of whole `divisor`s in `self`, truncated, and zero when that count is negative.
    /// `divisor` must not be zero.
    fn div_count(self, divisor: Self) -> usize;
}

impl DivCount for f32 {
    fn div_count(self, divisor: Self) -> usize {
        // Casts from float saturate, so a negative or NaN ratio ends up at zero.
        (self / divisor) as usize
    }
}

impl DivCount for i16 {
    fn div_count(self, divisor: Self) -> usize {
        // Widen so that i16::MIN / -1 can't overflow.
        (i32::from(self) / i32::from(divisor)).max(0) as usize
    }
}

impl<T: DivCount + Clone, U> DivCount for euclid::Length<T, U> {
    fn div_count(self, divisor: Self) -> usize {
        self.get().div_count(divisor.get())
    }
}

pub trait FontMetrics<Length: Copy + core::ops::Sub<Output = Length>> {
    fn height(&self) -> Length {
        self.ascent() - self.descent()
    }
    fn ascent(&self) -> Length;
    fn descent(&self) -> Length;
    fn x_height(&self) -> Length;
    fn cap_height(&self) -> Length;
}

pub trait AbstractFont: TextShaper + FontMetrics<<Self as TextShaper>::Length> {}

impl<T> AbstractFont for T where T: TextShaper + FontMetrics<<Self as TextShaper>::Length> {}

pub struct ShapeBoundaries<'a> {
    text: &'a str,
    #[cfg(feature = "unicode-script")]
    // TODO: We should do a better analysis to find boundaries for text shaping; including
    // boundaries when the bidi level changes or an explicit separator like
    // paragraph/lineseparator/space is encountered.
    chars: core::str::CharIndices<'a>,
    next_boundary_start: Option<usize>,
    #[cfg(feature = "unicode-script")]
    last_script: Option<unicode_script::Script>,
}

impl<'a> ShapeBoundaries<'a> {
    pub fn new(text: &'a str) -> Self {
        let next_boundary_start = if !text.is_empty() { Some(0) } else { None };
        Self {
            text,
            #[cfg(feature = "unicode-script")]
            chars: text.char_indices(),
            next_boundary_start,
            #[cfg(feature = "unicode-script")]
            last_script: None,
        }
    }
}

impl Iterator for ShapeBoundaries<'_> {
    type Item = usize;

    #[cfg(feature = "unicode-script")]
    fn next(&mut self) -> Option<Self::Item> {
        self.next_boundary_start?;

        let (next_offset, script) = loop {
            match self.chars.next() {
                Some((byte_offset, ch)) => {
                    use unicode_script::UnicodeScript;
                    let next_script = ch.script();
                    let previous_script = *self.last_script.get_or_insert(next_script);

                    if next_script == previous_script {
                        continue;
                    }
                    if matches!(
                        next_script,
                        unicode_script::Script::Unknown
                            | unicode_script::Script::Common
                            | unicode_script::Script::Inherited,
                    ) {
                        continue;
                    }

                    break (Some(byte_offset), Some(next_script));
                }
                None => {
                    break (None, None);
                }
            }
        };

        self.last_script = script;
        self.next_boundary_start = next_offset;

        Some(self.next_boundary_start.unwrap_or(self.text.len()))
    }

    #[cfg(not(feature = "unicode-script"))]
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_boundary_start {
            Some(_) => {
                self.next_boundary_start = None;
                Some(self.text.len())
            }
            None => None,
        }
    }
}

#[derive(Debug)]
pub struct TextRun {
    pub byte_range: Range<usize>,
    pub glyph_range: Range<usize>,
    // TODO: direction, etc.
}

pub struct ShapeBuffer<Length> {
    pub glyphs: Vec<Glyph<Length>>,
    pub text_runs: Vec<TextRun>,
}

impl<Length> ShapeBuffer<Length> {
    pub fn new<Font>(layout: &TextLayout<Font>, text: &str) -> Self
    where
        Font: AbstractFont<Length = Length>,
        Length: Copy + core::ops::AddAssign,
    {
        let mut glyphs = Vec::new();
        let text_runs = ShapeBoundaries::new(text)
            .scan(0, |run_start, run_end| {
                let glyphs_start = glyphs.len();

                layout.font.shape_text(&text[*run_start..run_end], &mut glyphs);

                // Make the cluster index absolute.
                //
                // A shaper sees one run's slice, so the offset it reports is
                // relative to that slice. Everything downstream compares these
                // against indices into the whole string: TextLine::byte_range,
                // a selection range, and the byte offset a click maps to. This
                // is the one place the run's start is still in hand, and
                // folding it in here makes TextRun::byte_range and the glyph
                // offsets mean the same thing.
                for glyph in &mut glyphs[glyphs_start..] {
                    glyph.text_byte_offset += *run_start;
                }

                if let Some(letter_spacing) = layout.letter_spacing
                    && glyphs.len() > glyphs_start
                {
                    let mut last_byte_offset = glyphs[glyphs_start].text_byte_offset;
                    for index in glyphs_start + 1..glyphs.len() {
                        let current_glyph_byte_offset = glyphs[index].text_byte_offset;
                        if current_glyph_byte_offset != last_byte_offset {
                            let previous_glyph = &mut glyphs[index - 1];
                            previous_glyph.advance += letter_spacing;
                        }
                        last_byte_offset = current_glyph_byte_offset;
                    }

                    glyphs.last_mut().unwrap().advance += letter_spacing;
                }

                let run = TextRun {
                    byte_range: Range { start: *run_start, end: run_end },
                    glyph_range: Range { start: glyphs_start, end: glyphs.len() },
                };
                *run_start = run_end;

                Some(run)
            })
            .collect();

        Self { glyphs, text_runs }
    }
}

#[test]
fn test_div_count() {
    assert_eq!(9.0_f32.div_count(3.0), 3);
    assert_eq!(10.0_f32.div_count(3.0), 3);
    assert_eq!((-10.0_f32).div_count(3.0), 0);
    assert_eq!(3.0_f32.div_count(10.0), 0);
    assert_eq!(f32::NAN.div_count(16.0), 0);

    assert_eq!(i16::MIN.div_count(-1), 32768);

    type IntLen = euclid::Length<i16, euclid::UnknownUnit>;
    assert_eq!(IntLen::new(10).div_count(IntLen::new(3)), 3);
    assert_eq!(IntLen::new(-10).div_count(IntLen::new(3)), 0);

    type FloatLen = euclid::Length<f32, euclid::UnknownUnit>;
    assert_eq!(FloatLen::new(10.).div_count(FloatLen::new(3.)), 3);
}

#[test]
fn test_shape_boundaries_simple() {
    {
        let simple_text = "Hello World";
        let mut itemizer = ShapeBoundaries::new(simple_text);
        assert_eq!(itemizer.next(), Some(simple_text.len()));
        assert_eq!(itemizer.next(), None);
    }
}

#[test]
fn test_shape_boundaries_empty() {
    {
        let mut itemizer = ShapeBoundaries::new("");
        assert_eq!(itemizer.next(), None);
    }
}

#[test]
#[cfg_attr(
    not(feature = "unicode-script"),
    ignore = "Not supported without the unicode-script feature"
)]
fn test_shape_boundaries_script_change() {
    {
        let text = "abc🍌🐒defதோசை.";
        let mut itemizer = ShapeBoundaries::new(text).scan(0, |start, end| {
            let str = &text[*start..end];
            *start = end;
            Some(str)
        });
        assert_eq!(itemizer.next(), Some("abc🍌🐒def"));
        assert_eq!(itemizer.next(), Some("தோசை."));
        assert_eq!(itemizer.next(), None);
    }
}

#[cfg(test)]
impl TextShaper for &rustybuzz::Face<'_> {
    type LengthPrimitive = f32;
    type Length = f32;
    fn shape_text<GlyphStorage: std::iter::Extend<Glyph<f32>>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    ) {
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        let glyph_buffer = rustybuzz::shape(self, &[], buffer);

        let output_glyph_generator =
            glyph_buffer.glyph_infos().iter().zip(glyph_buffer.glyph_positions().iter()).map(
                |(info, position)| {
                    let mut out_glyph = Glyph::default();
                    out_glyph.glyph_id = core::num::NonZeroU16::new(info.glyph_id as u16);
                    out_glyph.offset_x = position.x_offset as _;
                    out_glyph.offset_y = position.y_offset as _;
                    out_glyph.advance = position.x_advance as _;
                    out_glyph.text_byte_offset = info.cluster as usize;
                    out_glyph
                },
            );

        // Cannot return impl Iterator, so extend argument instead
        glyphs.extend(output_glyph_generator);
    }

    fn glyph_for_char(&self, _ch: char) -> Option<Glyph<f32>> {
        todo!()
    }
}

#[cfg(test)]
impl FontMetrics<f32> for &rustybuzz::Face<'_> {
    fn ascent(&self) -> f32 {
        self.ascender() as _
    }

    fn descent(&self) -> f32 {
        self.descender() as _
    }

    fn x_height(&self) -> f32 {
        rustybuzz::ttf_parser::Face::x_height(self).unwrap_or_default() as _
    }

    fn cap_height(&self) -> f32 {
        rustybuzz::ttf_parser::Face::capital_height(self).unwrap_or_default() as _
    }
}

#[cfg(test)]
fn with_default_font<R>(mut callback: impl FnMut(&rustybuzz::Face<'_>) -> R) -> R {
    let mut collection = fontique::Collection::new(fontique::CollectionOptions {
        system_fonts: false,
        ..Default::default()
    });
    let font_path: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "..", "common", "sharedfontique", "Inter-VariableFont.ttf"]
            .iter()
            .collect();
    let registered_fonts =
        collection.register_fonts(std::fs::read(&font_path).unwrap().into(), None);
    let mut cache = fontique::SourceCache::default();
    let mut query = collection.query(&mut cache);
    query.set_families(std::iter::once(fontique::QueryFamily::from(registered_fonts[0].0)));
    let mut font = None;
    query.matches_with(|query_font| {
        font = Some(query_font.clone());
        fontique::QueryStatus::Stop
    });
    let font = font.unwrap();
    let face =
        rustybuzz::Face::from_slice(font.blob.data(), font.index).expect("unable to parse font");
    callback(&face)
}

#[test]
fn test_shaping() {
    use TextShaper;

    with_default_font(|face| {
        {
            let mut shaped_glyphs = Vec::new();
            // two glyph clusters: ā́b
            face.shape_text("a\u{0304}\u{0301}b", &mut shaped_glyphs);

            assert_eq!(shaped_glyphs.len(), 3);
            assert!(shaped_glyphs[0].glyph_id.is_some());
            assert_eq!(shaped_glyphs[0].text_byte_offset, 0);

            assert!(shaped_glyphs[1].glyph_id.is_some());
            assert_eq!(shaped_glyphs[1].text_byte_offset, 0);

            assert!(shaped_glyphs[2].glyph_id.is_some());
            assert_eq!(shaped_glyphs[2].text_byte_offset, 5);
        }

        {
            let mut shaped_glyphs = Vec::new();
            // two glyph clusters: ā́b
            face.shape_text("a b", &mut shaped_glyphs);

            assert_eq!(shaped_glyphs.len(), 3);
            assert!(shaped_glyphs[0].glyph_id.is_some());
            assert_eq!(shaped_glyphs[0].text_byte_offset, 0);

            assert_eq!(shaped_glyphs[1].text_byte_offset, 1);

            assert!(shaped_glyphs[2].glyph_id.is_some());
            assert_eq!(shaped_glyphs[2].text_byte_offset, 2);
        }
    });
}

/// The byte offset on a glyph is an index into the whole string, on every run.
///
/// A shaper sees one run's slice, so the offset it reports is relative to that slice.
/// Text that changes script part way through is more than one run,
/// and a line that opens with a bracketed number is exactly that shape.
/// Line breaking, selection and the byte offset a click maps to
/// all compare these offsets against indices into the whole string.
#[test]
#[cfg_attr(
    not(feature = "unicode-script"),
    ignore = "Not supported without the unicode-script feature"
)]
fn test_byte_offsets_are_absolute() {
    with_default_font(|face| {
        // `Common` script up to the space, `Latin` from `a` on.
        let text = "[01] abc";
        let layout = TextLayout { font: &face, letter_spacing: None, line_height: None };
        let buffer = ShapeBuffer::new(&layout, text);

        assert_eq!(buffer.text_runs.len(), 2, "expected a run boundary at the script change");
        let second_run = &buffer.text_runs[1];
        assert_eq!(second_run.byte_range.start, 5);

        // Every offset is an index into `text`, and the first glyph of the second
        // run points at the character that run starts with.
        for glyph in &buffer.glyphs {
            assert!(
                text.is_char_boundary(glyph.text_byte_offset),
                "{} is not an index into {text:?}",
                glyph.text_byte_offset
            );
        }
        assert_eq!(buffer.glyphs[second_run.glyph_range.start].text_byte_offset, 5);
        assert_eq!(buffer.glyphs.last().unwrap().text_byte_offset, text.len() - 1);
    });
}

#[test]
fn test_letter_spacing() {
    use TextShaper;

    with_default_font(|face| {
        // two glyph clusters: ā́b
        let text = "a\u{0304}\u{0301}b";
        let advances = {
            let mut shaped_glyphs = Vec::new();
            face.shape_text(text, &mut shaped_glyphs);

            assert_eq!(shaped_glyphs.len(), 3);

            shaped_glyphs.iter().map(|g| g.advance).collect::<Vec<_>>()
        };

        let layout = TextLayout { font: &face, letter_spacing: Some(20.), line_height: None };
        let buffer = ShapeBuffer::new(&layout, text);

        assert_eq!(buffer.glyphs.len(), advances.len());

        let mut expected_advances = advances;
        expected_advances[1] += layout.letter_spacing.unwrap();
        *expected_advances.last_mut().unwrap() += layout.letter_spacing.unwrap();

        assert_eq!(
            buffer.glyphs.iter().map(|glyph| glyph.advance).collect::<Vec<_>>(),
            expected_advances
        );
    });
}
