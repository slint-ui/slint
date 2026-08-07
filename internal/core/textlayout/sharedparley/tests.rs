// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore bidi

use super::shaping::paragraph_ranges;
use super::*;

fn paragraphs(text: &str) -> Vec<&str> {
    paragraph_ranges(text).map(|r| &text[r]).collect()
}

fn layout_text_with_options(text: &str, options: LayoutOptions) -> Layout {
    // Don't load system fonts: that goes through fontconfig FFI, which Miri
    // can't execute. Use the bundled Inter font instead.
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
    let builder = super::shaping::plain_builder_for_tests();
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
    assert_eq!(
        layout.cursor_rect_for_byte_offset(6, cursor_width),
        layout.cursor_rect_for_byte_offset(5, cursor_width)
    );
    assert_ne!(
        layout.cursor_rect_for_byte_offset(6, cursor_width),
        layout.cursor_rect_for_byte_offset(0, cursor_width)
    );
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
