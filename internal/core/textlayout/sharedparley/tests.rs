// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore bidi

use super::shaping::paragraph_ranges;
use super::*;

fn paragraphs(text: &str) -> Vec<&str> {
    paragraph_ranges(text).map(|r| &text[r]).collect()
}

fn layout_text_with_options(text: &str, options: LayoutOptions) -> Layout {
    layout_text_with_builder(text, super::shaping::plain_builder_for_tests(), options)
}

// Don't load system fonts: that goes through fontconfig FFI, which Miri
// can't execute. Use the bundled Inter font instead.
fn test_font_context() -> parley::FontContext {
    let mut font_ctx = parley::FontContext {
        collection: fontique::Collection::new(fontique::CollectionOptions {
            system_fonts: false,
            ..Default::default()
        }),
        source_cache: Default::default(),
    };
    let data = include_bytes!("../../../common/sharedfontique/Inter-VariableFont.ttf");
    let families = font_ctx.collection.register_fonts(fontique::Blob::new(Arc::new(data)), None);
    font_ctx.collection.set_generic_families(
        fontique::GenericFamily::SansSerif,
        families.iter().map(|(id, _)| *id),
    );
    font_ctx
}

fn layout_text_with_builder(
    text: &str,
    builder: super::shaping::LayoutWithoutLineBreaksBuilder,
    options: LayoutOptions,
) -> Layout {
    let mut font_ctx = test_font_context();
    let paragraphs = create_text_paragraphs(
        &builder,
        &mut font_ctx,
        PlainOrStyledText::Plain(text.into()),
        Color::default(),
    );
    layout(&builder, &mut font_ctx, paragraphs, ScaleFactor::new(1.0), options, None)
}

fn layout_text(text: &str) -> Layout {
    layout_text_with_options(text, LayoutOptions::default())
}

fn visual_line_count(text: &str) -> usize {
    layout_text(text).paragraphs.iter().map(|p| p.layout.lines().len()).sum()
}

#[test]
fn bidi_selection_spans_are_ascending_in_x() {
    // The segment walk in `draw_glyph_run_with_selection` steps through a line's spans left to
    // right, so they have to arrive ascending in x. A bidi line is where that could plausibly
    // break: a logically contiguous selection reaching into an RTL run maps to several
    // disjoint visual rects, and the second one can be the *leftmost* on screen.
    let text = "abc\u{5d0}\u{5d1}\u{5d2}def";
    let layout = layout_text(text);
    let mut saw_line_with_several_spans = false;
    for start in [0, 2, 3, 4] {
        for end in [4, 6, 8, text.len()] {
            if start >= end {
                continue;
            }
            let spans = layout.selection_geometry(
                start..end,
                &(PhysicalLength::new(f32::NEG_INFINITY)..PhysicalLength::new(f32::INFINITY)),
            );
            saw_line_with_several_spans |= spans.0.len() > 1;
            for pair in spans.0.windows(2) {
                let (left, right) = (&pair[0], &pair[1]);
                if (left.paragraph, left.line) != (right.paragraph, right.line) {
                    continue;
                }
                assert!(
                    left.x().start <= right.x().start,
                    "spans of {start}..{end} are out of order: {:?} before {:?}",
                    left.x(),
                    right.x()
                );
            }
        }
    }
    // Otherwise the ranges above stopped producing a split line and this proves nothing.
    assert!(saw_line_with_several_spans, "expected a bidi selection to split into spans");
}

#[test]
fn test_text_line_height_matches_shaped_single_line() {
    for (pixel_size, line_height_factor) in [(12.0, None), (25.5, None), (12.0, Some(1.5))] {
        let font_request = FontRequest {
            pixel_size: Some(LogicalLength::new(pixel_size)),
            line_height_factor,
            ..Default::default()
        };
        let builder = super::shaping::LayoutWithoutLineBreaksBuilder::new(
            Some(font_request.clone()),
            TextWrap::NoWrap,
            None,
            ScaleFactor::new(1.0),
        );
        let shaped = layout_text_with_builder("Hello world", builder, LayoutOptions::default());
        let fast = text_line_height(&mut test_font_context(), &font_request).unwrap();
        assert!(
            (shaped.height.get() - fast.get()).abs() < 0.01,
            "shaped {} != estimated {} (size {pixel_size}, factor {line_height_factor:?})",
            shaped.height.get(),
            fast.get(),
        );
    }
}

#[test]
fn test_crlf_line_count() {
    assert_eq!(visual_line_count("hello\r\nworld"), visual_line_count("hello\nworld"));
    assert_eq!(visual_line_count("hello\r\nworld"), 2);
}

#[test]
fn test_cursor_between_cr_and_lf() {
    // The cursor can land between the '\r' and the '\n' (e.g. moving left from the start of
    // the next line); it draws at the end of the preceding paragraph, like on the '\r'.
    let layout = layout_text("hello\r\nworld");
    let cursor_width = PhysicalLength::new(1.0);
    let downstream = crate::items::TextCursorAffinity::NextCharacter;
    assert_eq!(
        layout.cursor_rect_for_byte_offset(6, downstream, cursor_width),
        layout.cursor_rect_for_byte_offset(5, downstream, cursor_width)
    );
    assert_ne!(
        layout.cursor_rect_for_byte_offset(6, downstream, cursor_width),
        layout.cursor_rect_for_byte_offset(0, downstream, cursor_width)
    );
}

#[test]
fn test_cursor_affinity_at_soft_line_break() {
    use crate::items::TextCursorAffinity;

    // Wraps before "wrapping", whose offset is both the end of line 1 and the start of line 2.
    let text = "When the amount of lines - due to wrapping and number of paragraphs";
    let layout = layout_text_with_builder(
        text,
        super::shaping::wrap_builder_for_tests(),
        LayoutOptions { max_width: Some(LogicalLength::new(200.)), ..Default::default() },
    );
    let break_offset = text.find("wrapping").unwrap();
    let line_of = |y: f32| {
        (y / layout.paragraphs[0].layout.lines().next().unwrap().metrics().line_height) as i32
    };

    // Hit-testing past the end of line 1 reports the break offset, upstream.
    let (offset, affinity) = layout.byte_offset_from_point(PhysicalPoint::new(1000., 1.));
    assert_eq!(offset, break_offset);
    assert_eq!(affinity, TextCursorAffinity::PreviousCharacter);

    // The two affinities of that offset resolve to its two visual positions.
    let cursor_width = PhysicalLength::new(1.0);
    let upstream = layout.cursor_rect_for_byte_offset(
        break_offset,
        TextCursorAffinity::PreviousCharacter,
        cursor_width,
    );
    let downstream = layout.cursor_rect_for_byte_offset(
        break_offset,
        TextCursorAffinity::NextCharacter,
        cursor_width,
    );
    assert_eq!(line_of(upstream.origin.y), 0);
    assert_eq!(line_of(downstream.origin.y), 1);
    assert!(upstream.origin.x > downstream.origin.x);
}

#[test]
fn test_paragraph_ranges() {
    assert_eq!(paragraphs(""), [""]);
    assert_eq!(paragraphs("hello"), ["hello"]);
    assert_eq!(paragraphs("hello\nworld"), ["hello", "world"]);
    assert_eq!(paragraphs("hello\n"), ["hello", ""]);
    assert_eq!(paragraphs("\n\n"), ["", "", ""]);
}

#[test]
fn test_paragraph_ranges_crlf() {
    assert_eq!(paragraphs("hello\r\nworld"), ["hello", "world"]);
    assert_eq!(paragraphs("hello\r\n"), ["hello", ""]);
    assert_eq!(paragraphs("\r\n\r\n"), ["", "", ""]);
    assert_eq!(paragraphs("a\r\n\nb"), ["a", "", "b"]);
    // A lone CR stays in the paragraph; parley breaks the line there.
    assert_eq!(paragraphs("hello\rworld"), ["hello\rworld"]);
}

fn layout_with_max_lines(text: &str, max_lines: usize) -> Layout {
    layout_text_with_options(
        text,
        LayoutOptions { max_lines: Some(max_lines), ..LayoutOptions::default() },
    )
}

#[test]
fn test_max_lines_cut_across_paragraphs() {
    // Three paragraphs with one line each; the limit lands on the paragraph boundary.
    let layout = layout_with_max_lines("a\nb\nc", 2);
    assert_eq!(layout.line_limit_cut, Some((1, 0)));
    assert_eq!(layout.visible_paragraphs().len(), 2);

    // Empty paragraphs still synthesize a line that counts towards the limit.
    let layout = layout_with_max_lines("a\n\nb", 2);
    assert_eq!(layout.line_limit_cut, Some((1, 0)));
}

#[test]
fn test_max_lines_cut_within_paragraph() {
    // A lone CR breaks lines within a single paragraph, so the limit lands mid-paragraph.
    let layout = layout_with_max_lines("a\rb\rc", 2);
    assert_eq!(layout.line_limit_cut, Some((0, 1)));
    assert_eq!(layout.visible_paragraphs().len(), 1);
}

#[test]
fn test_max_lines_no_cut_when_all_lines_fit() {
    // The limit only cuts when lines are actually dropped, and layout results (notably the
    // height) are unchanged when it doesn't.
    let unlimited = layout_text("a\nb\nc");
    for max_lines in [3, 4] {
        let layout = layout_with_max_lines("a\nb\nc", max_lines);
        assert_eq!(layout.line_limit_cut, None);
        assert_eq!(layout.visible_paragraphs().len(), 3);
        assert_eq!(layout.height, unlimited.height);
    }
}

#[test]
fn test_max_lines_caps_preferred_width() {
    // The cut lands mid-paragraph (a lone CR breaks lines within one paragraph); the
    // dropped, longer line must not count towards the preferred width, so the layout is
    // exactly as wide as the kept line alone.
    let limited = layout_with_max_lines("ab\rlonger", 1);
    assert_eq!(limited.line_limit_cut, Some((0, 0)));
    assert!(limited.max_width < layout_text("ab\rlonger").max_width);
    assert_eq!(limited.max_width, layout_text("ab").max_width);

    // The per-line width formula used for the cut paragraph mirrors parley's `full_width`;
    // pin the equivalence so a change in parley's formula doesn't silently diverge.
    let unlimited = layout_text("ab\rlonger");
    let per_line_max = unlimited.paragraphs[0]
        .layout
        .lines()
        .map(|line| {
            let metrics = line.metrics();
            metrics.inline_min_coord + metrics.advance
        })
        .fold(0.0f32, f32::max);
    assert_eq!(per_line_max, unlimited.paragraphs[0].layout.full_width());
}

#[test]
fn test_max_lines_below_line_limit() {
    let limited = layout_with_max_lines("a\nb\nc", 2);
    // Within the visible text: hit-testing stays active.
    assert!(!limited.below_line_limit(PhysicalLength::zero()));
    assert!(!limited.below_line_limit(limited.height - PhysicalLength::new(1.0)));
    // At and below the bottom of the last kept line: dropped-line territory.
    assert!(limited.below_line_limit(limited.height));
    assert!(limited.below_line_limit(limited.height + PhysicalLength::new(100.0)));

    // Without an active cut nothing is below the limit, no matter the y.
    let unlimited = layout_text("a\nb\nc");
    assert!(!unlimited.below_line_limit(unlimited.height + PhysicalLength::new(100.0)));

    // paragraph_by_y honors the guard, so no hit-testing consumer sees dropped lines.
    assert!(limited.paragraph_by_y(limited.height).is_none());
    assert!(limited.paragraph_by_y(PhysicalLength::zero()).is_some());
}

#[test]
fn test_max_lines_caps_height() {
    let unlimited = layout_text("a\nb\nc");
    let limited = layout_with_max_lines("a\nb\nc", 1);
    assert!(limited.height < unlimited.height);
    assert!(limited.height > PhysicalLength::zero());
    // The capped height matches the bottom of the last kept line.
    let first_line_bottom = PhysicalLength::new(
        limited.paragraphs[0].layout.lines().next().unwrap().metrics().block_max_coord,
    );
    assert_eq!(limited.height, first_line_bottom);
}

/// Issue #6739: a right-aligned box whose right edge is pinned via something like
/// `x: parent.width - self.width` has `origin + max_width` exactly constant regardless of
/// `max_width`'s fractional part. Renderers that round their own screen position to the
/// device-pixel grid before drawing text (see `GlyphRenderer::text_origin_snap_delta`) apply that
/// same, unrounded delta to every edge of the box, including the pinned one; `x_offset` cancels it
/// there, using the *actual* delta the origin-snap applied -- not a guess inferred from the box's
/// own width, which issue #6739's review found to be wrong whenever the origin wasn't itself
/// derived from that width (see `test_pixel_snap_alignment_zero_delta_leaves_content_exact` below).
///
/// Returns `(x_offset, unaligned_line_offset)`: the correction consumers add on top of what
/// parley computed (see `Layout::x_offset`), and parley's own per-line offset from `align()` --
/// which must always come from the real, unrounded width (see `pixel_snap_correction`'s doc), so
/// neither a fractional `max_width` nor `origin_snap_delta` ever changes what line breaking or
/// elision see.
fn right_align_offsets(width: f32, origin_snap_delta_x: f32) -> (f32, f32) {
    let layout = layout_text_with_options(
        "000",
        LayoutOptions {
            max_width: Some(LogicalLength::new(width)),
            horizontal_align: TextHorizontalAlignment::Right,
            origin_snap_delta: PhysicalPoint::new(origin_snap_delta_x, 0.0),
            ..LayoutOptions::default()
        },
    );
    (layout.x_offset.get(), layout.paragraphs[0].layout.lines().next().unwrap().metrics().offset)
}

#[test]
fn test_pixel_snap_alignment_cancels_the_origin_snap_delta() {
    // `Right` alignment (fraction 1.0) must cancel the delta exactly, regardless of the box's own
    // (fractional or not) width: `x_offset` always comes out to `-origin_snap_delta`.
    for delta in [-0.4, -0.1, 0.0, 0.1, 0.3, 0.49] {
        assert_eq!(right_align_offsets(30.25, delta).0, -delta, "delta {delta}");
        assert_eq!(right_align_offsets(30.0, delta).0, -delta, "delta {delta}");
    }

    // Whatever the correction does, parley's own per-line offset -- what line breaking and
    // elision also see -- must never itself depend on the delta: it comes from the real,
    // unrounded width either way.
    assert_eq!(right_align_offsets(30.25, 0.0).1, right_align_offsets(30.25, 0.3).1);
    assert_eq!(right_align_offsets(30.49, 0.0).1, right_align_offsets(30.49, -0.2).1);
}

/// Issue #6739's review: a zero delta -- no draw call snapped this item's origin at all, or one
/// did but the origin was already exactly on a device pixel -- must leave content exactly where
/// its real, unrounded width puts it, no matter how fractional that width is. This is the
/// counter-example that broke the original (width-rounding) formula: `x: 0` (an origin that is
/// always integral, independent of width) with a fractional physical width like `30.25` must not
/// spuriously shift content just because `30.25` itself isn't a whole number.
#[test]
fn test_pixel_snap_alignment_zero_delta_leaves_content_exact() {
    assert_eq!(right_align_offsets(30.25, 0.0).0, 0.0);
    assert_eq!(right_align_offsets(30.49, 0.0).0, 0.0);
    assert_eq!(right_align_offsets(30.0, 0.0).0, 0.0);
}

#[test]
fn test_pixel_snap_alignment_fraction_by_horizontal_alignment() {
    // `Left`/`Start`: that edge *is* the origin, so letting the origin-snap move it is the whole
    // point -- no correction. `Right`/`End`: the far edge must not move at all -- the full delta,
    // negated, cancels it. `Center`: half of that, splitting the difference between the two edges.
    let delta = 0.3f32;
    let x_offset_for = |horizontal_align: TextHorizontalAlignment| {
        layout_text_with_options(
            "000",
            LayoutOptions {
                max_width: Some(LogicalLength::new(30.25)),
                horizontal_align,
                origin_snap_delta: PhysicalPoint::new(delta, 0.0),
                ..LayoutOptions::default()
            },
        )
        .x_offset
        .get()
    };
    assert_eq!(x_offset_for(TextHorizontalAlignment::Left), 0.0);
    assert_eq!(x_offset_for(TextHorizontalAlignment::Start), 0.0);
    assert_eq!(x_offset_for(TextHorizontalAlignment::Right), -delta);
    assert_eq!(x_offset_for(TextHorizontalAlignment::End), -delta);
    assert_eq!(x_offset_for(TextHorizontalAlignment::Center), -delta * 0.5);
}

/// Issue #6739's review: the pixel-snap correction must never reach line breaking. A word whose
/// advance sits strictly between a fractional `max_width` and its rounded neighbor has to keep
/// fitting (or not) exactly as it would without any snapping -- wrapping it (or eliding it) just
/// because an edge needed correcting would trade the pixel-alignment bug for a content-fit one.
///
/// Rather than aim for one specific width where a word's advance happens to straddle a rounding
/// boundary (which would depend on this test's font's exact metrics), this sweeps every width in
/// a wide range at a fine enough step that some of them are guaranteed to land there, and checks
/// that line breaking never once differs between a zero and a nonzero (and, incidentally,
/// width-independent) origin-snap delta.
#[test]
fn test_pixel_snap_alignment_never_moves_the_wrap_boundary() {
    let text = "The quick brown fox jumps over the lazy dog and then goes home again";
    let line_count = |layout: &Layout| layout.paragraphs[0].layout.lines().len();
    let line_range = |layout: &Layout, index: usize| {
        layout.paragraphs[0].layout.lines().nth(index).unwrap().text_range()
    };
    let layout_at = |width: f32, origin_snap_delta_x: f32| {
        layout_text_with_builder(
            text,
            super::shaping::wrap_builder_for_tests(),
            LayoutOptions {
                max_width: Some(LogicalLength::new(width)),
                horizontal_align: TextHorizontalAlignment::Right,
                origin_snap_delta: PhysicalPoint::new(origin_snap_delta_x, 0.0),
                ..LayoutOptions::default()
            },
        )
    };

    let mut saw_a_snap_correction = false;
    let mut width = 20.0f32;
    while width < 220.0 {
        let unsnapped = layout_at(width, 0.0);
        let snapped = layout_at(width, 0.35);
        saw_a_snap_correction |= snapped.x_offset.get() != 0.0;

        assert_eq!(
            line_count(&unsnapped),
            line_count(&snapped),
            "line count differs at width {width}"
        );
        for i in 0..line_count(&unsnapped) {
            assert_eq!(
                line_range(&unsnapped, i),
                line_range(&snapped, i),
                "line {i} differs at width {width}"
            );
        }

        width += 0.1;
    }
    // The correction was real throughout the sweep (otherwise this test exercised nothing):
    // confirm the snap actually did something, just never to where lines break.
    assert!(saw_a_snap_correction);
}

/// `x_offset` is added on the way out of the layout (glyphs, cursor rects, selection spans, ...)
/// and subtracted on the way back in (`byte_offset_from_point`, hit-testing). Pin the round trip:
/// with snapping on and a fractional width -- so the correction is actually nonzero -- placing a
/// point at a cursor's own reported x has to hit that same offset back, or the two additions above
/// have their signs flipped relative to each other and query paths would disagree with drawing in
/// a way none of the other tests here would catch (they don't exercise both directions at once).
#[test]
fn test_pixel_snap_alignment_x_offset_round_trips_through_hit_testing() {
    let width = 30.25;
    let layout = layout_text_with_options(
        "hello",
        LayoutOptions {
            max_width: Some(LogicalLength::new(width)),
            horizontal_align: TextHorizontalAlignment::Right,
            origin_snap_delta: PhysicalPoint::new(0.3, 0.0),
            ..LayoutOptions::default()
        },
    );
    // Sanity: this width actually produces a nonzero correction, or the round trip below would
    // pass vacuously even with a flipped sign.
    assert_ne!(layout.x_offset.get(), 0.0);

    let cursor_width = PhysicalLength::new(1.0);
    for byte_offset in [0, 2, 5] {
        let affinity = crate::items::TextCursorAffinity::NextCharacter;
        let rect = layout.cursor_rect_for_byte_offset(byte_offset, affinity, cursor_width);
        let (hit_offset, _) =
            layout.byte_offset_from_point(PhysicalPoint::new(rect.origin.x, rect.origin.y));
        assert_eq!(hit_offset, byte_offset, "round trip failed for byte offset {byte_offset}");
    }
}
