// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub use parley;

use alloc::vec::Vec;
use core::ops::Range;
use core::pin::Pin;
use euclid::num::Zero;
use std::boxed::Box;
use std::cell::RefCell;

use crate::{
    Color, SharedString,
    graphics::FontRequest,
    item_rendering::PlainOrStyledText,
    items::TextStrokeStyle,
    lengths::{
        LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalRect, LogicalSize, PhysicalPx,
        PointLengths, ScaleFactor, SizeLengths,
    },
    renderer::RendererSealed,
    textlayout::{TextHorizontalAlignment, TextOverflow, TextVerticalAlignment, TextWrap},
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
    fn platform_brush_for_color(&mut self, color: &Color) -> Option<Self::PlatformBrush>;

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

    fn fill_rectange_with_color(&mut self, physical_rect: PhysicalRect, color: Color) {
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
    /// When set, this overrides the fill/stroke to use this color.
    override_fill_color: Option<Color>,
    stroke: Option<TextStrokeStyle>,
    link_color: Option<Color>,
}

struct LayoutOptions {
    max_width: Option<LogicalLength>,
    max_height: Option<LogicalLength>,
    horizontal_align: TextHorizontalAlignment,
    vertical_align: TextVerticalAlignment,
    text_overflow: TextOverflow,
}

impl LayoutOptions {
    fn new_from_textinput(
        text_input: Pin<&crate::items::TextInput>,
        max_width: Option<LogicalLength>,
        max_height: Option<LogicalLength>,
    ) -> Self {
        Self {
            max_width,
            max_height,
            horizontal_align: text_input.horizontal_alignment(),
            vertical_align: text_input.vertical_alignment(),
            text_overflow: TextOverflow::Clip,
        }
    }
}

struct LayoutWithoutLineBreaksBuilder {
    font_request: Option<FontRequest>,
    text_wrap: TextWrap,
    stroke: Option<TextStrokeStyle>,
    scale_factor: ScaleFactor,
    pixel_size: LogicalLength,
}

impl LayoutWithoutLineBreaksBuilder {
    fn new(
        font_request: Option<FontRequest>,
        text_wrap: TextWrap,
        stroke: Option<TextStrokeStyle>,
        scale_factor: ScaleFactor,
    ) -> Self {
        let pixel_size = font_request
            .as_ref()
            .and_then(|font_request| font_request.pixel_size)
            .unwrap_or(DEFAULT_FONT_SIZE);

        Self { font_request, text_wrap, stroke, scale_factor, pixel_size }
    }

    fn ranged_builder<'a>(
        &self,
        contexts: &'a mut Contexts,
        text: &'a str,
    ) -> parley::RangedBuilder<'a, Brush> {
        let mut builder =
            contexts.layout.ranged_builder(&mut contexts.font, text, self.scale_factor.get(), true);

        if let Some(ref font_request) = self.font_request {
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
        builder.push_default(parley::StyleProperty::FontSize(self.pixel_size.get()));
        builder.push_default(parley::StyleProperty::WordBreak(match self.text_wrap {
            TextWrap::NoWrap => parley::style::WordBreakStrength::KeepAll,
            TextWrap::WordWrap => parley::style::WordBreakStrength::Normal,
            TextWrap::CharWrap => parley::style::WordBreakStrength::BreakAll,
        }));
        builder.push_default(parley::StyleProperty::OverflowWrap(match self.text_wrap {
            TextWrap::NoWrap => parley::style::OverflowWrap::Normal,
            TextWrap::WordWrap | TextWrap::CharWrap => parley::style::OverflowWrap::Anywhere,
        }));

        builder.push_default(parley::StyleProperty::Brush(Brush {
            override_fill_color: None,
            stroke: self.stroke,
            link_color: None,
        }));

        builder
    }

    fn build(
        &self,
        text: &str,
        selection: Option<(Range<usize>, Color)>,
        formatting: impl IntoIterator<Item = crate::styled_text::FormattedSpan>,
        link_color: Option<Color>,
    ) -> parley::Layout<Brush> {
        use crate::styled_text::Style;

        CONTEXTS.with_borrow_mut(|contexts| {
            let mut builder = self.ranged_builder(contexts.as_mut(), text);

            if let Some((selection_range, selection_color)) = selection {
                {
                    builder.push(
                        parley::StyleProperty::Brush(Brush {
                            override_fill_color: Some(selection_color),
                            stroke: self.stroke,
                            link_color: None,
                        }),
                        selection_range,
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
                                override_fill_color: None,
                                stroke: self.stroke,
                                link_color: link_color.clone(),
                            }),
                            span.range,
                        );
                    }
                    Style::Color(color) => {
                        builder.push(
                            parley::StyleProperty::Brush(Brush {
                                override_fill_color: Some(color),
                                stroke: self.stroke,
                                link_color: None,
                            }),
                            span.range,
                        );
                    }
                }
            }

            builder.build(text)
        })
    }
}

fn create_text_paragraphs(
    layout_builder: &LayoutWithoutLineBreaksBuilder,
    text: PlainOrStyledText,
    selection: Option<(Range<usize>, Color)>,
    link_color: Color,
) -> Vec<TextParagraph> {
    let paragraph_from_text =
        |text: &str,
         range: std::ops::Range<usize>,
         formatting: Vec<crate::styled_text::FormattedSpan>,
         links: Vec<(std::ops::Range<usize>, std::string::String)>| {
            let selection = selection.clone().and_then(|(selection, selection_color)| {
                let sel_start = selection.start.max(range.start);
                let sel_end = selection.end.min(range.end);

                if sel_start < sel_end {
                    let local_selection = (sel_start - range.start)..(sel_end - range.start);
                    Some((local_selection, selection_color))
                } else {
                    None
                }
            });

            let layout =
                layout_builder.build(text, selection, formatting.into_iter(), Some(link_color));

            TextParagraph { range, y: PhysicalLength::default(), layout, links }
        };

    let mut paragraphs = Vec::with_capacity(1);

    match text {
        PlainOrStyledText::Plain(ref text) => {
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
        #[cfg_attr(not(feature = "experimental-rich-text"), allow(unused))]
        PlainOrStyledText::Styled(rich_text) =>
        {
            #[cfg(feature = "experimental-rich-text")]
            for paragraph in rich_text.paragraphs {
                paragraphs.push(paragraph_from_text(
                    &paragraph.text,
                    0..0,
                    paragraph.formatting,
                    paragraph.links,
                ));
            }
        }
    };

    paragraphs
}

fn layout(
    layout_builder: &LayoutWithoutLineBreaksBuilder,
    mut paragraphs: Vec<TextParagraph>,
    scale_factor: ScaleFactor,
    options: LayoutOptions,
) -> Layout {
    let max_physical_width = options.max_width.map(|max_width| max_width * scale_factor);
    let max_physical_height = options.max_height.map(|max_height| max_height * scale_factor);

    // Returned None if failed to get the elipsis glyph for some rare reason.
    let get_elipsis_glyph = || {
        let mut layout = layout_builder.build("…", None, None, None);
        layout.break_all_lines(None);
        let line = layout.lines().next()?;
        let item = line.items().next()?;
        let run = match item {
            parley::layout::PositionedLayoutItem::GlyphRun(run) => Some(run),
            _ => return None,
        }?;
        let glyph = run.positioned_glyphs().next()?;
        Some((glyph, run.run().font().clone()))
    };

    let elision_info = if let (TextOverflow::Elide, Some(max_physical_width)) =
        (options.text_overflow, max_physical_width)
    {
        get_elipsis_glyph().map(|(elipsis_glyph, font_for_elipsis_glyph)| ElisionInfo {
            elipsis_glyph,
            font_for_elipsis_glyph,
            max_physical_width,
        })
    } else {
        None
    };

    let mut para_y = 0.0;
    for para in paragraphs.iter_mut() {
        para.layout.break_all_lines(
            max_physical_width
                .filter(|_| layout_builder.text_wrap != TextWrap::NoWrap)
                .map(|width| width.get()),
        );
        para.layout.align(
            max_physical_width.map(|width| width.get()),
            match options.horizontal_align {
                TextHorizontalAlignment::Left => parley::Alignment::Left,
                TextHorizontalAlignment::Center => parley::Alignment::Center,
                TextHorizontalAlignment::Right => parley::Alignment::Right,
            },
            parley::AlignmentOptions::default(),
        );

        para.y = PhysicalLength::new(para_y);
        para_y += para.layout.height();
    }

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
    font_for_elipsis_glyph: parley::FontData,
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
                        max_physical_height.get().ceil() >= metrics.max_coord
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
                        let elipsis = if last_line {
                            let (truncated_glyphs, elipsis) =
                                layout.glyphs_with_elision(&glyph_run);

                            Self::draw_glyph_run(
                                &glyph_run,
                                item_renderer,
                                default_fill_brush,
                                default_stroke_brush,
                                para_y,
                                &mut truncated_glyphs.into_iter(),
                                draw_glyphs,
                            );
                            elipsis
                        } else {
                            Self::draw_glyph_run(
                                &glyph_run,
                                item_renderer,
                                default_fill_brush,
                                default_stroke_brush,
                                para_y,
                                &mut glyph_run.positioned_glyphs(),
                                draw_glyphs,
                            );
                            None
                        };

                        if let Some((elipsis_glyph, elipsis_font, font_size)) = elipsis {
                            draw_glyphs(
                                item_renderer,
                                &elipsis_font,
                                font_size,
                                default_fill_brush.clone(),
                                para_y,
                                &mut core::iter::once(elipsis_glyph),
                            );
                        }
                    }
                    parley::PositionedLayoutItem::InlineBox(_inline_box) => {}
                };
            }
        }
    }

    fn draw_glyph_run<R: GlyphRenderer>(
        glyph_run: &parley::layout::GlyphRun<Brush>,
        item_renderer: &mut R,
        default_fill_brush: &<R as GlyphRenderer>::PlatformBrush,
        default_stroke_brush: &Option<<R as GlyphRenderer>::PlatformBrush>,
        para_y: PhysicalLength,
        glyphs_it: &mut dyn Iterator<Item = parley::layout::Glyph>,
        draw_glyphs: &mut dyn FnMut(
            &mut R,
            &parley::FontData,
            PhysicalLength,
            <R as GlyphRenderer>::PlatformBrush,
            PhysicalLength,
            &mut dyn Iterator<Item = parley::layout::Glyph>,
        ),
    ) {
        let run = glyph_run.run();
        let brush = &glyph_run.style().brush;

        let (fill_brush, stroke_style) = match (brush.override_fill_color, brush.link_color) {
            (Some(color), _) => {
                let Some(selection_brush) = item_renderer.platform_brush_for_color(&color) else {
                    return;
                };
                (selection_brush.clone(), &None)
            }
            (None, Some(color)) => {
                let Some(link_brush) = item_renderer.platform_brush_for_color(&color) else {
                    return;
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
                        para_y + PhysicalLength::new(run.font_size() - metrics.underline_offset),
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
                            + PhysicalLength::new(run.font_size() - metrics.strikethrough_offset),
                    ),
                    PhysicalSize::new(glyph_run.advance(), metrics.strikethrough_size),
                ),
                fill_brush,
            );
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

                let selection = parley::editing::Selection::new(
                    parley::editing::Cursor::from_byte_index(
                        &paragraph.layout,
                        local_start,
                        Default::default(),
                    ),
                    parley::editing::Cursor::from_byte_index(
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
        let cursor = parley::editing::Cursor::from_point(
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
        let cursor = parley::editing::Cursor::from_byte_index(
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

    /// Returns an iterator over the run's glyphs, truncated if necessary to fit within the max width,
    /// plus an optional elipsis glyph with its font and size to be drawn separately.
    /// Call this function only for the last line of the layout.
    fn glyphs_with_elision<'a>(
        &'a self,
        glyph_run: &'a parley::layout::GlyphRun<Brush>,
    ) -> (
        impl Iterator<Item = parley::layout::Glyph> + Clone + 'a,
        Option<(parley::layout::Glyph, parley::FontData, PhysicalLength)>,
    ) {
        let elipsis_advance =
            self.elision_info.as_ref().map(|info| info.elipsis_glyph.advance).unwrap_or(0.0);
        let max_width = self
            .elision_info
            .as_ref()
            .map(|info| info.max_physical_width)
            .unwrap_or(PhysicalLength::new(f32::MAX));

        let run_start = PhysicalLength::new(glyph_run.offset());
        let run_end = PhysicalLength::new(glyph_run.offset() + glyph_run.advance());

        // Run starts after where the elipsis would go - skip entirely
        let run_beyond_elision = run_start > max_width;
        // Run extends beyond max width and needs truncation + elipsis
        let needs_elision = !run_beyond_elision && run_end.get().floor() > max_width.get().ceil();

        let truncated_glyphs = glyph_run.positioned_glyphs().take_while(move |glyph| {
            !run_beyond_elision
                && (!needs_elision
                    || PhysicalLength::new(glyph.x + glyph.advance + elipsis_advance) <= max_width)
        });

        let elipsis = if needs_elision {
            self.elision_info.as_ref().map(|info| {
                let elipsis_x = glyph_run
                    .positioned_glyphs()
                    .find(|glyph| {
                        PhysicalLength::new(glyph.x + glyph.advance + info.elipsis_glyph.advance)
                            > info.max_physical_width
                    })
                    .map(|g| g.x)
                    .unwrap_or(0.0);

                let mut elipsis_glyph = info.elipsis_glyph.clone();
                elipsis_glyph.x = elipsis_x;

                let font_size = PhysicalLength::new(glyph_run.run().font_size());
                (elipsis_glyph, info.font_for_elipsis_glyph.clone(), font_size)
            })
        } else {
            None
        };

        (truncated_glyphs, elipsis)
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

pub fn draw_text(
    item_renderer: &mut impl GlyphRenderer,
    text: Pin<&dyn crate::item_rendering::RenderText>,
    item_rc: Option<&crate::item_tree::ItemRc>,
    size: LogicalSize,
) {
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
    let platform_stroke_brush = if !stroke_brush.is_transparent() {
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
        item_renderer.platform_text_stroke_brush(stroke_brush, stroke_width, size)
    } else {
        None
    };

    let layout_builder = LayoutWithoutLineBreaksBuilder::new(
        item_rc.map(|item_rc| text.font_request(item_rc)),
        text.wrap(),
        platform_stroke_brush.is_some().then_some(stroke_style),
        scale_factor,
    );

    let paragraphs_without_linebreaks =
        create_text_paragraphs(&layout_builder, text.text(), None, text.link_color());

    let (horizontal_align, vertical_align) = text.alignment();
    let text_overflow = text.overflow();

    let layout = layout(
        &layout_builder,
        paragraphs_without_linebreaks,
        scale_factor,
        LayoutOptions {
            horizontal_align,
            vertical_align,
            max_height: Some(max_height),
            max_width: Some(max_width),
            text_overflow: text.overflow(),
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
    item_rc: &crate::item_tree::ItemRc,
    size: LogicalSize,
    cursor: PhysicalPoint,
) -> Option<std::string::String> {
    let layout_builder = LayoutWithoutLineBreaksBuilder::new(
        Some(text.font_request(item_rc)),
        text.wrap(),
        None,
        scale_factor,
    );

    let layout_text = text.text();

    let paragraphs_without_linebreaks =
        create_text_paragraphs(&layout_builder, layout_text, None, text.link_color());

    let (horizontal_align, vertical_align) = text.alignment();

    let layout = layout(
        &layout_builder,
        paragraphs_without_linebreaks,
        scale_factor,
        LayoutOptions {
            horizontal_align,
            vertical_align,
            max_height: Some(size.height_length()),
            max_width: Some(size.width_length()),
            text_overflow: text.overflow(),
        },
    );

    let Some(paragraph) = layout.paragraph_by_y(cursor.y_length()) else {
        return None;
    };

    let paragraph_y: f64 = paragraph.y.cast::<f64>().get();

    let (_, link) = paragraph.links.iter().find(|(range, _)| {
        let start = parley::editing::Cursor::from_byte_index(
            &paragraph.layout,
            range.start,
            Default::default(),
        );
        let end = parley::editing::Cursor::from_byte_index(
            &paragraph.layout,
            range.end,
            Default::default(),
        );
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
    item_rc: &crate::item_tree::ItemRc,
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

    let selection_range = if !visual_representation.preedit_range.is_empty() {
        visual_representation.preedit_range.start..visual_representation.preedit_range.end
    } else {
        visual_representation.selection_range.start..visual_representation.selection_range.end
    };

    let scale_factor = ScaleFactor::new(item_renderer.scale_factor());

    let layout_builder = LayoutWithoutLineBreaksBuilder::new(
        Some(text_input.font_request(item_rc)),
        text_input.wrap(),
        None,
        scale_factor,
    );

    let text: SharedString = visual_representation.text.into();

    // When a piece of text is first selected, it gets an empty range like `Some(1..1)`.
    // If the text starts with a multi-byte character then this selection will be within
    // that character and parley will panic. We just filter out empty selection ranges.
    let selection_and_color = if !selection_range.is_empty() {
        Some((selection_range.clone(), text_input.selection_foreground_color()))
    } else {
        None
    };

    let paragraphs_without_linebreaks = create_text_paragraphs(
        &layout_builder,
        PlainOrStyledText::Plain(text),
        selection_and_color,
        Color::default(),
    );

    let layout = layout(
        &layout_builder,
        paragraphs_without_linebreaks,
        scale_factor,
        LayoutOptions::new_from_textinput(text_input, Some(width), Some(height)),
    );

    layout.selection_geometry(selection_range, |selection_rect| {
        item_renderer
            .fill_rectange_with_color(selection_rect, text_input.selection_background_color());
    });

    item_renderer.save_state();

    let render = item_renderer.combine_clip(
        LogicalRect::new(LogicalPoint::default(), size),
        LogicalBorderRadius::zero(),
        LogicalLength::zero(),
    );

    if render {
        layout.draw(
            item_renderer,
            platform_fill_brush,
            None,
            &mut |item_renderer, font, font_size, brush, y_offset, glyphs_it| {
                item_renderer.draw_glyph_run(font, font_size, brush, y_offset, glyphs_it);
            },
        );

        if let Some(cursor_pos) = visual_representation.cursor_position {
            let cursor_rect = layout.cursor_rect_for_byte_offset(
                cursor_pos,
                text_input.text_cursor_width() * scale_factor,
            );
            item_renderer.fill_rectange_with_color(cursor_rect, visual_representation.cursor_color);
        }
    }

    item_renderer.restore_state();
}

pub fn text_size(
    renderer: &dyn RendererSealed,
    text_item: Pin<&dyn crate::item_rendering::RenderString>,
    item_rc: &crate::item_tree::ItemRc,
    max_width: Option<LogicalLength>,
    text_wrap: TextWrap,
) -> LogicalSize {
    let Some(scale_factor) = renderer.scale_factor() else {
        return LogicalSize::default();
    };

    let layout_builder = LayoutWithoutLineBreaksBuilder::new(
        Some(text_item.font_request(item_rc)),
        text_wrap,
        None,
        scale_factor,
    );

    let text = text_item.text();

    let paragraphs_without_linebreaks =
        create_text_paragraphs(&layout_builder, text, None, Color::default());

    let layout = layout(
        &layout_builder,
        paragraphs_without_linebreaks,
        scale_factor,
        LayoutOptions {
            max_width,
            max_height: None,
            horizontal_align: TextHorizontalAlignment::Left,
            vertical_align: TextVerticalAlignment::Top,
            text_overflow: TextOverflow::Clip,
        },
    );
    PhysicalSize::from_lengths(layout.max_width, layout.height) / scale_factor
}

pub fn char_size(
    text_item: Pin<&dyn crate::item_rendering::HasFont>,
    item_rc: &crate::item_tree::ItemRc,
    ch: char,
) -> Option<LogicalSize> {
    let font_request = text_item.font_request(item_rc);
    let font = font_request.query_fontique()?;

    let char_map = font.charmap()?;

    let face = skrifa::FontRef::from_index(font.blob.data(), font.index).unwrap();

    let glyph_index = char_map.map(ch)?;

    let pixel_size = font_request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE);

    let glyph_metrics = skrifa::metrics::GlyphMetrics::new(
        &face,
        skrifa::instance::Size::new(pixel_size.get()),
        skrifa::instance::LocationRef::new(&[]),
    );

    let advance_width = LogicalLength::new(glyph_metrics.advance_width(glyph_index.into())?);

    let font_metrics = skrifa::metrics::Metrics::new(
        &face,
        skrifa::instance::Size::new(pixel_size.get()),
        skrifa::instance::LocationRef::new(&[]),
    );

    Some(LogicalSize::from_lengths(
        advance_width,
        LogicalLength::new(font_metrics.ascent - font_metrics.descent),
    ))
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
    renderer: &dyn RendererSealed,
    text_input: Pin<&crate::items::TextInput>,
    item_rc: &crate::item_tree::ItemRc,
    pos: LogicalPoint,
) -> usize {
    let Some(scale_factor) = renderer.scale_factor() else {
        return 0;
    };
    let pos: PhysicalPoint = pos * scale_factor;

    let width = text_input.width();
    let height = text_input.height();
    if width.get() <= 0. || height.get() <= 0. || pos.y < 0. {
        return 0;
    }

    let layout_builder = LayoutWithoutLineBreaksBuilder::new(
        Some(text_input.font_request(item_rc)),
        text_input.wrap(),
        None,
        scale_factor,
    );

    let text = text_input.text();
    let paragraphs_without_linebreaks = create_text_paragraphs(
        &layout_builder,
        PlainOrStyledText::Plain(text),
        None,
        Color::default(),
    );

    let layout = layout(
        &layout_builder,
        paragraphs_without_linebreaks,
        scale_factor,
        LayoutOptions::new_from_textinput(text_input, Some(width), Some(height)),
    );
    let byte_offset = layout.byte_offset_from_point(pos);
    let visual_representation = text_input.visual_representation(None);
    visual_representation.map_byte_offset_from_byte_offset_in_visual_text(byte_offset)
}

pub fn text_input_cursor_rect_for_byte_offset(
    renderer: &dyn RendererSealed,
    text_input: Pin<&crate::items::TextInput>,
    item_rc: &crate::item_tree::ItemRc,
    byte_offset: usize,
) -> LogicalRect {
    let Some(scale_factor) = renderer.scale_factor() else {
        return LogicalRect::default();
    };

    let layout_builder = LayoutWithoutLineBreaksBuilder::new(
        Some(text_input.font_request(item_rc)),
        text_input.wrap(),
        None,
        scale_factor,
    );

    let width = text_input.width();
    let height = text_input.height();
    if width.get() <= 0. || height.get() <= 0. {
        return LogicalRect::new(
            LogicalPoint::default(),
            LogicalSize::from_lengths(LogicalLength::new(1.0), layout_builder.pixel_size),
        );
    }

    let text = text_input.text();
    let paragraphs_without_linebreaks = create_text_paragraphs(
        &layout_builder,
        PlainOrStyledText::Plain(text),
        None,
        Color::default(),
    );

    let layout = layout(
        &layout_builder,
        paragraphs_without_linebreaks,
        scale_factor,
        LayoutOptions::new_from_textinput(text_input, Some(width), Some(height)),
    );
    let cursor_rect = layout
        .cursor_rect_for_byte_offset(byte_offset, text_input.text_cursor_width() * scale_factor);
    cursor_rect / scale_factor
}
