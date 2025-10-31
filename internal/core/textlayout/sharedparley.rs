// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use euclid::num::Zero;
pub use parley;

use alloc::vec::Vec;
use core::ops::Range;
use core::pin::Pin;
use std::boxed::Box;
use std::cell::RefCell;

use crate::{
    graphics::FontRequest,
    items::TextStrokeStyle,
    lengths::{
        LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalRect, LogicalSize, PhysicalPx,
        PointLengths, ScaleFactor, SizeLengths,
    },
    textlayout::{TextHorizontalAlignment, TextOverflow, TextVerticalAlignment, TextWrap},
    SharedString,
};

pub type PhysicalLength = euclid::Length<f32, PhysicalPx>;
pub type PhysicalRect = euclid::Rect<f32, PhysicalPx>;
type PhysicalSize = euclid::Size2D<f32, PhysicalPx>;
type PhysicalPoint = euclid::Point2D<f32, PhysicalPx>;

use i_slint_common::sharedfontique;

/// Trait used for drawing text and text input elements with parley, where parley does the
/// shaping and positioning, and the renderer is responsible for drawing just the glyphs.
pub trait GlyphRenderer: crate::item_rendering::ItemRenderer {
    /// A renderer-specific type for a brush used for fill and stroke of glyphs.
    type PlatformBrush: Clone;

    /// Returns the brush to be used for filling text.
    fn platform_text_fill_brush(
        &mut self,
        brush: crate::Brush,
        size: LogicalSize,
    ) -> Option<Self::PlatformBrush>;

    /// Returns a brush that's a solid fill of the specified color.
    fn platform_brush_for_color(&mut self, color: &crate::Color) -> Option<Self::PlatformBrush>;

    /// Returns the brush to be used for stroking text.
    fn platform_text_stroke_brush(
        &mut self,
        brush: crate::Brush,
        physical_stroke_width: f32,
        size: LogicalSize,
    ) -> Option<Self::PlatformBrush>;

    /// Draws the glyphs provided by glyphs_it with the specified font, font_size, and brush at the
    /// given y offset.
    fn draw_glyph_run(
        &mut self,
        font: &parley::FontData,
        font_size: PhysicalLength,
        brush: Self::PlatformBrush,
        y_offset: PhysicalLength,
        glyphs_it: &mut dyn Iterator<Item = parley::layout::Glyph>,
    );

    fn fill_rectange_with_color(&mut self, physical_rect: PhysicalRect, color: crate::Color) {
        if let Some(platform_brush) = self.platform_brush_for_color(&color) {
            self.fill_rectangle(physical_rect, platform_brush);
        }
    }

    /// Fills the given rectangle with the specified color. This is used for drawing selection
    /// rectangles as well as the text cursor.
    fn fill_rectangle(&mut self, physical_rect: PhysicalRect, brush: Self::PlatformBrush);
}

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
struct Brush {
    /// When set, this overrides the fill/stroke to use this color for just a fill, for selection.
    selection_fill_color: Option<crate::Color>,
    stroke: Option<TextStrokeStyle>,
    link_color: Option<crate::Color>,
}

struct LayoutOptions {
    max_width: Option<LogicalLength>,
    max_height: Option<LogicalLength>,
    horizontal_align: TextHorizontalAlignment,
    vertical_align: TextVerticalAlignment,
    stroke: Option<TextStrokeStyle>,
    font_request: Option<FontRequest>,
    text_wrap: TextWrap,
    text_overflow: TextOverflow,
    selection: Option<core::ops::Range<usize>>,
    selection_foreground_color: Option<crate::Color>,
    link_color: crate::Color,
}

impl LayoutOptions {
    fn new_from_textinput(
        text_input: Pin<&crate::items::TextInput>,
        font_request: Option<FontRequest>,
        max_width: Option<LogicalLength>,
        max_height: Option<LogicalLength>,
        selection: Option<core::ops::Range<usize>>,
    ) -> Self {
        let selection_foreground_color =
            selection.is_some().then(|| text_input.selection_foreground_color());

        Self {
            max_width,
            max_height,
            horizontal_align: text_input.horizontal_alignment(),
            vertical_align: text_input.vertical_alignment(),
            font_request,
            selection,
            selection_foreground_color,
            stroke: None,
            text_wrap: text_input.wrap(),
            text_overflow: TextOverflow::Clip,
            link_color: Default::default(),
        }
    }
}

enum Text<'a> {
    PlainText(&'a str),
    #[cfg(feature = "experimental-rich-text")]
    RichText(RichText<'a>),
}

fn layout(text: Text, scale_factor: ScaleFactor, mut options: LayoutOptions) -> Layout {
    // When a piece of text is first selected, it gets an empty range like `Some(1..1)`.
    // If the text starts with a multi-byte character then this selection will be within
    // that character and parley will panic. We just filter out empty selection ranges.
    options.selection = options.selection.filter(|selection| !selection.is_empty());

    let max_physical_width = options.max_width.map(|max_width| max_width * scale_factor);
    let max_physical_height = options.max_height.map(|max_height| max_height * scale_factor);
    let pixel_size = options
        .font_request
        .as_ref()
        .and_then(|font_request| font_request.pixel_size)
        .unwrap_or(DEFAULT_FONT_SIZE);

    let push_to_builder = |builder: &mut parley::RangedBuilder<_>| {
        if let Some(ref font_request) = options.font_request {
            let mut fallback_family_iter = sharedfontique::FALLBACK_FAMILIES
                .into_iter()
                .map(parley::style::FontFamily::Generic);

            let font_stack: &[parley::style::FontFamily] = if let Some(family) =
                &font_request.family
            {
                let mut iter =
                    core::iter::once(parley::style::FontFamily::Named(family.as_str().into()))
                        .chain(fallback_family_iter);
                &core::array::from_fn::<
                    _,
                    { sharedfontique::FALLBACK_FAMILIES.as_slice().len() + 1 },
                    _,
                >(|_| iter.next().unwrap())
            } else {
                &core::array::from_fn::<_, { sharedfontique::FALLBACK_FAMILIES.as_slice().len() }, _>(
                    |_| fallback_family_iter.next().unwrap(),
                )
            };

            builder.push_default(parley::style::FontStack::List(std::borrow::Cow::Borrowed(
                &font_stack,
            )));

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
        builder.push_default(parley::StyleProperty::OverflowWrap(match options.text_wrap {
            TextWrap::NoWrap => parley::style::OverflowWrap::Normal,
            TextWrap::WordWrap | TextWrap::CharWrap => parley::style::OverflowWrap::Anywhere,
        }));

        builder.push_default(parley::StyleProperty::Brush(Brush {
            selection_fill_color: None,
            stroke: options.stroke,
            link_color: None,
        }));
    };

    let (paragraphs, elision_info) = CONTEXTS.with_borrow_mut(move |contexts| {
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

        let mut paragraphs = Vec::with_capacity(1);
        let mut para_y = 0.0;

        let mut paragraph_from_text = |text: &str,
                                       range: std::ops::Range<usize>,
                                       formatting: Vec<FormattedSpan>,
                                       links: Vec<(
            std::ops::Range<usize>,
            std::string::String,
        )>| {
            let mut builder =
                contexts.layout.ranged_builder(&mut contexts.font, text, scale_factor.get(), true);
            push_to_builder(&mut builder);

            if let Some((selection, selection_color)) =
                options.selection.as_ref().zip(options.selection_foreground_color)
            {
                let sel_start = selection.start.max(range.start);
                let sel_end = selection.end.min(range.end);
                if sel_start < sel_end {
                    let local_selection = (sel_start - range.start)..(sel_end - range.start);
                    builder.push(
                        parley::StyleProperty::Brush(Brush {
                            selection_fill_color: Some(selection_color),
                            stroke: options.stroke,
                            link_color: None,
                        }),
                        local_selection,
                    );
                }
            }

            for span in formatting {
                match span.style {
                    Style::Emphasis => {
                        builder.push(
                            parley::StyleProperty::FontStyle(parley::style::FontStyle::Italic),
                            span.range,
                        );
                    }
                    Style::Strikethrough => {
                        builder.push(parley::StyleProperty::Strikethrough(true), span.range);
                    }
                    Style::Strong => {
                        builder.push(
                            parley::StyleProperty::FontWeight(parley::style::FontWeight::BOLD),
                            span.range,
                        );
                    }
                    Style::Code => {
                        builder.push(
                            parley::StyleProperty::FontStack(parley::style::FontStack::Single(
                                parley::style::FontFamily::Generic(
                                    parley::style::GenericFamily::Monospace,
                                ),
                            )),
                            span.range,
                        );
                    }
                    Style::Underline => {
                        builder.push(parley::StyleProperty::Underline(true), span.range);
                    }
                    Style::Link => {
                        builder.push(parley::StyleProperty::Underline(true), span.range.clone());
                        builder.push(
                            parley::StyleProperty::Brush(Brush {
                                selection_fill_color: None,
                                stroke: options.stroke,
                                link_color: Some(options.link_color),
                            }),
                            span.range,
                        );
                    }
                }
            }

            let mut layout = builder.build(text);

            layout.break_all_lines(
                max_physical_width
                    .filter(|_| options.text_wrap != TextWrap::NoWrap)
                    .map(|width| width.get()),
            );
            layout.align(
                max_physical_width.map(|width| width.get()),
                match options.horizontal_align {
                    TextHorizontalAlignment::Left => parley::Alignment::Left,
                    TextHorizontalAlignment::Center => parley::Alignment::Center,
                    TextHorizontalAlignment::Right => parley::Alignment::Right,
                },
                parley::AlignmentOptions::default(),
            );

            let y = PhysicalLength::new(para_y);
            para_y += layout.height();
            TextParagraph { range, y, layout, links }
        };

        match text {
            Text::PlainText(text) => {
                let paragraph_ranges = core::iter::from_fn({
                    let mut start = 0;
                    let mut char_it = text.char_indices().peekable();
                    let mut eot = false;
                    move || {
                        while let Some((idx, ch)) = char_it.next() {
                            if ch == '\n' {
                                let next_range = start..idx;
                                start = idx + ch.len_utf8();
                                return Some(next_range);
                            }
                        }

                        if eot {
                            return None;
                        }
                        eot = true;
                        return Some(start..text.len());
                    }
                });

                for range in paragraph_ranges {
                    paragraphs.push(paragraph_from_text(
                        &text[range.clone()],
                        range,
                        Default::default(),
                        Default::default(),
                    ));
                }
            }
            #[cfg(feature = "experimental-rich-text")]
            Text::RichText(rich_text) => {
                for paragraph in rich_text.paragraphs {
                    paragraphs.push(paragraph_from_text(
                        &paragraph.text,
                        0..0,
                        paragraph.formatting,
                        paragraph
                            .links
                            .into_iter()
                            .map(|(range, link)| (range, link.into_string()))
                            .collect(),
                    ));
                }
            }
        };

        (paragraphs, elision_info)
    });

    let max_width = paragraphs
        .iter()
        .map(|p| {
            // The max width is used for the elipsis computation when eliding text. We *want* to exclude whitespace
            // for that, but we can't at the glyph run level, so the glyph runs always *do* include whitespace glyphs,
            // and as such we must also accept the full width here including trailing whitespace, otherwise text with
            // trailing whitespace will assigned a smaller width for rendering and thus the elipsis will be placed.
            PhysicalLength::new(p.layout.full_width())
        })
        .fold(PhysicalLength::zero(), PhysicalLength::max);
    let height = paragraphs
        .last()
        .map_or(PhysicalLength::zero(), |p| p.y + PhysicalLength::new(p.layout.height()));

    let y_offset = match (max_physical_height, options.vertical_align) {
        (Some(max_height), TextVerticalAlignment::Center) => (max_height - height) / 2.0,
        (Some(max_height), TextVerticalAlignment::Bottom) => max_height - height,
        (None, _) | (Some(_), TextVerticalAlignment::Top) => PhysicalLength::new(0.0),
    };

    Layout { paragraphs, y_offset, elision_info, max_width, height, max_physical_height }
}

struct ElisionInfo {
    elipsis_glyph: parley::layout::Glyph,
    max_physical_width: PhysicalLength,
}

struct TextParagraph {
    range: Range<usize>,
    y: PhysicalLength,
    layout: parley::Layout<Brush>,
    #[cfg_attr(not(feature = "experimental-rich-text"), allow(unused))]
    links: std::vec::Vec<(Range<usize>, std::string::String)>,
}

impl TextParagraph {
    fn draw<R: GlyphRenderer>(
        &self,
        layout: &Layout,
        item_renderer: &mut R,
        default_fill_brush: &<R as GlyphRenderer>::PlatformBrush,
        default_stroke_brush: &Option<<R as GlyphRenderer>::PlatformBrush>,
        draw_glyphs: &mut dyn FnMut(
            &mut R,
            &parley::FontData,
            PhysicalLength,
            <R as GlyphRenderer>::PlatformBrush,
            PhysicalLength, // y offset for paragraph
            &mut dyn Iterator<Item = parley::layout::Glyph>,
        ),
    ) {
        let para_y = layout.y_offset + self.y;

        let mut lines = self
            .layout
            .lines()
            .take_while(|line| {
                let metrics = line.metrics();
                match layout.max_physical_height {
                    // If overflow: clip is set, we apply a hard pixel clip, but with overflow: elide,
                    // we want to place an elipsis on the last line and not draw any lines beyond the
                    // given max height.
                    Some(max_physical_height) if layout.elision_info.is_some() => {
                        max_physical_height.get() >= metrics.max_coord
                    }
                    _ => true,
                }
            })
            .peekable();

        while let Some(line) = lines.next() {
            let last_line = lines.peek().is_none();
            for item in line.items() {
                match item {
                    parley::PositionedLayoutItem::GlyphRun(glyph_run) => {
                        let run = glyph_run.run();

                        let brush = &glyph_run.style().brush;

                        let mut elided_glyphs_it;
                        let mut unelided_glyphs_it;
                        let glyphs_it: &mut dyn Iterator<Item = parley::layout::Glyph>;

                        if last_line {
                            elided_glyphs_it = layout.glyphs_with_elision(&glyph_run);
                            glyphs_it = &mut elided_glyphs_it;
                        } else {
                            unelided_glyphs_it = glyph_run.positioned_glyphs();
                            glyphs_it = &mut unelided_glyphs_it;
                        };

                        let (fill_brush, stroke_style) =
                            match (brush.selection_fill_color, brush.link_color) {
                                (Some(color), _) => {
                                    let Some(selection_brush) =
                                        item_renderer.platform_brush_for_color(&color)
                                    else {
                                        // Weird, a transparent selection color, but ok...
                                        continue;
                                    };
                                    (selection_brush.clone(), &None)
                                }
                                (None, Some(color)) => {
                                    let Some(link_brush) =
                                        item_renderer.platform_brush_for_color(&color)
                                    else {
                                        // Weird, a transparent selection color, but ok...
                                        continue;
                                    };
                                    (link_brush.clone(), &None)
                                }
                                (None, None) => (default_fill_brush.clone(), &brush.stroke),
                            };

                        match stroke_style {
                            Some(TextStrokeStyle::Outside) => {
                                let glyphs = glyphs_it.collect::<alloc::vec::Vec<_>>();

                                if let Some(stroke_brush) = default_stroke_brush.clone() {
                                    draw_glyphs(
                                        item_renderer,
                                        run.font(),
                                        PhysicalLength::new(run.font_size()),
                                        stroke_brush,
                                        para_y,
                                        &mut glyphs.iter().cloned(),
                                    );
                                }

                                draw_glyphs(
                                    item_renderer,
                                    run.font(),
                                    PhysicalLength::new(run.font_size()),
                                    fill_brush.clone(),
                                    para_y,
                                    &mut glyphs.into_iter(),
                                );
                            }
                            Some(TextStrokeStyle::Center) => {
                                let glyphs = glyphs_it.collect::<alloc::vec::Vec<_>>();

                                draw_glyphs(
                                    item_renderer,
                                    run.font(),
                                    PhysicalLength::new(run.font_size()),
                                    fill_brush.clone(),
                                    para_y,
                                    &mut glyphs.iter().cloned(),
                                );

                                if let Some(stroke_brush) = default_stroke_brush.clone() {
                                    draw_glyphs(
                                        item_renderer,
                                        run.font(),
                                        PhysicalLength::new(run.font_size()),
                                        stroke_brush,
                                        para_y,
                                        &mut glyphs.into_iter(),
                                    );
                                }
                            }
                            None => {
                                draw_glyphs(
                                    item_renderer,
                                    run.font(),
                                    PhysicalLength::new(run.font_size()),
                                    fill_brush.clone(),
                                    para_y,
                                    glyphs_it,
                                );
                            }
                        }

                        let metrics = run.metrics();

                        if glyph_run.style().underline.is_some() {
                            item_renderer.fill_rectangle(
                                PhysicalRect::new(
                                    PhysicalPoint::from_lengths(
                                        PhysicalLength::new(glyph_run.offset()),
                                        para_y
                                            + PhysicalLength::new(
                                                run.font_size() - metrics.underline_offset,
                                            ),
                                    ),
                                    PhysicalSize::new(glyph_run.advance(), metrics.underline_size),
                                ),
                                fill_brush.clone(),
                            );
                        }

                        if glyph_run.style().strikethrough.is_some() {
                            item_renderer.fill_rectangle(
                                PhysicalRect::new(
                                    PhysicalPoint::from_lengths(
                                        PhysicalLength::new(glyph_run.offset()),
                                        para_y
                                            + PhysicalLength::new(
                                                run.font_size() - metrics.strikethrough_offset,
                                            ),
                                    ),
                                    PhysicalSize::new(
                                        glyph_run.advance(),
                                        metrics.strikethrough_size,
                                    ),
                                ),
                                fill_brush,
                            );
                        }
                    }
                    parley::PositionedLayoutItem::InlineBox(_inline_box) => {}
                };
            }
        }
    }
}

struct Layout {
    paragraphs: Vec<TextParagraph>,
    y_offset: PhysicalLength,
    max_width: PhysicalLength,
    height: PhysicalLength,
    max_physical_height: Option<PhysicalLength>,
    elision_info: Option<ElisionInfo>,
}

impl Layout {
    fn paragraph_by_byte_offset(&self, byte_offset: usize) -> Option<&TextParagraph> {
        self.paragraphs.iter().find(|p| byte_offset >= p.range.start && byte_offset <= p.range.end)
    }

    fn paragraph_by_y(&self, y: PhysicalLength) -> Option<&TextParagraph> {
        // Adjust for vertical alignment
        let y = y - self.y_offset;

        if y < PhysicalLength::zero() {
            return self.paragraphs.first();
        }

        let idx = self.paragraphs.binary_search_by(|paragraph| {
            if y < paragraph.y {
                core::cmp::Ordering::Greater
            } else if y >= paragraph.y + PhysicalLength::new(paragraph.layout.height()) {
                core::cmp::Ordering::Less
            } else {
                core::cmp::Ordering::Equal
            }
        });

        match idx {
            Ok(i) => self.paragraphs.get(i),
            Err(_) => self.paragraphs.last(),
        }
    }

    fn selection_geometry(
        &self,
        selection_range: Range<usize>,
        mut callback: impl FnMut(PhysicalRect),
    ) {
        for paragraph in &self.paragraphs {
            let selection_start = selection_range.start.max(paragraph.range.start);
            let selection_end = selection_range.end.min(paragraph.range.end);

            if selection_start < selection_end {
                let local_start = selection_start - paragraph.range.start;
                let local_end = selection_end - paragraph.range.start;

                let selection = parley::layout::cursor::Selection::new(
                    parley::layout::cursor::Cursor::from_byte_index(
                        &paragraph.layout,
                        local_start,
                        Default::default(),
                    ),
                    parley::layout::cursor::Cursor::from_byte_index(
                        &paragraph.layout,
                        local_end,
                        Default::default(),
                    ),
                );

                selection.geometry_with(&paragraph.layout, |rect, _| {
                    callback(PhysicalRect::new(
                        PhysicalPoint::from_lengths(
                            PhysicalLength::new(rect.x0 as _),
                            PhysicalLength::new(rect.y0 as _) + self.y_offset + paragraph.y,
                        ),
                        PhysicalSize::new(rect.width() as _, rect.height() as _),
                    ));
                });
            }
        }
    }

    fn byte_offset_from_point(&self, pos: PhysicalPoint) -> usize {
        let Some(paragraph) = self.paragraph_by_y(pos.y_length()) else {
            return 0;
        };
        let cursor = parley::layout::cursor::Cursor::from_point(
            &paragraph.layout,
            pos.x,
            (pos.y_length() - self.y_offset - paragraph.y).get(),
        );
        paragraph.range.start + cursor.index()
    }

    fn cursor_rect_for_byte_offset(
        &self,
        byte_offset: usize,
        cursor_width: PhysicalLength,
    ) -> PhysicalRect {
        let Some(paragraph) = self.paragraph_by_byte_offset(byte_offset) else {
            return PhysicalRect::new(PhysicalPoint::default(), PhysicalSize::new(1.0, 1.0));
        };

        let local_offset = byte_offset - paragraph.range.start;
        let cursor = parley::layout::cursor::Cursor::from_byte_index(
            &paragraph.layout,
            local_offset,
            Default::default(),
        );
        let rect = cursor.geometry(&paragraph.layout, cursor_width.get());

        PhysicalRect::new(
            PhysicalPoint::from_lengths(
                PhysicalLength::new(rect.x0 as _),
                PhysicalLength::new(rect.y0 as _) + self.y_offset + paragraph.y,
            ),
            PhysicalSize::new(rect.width() as _, rect.height() as _),
        )
    }

    /// Returns an iterator over the run's glyphs but with an optional elision
    /// glyph replacing the last line's last glyph that's exceeding the max width - if applicable.
    /// Call this function only for the last line of the layout.
    fn glyphs_with_elision<'a>(
        &'a self,
        glyph_run: &'a parley::layout::GlyphRun<Brush>,
    ) -> impl Iterator<Item = parley::layout::Glyph> + Clone + 'a {
        let run_beyond_max_width = self.elision_info.as_ref().map_or(false, |info| {
            let run_end = PhysicalLength::new(glyph_run.offset() + glyph_run.advance());

            run_end > info.max_physical_width
        });

        let mut elipsis_emitted = false;
        glyph_run.positioned_glyphs().filter_map(move |mut glyph| {
            if !run_beyond_max_width {
                return Some(glyph);
            }
            let Some(elision_info) = &self.elision_info else {
                return Some(glyph);
            };

            if PhysicalLength::new(glyph.x + glyph.advance + elision_info.elipsis_glyph.advance)
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

    fn draw<R: GlyphRenderer>(
        &self,
        item_renderer: &mut R,
        default_fill_brush: <R as GlyphRenderer>::PlatformBrush,
        default_stroke_brush: Option<<R as GlyphRenderer>::PlatformBrush>,
        draw_glyphs: &mut dyn FnMut(
            &mut R,
            &parley::FontData,
            PhysicalLength,
            <R as GlyphRenderer>::PlatformBrush,
            PhysicalLength, // y offset for paragraph
            &mut dyn Iterator<Item = parley::layout::Glyph>,
        ),
    ) {
        for paragraph in &self.paragraphs {
            paragraph.draw(
                self,
                item_renderer,
                &default_fill_brush,
                &default_stroke_brush,
                draw_glyphs,
            );
        }
    }
}

#[cfg_attr(not(feature = "experimental-rich-text"), allow(unused))]
#[derive(Debug, PartialEq)]
enum Style {
    Emphasis,
    Strong,
    Strikethrough,
    Code,
    Link,
    Underline,
}

#[derive(Debug, PartialEq)]
struct FormattedSpan {
    range: Range<usize>,
    style: Style,
}

#[cfg(feature = "experimental-rich-text")]
#[derive(Debug)]
enum ListItemType {
    Ordered(u64),
    Unordered,
}

#[cfg(feature = "experimental-rich-text")]
#[derive(Debug, PartialEq)]
struct RichTextParagraph<'a> {
    text: std::string::String,
    formatting: Vec<FormattedSpan>,
    #[cfg(feature = "experimental-rich-text")]
    links: std::vec::Vec<(Range<usize>, pulldown_cmark::CowStr<'a>)>,
    #[cfg(not(feature = "experimental-rich-text"))]
    _phantom: std::marker::PhantomData<&'a ()>,
}

#[cfg(feature = "experimental-rich-text")]
#[derive(Debug, Default)]
struct RichText<'a> {
    paragraphs: Vec<RichTextParagraph<'a>>,
}

#[cfg(feature = "experimental-rich-text")]
impl<'a> RichText<'a> {
    fn begin_paragraph(&mut self, indentation: u32, list_item_type: Option<ListItemType>) {
        let mut text = std::string::String::with_capacity(indentation as usize * 4);
        for _ in 0..indentation {
            text.push_str("    ");
        }
        match list_item_type {
            Some(ListItemType::Unordered) => {
                if indentation % 3 == 0 {
                    text.push_str("• ")
                } else if indentation % 3 == 1 {
                    text.push_str("◦ ")
                } else {
                    text.push_str("▪ ")
                }
            }
            Some(ListItemType::Ordered(num)) => text.push_str(&std::format!("{}. ", num)),
            None => {}
        };
        self.paragraphs.push(RichTextParagraph {
            text,
            formatting: Default::default(),
            links: Default::default(),
        });
    }
}

#[cfg(feature = "experimental-rich-text")]
fn parse_markdown(string: &str) -> RichText<'_> {
    let parser =
        pulldown_cmark::Parser::new_ext(string, pulldown_cmark::Options::ENABLE_STRIKETHROUGH);

    let mut rich_text = RichText::default();
    let mut list_state_stack: std::vec::Vec<Option<u64>> = std::vec::Vec::new();
    let mut style_stack = std::vec::Vec::new();
    let mut current_url = None;

    for event in parser {
        let indentation = list_state_stack.len().saturating_sub(1) as _;

        match event {
            pulldown_cmark::Event::SoftBreak
            | pulldown_cmark::Event::HardBreak
            | pulldown_cmark::Event::Start(pulldown_cmark::Tag::Paragraph) => {
                rich_text.begin_paragraph(indentation, None);
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Item) => {
                rich_text.begin_paragraph(
                    indentation,
                    Some(match list_state_stack.last().copied() {
                        Some(Some(index)) => ListItemType::Ordered(index),
                        _ => ListItemType::Unordered,
                    }),
                );
                if let Some(state) = list_state_stack.last_mut() {
                    *state = state.map(|state| state + 1);
                }
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::List(index)) => {
                list_state_stack.push(index);
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::List(_)) => {
                list_state_stack.pop();
            }
            pulldown_cmark::Event::End(
                pulldown_cmark::TagEnd::Paragraph | pulldown_cmark::TagEnd::Item,
            ) => {}
            pulldown_cmark::Event::Start(tag) => {
                let style = match tag {
                    pulldown_cmark::Tag::Strong => Style::Strong,
                    pulldown_cmark::Tag::Emphasis => Style::Emphasis,
                    pulldown_cmark::Tag::Strikethrough => Style::Strikethrough,
                    pulldown_cmark::Tag::Link { dest_url, .. } => {
                        current_url = Some(dest_url);
                        Style::Link
                    }
                    pulldown_cmark::Tag::Paragraph
                    | pulldown_cmark::Tag::List(_)
                    | pulldown_cmark::Tag::Item => unreachable!(),
                    pulldown_cmark::Tag::Heading { .. }
                    | pulldown_cmark::Tag::Image { .. }
                    | pulldown_cmark::Tag::DefinitionList
                    | pulldown_cmark::Tag::DefinitionListTitle
                    | pulldown_cmark::Tag::DefinitionListDefinition
                    | pulldown_cmark::Tag::TableHead
                    | pulldown_cmark::Tag::TableRow
                    | pulldown_cmark::Tag::TableCell
                    | pulldown_cmark::Tag::HtmlBlock
                    | pulldown_cmark::Tag::Superscript
                    | pulldown_cmark::Tag::Subscript
                    | pulldown_cmark::Tag::Table(_)
                    | pulldown_cmark::Tag::MetadataBlock(_)
                    | pulldown_cmark::Tag::BlockQuote(_)
                    | pulldown_cmark::Tag::CodeBlock(_)
                    | pulldown_cmark::Tag::FootnoteDefinition(_) => {
                        unimplemented!("{:?}", tag)
                    }
                };

                style_stack.push((style, rich_text.paragraphs.last().unwrap().text.len()));
            }
            pulldown_cmark::Event::Text(text) => {
                rich_text.paragraphs.last_mut().unwrap().text.push_str(&text);
            }
            pulldown_cmark::Event::End(_) => {
                let (style, start) = style_stack.pop().unwrap();

                let paragraph = rich_text.paragraphs.last_mut().unwrap();
                let end = paragraph.text.len();

                if let Some(url) = current_url.take() {
                    paragraph.links.push((start..end, url));
                }

                paragraph.formatting.push(FormattedSpan { range: start..end, style });
            }
            pulldown_cmark::Event::Code(text) => {
                let paragraph = rich_text.paragraphs.last_mut().unwrap();
                let start = paragraph.text.len();
                paragraph.text.push_str(&text);
                paragraph
                    .formatting
                    .push(FormattedSpan { range: start..paragraph.text.len(), style: Style::Code });
            }
            pulldown_cmark::Event::InlineHtml(html) => match &*html {
                "<u>" => {
                    style_stack
                        .push((Style::Underline, rich_text.paragraphs.last().unwrap().text.len()));
                }
                "</u>" => {
                    let (style, start) = style_stack.pop().unwrap();

                    let paragraph = rich_text.paragraphs.last_mut().unwrap();
                    let end = paragraph.text.len();
                    paragraph.formatting.push(FormattedSpan { range: start..end, style });
                }
                other => unimplemented!("{}", other),
            },
            pulldown_cmark::Event::Rule
            | pulldown_cmark::Event::TaskListMarker(_)
            | pulldown_cmark::Event::FootnoteReference(_)
            | pulldown_cmark::Event::InlineMath(_)
            | pulldown_cmark::Event::DisplayMath(_)
            | pulldown_cmark::Event::Html(_) => unimplemented!("{:?}", event),
        }
    }

    rich_text
}

#[cfg(feature = "experimental-rich-text")]
#[test]
fn markdown_parsing() {
    assert_eq!(
        parse_markdown("hello *world*").paragraphs,
        [RichTextParagraph {
            text: "hello world".into(),
            formatting: std::vec![FormattedSpan { range: 6..11, style: Style::Emphasis }],
            links: std::vec![]
        }]
    );

    assert_eq!(
        parse_markdown(
            "
- line 1
- line 2
            "
        )
        .paragraphs,
        [
            RichTextParagraph {
                text: "• line 1".into(),
                formatting: std::vec![],
                links: std::vec![]
            },
            RichTextParagraph {
                text: "• line 2".into(),
                formatting: std::vec![],
                links: std::vec![]
            }
        ]
    );

    assert_eq!(
        parse_markdown(
            "
1. a
2. b
4. c
        "
        )
        .paragraphs,
        [
            RichTextParagraph { text: "1. a".into(), formatting: std::vec![], links: std::vec![] },
            RichTextParagraph { text: "2. b".into(), formatting: std::vec![], links: std::vec![] },
            RichTextParagraph { text: "3. c".into(), formatting: std::vec![], links: std::vec![] }
        ]
    );

    assert_eq!(
        parse_markdown(
            "
Normal _italic_ **strong** ~~strikethrough~~ `code`
new *line*
"
        )
        .paragraphs,
        [
            RichTextParagraph {
                text: "Normal italic strong strikethrough code".into(),
                formatting: std::vec![
                    FormattedSpan { range: 7..13, style: Style::Emphasis },
                    FormattedSpan { range: 14..20, style: Style::Strong },
                    FormattedSpan { range: 21..34, style: Style::Strikethrough },
                    FormattedSpan { range: 35..39, style: Style::Code }
                ],
                links: std::vec![]
            },
            RichTextParagraph {
                text: "new line".into(),
                formatting: std::vec![FormattedSpan { range: 4..8, style: Style::Emphasis },],
                links: std::vec![]
            }
        ]
    );

    assert_eq!(
        parse_markdown(
            "
- root
  - child
    - grandchild
      - great grandchild
"
        )
        .paragraphs,
        [
            RichTextParagraph {
                text: "• root".into(),
                formatting: std::vec![],
                links: std::vec![]
            },
            RichTextParagraph {
                text: "    ◦ child".into(),
                formatting: std::vec![],
                links: std::vec![]
            },
            RichTextParagraph {
                text: "        ▪ grandchild".into(),
                formatting: std::vec![],
                links: std::vec![]
            },
            RichTextParagraph {
                text: "            • great grandchild".into(),
                formatting: std::vec![],
                links: std::vec![]
            },
        ]
    );

    assert_eq!(
        parse_markdown("hello [*world*](https://example.com)").paragraphs,
        [RichTextParagraph {
            text: "hello world".into(),
            formatting: std::vec![
                FormattedSpan { range: 6..11, style: Style::Emphasis },
                FormattedSpan { range: 6..11, style: Style::Link }
            ],
            links: std::vec![(6..11, pulldown_cmark::CowStr::Borrowed("https://example.com"))]
        }]
    );

    assert_eq!(
        parse_markdown("<u>hello world</u>").paragraphs,
        [RichTextParagraph {
            text: "hello world".into(),
            formatting: std::vec![FormattedSpan { range: 0..11, style: Style::Underline },],
            links: std::vec![]
        }]
    );
}

pub fn draw_text(
    item_renderer: &mut impl GlyphRenderer,
    text: Pin<&dyn crate::item_rendering::RenderText>,
    font_request: Option<FontRequest>,
    size: LogicalSize,
) {
    let str = text.text();

    #[cfg(feature = "experimental-rich-text")]
    let layout_text = if text.is_markdown() {
        Text::RichText(parse_markdown(&str))
    } else {
        Text::PlainText(&str)
    };

    #[cfg(not(feature = "experimental-rich-text"))]
    let layout_text = Text::PlainText(&str);

    let max_width = size.width_length();
    let max_height = size.height_length();

    if max_width.get() <= 0. || max_height.get() <= 0. {
        return;
    }

    let Some(platform_fill_brush) = item_renderer.platform_text_fill_brush(text.color(), size)
    else {
        // Nothing to draw
        return;
    };

    let scale_factor = ScaleFactor::new(item_renderer.scale_factor());

    let (stroke_brush, stroke_width, stroke_style) = text.stroke();
    let stroke_width = if stroke_width.get() != 0.0 {
        (stroke_width * scale_factor).get()
    } else {
        // Hairline stroke
        1.0
    };
    let stroke_width = match stroke_style {
        TextStrokeStyle::Outside => stroke_width * 2.0,
        TextStrokeStyle::Center => stroke_width,
    };
    let platform_stroke_brush = if !stroke_brush.is_transparent() {
        item_renderer.platform_text_stroke_brush(stroke_brush, stroke_width, size)
    } else {
        None
    };

    let (horizontal_align, vertical_align) = text.alignment();

    let text_overflow = text.overflow();

    let layout = layout(
        layout_text,
        scale_factor,
        LayoutOptions {
            horizontal_align,
            vertical_align,
            max_height: Some(max_height),
            max_width: Some(max_width),
            stroke: platform_stroke_brush.is_some().then_some(stroke_style),
            font_request,
            text_wrap: text.wrap(),
            text_overflow,
            selection: None,
            selection_foreground_color: None,
            link_color: text.link_color(),
        },
    );

    let render = if text_overflow == TextOverflow::Clip {
        item_renderer.save_state();

        item_renderer.combine_clip(
            LogicalRect::new(LogicalPoint::default(), size),
            LogicalBorderRadius::zero(),
            LogicalLength::zero(),
        )
    } else {
        true
    };

    if render {
        layout.draw(
            item_renderer,
            platform_fill_brush,
            platform_stroke_brush,
            &mut |item_renderer, font, font_size, brush, y_offset, glyphs_it| {
                item_renderer.draw_glyph_run(font, font_size, brush, y_offset, glyphs_it);
            },
        );
    }

    if text_overflow == TextOverflow::Clip {
        item_renderer.restore_state();
    }
}

#[cfg(feature = "experimental-rich-text")]
pub fn link_under_cursor(
    scale_factor: ScaleFactor,
    text: Pin<&dyn crate::item_rendering::RenderText>,
    font_request: Option<FontRequest>,
    size: LogicalSize,
    cursor: PhysicalPoint,
) -> Option<std::string::String> {
    let str = text.text();

    let layout_text = Text::RichText(parse_markdown(&str));

    let (horizontal_align, vertical_align) = text.alignment();

    let layout = layout(
        layout_text,
        scale_factor,
        LayoutOptions {
            horizontal_align,
            vertical_align,
            max_height: Some(size.height_length()),
            max_width: Some(size.width_length()),
            stroke: None,
            font_request,
            text_wrap: text.wrap(),
            text_overflow: text.overflow(),
            selection: None,
            selection_foreground_color: None,
            link_color: crate::Color::from_rgb_u8(64, 64, 255),
        },
    );

    let Some(paragraph) = layout.paragraph_by_y(cursor.y_length()) else {
        return None;
    };

    let paragraph_y: f64 = paragraph.y.cast::<f64>().get();

    let (_, link) = paragraph.links.iter().find(|(range, _)| {
        let start =
            parley::Cursor::from_byte_index(&paragraph.layout, range.start, Default::default());
        let end = parley::Cursor::from_byte_index(&paragraph.layout, range.end, Default::default());
        let mut clicked = false;
        let link_range = parley::Selection::new(start, end);
        link_range.geometry_with(&paragraph.layout, |mut bounding_box, _line| {
            bounding_box.y0 += paragraph_y;
            bounding_box.y1 += paragraph_y;
            clicked = bounding_box.union(parley::BoundingBox::new(
                cursor.x.into(),
                cursor.y.into(),
                cursor.x.into(),
                cursor.y.into(),
            )) == bounding_box;
        });
        clicked
    })?;

    Some(link.clone())
}

pub fn draw_text_input(
    item_renderer: &mut impl GlyphRenderer,
    text_input: Pin<&crate::items::TextInput>,
    font_request: Option<FontRequest>,
    size: LogicalSize,
    password_character: Option<fn() -> char>,
) {
    let width = size.width_length();
    let height = size.height_length();
    if width.get() <= 0. || height.get() <= 0. {
        return;
    }

    let visual_representation = text_input.visual_representation(password_character);

    let Some(platform_fill_brush) =
        item_renderer.platform_text_fill_brush(visual_representation.text_color, size)
    else {
        return;
    };

    let (min_select, max_select) = if !visual_representation.preedit_range.is_empty() {
        (visual_representation.preedit_range.start, visual_representation.preedit_range.end)
    } else {
        (visual_representation.selection_range.start, visual_representation.selection_range.end)
    };

    let (cursor_visible, cursor_pos) =
        if let Some(cursor_pos) = visual_representation.cursor_position {
            (true, cursor_pos)
        } else {
            (false, 0)
        };

    let scale_factor = ScaleFactor::new(item_renderer.scale_factor());

    let text: SharedString = visual_representation.text.into();

    let layout = layout(
        Text::PlainText(&text),
        scale_factor,
        LayoutOptions::new_from_textinput(
            text_input,
            font_request,
            Some(width),
            Some(height),
            Some(min_select..max_select),
        ),
    );

    layout.selection_geometry(min_select..max_select, |selection_rect| {
        item_renderer
            .fill_rectange_with_color(selection_rect, text_input.selection_background_color());
    });

    layout.draw(
        item_renderer,
        platform_fill_brush,
        None,
        &mut |item_renderer, font, font_size, brush, y_offset, glyphs_it| {
            item_renderer.draw_glyph_run(font, font_size, brush, y_offset, glyphs_it);
        },
    );

    if cursor_visible {
        let cursor_rect = layout
            .cursor_rect_for_byte_offset(cursor_pos, text_input.text_cursor_width() * scale_factor);
        item_renderer.fill_rectange_with_color(cursor_rect, visual_representation.cursor_color);
    }
}

pub fn text_size(
    font_request: FontRequest,
    text: &str,
    max_width: Option<LogicalLength>,
    scale_factor: ScaleFactor,
    text_wrap: TextWrap,
) -> LogicalSize {
    let layout = layout(
        Text::PlainText(text),
        scale_factor,
        LayoutOptions {
            max_width,
            text_wrap,
            font_request: Some(font_request),
            max_height: None,
            horizontal_align: TextHorizontalAlignment::Left,
            vertical_align: TextVerticalAlignment::Top,
            stroke: None,
            text_overflow: TextOverflow::Clip,
            selection: None,
            selection_foreground_color: None,
            link_color: Default::default(),
        },
    );
    PhysicalSize::from_lengths(layout.max_width, layout.height) / scale_factor
}

pub fn font_metrics(font_request: FontRequest) -> crate::items::FontMetrics {
    let logical_pixel_size = font_request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE).get();

    let Some(font) = font_request.query_fontique() else {
        return crate::items::FontMetrics::default();
    };
    let face = sharedfontique::ttf_parser::Face::parse(font.blob.data(), font.index).unwrap();

    let metrics = sharedfontique::DesignFontMetrics::new_from_face(&face);

    crate::items::FontMetrics {
        ascent: metrics.ascent * logical_pixel_size / metrics.units_per_em,
        descent: metrics.descent * logical_pixel_size / metrics.units_per_em,
        x_height: metrics.x_height * logical_pixel_size / metrics.units_per_em,
        cap_height: metrics.cap_height * logical_pixel_size / metrics.units_per_em,
    }
}

pub fn text_input_byte_offset_for_position(
    text_input: Pin<&crate::items::TextInput>,
    pos: LogicalPoint,
    font_request: FontRequest,
    scale_factor: ScaleFactor,
) -> usize {
    let pos: PhysicalPoint = pos * scale_factor;
    let text = text_input.text();

    let width = text_input.width();
    let height = text_input.height();
    if width.get() <= 0. || height.get() <= 0. || pos.y < 0. {
        return 0;
    }

    let layout = layout(
        Text::PlainText(&text),
        scale_factor,
        LayoutOptions::new_from_textinput(
            text_input,
            Some(font_request),
            Some(width),
            Some(height),
            None,
        ),
    );
    let byte_offset = layout.byte_offset_from_point(pos);
    let visual_representation = text_input.visual_representation(None);
    visual_representation.map_byte_offset_from_byte_offset_in_visual_text(byte_offset)
}

pub fn text_input_cursor_rect_for_byte_offset(
    text_input: Pin<&crate::items::TextInput>,
    byte_offset: usize,
    font_request: FontRequest,
    scale_factor: ScaleFactor,
) -> LogicalRect {
    let text = text_input.text();

    let font_size = font_request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE);

    let width = text_input.width();
    let height = text_input.height();
    if width.get() <= 0. || height.get() <= 0. {
        return LogicalRect::new(
            LogicalPoint::default(),
            LogicalSize::from_lengths(LogicalLength::new(1.0), font_size),
        );
    }

    let layout = layout(
        Text::PlainText(&text),
        scale_factor,
        LayoutOptions::new_from_textinput(
            text_input,
            Some(font_request),
            Some(width),
            Some(height),
            None,
        ),
    );
    let cursor_rect = layout
        .cursor_rect_for_byte_offset(byte_offset, text_input.text_cursor_width() * scale_factor);
    cursor_rect / scale_factor
}
