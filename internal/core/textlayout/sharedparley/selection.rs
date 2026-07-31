// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore bidi

//! Selection geometry, resolved once per draw into per-line horizontal spans.
//!
//! The same rounded span edges fill the highlight and clip the glyph runs, so the two can't
//! disagree -- see [`SelectionSpan`].

use super::*;

/// One contiguous run of selected text on a single line.
///
/// Selection is deliberately *not* expressed as a text style. A style can only ever recolor a
/// whole glyph, but a selection boundary may fall in the middle of one: with an `fi` ligature,
/// selecting just the `i` leaves parley with a single glyph whose style comes from the cluster's
/// first character (`Glyph::style_index` is `char_infos[cluster_id]`), so the whole ligature would
/// be painted unselected while the highlight covers only its right half. Instead the spans below
/// are used twice — to fill the highlight background, and to clip the glyph runs that straddle a
/// boundary so each half is drawn in its own color.
#[derive(Clone, Debug)]
pub(super) struct SelectionSpan {
    /// Index into `Layout::paragraphs`.
    pub(super) paragraph: usize,
    /// Line within that paragraph.
    pub(super) line: usize,
    /// Highlight rectangle in item coordinates, ready to fill. Its horizontal edges are snapped to
    /// whole device pixels where they are computed, and [`Self::x`] hands the very same edges to
    /// the glyph clip -- so the highlight edge and the clip edge cannot disagree and leave a sliver
    /// of wrongly-colored glyph on top of the highlight.
    background: PhysicalRect,
}

impl SelectionSpan {
    /// Horizontal extent of the highlight, in the same coordinate space as `GlyphRun::offset()`.
    pub(super) fn x(&self) -> Range<f32> {
        self.background.min_x()..self.background.max_x()
    }
}

/// Sorted by `(paragraph, line, x.start)`: the spans belonging to one line form a contiguous slice,
/// and within that slice they run left to right. Both halves are load-bearing -- see
/// [`Self::for_line`] and the segment walk in `draw_glyph_run_with_selection`.
#[derive(Clone, Debug, Default)]
pub(super) struct SelectionSpans(pub(super) Vec<SelectionSpan>);

impl SelectionSpans {
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(super) fn backgrounds(&self) -> impl Iterator<Item = PhysicalRect> + '_ {
        self.0.iter().map(|span| span.background)
    }

    /// The spans covering one line. Both the stored spans and the draw loop walk paragraphs and
    /// lines in order, but a binary search keeps this independent of that ordering.
    pub(super) fn for_line(&self, paragraph: usize, line: usize) -> &[SelectionSpan] {
        let key = (paragraph, line);
        let start = self.0.partition_point(|span| (span.paragraph, span.line) < key);
        let len = self.0[start..].partition_point(|span| (span.paragraph, span.line) == key);
        &self.0[start..start + len]
    }
}

/// How selected glyphs are painted, resolved once per draw call.
pub(super) struct SelectionRendering<'a, R: GlyphRenderer> {
    pub(super) spans: &'a SelectionSpans,
    /// Forced fill for selected glyphs. It wins over `Brush::override_fill_color` and
    /// `Brush::link_color`, so a colored span or a link inside the selection still reads as
    /// selected.
    pub(super) foreground: <R as GlyphRenderer>::PlatformBrush,
}

/// How much of one glyph run a selection covers.
pub(super) enum RunCoverage {
    /// No selected pixels: draw once, in the run's own brush.
    Unselected,
    /// Fully selected: draw once, in the selection foreground. No clip needed.
    Full,
    /// A boundary falls inside the run — possibly inside a ligature. The line's spans have to be
    /// drawn separately, each clipped to its own horizontal band.
    Partial,
}

/// Classifies `run_x` against the selection spans of the line it sits on.
pub(super) fn run_coverage(run_x: &Range<f32>, spans: &[SelectionSpan]) -> RunCoverage {
    // Empty runs (and the degenerate zero-advance runs parley emits for ligature tails) can't
    // show a boundary.
    if spans.is_empty() || run_x.end <= run_x.start {
        return RunCoverage::Unselected;
    }
    let mut overlapping = false;
    for span in spans {
        let span_x = span.x();
        if span_x.start <= run_x.start && span_x.end >= run_x.end {
            return RunCoverage::Full;
        }
        overlapping |= span_x.start < run_x.end && span_x.end > run_x.start;
    }
    if overlapping { RunCoverage::Partial } else { RunCoverage::Unselected }
}

impl Layout {
    /// Resolves `selection_range` into per-line horizontal spans, for the lines whose boxes may
    /// intersect `visible_band` (a physical y range in item coordinates; spans feed drawing only,
    /// so off-screen lines need none). Pass an unbounded band to resolve everything.
    ///
    /// Parley already splits a ligature into one cluster per character and apportions the
    /// advance between them, so the geometry it reports is accurate to sub-glyph precision --
    /// selecting the `i` of an `fi` ligature yields exactly the ligature's right half. That
    /// precision is what makes clip-based selection drawing possible; see [`SelectionSpan`].
    pub(super) fn selection_geometry(
        &self,
        selection_range: Range<usize>,
        visible_band: &Range<PhysicalLength>,
    ) -> SelectionSpans {
        let mut spans = Vec::new();

        for (paragraph_index, paragraph) in self.visible_paragraphs().iter().enumerate() {
            // Like the draw cull, padded by an (average) line height for ink that overhangs its
            // line box: such a line still draws, so it still needs its spans.
            let paragraph_top = self.y_offset + paragraph.y;
            let paragraph_height = PhysicalLength::new(paragraph.layout.height());
            let line_pad = paragraph_height / paragraph.layout.lines().len().max(1) as f32;
            if paragraph_top + paragraph_height + line_pad < visible_band.start
                || paragraph_top - line_pad > visible_band.end
            {
                continue;
            }

            let selection_start = selection_range.start.max(paragraph.range.start);
            let selection_end = selection_range.end.min(paragraph.range.end);

            if selection_start >= selection_end {
                continue;
            }

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

            selection.geometry_with(&paragraph.layout, |rect, line| {
                // Snap the horizontal edges to device pixels once, here, so that the highlight
                // rectangle and the glyph clip derived from the same span are pixel-identical.
                let x = (rect.x0 as f32).round()..(rect.x1 as f32).round();
                if x.end <= x.start {
                    return;
                }
                // A giant wrapped paragraph passes the paragraph test above with all its lines;
                // keep only the ones that can reach the band.
                let top = PhysicalLength::new(rect.y0 as _) + paragraph_top;
                let bottom = PhysicalLength::new(rect.y1 as _) + paragraph_top;
                if bottom + line_pad < visible_band.start || top - line_pad > visible_band.end {
                    return;
                }
                let background = PhysicalRect::new(
                    PhysicalPoint::from_lengths(
                        PhysicalLength::new(x.start),
                        PhysicalLength::new(rect.y0 as _) + paragraph_top,
                    ),
                    PhysicalSize::new(x.end - x.start, rect.height() as _),
                );
                spans.push(SelectionSpan { paragraph: paragraph_index, line, background });
            });
        }

        // Already in this order: paragraphs are visited in order, `geometry_with` walks a
        // paragraph's lines in order, and within a line it accumulates x left to right over
        // visually reordered items -- so even a bidi line yields ascending spans. Sort defensively
        // anyway, since both consumers depend on it and neither would fail loudly: `for_line`
        // needs the `(paragraph, line)` grouping, and the segment walk in
        // `draw_glyph_run_with_selection` needs ascending x within a line.
        spans.sort_by(|a, b| {
            (a.paragraph, a.line)
                .cmp(&(b.paragraph, b.line))
                .then_with(|| a.x().start.total_cmp(&b.x().start))
        });

        SelectionSpans(spans)
    }
}
