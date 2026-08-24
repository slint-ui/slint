# Text Layout System

> Note for AI coding assistants (agents):
> **When to load this document:** Working on `internal/core/textlayout.rs`,
> `internal/core/textlayout/`, `internal/core/styled_text.rs`,
> text rendering, line breaking, or font handling.
> For general build commands and project structure, see `/AGENTS.md`.

## Overview

Slint's text layout system handles the complex process of converting text strings into positioned glyphs for rendering. It supports:

- **Text shaping**: Converting characters to glyphs with proper metrics
- **Script-aware boundaries**: Splitting text by Unicode script for font selection
- **Line breaking**: Unicode-compliant line break algorithm
- **Text wrapping**: Word wrap, character wrap, and no wrap modes
- **Line height**: Natural font metrics or an explicit line height
- **Text overflow**: Clipping and elision (ellipsis)
- **Styled text**: Markdown parsing with formatting spans

## Key Files

| File | Purpose |
|------|---------|
| `internal/core/textlayout.rs` | Main layout algorithms, TextParagraphLayout |
| `internal/core/textlayout/shaping.rs` | TextShaper trait, Glyph, ShapeBuffer |
| `internal/core/textlayout/linebreaker.rs` | TextLineBreaker, TextLine |
| `internal/core/textlayout/fragments.rs` | TextFragment, fragment iteration |
| `internal/core/textlayout/glyphclusters.rs` | Glyph cluster grouping |
| `internal/core/textlayout/linebreak_simple.rs` | ASCII line break fallback |
| `internal/core/textlayout/sharedparley.rs` | Parley-based layout (`shared-parley` feature) |
| `internal/core/textlayout/linebreak_unicode.rs` | Unicode line break algorithm |
| `internal/core/styled_text.rs` | Public `StyledText` API, FFI |
| `internal/common/styled_text.rs` | Markdown/HTML parsing, `Style`/`FormattedSpan`/`StyledTextParagraph` |

## Text Layout Pipeline

```
Input Text
    │
    ▼
┌─────────────────────────────┐
│ 1. Script Boundary Detection│  ShapeBoundaries
│    Split by Unicode script  │  (e.g., Latin vs Arabic)
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│ 2. Text Shaping             │  TextShaper::shape_text()
│    Characters → Glyphs      │  (rustybuzz, platform shaper)
│    Apply letter spacing     │
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│ 3. Glyph Clustering         │  GlyphClusterIterator
│    Group glyphs by source   │  (combining chars, ligatures)
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│ 4. Fragment Creation        │  TextFragmentIterator
│    Group clusters between   │  LineBreakIterator
│    break opportunities      │
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│ 5. Line Breaking            │  TextLineBreaker
│    Fit fragments to width   │  WordWrap/CharWrap/NoWrap
│    Handle elision           │
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│ 6. Paragraph Layout         │  TextParagraphLayout
│    Vertical/horizontal      │  layout_lines()
│    alignment, selection     │
└─────────────────────────────┘
```

## Core Types

### Glyph

A single shaped glyph: its advance, its x/y offsets, the font-specific glyph id, and the byte
offset it came from in the source string. Generic over the font's `Length`.
See `Glyph` in `internal/core/textlayout/shaping.rs`.

### TextShaper Trait

What a font backend implements: shape a string into glyphs, and produce the glyph for a single
character (used for the ellipsis). The associated `Length` and `LengthPrimitive` types let the
same code serve plain `f32` and euclid-typed lengths.
See `TextShaper` in `internal/core/textlayout/shaping.rs`.

### FontMetrics Trait

Font measurements: ascent, descent, x-height and cap-height, with `height()` defaulting to
`ascent - descent`. Descent is negative.
See `FontMetrics` in `internal/core/textlayout/shaping.rs`.

### AbstractFont

`TextShaper` and `FontMetrics` over the same `Length`. This is the bound the layout code takes.
See `AbstractFont` in `internal/core/textlayout/shaping.rs`.

### TextLayout

A font plus the paragraph-wide spacing options: an optional letter spacing and an optional line
height. See `TextLayout` in `internal/core/textlayout.rs`.

Both spacing values use the font's `Length` unit.
`None` leaves letter spacing unchanged and uses `FontMetrics::height()` for line height.
The `.slint` `line-height-factor` value is converted to an absolute value before this stage.

An explicit line height only stretches (or shrinks) the line boxes; the glyphs are centered
within each box by distributing the leading half above and half below them
(`TextLayout::half_leading()`), following the CSS model like the parley-based layout.
Cursor and selection rectangles cover the full line box, clamped so they never get smaller
than the glyph box when the leading is negative (`TextLayout::cursor_band()`).

## Script Boundary Detection

The `ShapeBoundaries` iterator splits text by Unicode script so each run can use a font that
covers it. See `ShapeBoundaries` in `internal/core/textlayout/shaping.rs`.

For example, `"Hello தோசை"` splits into `"Hello "` (Latin/Common) and `"தோசை"` (Tamil).

**Why it matters:**
- Different scripts may need different fonts
- Shaping rules differ by script (e.g., Arabic ligatures)
- Allows fallback font selection per script

## Shape Buffer

Holds the shaped glyphs plus the `TextRun`s that map each source byte range to its slice of those
glyphs. See `ShapeBuffer` and `TextRun` in `internal/core/textlayout/shaping.rs`.

Letter spacing is applied during shaping:
- Added to advance of last glyph in each grapheme cluster
- Preserves proper spacing between characters

## Line Breaking

### Line Break Opportunities

Uses the Unicode Line Break Algorithm (UAX #14) behind the `unicode-linebreak` feature, or a
simple ASCII fallback. Either way a break opportunity is `Allowed` or `Mandatory`.
See `internal/core/textlayout/linebreak_unicode.rs` and `linebreak_simple.rs`.

### Text Fragments

The units between break opportunities: a byte range, its glyph range, its width, and the trailing
whitespace kept separate from that width.
See `TextFragment` in `internal/core/textlayout/fragments.rs`.

**Whitespace handling:**
- Trailing whitespace width tracked separately
- Allows line to exceed width by trailing whitespace
- Whitespace at line end not counted for alignment

### TextLine

One laid-out line: the source byte range excluding trailing whitespace, the glyph range, and the
measured text width. The trailing whitespace width is tracked alongside, so alignment can ignore
it and `width_including_trailing_whitespace()` can add it back.
See `TextLine` in `internal/core/textlayout/linebreaker.rs`.

### TextLineBreaker

The iterator that turns fragments into lines, applying the wrap mode, the available width and any
line limit. See `TextLineBreaker` in `internal/core/textlayout/linebreaker.rs`.

**Wrap modes:**
- `TextWrap::NoWrap`: Single line, no wrapping
- `TextWrap::WordWrap`: Break at word boundaries, fallback to anywhere
- `TextWrap::CharWrap`: Break anywhere (character boundaries)

**Break anywhere fallback:**
When a word doesn't fit even on its own line, WordWrap falls back to breaking anywhere.

## Paragraph Layout

### TextParagraphLayout

The whole paragraph: the string and its `TextLayout`, the max width and height, the horizontal and
vertical alignment, the wrap and overflow modes, and the single-line and max-lines limits.
See `TextParagraphLayout` in `internal/core/textlayout.rs`.

### layout_lines()

The main entry point. It calls a callback once per line with the positioned glyphs, the line's x
and y, the `TextLine` itself, and the selected sub-range if there is one. The callback returns a
`ControlFlow`, so rendering can stop early. The return value is the baseline y.
See `TextParagraphLayout::layout_lines` in `internal/core/textlayout.rs`.

### PositionedGlyph

A glyph with its position resolved: x relative to the line, y, advance, glyph id, and source byte
offset. See `PositionedGlyph` in `internal/core/textlayout.rs`.

### Alignment

Horizontal alignment is `TextHorizontalAlignment`. `Start` and `End` resolve by text direction,
while `Left`, `Center` and `Right` are absolute. `Start` and `Left` put the line at x = 0, `End`
and `Right` at `max_width - text_width`, and `Center` halfway between.

Vertical alignment is `TextVerticalAlignment` — `Top`, `Center`, `Bottom` — applied to the
baseline against `max_height`. Both enums are in `internal/common/enums.rs`; the arithmetic is in
`TextParagraphLayout::layout_lines`.

### Text Overflow

**Clip:** Text is simply clipped at boundaries

**Elide:** an ellipsis (…) replaces the truncated text. The ellipsis glyph is measured first, and
a line is cut once the next glyph would pass `max_width` minus that width. The last visible line
is also elided when further lines exist.

## Cursor Positioning

### cursor_pos_for_byte_offset()

Takes a byte offset and returns the cursor's x and y, in
`TextParagraphLayout::cursor_pos_for_byte_offset` (`internal/core/textlayout.rs`).

### byte_offset_for_position()

Takes a position and returns the byte offset for it, in
`TextParagraphLayout::byte_offset_for_position` (`internal/core/textlayout.rs`).

**Click position logic:**
- Find line by y position
- Iterate glyphs to find x position
- If click is in left half of glyph → return glyph offset
- If click is in right half → return next glyph offset

## Styled Text

`Style`, `FormattedSpan` and `StyledTextParagraph` live in `internal/common/styled_text.rs`
(crate `i-slint-common`) and are always available. The markdown and HTML parsing in that same file
is behind the `markdown` feature. `internal/core/styled_text.rs` adds the public `StyledText` API
and its FFI on top.

### Style Types

`Style` is the formatting applied to one span: emphasis, strong, strikethrough, code, link,
underline, and a color carrying an ARGB value. See `Style` in `internal/common/styled_text.rs`;
the markdown and HTML that produce each one are listed below.

### StyledTextParagraph

A paragraph is its raw text, the `FormattedSpan`s applying a `Style` to a byte range, and the link
destinations for those ranges.
See `StyledTextParagraph` and `FormattedSpan` in `internal/common/styled_text.rs`.

### StyledText

The public API: a shared vector of paragraphs, built either with `from_plain_text()`, which does
no parsing, or `from_markdown()`, which reports failures as a `StyledTextFromMarkdownError`.
See `StyledText` in `internal/core/styled_text.rs`.

**Supported Markdown:**
- `*emphasis*` / `_emphasis_`
- `**strong**` / `__strong__`
- `~~strikethrough~~`
- `[link](url)`
- Lists (ordered and unordered)
- Soft/hard breaks

**Supported HTML:**
- `<u>underline</u>`
- `<span style="color:...">colored</span>`

## Common Patterns

### Measuring Text

```rust
let layout = TextLayout { font: &font, letter_spacing: None, line_height: None };
let (width, height) = layout.text_size(
    "Hello World",
    Some(max_width),  // None for unconstrained
    TextWrap::WordWrap,
    None,             // No line limit
);
```

### Rendering Text

```rust
let paragraph = TextParagraphLayout {
    string: text,
    layout: TextLayout { font: &font, letter_spacing: None, line_height: None },
    max_width: 200.0,
    max_height: 100.0,
    horizontal_alignment: TextHorizontalAlignment::Left,
    vertical_alignment: TextVerticalAlignment::Top,
    wrap: TextWrap::WordWrap,
    overflow: TextOverflow::Elide,
    single_line: false,
    max_lines: None,
};

paragraph.layout_lines::<()>(
    |glyphs, line_x, line_y, line, selection| {
        for glyph in glyphs {
            draw_glyph(
                glyph.glyph_id,
                line_x + glyph.x,
                line_y,
            );
        }
        ControlFlow::Continue(())
    },
    None,  // selection
).ok();
```

### Implementing TextShaper

```rust
impl TextShaper for MyFont {
    type LengthPrimitive = f32;
    type Length = f32;

    fn shape_text<G: Extend<Glyph<f32>>>(&self, text: &str, glyphs: &mut G) {
        // Use rustybuzz or platform shaper
        let buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        let output = rustybuzz::shape(&self.face, &[], buffer);

        for (info, pos) in output.glyph_infos().iter()
            .zip(output.glyph_positions())
        {
            glyphs.extend(std::iter::once(Glyph {
                glyph_id: NonZeroU16::new(info.glyph_id as u16),
                advance: pos.x_advance as f32,
                offset_x: pos.x_offset as f32,
                offset_y: pos.y_offset as f32,
                text_byte_offset: info.cluster as usize,
            }));
        }
    }

    fn glyph_for_char(&self, ch: char) -> Option<Glyph<f32>> {
        let glyph_id = self.face.glyph_index(ch)?;
        // ... build glyph
    }
}
```

## Feature Flags

| Feature | Effect |
|---------|--------|
| `unicode-linebreak` | Full Unicode line break algorithm |
| `unicode-script` | Script boundary detection for font selection |
| `shared-parley` | Parley text shaping integration |
| `std` | Markdown parsing (pulldown-cmark) |

## Debugging Tips

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| Missing glyphs | Font doesn't cover script | Check script boundaries, font fallback |
| Wrong line breaks | Unicode linebreak rules | Check BreakOpportunity detection |
| Alignment off | Trailing whitespace counted | Check width_including_trailing_whitespace |
| Elision wrong | Ellipsis width not subtracted | Check max_width_without_elision |
| Cursor position wrong | Byte vs glyph offset mismatch | Check text_byte_offset mapping |

### Inspecting Layout

```rust
// Debug line breaking
for line in TextLineBreaker::new(text, &shape_buffer, Some(width), None, wrap) {
    println!("Line: {:?} width={:?}", line.line_text(text), line.text_width);
}

// Debug fragments
for fragment in TextFragmentIterator::new(text, &shape_buffer) {
    println!("Fragment: {:?}", fragment);
}

// Debug glyphs
for glyph in &shape_buffer.glyphs {
    println!("Glyph: id={:?} advance={:?} offset={}",
             glyph.glyph_id, glyph.advance, glyph.text_byte_offset);
}
```

## Testing

```sh
# Run text layout tests
cargo test -p i-slint-core textlayout

# Run with specific test
cargo test -p i-slint-core test_elision
cargo test -p i-slint-core test_basic_line_break

# Run styled text tests
cargo test -p i-slint-core styled_text
```
