// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! From shaped paragraphs to a [`Layout`]: line breaking, alignment, and the elision and
//! `max-lines` cuts, plus the queries the entry points ask of the result.

use super::shaping::{Brush, TextParagraph};
use super::*;

#[derive(Default)]
pub(super) struct LayoutOptions {
    pub(super) max_width: Option<LogicalLength>,
    pub(super) max_height: Option<LogicalLength>,
    /// Maximum number of visible lines across all paragraphs.
    pub(super) max_lines: Option<usize>,
    pub(super) horizontal_align: TextHorizontalAlignment,
    pub(super) vertical_align: TextVerticalAlignment,
    pub(super) text_overflow: TextOverflow,
}

impl LayoutOptions {
    pub(super) fn new_from_textinput(
        text_input: Pin<&crate::items::TextInput>,
        max_width: Option<LogicalLength>,
        max_height: Option<LogicalLength>,
    ) -> Self {
        Self {
            max_width,
            max_height,
            max_lines: None,
            horizontal_align: text_input.horizontal_alignment(),
            vertical_align: text_input.vertical_alignment(),
            text_overflow: TextOverflow::Clip,
        }
    }
}

/// The inputs the line breaking and its derived metrics depend on. Two [`layout`] calls with
/// equal inputs produce identical breaking for the same shaped paragraphs, so a matching
/// [`RetainedLineBreaking`] lets [`layout`] skip re-breaking every paragraph. Deliberately absent:
/// `max_height` and the vertical alignment, which only feed the per-call `y_offset` and the
/// height-based elision cut, both computed on the [`Layout`] itself.
#[derive(Clone, Copy, PartialEq)]
struct LineBreakingInputs {
    max_physical_width: Option<PhysicalLength>,
    /// The alignment as parley receives it, so `Start` and `Left` don't spuriously differ.
    alignment: parley::Alignment,
    max_lines: Option<usize>,
    text_overflow: TextOverflow,
}

impl LineBreakingInputs {
    fn new(options: &LayoutOptions, max_physical_width: Option<PhysicalLength>) -> Self {
        Self {
            max_physical_width,
            alignment: match options.horizontal_align {
                TextHorizontalAlignment::Start | TextHorizontalAlignment::Left => {
                    parley::Alignment::Left
                }
                TextHorizontalAlignment::Center => parley::Alignment::Center,
                TextHorizontalAlignment::End | TextHorizontalAlignment::Right => {
                    parley::Alignment::Right
                }
            },
            max_lines: options.max_lines,
            text_overflow: options.text_overflow,
        }
    }
}

/// What a full [`layout`] pass computed, retained in the cache entry alongside the shaped
/// paragraphs. The parley layouts already hold their broken lines and every `TextParagraph::y`
/// its position, so together with these metrics the next [`layout`] call with equal
/// [`LineBreakingInputs`] has nothing left to do.
pub(super) struct RetainedLineBreaking {
    inputs: LineBreakingInputs,
    line_limit_cut: Option<(usize, usize)>,
    max_width: PhysicalLength,
    height: PhysicalLength,
    elision_info: Option<ElisionInfo>,
}

/// Where vertical alignment puts the text within the box. Per call, not retained: it depends on
/// `max_height`, which changes freely (e.g. during a resize) without affecting the breaking.
fn vertical_offset(
    max_physical_height: Option<PhysicalLength>,
    vertical_align: TextVerticalAlignment,
    height: PhysicalLength,
) -> PhysicalLength {
    match (max_physical_height, vertical_align) {
        (Some(max_height), TextVerticalAlignment::Center) => (max_height - height) / 2.0,
        (Some(max_height), TextVerticalAlignment::Bottom) => max_height - height,
        (None, _) | (Some(_), TextVerticalAlignment::Top) => PhysicalLength::new(0.0),
    }
}

pub(super) fn layout(
    layout_builder: &LayoutWithoutLineBreaksBuilder,
    font_context: &mut parley::FontContext,
    mut paragraphs: Vec<TextParagraph>,
    scale_factor: ScaleFactor,
    options: LayoutOptions,
    line_breaking: Option<RetainedLineBreaking>,
) -> Layout {
    let max_physical_width = options.max_width.map(|max_width| max_width * scale_factor);
    let max_physical_height = options.max_height.map(|max_height| max_height * scale_factor);

    let inputs = LineBreakingInputs::new(&options, max_physical_width);
    if let Some(line_breaking) =
        line_breaking.filter(|line_breaking| line_breaking.inputs == inputs)
    {
        return Layout {
            y_offset: vertical_offset(
                max_physical_height,
                options.vertical_align,
                line_breaking.height,
            ),
            paragraphs,
            max_width: line_breaking.max_width,
            height: line_breaking.height,
            max_physical_height,
            elision_info: line_breaking.elision_info,
            line_limit_cut: line_breaking.line_limit_cut,
            line_breaking_inputs: inputs,
            broke_lines: false,
        };
    }

    // Returned None if failed to get the ellipsis glyph for some rare reason.
    let get_ellipsis_glyph = |font_context: &mut parley::FontContext| {
        let mut layout = layout_builder.build(font_context, "…", None, None);
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
        get_ellipsis_glyph(font_context).map(|(ellipsis_glyph, font_for_ellipsis_glyph)| {
            ElisionInfo { ellipsis_glyph, font_for_ellipsis_glyph, max_physical_width }
        })
    } else {
        None
    };

    let mut para_y = 0.0;
    for para in paragraphs.iter_mut() {
        para.layout.break_all_lines(max_physical_width.map(|width| width.get()));
        para.layout.align(inputs.alignment, parley::AlignmentOptions::default());

        para.y = PhysicalLength::new(para_y);
        para_y += para.layout.height();
    }

    let line_limit_cut =
        options.max_lines.and_then(|max_lines| line_limit_cut(&paragraphs, max_lines));
    let visible_paragraph_count =
        line_limit_cut.map_or(paragraphs.len(), |(last_paragraph, _)| last_paragraph + 1);

    let max_width = paragraphs
        .iter()
        .take(visible_paragraph_count)
        .enumerate()
        .map(|(paragraph_index, p)| {
            // The max width is used for the ellipsis computation when eliding text. We *want* to exclude whitespace
            // for that, but we can't at the glyph run level, so the glyph runs always *do* include whitespace glyphs,
            // and as such we must also accept the full width here including trailing whitespace, otherwise text with
            // trailing whitespace will assigned a smaller width for rendering and thus the ellipsis will be placed.
            match line_limit_cut {
                // In the paragraph where the line limit lands, only the kept lines count towards
                // the width; `full_width()` would also span the dropped lines below the cut. Per
                // line, mirror parley's `full_width` formula (Slint doesn't use indentation).
                Some((last_paragraph, last_line)) if paragraph_index == last_paragraph => p
                    .layout
                    .lines()
                    .take(last_line + 1)
                    .map(|line| {
                        let metrics = line.metrics();
                        PhysicalLength::new(metrics.inline_min_coord + metrics.advance)
                    })
                    .fold(PhysicalLength::zero(), PhysicalLength::max),
                _ => PhysicalLength::new(p.layout.full_width()),
            }
        })
        .fold(PhysicalLength::zero(), PhysicalLength::max);
    // With an active line limit, the height only extends to the bottom of the last kept line, so
    // that the preferred height and vertical alignment are based on what is actually shown.
    let height = match line_limit_cut {
        Some((last_paragraph, last_line)) => {
            let para = &paragraphs[last_paragraph];
            let line = para
                .layout
                .lines()
                .nth(last_line)
                .expect("line_limit_cut returns an existing line index");
            para.y + PhysicalLength::new(line.metrics().block_max_coord)
        }
        None => paragraphs
            .last()
            .map_or(PhysicalLength::zero(), |p| p.y + PhysicalLength::new(p.layout.height())),
    };

    let y_offset = vertical_offset(max_physical_height, options.vertical_align, height);

    Layout {
        paragraphs,
        y_offset,
        elision_info,
        max_width,
        height,
        max_physical_height,
        line_limit_cut,
        line_breaking_inputs: inputs,
        broke_lines: true,
    }
}

/// Where a `max-lines` limit cuts the text off: the (paragraph index, line index within that
/// paragraph) of the last kept line. Returns `None` when all lines fit the limit, so an active
/// cut always means that at least one line was dropped.
fn line_limit_cut(paragraphs: &[TextParagraph], max_lines: usize) -> Option<(usize, usize)> {
    let total_lines: usize = paragraphs.iter().map(|p| p.layout.lines().len()).sum();
    if total_lines <= max_lines {
        return None;
    }

    let mut seen_lines = 0;
    for (paragraph_index, para) in paragraphs.iter().enumerate() {
        let line_count = para.layout.lines().len();
        // seen_lines < max_lines holds on entry, so the cut line index can't underflow and
        // lands within this paragraph's lines.
        if seen_lines + line_count >= max_lines {
            return Some((paragraph_index, max_lines - seen_lines - 1));
        }
        seen_lines += line_count;
    }
    unreachable!("total_lines > max_lines, so the paragraph with the last kept line exists")
}

struct ElisionInfo {
    ellipsis_glyph: parley::layout::Glyph,
    font_for_ellipsis_glyph: parley::FontData,
    max_physical_width: PhysicalLength,
}

/// Whether a line whose bottom edge is at `block_max_coord` fits within `max_physical_height`,
/// rounding the height up so a sub-pixel overflow still counts as fitting.
fn line_fits_height(block_max_coord: f32, max_physical_height: PhysicalLength) -> bool {
    max_physical_height.get().ceil() >= block_max_coord
}

/// Where `overflow: elide` cuts text off, computed across all paragraphs (each explicit `\n`
/// produces one paragraph). See [`Layout::elision_extent`].
#[derive(Clone, Copy)]
pub(super) struct ElisionCut {
    /// Paragraph holding the last kept line.
    pub(super) last_paragraph: usize,
    /// Last kept line within `last_paragraph`.
    pub(super) last_line: usize,
    /// A line below the kept one was dropped for the height, so the kept line shows an ellipsis.
    pub(super) needs_ellipsis: bool,
}

pub(super) struct Layout {
    pub(super) paragraphs: Vec<TextParagraph>,
    pub(super) y_offset: PhysicalLength,
    pub(super) max_width: PhysicalLength,
    pub(super) height: PhysicalLength,
    max_physical_height: Option<PhysicalLength>,
    elision_info: Option<ElisionInfo>,
    /// Where an active `max-lines` limit drops lines, in the same coordinates as [`ElisionCut`]:
    /// the (paragraph index, line index) of the last kept line. See [`line_limit_cut`].
    pub(super) line_limit_cut: Option<(usize, usize)>,
    /// What the paragraphs' lines are currently broken for; travels back into the cache entry.
    line_breaking_inputs: LineBreakingInputs,
    /// Whether this layout ran the full breaking pass rather than reusing a [`RetainedLineBreaking`].
    /// Read by the `layout_miss_count` test counter.
    pub(super) broke_lines: bool,
}

impl Layout {
    /// Takes the layout apart into what the cache entry retains: the paragraphs (holding their
    /// broken lines and y positions) and the [`RetainedLineBreaking`] that lets the next [`layout`] call
    /// with equal inputs skip the breaking.
    pub(super) fn dismantle(self) -> (Vec<TextParagraph>, RetainedLineBreaking) {
        (
            self.paragraphs,
            RetainedLineBreaking {
                inputs: self.line_breaking_inputs,
                line_limit_cut: self.line_limit_cut,
                max_width: self.max_width,
                height: self.height,
                elision_info: self.elision_info,
            },
        )
    }
}

impl Layout {
    /// The paragraphs that have at least one line to show. Only differs from `paragraphs` when a
    /// `max-lines` limit drops lines: paragraphs entirely below the cut don't take part in
    /// hit-testing or selection.
    pub(super) fn visible_paragraphs(&self) -> &[TextParagraph] {
        match self.line_limit_cut {
            Some((last_paragraph, _)) => &self.paragraphs[..=last_paragraph],
            None => &self.paragraphs,
        }
    }

    /// True when an active line limit dropped lines and `y` (in item coordinates) falls below
    /// the last kept line, i.e. into the item region where the dropped lines would have been.
    /// Nothing is shown there, so nothing there should hit-test. With an active cut, `height`
    /// is the bottom of the last kept line.
    pub(super) fn below_line_limit(&self, y: PhysicalLength) -> bool {
        self.line_limit_cut.is_some() && y >= self.y_offset + self.height
    }

    /// The last line to draw, combining the height-based elision cut with the `max-lines` limit:
    /// whichever cuts earlier wins. Unlike the elision cut, the line limit also applies with
    /// `overflow: clip` -- just without the ellipsis.
    pub(super) fn visible_extent(&self) -> Option<ElisionCut> {
        let line_limit_cut = self.line_limit_cut.map(|(last_paragraph, last_line)| ElisionCut {
            last_paragraph,
            last_line,
            // The cut only exists when lines were dropped below it, so when eliding, the last
            // kept line always signals the truncation.
            needs_ellipsis: self.elision_info.is_some(),
        });
        match (self.elision_extent(), line_limit_cut) {
            (Some(elision), Some(line_limit)) => {
                Some(core::cmp::min_by_key(elision, line_limit, |cut| {
                    (cut.last_paragraph, cut.last_line)
                }))
            }
            (elision, line_limit) => elision.or(line_limit),
        }
    }

    /// Returns true if the very first line is taller than the available height, meaning the
    /// vertical line dropping used for `overflow: elide` would discard it and render nothing.
    /// In that case the caller keeps drawing the first line but applies a hard pixel clip to
    /// trim its vertical overflow, so it is shown (clipped) rather than disappearing entirely.
    pub(super) fn first_line_exceeds_height(&self) -> bool {
        let Some(max_physical_height) = self.max_physical_height else {
            return false;
        };
        self.paragraphs.first().and_then(|paragraph| paragraph.layout.lines().next()).is_some_and(
            |line| !line_fits_height(line.metrics().block_max_coord, max_physical_height),
        )
    }

    /// Whether a line of `paragraph` (with the metrics block range `block_min`..`block_max` in the
    /// paragraph's local coordinates) falls within the box for `overflow: elide` with a height
    /// limit. Accounts for vertical alignment via `y_offset`, which is negative for bottom/center
    /// alignment. Without a height limit, or when not eliding, every line counts as within the box.
    pub(super) fn paragraph_line_within_box(
        &self,
        paragraph: &TextParagraph,
        block_min: f32,
        block_max: f32,
    ) -> bool {
        match self.max_physical_height {
            Some(max_physical_height) if self.elision_info.is_some() => {
                let para_y = self.y_offset + paragraph.y;
                // `line_fits_height` rounds the bottom up by a pixel; allow the same slack at the
                // top so a line sitting right on the box edge isn't dropped to a rounding error.
                line_fits_height(para_y.get() + block_max, max_physical_height)
                    && para_y.get() + block_min >= -0.5
            }
            _ => true,
        }
    }

    /// For `overflow: elide` with a height limit, work out the last line to keep across all
    /// paragraphs. Explicit `\n` line breaks each produce a paragraph, and they have to elide as a
    /// single block: lines below the box are dropped and the ellipsis goes on the last visible
    /// line. Returns `None` when there is no height limit or elision (draw everything). When
    /// nothing fits at all the very first line is kept (#12197) so the text never vanishes
    /// entirely; `draw_text` then clips its vertical overflow.
    fn elision_extent(&self) -> Option<ElisionCut> {
        self.max_physical_height?;
        self.elision_info.as_ref()?;

        // The deepest line still within the box, scanning paragraphs and their lines from the
        // bottom up. Bottom/center alignment clips lines off the top, so the visible block can
        // start partway down, but its last line is always the lowest one that fits.
        let last_within_box = self.paragraphs.iter().enumerate().rev().find_map(|(pi, para)| {
            para.layout
                .lines()
                .enumerate()
                .rev()
                .find(|(_, line)| {
                    let m = line.metrics();
                    self.paragraph_line_within_box(para, m.block_min_coord, m.block_max_coord)
                })
                .map(|(li, _)| (pi, li))
        });

        // The very last line in document order, used to tell whether anything was dropped below
        // the kept line (and so whether an ellipsis is needed).
        let final_line = self
            .paragraphs
            .iter()
            .enumerate()
            .rev()
            .find_map(|(pi, para)| para.layout.lines().len().checked_sub(1).map(|li| (pi, li)));

        let (last_paragraph, last_line) = last_within_box.unwrap_or((0, 0));
        let needs_ellipsis =
            final_line.is_some_and(|final_line| final_line != (last_paragraph, last_line));
        Some(ElisionCut { last_paragraph, last_line, needs_ellipsis })
    }

    /// Returns the last paragraph starting at or before the given byte offset. An offset in the
    /// gap between two paragraph ranges (between a '\r' and its '\n') thus maps to the preceding
    /// paragraph; callers have to clamp their local offset to the paragraph's range.
    fn paragraph_by_byte_offset(&self, byte_offset: usize) -> Option<&TextParagraph> {
        self.visible_paragraphs().iter().take_while(|p| p.range.start <= byte_offset).last()
    }

    pub(super) fn paragraph_by_y(&self, y: PhysicalLength) -> Option<&TextParagraph> {
        // Positions on lines dropped by `max-lines` (within the cut paragraph, when the item is
        // taller than the visible text) don't hit-test: nothing is rendered there.
        if self.below_line_limit(y) {
            return None;
        }

        // Adjust for vertical alignment
        let y = y - self.y_offset;

        if y < PhysicalLength::zero() {
            return self.visible_paragraphs().first();
        }

        let idx = self.visible_paragraphs().binary_search_by(|paragraph| {
            if y < paragraph.y {
                core::cmp::Ordering::Greater
            } else if y >= paragraph.y + PhysicalLength::new(paragraph.layout.height()) {
                core::cmp::Ordering::Less
            } else {
                core::cmp::Ordering::Equal
            }
        });

        match idx {
            Ok(i) => self.visible_paragraphs().get(i),
            Err(_) => self.visible_paragraphs().last(),
        }
    }

    pub(super) fn byte_offset_from_point(&self, pos: PhysicalPoint) -> usize {
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

    pub(super) fn cursor_rect_for_byte_offset(
        &self,
        byte_offset: usize,
        cursor_width: PhysicalLength,
    ) -> PhysicalRect {
        let Some(paragraph) = self.paragraph_by_byte_offset(byte_offset) else {
            return PhysicalRect::new(PhysicalPoint::default(), PhysicalSize::new(1.0, 1.0));
        };

        let local_offset = (byte_offset - paragraph.range.start).min(paragraph.range.len());
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
    /// plus an optional ellipsis glyph with its font and size to be drawn separately.
    /// Call this function only for the last line of the layout.
    pub(super) fn glyphs_with_elision<'a>(
        &'a self,
        glyph_run: &'a parley::layout::GlyphRun<Brush>,
        // When set, place an ellipsis even if the run fits the width. Used when lines below were
        // dropped for the height, so the last visible line signals the vertical truncation.
        force_elision: bool,
        // Advance width of the line's trailing whitespace. A vertically truncated line that fits
        // the width anchors the appended ellipsis after the last non-whitespace glyph, so trailing
        // spaces (e.g. left at a word-wrap break) don't push it away from the text.
        trailing_whitespace: f32,
    ) -> (
        impl Iterator<Item = parley::layout::Glyph> + Clone + 'a,
        Option<(parley::layout::Glyph, parley::FontData, PhysicalLength)>,
    ) {
        let ellipsis_advance =
            self.elision_info.as_ref().map(|info| info.ellipsis_glyph.advance).unwrap_or(0.0);
        let max_width = self
            .elision_info
            .as_ref()
            .map(|info| info.max_physical_width)
            .unwrap_or(PhysicalLength::new(f32::MAX));

        let run_start = PhysicalLength::new(glyph_run.offset());
        let run_end = PhysicalLength::new(glyph_run.offset() + glyph_run.advance());

        // Run starts after where the ellipsis would go - skip entirely
        let run_beyond_elision = run_start > max_width;
        // Run extends beyond max width (or the lines below it were dropped) and needs an ellipsis
        let needs_elision = !run_beyond_elision
            && (force_elision || run_end.get().floor() > max_width.get().ceil());

        let truncated_glyphs = glyph_run.positioned_glyphs().take_while(move |glyph| {
            !run_beyond_elision
                && (!needs_elision
                    || PhysicalLength::new(glyph.x + glyph.advance + ellipsis_advance) <= max_width)
        });

        let ellipsis = if needs_elision {
            self.elision_info.as_ref().map(|info| {
                let ellipsis_x = glyph_run
                    .positioned_glyphs()
                    .find(|glyph| {
                        PhysicalLength::new(glyph.x + glyph.advance + info.ellipsis_glyph.advance)
                            > info.max_physical_width
                    })
                    .map(|g| g.x)
                    // Nothing overflows horizontally (force_elision): put the ellipsis right after
                    // the run's last non-whitespace glyph, i.e. before any trailing whitespace.
                    .unwrap_or(run_end.get() - trailing_whitespace);

                let mut ellipsis_glyph = info.ellipsis_glyph;
                ellipsis_glyph.x = ellipsis_x;
                // The ellipsis glyph comes from a standalone layout; place it on this run's
                // baseline so it lands on the right line (not just the first one).
                ellipsis_glyph.y = glyph_run.baseline();

                let font_size = PhysicalLength::new(glyph_run.run().font_size());
                (ellipsis_glyph, info.font_for_ellipsis_glyph.clone(), font_size)
            })
        } else {
            None
        };

        (truncated_glyphs, ellipsis)
    }
}
