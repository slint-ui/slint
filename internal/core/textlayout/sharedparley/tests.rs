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
pub(super) fn test_font_context() -> FontContext {
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
    FontContext::new(font_ctx)
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
fn styled_text_across_a_line_break_shapes() {
    // The shaper slices the text with the spans it is given, so a span that starts inside a
    // character panics there (#13548). The style opens at byte 5 of the first paragraph, which
    // in the second one falls inside its third character.
    let text = crate::styled_text::StyledText::from_markdown(
        "\u{44f}\u{44f} **a\n\u{44f}\u{44f}\u{44f}**",
    )
    .unwrap();
    let paragraphs = create_text_paragraphs(
        &super::shaping::plain_builder_for_tests(),
        &mut test_font_context(),
        PlainOrStyledText::Styled(text),
        Color::default(),
    );
    assert_eq!(paragraphs.len(), 2);
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
                f32::round,
            );
            saw_line_with_several_spans |= spans.0.len() > 1;
            for [left, right] in spans.0.array_windows() {
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

fn layout_markdown_with_builder(
    markdown: &str,
    builder: super::shaping::LayoutWithoutLineBreaksBuilder,
    options: LayoutOptions,
) -> Layout {
    let mut font_ctx = test_font_context();
    let text = crate::styled_text::StyledText::from_markdown(markdown).unwrap();
    let paragraphs = create_text_paragraphs(
        &builder,
        &mut font_ctx,
        PlainOrStyledText::Styled(text),
        Color::default(),
    );
    let scale_factor = builder.scale_factor;
    layout(&builder, &mut font_ctx, paragraphs, scale_factor, options, None)
}

fn layout_markdown(markdown: &str, options: LayoutOptions) -> Layout {
    layout_markdown_with_builder(markdown, super::shaping::wrap_builder_for_tests(), options)
}

const QUOTED_TEXT: &str = "> The quick brown fox jumps over the lazy dog again and again";

fn quote_step(scale_factor: f32) -> f32 {
    (DEFAULT_FONT_SIZE.get() * scale_factor * super::shaping::QUOTE_INDENT_EM).round()
}

#[test]
fn quote_paragraph_x_per_depth() {
    let layout = layout_markdown("a\n\n> b\n>> c", LayoutOptions::default());
    let xs: Vec<f32> = layout.paragraphs.iter().map(|p| p.x().get()).collect();
    assert_eq!(xs, [0.0, quote_step(1.0), 2.0 * quote_step(1.0)]);

    let builder = super::shaping::LayoutWithoutLineBreaksBuilder::new(
        None,
        TextWrap::WordWrap,
        None,
        ScaleFactor::new(2.0),
    );
    let layout = layout_markdown_with_builder("> b", builder, LayoutOptions::default());
    assert_eq!(layout.paragraphs[0].x().get(), quote_step(2.0));
}

#[test]
fn quote_wraps_within_reduced_width() {
    let max_width = LogicalLength::new(150.);
    let options = || LayoutOptions { max_width: Some(max_width), ..LayoutOptions::default() };
    let quoted = layout_markdown(QUOTED_TEXT, options());
    let plain = layout_markdown(&QUOTED_TEXT[2..], options());

    let paragraph = &quoted.paragraphs[0];
    for line in paragraph.layout.lines() {
        let metrics = line.metrics();
        let right = paragraph.x().get() + metrics.inline_min_coord + metrics.advance
            - metrics.hanging_advance;
        assert!(right <= max_width.get() + 0.5, "line ends at {right}");
    }
    assert!(paragraph.layout.lines().len() >= plain.paragraphs[0].layout.lines().len());
}

#[test]
fn quote_indent_counts_toward_max_width() {
    let builder = super::shaping::plain_builder_for_tests;
    let quoted = layout_markdown_with_builder("> word", builder(), LayoutOptions::default());
    let plain = layout_markdown_with_builder("word", builder(), LayoutOptions::default());
    assert_eq!(quoted.max_width.get(), plain.max_width.get() + quote_step(1.0));
}

#[test]
fn link_hit_test_respects_quote_indent() {
    let layout = layout_markdown("> [link](u)", LayoutOptions::default());
    let paragraph = &layout.paragraphs[0];
    let y = paragraph.y.get() + paragraph.layout.height() / 2.;
    assert_eq!(link_in_layout(&layout, PhysicalPoint::new(1., y)), None);
    assert_eq!(
        link_in_layout(&layout, PhysicalPoint::new(paragraph.x().get() + 2., y)).as_deref(),
        Some("u")
    );
}

#[test]
fn elision_in_quote_respects_indent() {
    let max_width = LogicalLength::new(100.);
    let options = LayoutOptions {
        max_width: Some(max_width),
        text_overflow: TextOverflow::Elide,
        ..LayoutOptions::default()
    };
    let layout = layout_markdown_with_builder(
        QUOTED_TEXT,
        super::shaping::plain_builder_for_tests(),
        options,
    );
    let paragraph = &layout.paragraphs[0];
    let x = paragraph.x();
    let line = paragraph.layout.lines().next().unwrap();
    let mut elided = false;
    for item in line.items() {
        let parley::PositionedLayoutItem::GlyphRun(run) = item else { continue };
        let (glyphs, ellipsis) = layout.glyphs_with_elision(&run, false, 0., x);
        let Some((ellipsis, ..)) = ellipsis else { continue };
        elided = true;
        let kept_end = glyphs.last().map_or(0., |glyph| glyph.x + glyph.advance);
        assert!(kept_end + ellipsis.advance <= (max_width.get() - x.get()) + 0.5);
    }
    assert!(elided);
}

#[test]
fn quote_bars_cover_only_drawn_lines() {
    let options =
        LayoutOptions { max_width: Some(LogicalLength::new(150.)), ..LayoutOptions::default() };
    let layout = layout_markdown(QUOTED_TEXT, options);
    let paragraph = &layout.paragraphs[0];
    let lines: Vec<_> = paragraph.layout.lines().map(|line| *line.metrics()).collect();
    assert!(lines.len() >= 3);
    let last = lines.len() - 1;
    let para_y = layout.y_offset + paragraph.y;
    let everything = PhysicalLength::new(f32::MIN)..PhysicalLength::new(f32::MAX);

    assert_eq!(
        paragraph.quote_bar_extent(&layout, para_y, &everything, last),
        Some(lines[0].block_min_coord.round()..lines[last].block_max_coord.round())
    );

    let far_below = PhysicalLength::new(10_000.)..PhysicalLength::new(20_000.);
    assert_eq!(paragraph.quote_bar_extent(&layout, para_y, &far_below, last), None);
}

#[test]
fn quote_bars_stop_at_max_lines_cut() {
    let options = LayoutOptions {
        max_width: Some(LogicalLength::new(150.)),
        max_lines: Some(2),
        ..LayoutOptions::default()
    };
    let layout = layout_markdown(QUOTED_TEXT, options);
    let cut = layout.visible_extent().unwrap();
    assert_eq!((cut.last_paragraph, cut.last_line), (0, 1));
    let paragraph = &layout.paragraphs[0];
    let line = *paragraph.layout.lines().nth(1).unwrap().metrics();
    let everything = PhysicalLength::new(f32::MIN)..PhysicalLength::new(f32::MAX);
    let extent = paragraph
        .quote_bar_extent(&layout, layout.y_offset + paragraph.y, &everything, cut.last_line)
        .unwrap();
    assert_eq!(extent.end, line.block_max_coord.round());
}
