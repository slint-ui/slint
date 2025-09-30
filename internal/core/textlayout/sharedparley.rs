// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub use parley;

use core::pin::Pin;
use std::boxed::Box;
use std::cell::RefCell;

use crate::{
    graphics::FontRequest,
    items::TextStrokeStyle,
    lengths::{LogicalLength, LogicalSize, PhysicalPx, ScaleFactor, SizeLengths},
    textlayout::{TextHorizontalAlignment, TextOverflow, TextVerticalAlignment, TextWrap},
    Coord, SharedString,
};
use i_slint_common::sharedfontique;

/// Trait used for drawing text and text input elements with parley, where parley does the
/// shaping and positioning, and the renderer is responsible for drawing just the glyphs.
pub trait GlyphRenderer: crate::item_rendering::ItemRenderer {
    /// A renderer-specific type for a brush used for fill and stroke of glyphs.
    type PlatformBrush;

    /// Returns the brush to be used for filling text.
    fn platform_text_fill_brush(
        &mut self,
        brush: crate::Brush,
        size: LogicalSize,
    ) -> Option<Self::PlatformBrush>;

    /// Returns a brush that's a solid fill of the specified color.
    fn platform_brush_for_color(&mut self, color: &crate::Color) -> Self::PlatformBrush;

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
        font: &parley::Font,
        font_size: f32,
        brush: &Self::PlatformBrush,
        stroke_style: &Option<TextStrokeStyle>,
        y_offset: f32,
        glyphs_it: &mut dyn Iterator<Item = parley::layout::Glyph>,
    );

    /// Fills the given rectangle with the specified brush. This is used for drawing selection
    /// rectangles as well as the text cursor.
    fn fill_rectangle(
        &mut self,
        physical_x: f32,
        physical_y: f32,
        physical_width: f32,
        physical_height: f32,
        brush: Self::PlatformBrush,
    );
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
pub struct Brush {
    /// Color to use for filling text. `None` means use of the default color/brush.
    pub color: Option<crate::Color>,
    pub stroke: Option<TextStrokeStyle>,
}

pub struct LayoutOptions {
    pub max_width: Option<LogicalLength>,
    pub max_height: Option<LogicalLength>,
    pub horizontal_align: TextHorizontalAlignment,
    pub vertical_align: TextVerticalAlignment,
    pub stroke: Option<TextStrokeStyle>,
    pub font_request: Option<FontRequest>,
    pub text_wrap: TextWrap,
    pub text_overflow: TextOverflow,
    pub selection: Option<core::ops::Range<usize>>,
    pub selection_foreground_color: Option<crate::Color>,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            max_width: None,
            max_height: None,
            horizontal_align: TextHorizontalAlignment::Left,
            vertical_align: TextVerticalAlignment::Top,
            stroke: None,
            font_request: None,
            text_wrap: TextWrap::WordWrap,
            text_overflow: TextOverflow::Clip,
            selection: None,
            selection_foreground_color: None,
        }
    }
}

pub fn layout(text: &str, scale_factor: ScaleFactor, options: LayoutOptions) -> Layout {
    let max_physical_width = options.max_width.map(|max_width| (max_width * scale_factor).get());
    let max_physical_height = options.max_height.map(|max_height| max_height * scale_factor);
    let pixel_size = options
        .font_request
        .as_ref()
        .and_then(|font_request| font_request.pixel_size)
        .unwrap_or(DEFAULT_FONT_SIZE);

    let push_to_builder = |builder: &mut parley::RangedBuilder<_>| {
        if let Some(ref font_request) = options.font_request {
            if let Some(family) = &font_request.family {
                builder.push_default(parley::StyleProperty::FontStack(
                    parley::style::FontStack::Single(parley::style::FontFamily::Named(
                        family.as_str().into(),
                    )),
                ));
            }
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

        builder.push_default(parley::StyleProperty::Brush(Brush {
            color: None,
            stroke: options.stroke,
        }));
    };

    let (mut layout, elision_info) = CONTEXTS.with_borrow_mut(move |contexts| {
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

        let mut builder =
            contexts.layout.ranged_builder(&mut contexts.font, text, scale_factor.get(), true);
        push_to_builder(&mut builder);

        if let Some((selection, selection_color)) =
            options.selection.zip(options.selection_foreground_color)
        {
            builder.push(
                parley::StyleProperty::Brush(Brush {
                    color: Some(selection_color),
                    stroke: options.stroke,
                }),
                selection,
            );
        }

        (builder.build(text), elision_info)
    });

    layout.break_all_lines(max_physical_width.filter(|_| options.text_wrap != TextWrap::NoWrap));
    layout.align(
        max_physical_width,
        match options.horizontal_align {
            TextHorizontalAlignment::Left => parley::Alignment::Left,
            TextHorizontalAlignment::Center => parley::Alignment::Middle,
            TextHorizontalAlignment::Right => parley::Alignment::Right,
        },
        parley::AlignmentOptions::default(),
    );

    let y_offset = match (max_physical_height, options.vertical_align) {
        (Some(max_height), TextVerticalAlignment::Center) => {
            (max_height.get() - layout.height()) / 2.0
        }
        (Some(max_height), TextVerticalAlignment::Bottom) => max_height.get() - layout.height(),
        (None, _) | (Some(_), TextVerticalAlignment::Top) => 0.0,
    };

    Layout { inner: layout, y_offset, elision_info }
}

struct ElisionInfo {
    elipsis_glyph: parley::layout::Glyph,
    max_physical_width: f32,
}

pub struct Layout {
    inner: parley::Layout<Brush>,
    pub y_offset: f32,
    elision_info: Option<ElisionInfo>,
}

impl Layout {
    /// Returns an iterator over the run's glyphs but with an optional elision
    /// glyph replacing the last line's last glyph that's exceeding the max width - if applicable.
    /// Call this function only for the last line of the layout.
    fn glyphs_with_elision<'a>(
        &'a self,
        glyph_run: &'a parley::layout::GlyphRun<Brush>,
    ) -> impl Iterator<Item = parley::layout::Glyph> + Clone + 'a {
        let run_beyond_max_width = self.elision_info.as_ref().map_or(false, |info| {
            let run_end = glyph_run.offset() + glyph_run.advance();

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

            if glyph.x + glyph.advance + elision_info.elipsis_glyph.advance
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
        default_brush: <R as GlyphRenderer>::PlatformBrush,
        draw_glyphs: &mut dyn FnMut(
            &mut R,
            &parley::Font,
            f32,
            &<R as GlyphRenderer>::PlatformBrush,
            &Option<TextStrokeStyle>,
            &mut dyn Iterator<Item = parley::layout::Glyph>,
        ),
    ) {
        for (line_index, line) in self.lines().enumerate() {
            let last_line = line_index == self.len() - 1;
            for item in line.items() {
                match item {
                    parley::PositionedLayoutItem::GlyphRun(glyph_run) => {
                        let run = glyph_run.run();

                        let brush = &glyph_run.style().brush;

                        let mut elided_glyphs_it;
                        let mut unelided_glyphs_it;
                        let glyphs_it: &mut dyn Iterator<Item = parley::layout::Glyph>;

                        if last_line {
                            elided_glyphs_it = self.glyphs_with_elision(&glyph_run);
                            glyphs_it = &mut elided_glyphs_it;
                        } else {
                            unelided_glyphs_it = glyph_run.positioned_glyphs();
                            glyphs_it = &mut unelided_glyphs_it;
                        };

                        let stroke = &brush.stroke;

                        let brush = match brush.color {
                            Some(color) => &item_renderer.platform_brush_for_color(&color),
                            None => &default_brush,
                        };

                        draw_glyphs(
                            item_renderer,
                            run.font(),
                            run.font_size(),
                            &brush,
                            &stroke,
                            glyphs_it,
                        );
                    }
                    parley::PositionedLayoutItem::InlineBox(_inline_box) => {}
                };
            }
        }
    }
}

impl std::ops::Deref for Layout {
    type Target = parley::Layout<Brush>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub fn draw_text(
    item_renderer: &mut impl GlyphRenderer,
    text: Pin<&dyn crate::item_rendering::RenderText>,
    font_request: Option<FontRequest>,
    size: LogicalSize,
) {
    let max_width = size.width_length();
    let max_height = size.height_length();

    if max_width.get() <= 0. || max_height.get() <= 0. {
        return;
    }

    let Some(platform_fill_brush) = item_renderer.platform_text_fill_brush(text.color(), size)
    else {
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
    let platform_stroke_brush =
        item_renderer.platform_text_stroke_brush(stroke_brush, stroke_width, size);

    let (horizontal_align, vertical_align) = text.alignment();

    let layout = layout(
        text.text().as_str(),
        scale_factor,
        LayoutOptions {
            horizontal_align,
            vertical_align,
            max_height: Some(max_height),
            max_width: Some(max_width),
            stroke: platform_stroke_brush.is_some().then_some(stroke_style),
            font_request,
            text_wrap: text.wrap(),
            text_overflow: text.overflow(),
            ..Default::default()
        },
    );

    layout.draw(
        item_renderer,
        platform_fill_brush,
        &mut |item_renderer, font, font_size, brush, stroke_style, glyphs_it| {
            item_renderer.draw_glyph_run(
                font,
                font_size,
                brush,
                stroke_style,
                layout.y_offset,
                glyphs_it,
            );
        },
    );
}

pub fn draw_text_input(
    item_renderer: &mut impl GlyphRenderer,
    text_input: Pin<&crate::items::TextInput>,
    font_request: Option<FontRequest>,
    size: LogicalSize,
) {
    let width = size.width_length();
    let height = size.height_length();
    if width.get() <= 0. || height.get() <= 0. {
        return;
    }

    let visual_representation = text_input.visual_representation(None);

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
        &text,
        scale_factor,
        LayoutOptions {
            max_width: Some(width),
            max_height: Some(height),
            vertical_align: text_input.vertical_alignment(),
            font_request,
            selection: Some(min_select..max_select),
            selection_foreground_color: Some(text_input.selection_foreground_color()),
            ..Default::default()
        },
    );

    let selection = parley::layout::cursor::Selection::new(
        parley::layout::cursor::Cursor::from_byte_index(&layout, min_select, Default::default()),
        parley::layout::cursor::Cursor::from_byte_index(&layout, max_select, Default::default()),
    );
    selection.geometry_with(&layout, |rect, _| {
        if let Some(selection_brush) = item_renderer.platform_text_fill_brush(
            text_input.selection_background_color().into(),
            euclid::size2::<Coord, PhysicalPx>(rect.width() as _, rect.height() as _)
                / scale_factor,
        ) {
            item_renderer.fill_rectangle(
                rect.min_x() as _,
                rect.min_y() as f32 + layout.y_offset,
                rect.width() as _,
                rect.height() as _,
                selection_brush,
            );
        }
    });

    layout.draw(
        item_renderer,
        platform_fill_brush,
        &mut |item_renderer, font, font_size, brush, stroke_style, glyphs_it| {
            item_renderer.draw_glyph_run(
                font,
                font_size,
                brush,
                stroke_style,
                layout.y_offset,
                glyphs_it,
            );
        },
    );

    if cursor_visible {
        let cursor = parley::layout::cursor::Cursor::from_byte_index(
            &layout,
            cursor_pos,
            Default::default(),
        );
        let rect = cursor.geometry(&layout, (text_input.text_cursor_width() * scale_factor).get());

        if let Some(cursor_brush) = item_renderer.platform_text_fill_brush(
            visual_representation.cursor_color.into(),
            euclid::size2::<Coord, PhysicalPx>(rect.width() as _, rect.height() as _)
                / scale_factor,
        ) {
            item_renderer.fill_rectangle(
                rect.min_x() as _,
                rect.min_y() as f32 + layout.y_offset,
                rect.width() as _,
                rect.height() as _,
                cursor_brush,
            );
        }
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
        text,
        scale_factor,
        LayoutOptions {
            max_width,
            text_wrap,
            font_request: Some(font_request),
            ..Default::default()
        },
    );
    euclid::size2(layout.width(), layout.height()) / scale_factor
}
