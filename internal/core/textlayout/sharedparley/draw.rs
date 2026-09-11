// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Painting a [`Layout`] through a [`GlyphRenderer`]: glyph runs, decorations, inline-code
//! capsules, and the clip-based recoloring of selected glyphs.

use super::layout::ElisionCut;
use super::selection::{RunCoverage, SelectionSpan, run_coverage};
use super::shaping::{Brush, TextParagraph};
use super::*;

/// Outline drawn around a rectangle filled via [`GlyphRenderer::fill_rectangle`].
#[derive(Clone)]
pub struct RectangleBorder<Brush> {
    pub brush: Brush,
    pub width: PhysicalLength,
}

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
    /// given y offset. The `normalized_coords` are F2Dot14 values in fvar axis order for variable
    /// font rendering. The `synthesis` contains design-space variation settings and faux
    /// bold/italic hints from fontique.
    fn draw_glyph_run(
        &mut self,
        font: &parley::FontData,
        font_size: PhysicalLength,
        normalized_coords: &[i16],
        synthesis: &fontique::Synthesis,
        brush: Self::PlatformBrush,
        y_offset: PhysicalLength,
        glyphs_it: &mut dyn Iterator<Item = parley::layout::Glyph>,
    );

    /// Convenience wrapper around `fill_rectangle` that resolves `color` to a platform
    /// brush and fills `physical_rect` with sharp corners and no outline.
    fn fill_rectangle_with_color(&mut self, physical_rect: PhysicalRect, color: Color) {
        if let Some(platform_brush) = self.platform_brush_for_color(&color) {
            self.fill_rectangle(physical_rect, platform_brush, PhysicalLength::zero(), None);
        }
    }

    /// Fills `physical_rect` with `brush`, optionally rounding the corners by `radius`
    /// and outlining it with `border`. Passing a zero `radius` produces sharp corners;
    /// passing `None` for `border` skips the outline.
    fn fill_rectangle(
        &mut self,
        physical_rect: PhysicalRect,
        brush: Self::PlatformBrush,
        radius: PhysicalLength,
        border: Option<RectangleBorder<Self::PlatformBrush>>,
    );
}

/// The vertical extent the renderer clip lets anything be drawn in, as a physical y range in the
/// current item's coordinates. A conservative superset of what is visible -- see
/// [`crate::item_rendering::ItemRenderer::get_current_clip`] -- so it is a safe bound for
/// skipping draw work, never for layout decisions.
pub(super) fn visible_band(item_renderer: &impl GlyphRenderer) -> Range<PhysicalLength> {
    let scale_factor = item_renderer.scale_factor();
    let clip = item_renderer.get_current_clip();
    let top = clip.origin.y_length() * scale_factor;
    top..(top + clip.height_length() * scale_factor)
}

/// The horizontal counterpart of [`visible_band`].
pub(super) fn visible_x_range(item_renderer: &impl GlyphRenderer) -> Range<PhysicalLength> {
    let scale_factor = item_renderer.scale_factor();
    let x_range = item_renderer.get_current_clip().x_length_range();
    (x_range.start * scale_factor)..(x_range.end * scale_factor)
}

impl TextParagraph {
    #[allow(clippy::too_many_arguments)]
    fn draw<R: GlyphRenderer>(
        &self,
        layout: &Layout,
        paragraph_index: usize,
        visible_extent: Option<ElisionCut>,
        visible_band: &Range<PhysicalLength>,
        // `None` when eliding.
        visible_x_range: Option<&Range<PhysicalLength>>,
        item_renderer: &mut R,
        default_fill_brush: &<R as GlyphRenderer>::PlatformBrush,
        default_stroke_brush: &Option<<R as GlyphRenderer>::PlatformBrush>,
        default_text_color: Color,
        selection: Option<&SelectionRendering<'_, R>>,
    ) {
        let para_y = layout.y_offset + self.y;

        let line_count = self.layout.lines().len();

        // For `overflow: elide` with a height limit (`overflow: clip` applies a hard pixel clip
        // instead) and for `max-lines`, `visible_extent` decides -- across all paragraphs -- the
        // last line to keep and where the vertical-truncation ellipsis goes. Translate it to this
        // paragraph. `last_drawn` is the deepest line of this paragraph that we draw; it carries
        // the horizontal ellipsis when it overflows the width. `vertical_truncation` marks the
        // single global last kept line that must also show an ellipsis when lines below it were
        // dropped.
        let (last_drawn, vertical_truncation) = match visible_extent {
            // Entirely below the kept block: drop the paragraph (don't redraw a stray first line,
            // and don't paint inline-code backgrounds under text that isn't rendered).
            Some(cut) if paragraph_index > cut.last_paragraph => return,
            // The paragraph where the cut falls: stop at the global last kept line.
            Some(cut) if paragraph_index == cut.last_paragraph => {
                (cut.last_line, cut.needs_ellipsis)
            }
            // A paragraph fully above the cut, or no cut at all: draw every line that fits
            // the box; the last visual line still elides horizontally when it is too wide.
            _ => (line_count.saturating_sub(1), false),
        };

        self.draw_inline_code_backgrounds(item_renderer, para_y, default_text_color, last_drawn);

        for (index, line) in self.layout.lines().enumerate() {
            // Stop once we are past the last kept line of the last kept paragraph.
            if index > last_drawn {
                break;
            }
            let metrics = line.metrics();

            // Skip lines that can't reach the visible band. Ink may overhang the line's metrics
            // box (stacked diacritics, swashes), so pad by one line height on each side before
            // excluding -- the pad scales with the line itself. Lines are in block order, so the
            // first line past the band ends the walk.
            let line_height =
                PhysicalLength::new(metrics.block_max_coord - metrics.block_min_coord);
            if para_y + PhysicalLength::new(metrics.block_max_coord) + line_height
                < visible_band.start
            {
                continue;
            }
            if para_y + PhysicalLength::new(metrics.block_min_coord) - line_height
                > visible_band.end
            {
                break;
            }

            // The kept line is always drawn, even when it slightly exceeds the box (#12197); other
            // lines are kept only while they fall within the box, taking vertical alignment into
            // account (bottom/center alignment clips lines off the top, not the bottom).
            let last_line = index == last_drawn;
            if !last_line
                && !layout.paragraph_line_within_box(
                    self,
                    metrics.block_min_coord,
                    metrics.block_max_coord,
                )
            {
                continue;
            }
            // The last drawn line should show an ellipsis if real lines below it were dropped for
            // the height, even when it fits the width.
            let vertically_truncated = last_line && vertical_truncation;
            let line_spans =
                selection.map(|selection| selection.spans.for_line(paragraph_index, index));
            // Padded for ink overhanging the advance, like the vertical filtering of lines.
            let padded_x_range = visible_x_range
                .map(|x_range| (x_range.start - line_height)..(x_range.end + line_height));
            for item in line.items() {
                match item {
                    parley::PositionedLayoutItem::GlyphRun(glyph_run) => {
                        let mut glyph_x_range = None;
                        if let Some(x_range) = &padded_x_range {
                            let run_start = PhysicalLength::new(glyph_run.offset());
                            let run_end =
                                PhysicalLength::new(glyph_run.offset() + glyph_run.advance());
                            if run_end < x_range.start || run_start > x_range.end {
                                continue;
                            }
                            if run_start < x_range.start || run_end > x_range.end {
                                glyph_x_range = Some(x_range);
                            }
                        }
                        let ellipsis = if last_line {
                            let (truncated_glyphs, ellipsis) = layout.glyphs_with_elision(
                                &glyph_run,
                                vertically_truncated,
                                metrics.trailing_whitespace,
                            );

                            Self::draw_glyph_run_with_selection(
                                &glyph_run,
                                item_renderer,
                                default_fill_brush,
                                default_stroke_brush,
                                para_y,
                                glyph_x_range,
                                &mut truncated_glyphs.into_iter(),
                                selection.map(|selection| &selection.foreground),
                                line_spans.unwrap_or_default(),
                            );
                            ellipsis
                        } else {
                            Self::draw_glyph_run_with_selection(
                                &glyph_run,
                                item_renderer,
                                default_fill_brush,
                                default_stroke_brush,
                                para_y,
                                glyph_x_range,
                                &mut glyph_run.positioned_glyphs(),
                                selection.map(|selection| &selection.foreground),
                                line_spans.unwrap_or_default(),
                            );
                            None
                        };

                        if let Some((ellipsis_glyph, ellipsis_font, font_size)) = ellipsis {
                            let run = glyph_run.run();
                            item_renderer.draw_glyph_run(
                                &ellipsis_font,
                                font_size,
                                run.normalized_coords(),
                                &run.synthesis(),
                                default_fill_brush.clone(),
                                para_y,
                                &mut core::iter::once(ellipsis_glyph),
                            );
                        }
                    }
                    parley::PositionedLayoutItem::InlineBox(_inline_box) => {}
                };
            }
        }
    }

    /// Paints a translucent rounded capsule under every glyph run that lies inside one of
    /// this paragraph's `Style::Code` ranges. Capsule colors are derived from the luminance
    /// of `default_text_color`, so light and dark themes both get a sensible default
    /// without any user-facing styling property.
    fn draw_inline_code_backgrounds<R: GlyphRenderer>(
        &self,
        item_renderer: &mut R,
        para_y: PhysicalLength,
        default_text_color: Color,
        last_drawn: usize,
    ) {
        if self.code_ranges.is_empty() {
            return;
        }

        // Neutral gray fill (low alpha) on both themes — contrast against the page
        // background carries the "this is code" cue. The border picks up the same hue
        // but a higher alpha so the rounded outline stays visible against the fill.
        // Pick brighter values on dark backgrounds (luminance of the text gives us
        // that signal without poking at the window background).
        let fg_luminance = 0.299 * default_text_color.red() as f32
            + 0.587 * default_text_color.green() as f32
            + 0.114 * default_text_color.blue() as f32;
        let fill = Color::from_argb_u8(28, 128, 128, 128);
        let border = if fg_luminance > 140.0 {
            Color::from_argb_u8(88, 170, 170, 170)
        } else {
            Color::from_argb_u8(56, 128, 128, 128)
        };
        // Border width and radius bounds are logical so that the capsule looks the
        // same at every DPI; the part of the radius derived from the capsule height
        // already scales with the (physical) font size.
        const BORDER_WIDTH: LogicalLength = LogicalLength::new(1.0);
        const MIN_RADIUS: LogicalLength = LogicalLength::new(2.0);
        const MAX_RADIUS: LogicalLength = LogicalLength::new(5.0);
        // A touch of vertical padding above and below the cap-height / descender band
        // so the capsule edge doesn't sit flush against tall glyphs.
        const VERTICAL_PADDING_RATIO: f32 = 0.15;

        let scale_factor = item_renderer.scale_factor();
        let border_width = BORDER_WIDTH * scale_factor;

        // Capsules only under lines that are drawn: lines past the visible-extent cut
        // (`overflow: elide` height limit or `max-lines`) don't render their glyphs either.
        for line in self.layout.lines().take(last_drawn + 1) {
            for item in line.items() {
                let parley::PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let run_range = run.text_range();
                if run_range.is_empty() {
                    continue;
                }
                // `Style::Code` pushes its own FontFamily + FontSize, which forces a
                // run boundary, so a code run is always fully contained in one of the
                // recorded ranges — a single containment check is enough.
                let is_code = self
                    .code_ranges
                    .iter()
                    .any(|cr| cr.start <= run_range.start && run_range.end <= cr.end);
                if !is_code {
                    continue;
                }

                let metrics = run.metrics();
                let ascent = metrics.ascent;
                let descent = metrics.descent;
                let cap_height = metrics.cap_height.unwrap_or(ascent * 0.72);

                // Center the capsule on the midpoint between cap-top and a shallow
                // approximation of the descender bottom (roughly where parens, commas
                // and dots reach). This gives equal visible padding above and below
                // for typical code text (which has caps but rarely real descenders).
                let upper_extent = cap_height;
                let lower_extent = descent * 0.4;
                let center = glyph_run.baseline() + (lower_extent - upper_extent) / 2.0;
                let inner_half_height = (upper_extent + lower_extent) / 2.0;
                let extra_padding = ascent * VERTICAL_PADDING_RATIO;
                let half_height = inner_half_height + extra_padding;
                let bg_height = (half_height * 2.0).max(1.0);
                let bg_top = center - half_height;

                // Width hugs the glyphs tightly — `glyph_run.advance()` is exactly
                // the horizontal extent of the rendered run. The underlying text is
                // not modified, so selection, hit-testing and copy/paste keep working
                // on the underlying characters.
                let bg_width = glyph_run.advance().max(0.0);
                if bg_width <= 0.0 {
                    continue;
                }
                let bg_left = glyph_run.offset();

                let bg_rect = PhysicalRect::new(
                    PhysicalPoint::from_lengths(
                        PhysicalLength::new(bg_left),
                        PhysicalLength::new(bg_top) + para_y,
                    ),
                    PhysicalSize::new(bg_width, bg_height),
                );
                let radius = PhysicalLength::new(bg_height * 0.22)
                    .max(MIN_RADIUS * scale_factor)
                    .min(MAX_RADIUS * scale_factor);
                let Some(fill_brush) = item_renderer.platform_brush_for_color(&fill) else {
                    continue;
                };
                let border_brush = item_renderer
                    .platform_brush_for_color(&border)
                    .map(|brush| RectangleBorder { brush, width: border_width });
                item_renderer.fill_rectangle(bg_rect, fill_brush, radius, border_brush);
            }
        }
    }

    /// Draws one glyph run, splitting it where the selection starts or ends inside it.
    ///
    /// The overwhelmingly common cases -- a run that is entirely selected or entirely unselected
    /// -- draw exactly once with no clip, so an enormous selection costs no more than a tiny one.
    /// Only the at most two runs per selection edge that actually straddle a boundary are drawn
    /// twice against a clip, and that is precisely where a ligature has to be cut in half.
    #[allow(clippy::too_many_arguments)]
    fn draw_glyph_run_with_selection<R: GlyphRenderer>(
        glyph_run: &parley::layout::GlyphRun<Brush>,
        item_renderer: &mut R,
        default_fill_brush: &<R as GlyphRenderer>::PlatformBrush,
        default_stroke_brush: &Option<<R as GlyphRenderer>::PlatformBrush>,
        para_y: PhysicalLength,
        // A uniform `no-wrap` line is a single run, so culling whole runs is not enough.
        visible_x_range: Option<&Range<PhysicalLength>>,
        glyphs_it: &mut dyn Iterator<Item = parley::layout::Glyph>,
        // The selection foreground, and the spans it covers on this run's line. Both empty when
        // there is no selection, which `run_coverage` reports as `Unselected`.
        selection_brush: Option<&<R as GlyphRenderer>::PlatformBrush>,
        line_spans: &[SelectionSpan],
    ) {
        let run_x = glyph_run.offset()..glyph_run.offset() + glyph_run.advance();

        // Bidirectional text reorders glyphs within a run, so this filters rather than truncating.
        let x_range = visible_x_range.cloned();
        let mut glyphs_it = glyphs_it.filter(move |glyph| {
            x_range.as_ref().is_none_or(|x_range| {
                let start = PhysicalLength::new(glyph.x);
                let end = PhysicalLength::new(glyph.x + glyph.advance);
                end >= x_range.start && start <= x_range.end
            })
        });
        let glyphs_it: &mut dyn Iterator<Item = parley::layout::Glyph> = &mut glyphs_it;

        match run_coverage(&run_x, line_spans) {
            RunCoverage::Unselected => Self::draw_glyph_run(
                glyph_run,
                item_renderer,
                default_fill_brush,
                default_stroke_brush,
                para_y,
                glyphs_it,
                None,
            ),
            RunCoverage::Full => Self::draw_glyph_run(
                glyph_run,
                item_renderer,
                default_fill_brush,
                default_stroke_brush,
                para_y,
                glyphs_it,
                selection_brush,
            ),
            RunCoverage::Partial => {
                // The run has to be rasterized once per segment, so the glyphs can't stay behind
                // a one-shot iterator.
                let glyphs = glyphs_it.collect::<alloc::vec::Vec<_>>();

                // Walk the run left to right, alternating unselected and selected segments. This
                // relies on the spans being ascending in x, which [`SelectionSpans`] guarantees.
                let mut x = run_x.start;
                for span in line_spans {
                    let span_x = span.x();
                    if span_x.end <= run_x.start {
                        continue;
                    }
                    if span_x.start >= run_x.end {
                        break;
                    }
                    let start = span_x.start.max(run_x.start);
                    let end = span_x.end.min(run_x.end);
                    for (segment, brush) in [(x..start, None), (start..end, selection_brush)] {
                        Self::draw_glyph_run_segment(
                            glyph_run,
                            item_renderer,
                            default_fill_brush,
                            default_stroke_brush,
                            para_y,
                            &glyphs,
                            segment,
                            brush,
                        );
                    }
                    x = end;
                }
                Self::draw_glyph_run_segment(
                    glyph_run,
                    item_renderer,
                    default_fill_brush,
                    default_stroke_brush,
                    para_y,
                    &glyphs,
                    x..run_x.end,
                    None,
                );
            }
        }
    }

    /// Draws `glyphs` clipped to the horizontal band `x`, so that a glyph straddling the band's
    /// edge is cut rather than recolored as a whole.
    fn draw_glyph_run_segment<R: GlyphRenderer>(
        glyph_run: &parley::layout::GlyphRun<Brush>,
        item_renderer: &mut R,
        default_fill_brush: &<R as GlyphRenderer>::PlatformBrush,
        default_stroke_brush: &Option<<R as GlyphRenderer>::PlatformBrush>,
        para_y: PhysicalLength,
        glyphs: &[parley::layout::Glyph],
        x: Range<f32>,
        override_fill_brush: Option<&<R as GlyphRenderer>::PlatformBrush>,
    ) {
        if x.end <= x.start {
            return;
        }

        item_renderer.save_state();

        // Clip horizontally only: the vertical extent stays whatever is already in effect, so
        // accents and descenders reaching outside the line box are never sheared off.
        let scale_factor = item_renderer.scale_factor();
        let current_clip = item_renderer.get_current_clip();
        let render = item_renderer.combine_clip(
            LogicalRect::new(
                LogicalPoint::from_lengths(
                    PhysicalLength::new(x.start) / scale_factor,
                    current_clip.origin.y_length(),
                ),
                LogicalSize::from_lengths(
                    PhysicalLength::new(x.end - x.start) / scale_factor,
                    current_clip.height_length(),
                ),
            ),
            LogicalBorderRadius::zero(),
        );

        if render {
            Self::draw_glyph_run(
                glyph_run,
                item_renderer,
                default_fill_brush,
                default_stroke_brush,
                para_y,
                &mut glyphs.iter().cloned(),
                override_fill_brush,
            );
        }

        item_renderer.restore_state();
    }

    fn draw_glyph_run<R: GlyphRenderer>(
        glyph_run: &parley::layout::GlyphRun<Brush>,
        item_renderer: &mut R,
        default_fill_brush: &<R as GlyphRenderer>::PlatformBrush,
        default_stroke_brush: &Option<<R as GlyphRenderer>::PlatformBrush>,
        para_y: PhysicalLength,
        glyphs_it: &mut dyn Iterator<Item = parley::layout::Glyph>,
        // Forced fill for selected glyphs, overriding the run's own brush.
        override_fill_brush: Option<&<R as GlyphRenderer>::PlatformBrush>,
    ) {
        let run = glyph_run.run();
        let normalized_coords = run.normalized_coords();
        let synthesis = run.synthesis();
        let brush = &glyph_run.style().brush;

        let (fill_brush, stroke_style) = match override_fill_brush {
            // Selection wins over a `Style::Color` span and over a link color: text under the
            // highlight has to stay legible against the selection background.
            Some(selection_brush) => (selection_brush.clone(), &None),
            None => match (brush.override_fill_color, brush.link_color) {
                (Some(color), _) => {
                    let Some(color_brush) = item_renderer.platform_brush_for_color(&color) else {
                        return;
                    };
                    (color_brush.clone(), &None)
                }
                (None, Some(color)) => {
                    let Some(link_brush) = item_renderer.platform_brush_for_color(&color) else {
                        return;
                    };
                    (link_brush.clone(), &None)
                }
                (None, None) => (default_fill_brush.clone(), &brush.stroke),
            },
        };

        match stroke_style {
            Some(TextStrokeStyle::Outside) => {
                let glyphs = glyphs_it.collect::<alloc::vec::Vec<_>>();

                if let Some(stroke_brush) = default_stroke_brush.clone() {
                    item_renderer.draw_glyph_run(
                        run.font(),
                        PhysicalLength::new(run.font_size()),
                        normalized_coords,
                        &synthesis,
                        stroke_brush,
                        para_y,
                        &mut glyphs.iter().cloned(),
                    );
                }

                item_renderer.draw_glyph_run(
                    run.font(),
                    PhysicalLength::new(run.font_size()),
                    normalized_coords,
                    &synthesis,
                    fill_brush.clone(),
                    para_y,
                    &mut glyphs.into_iter(),
                );
            }
            Some(TextStrokeStyle::Center) => {
                let glyphs = glyphs_it.collect::<alloc::vec::Vec<_>>();

                item_renderer.draw_glyph_run(
                    run.font(),
                    PhysicalLength::new(run.font_size()),
                    normalized_coords,
                    &synthesis,
                    fill_brush.clone(),
                    para_y,
                    &mut glyphs.iter().cloned(),
                );

                if let Some(stroke_brush) = default_stroke_brush.clone() {
                    item_renderer.draw_glyph_run(
                        run.font(),
                        PhysicalLength::new(run.font_size()),
                        normalized_coords,
                        &synthesis,
                        stroke_brush,
                        para_y,
                        &mut glyphs.into_iter(),
                    );
                }
            }
            None => {
                item_renderer.draw_glyph_run(
                    run.font(),
                    PhysicalLength::new(run.font_size()),
                    normalized_coords,
                    &synthesis,
                    fill_brush.clone(),
                    para_y,
                    glyphs_it,
                );
            }
        }

        let metrics = run.metrics();

        // A decoration spans the whole run. Where a selection boundary cuts through it, the
        // renderer clip that cuts the glyphs cuts the rectangle too.
        if glyph_run.style().underline.is_some() {
            item_renderer.fill_rectangle(
                PhysicalRect::new(
                    PhysicalPoint::from_lengths(
                        PhysicalLength::new(glyph_run.offset()),
                        para_y
                            + PhysicalLength::new(glyph_run.baseline() - metrics.underline_offset),
                    ),
                    PhysicalSize::new(glyph_run.advance(), metrics.underline_size),
                ),
                fill_brush.clone(),
                PhysicalLength::zero(),
                None,
            );
        }

        if glyph_run.style().strikethrough.is_some() {
            item_renderer.fill_rectangle(
                PhysicalRect::new(
                    PhysicalPoint::from_lengths(
                        PhysicalLength::new(glyph_run.offset()),
                        para_y
                            + PhysicalLength::new(
                                glyph_run.baseline() - metrics.strikethrough_offset,
                            ),
                    ),
                    PhysicalSize::new(glyph_run.advance(), metrics.strikethrough_size),
                ),
                fill_brush,
                PhysicalLength::zero(),
                None,
            );
        }
    }
}

impl Layout {
    pub(super) fn draw<R: GlyphRenderer>(
        &self,
        item_renderer: &mut R,
        default_fill_brush: <R as GlyphRenderer>::PlatformBrush,
        default_stroke_brush: Option<<R as GlyphRenderer>::PlatformBrush>,
        default_text_color: Color,
        selection: Option<&SelectionRendering<'_, R>>,
    ) {
        // Compute the cut once: explicit `\n` breaks produce one paragraph each, but they must
        // elide as a single block (drop lines below the box, ellipsis on the last visible one).
        let visible_extent = self.visible_extent();

        // Everything drawn below is cut to the renderer clip anyway, so lines that can't reach
        // it are skipped instead of submitted: the clip is a bounding box of everything still
        // drawable (see [`crate::item_rendering::ItemRenderer::get_current_clip`]), which makes
        // skipping what lies outside it safe under any transform. The band only filters what is
        // *drawn*; it never influences elision or `max-lines` accounting.
        let visible_band = visible_band(item_renderer);
        // The ellipsis is positioned from the overflowing run, so don't cull while eliding.
        let visible_x_range = (!self.is_eliding()).then(|| visible_x_range(item_renderer));

        // Paragraphs are stacked in order, so binary-search the first one whose box reaches the
        // band. Start one paragraph earlier and stop one past the band: glyph ink may overhang
        // its line's metrics box, and the line-level cull in [`TextParagraph::draw`] trims those
        // two edge paragraphs down to their edge lines.
        let first = self
            .paragraphs
            .partition_point(|p| {
                self.y_offset + p.y + PhysicalLength::new(p.layout.height()) < visible_band.start
            })
            .saturating_sub(1);
        let mut past_band = false;
        for (paragraph_index, paragraph) in self.paragraphs.iter().enumerate().skip(first) {
            if past_band {
                break;
            }
            past_band = self.y_offset + paragraph.y > visible_band.end;
            paragraph.draw(
                self,
                paragraph_index,
                visible_extent,
                &visible_band,
                visible_x_range.as_ref(),
                item_renderer,
                &default_fill_brush,
                &default_stroke_brush,
                default_text_color,
                selection,
            );
        }
    }
}
