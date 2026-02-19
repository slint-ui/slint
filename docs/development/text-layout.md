# Text Layout System

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
| `internal/core/textlayout/linebreak_unicode.rs` | Unicode line break algorithm |
| `internal/core/styled_text.rs` | Markdown/HTML parsing |

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

Represents a single shaped glyph:

```rust
pub struct Glyph<Length> {
    pub advance: Length,           // Horizontal advance
    pub offset_x: Length,          // X offset from origin
    pub offset_y: Length,          // Y offset from origin
    pub glyph_id: Option<NonZeroU16>,  // Font-specific glyph ID
    pub text_byte_offset: usize,   // Byte offset in source string
}
```

### TextShaper Trait

Interface for platform-specific text shaping:

```rust
pub trait TextShaper {
    type LengthPrimitive;  // e.g., f32
    type Length;           // e.g., f32 or LogicalLength

    /// Shape text and append glyphs to storage
    fn shape_text<GlyphStorage: Extend<Glyph<Self::Length>>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    );

    /// Get glyph for a single character (e.g., ellipsis)
    fn glyph_for_char(&self, ch: char) -> Option<Glyph<Self::Length>>;

    /// Calculate max lines that fit in height
    fn max_lines(&self, max_height: Self::Length) -> usize;
}
```

### FontMetrics Trait

Font measurement interface:

```rust
pub trait FontMetrics<Length> {
    fn height(&self) -> Length { self.ascent() - self.descent() }
    fn ascent(&self) -> Length;   // Distance above baseline
    fn descent(&self) -> Length;  // Distance below baseline (negative)
    fn x_height(&self) -> Length; // Height of lowercase 'x'
    fn cap_height(&self) -> Length; // Height of capital letters
}
```

### AbstractFont

Combined trait for fonts:

```rust
pub trait AbstractFont: TextShaper + FontMetrics<<Self as TextShaper>::Length> {}
```

## Script Boundary Detection

The `ShapeBoundaries` iterator splits text by Unicode script for optimal font selection:

```rust
pub struct ShapeBoundaries<'a> {
    text: &'a str,
    chars: core::str::CharIndices<'a>,
    last_script: Option<unicode_script::Script>,
}

// Example: "Hello தோசை" splits into:
// ["Hello "] (Latin/Common)
// ["தோசை"]   (Tamil)
```

**Why it matters:**
- Different scripts may need different fonts
- Shaping rules differ by script (e.g., Arabic ligatures)
- Allows fallback font selection per script

## Shape Buffer

Holds shaped glyphs organized by text runs:

```rust
pub struct ShapeBuffer<Length> {
    pub glyphs: Vec<Glyph<Length>>,
    pub text_runs: Vec<TextRun>,
}

pub struct TextRun {
    pub byte_range: Range<usize>,   // Source text range
    pub glyph_range: Range<usize>,  // Glyphs for this run
}
```

Letter spacing is applied during shaping:
- Added to advance of last glyph in each grapheme cluster
- Preserves proper spacing between characters

## Line Breaking

### Line Break Opportunities

Uses Unicode Line Break Algorithm (UAX #14) or simple ASCII fallback:

```rust
pub enum BreakOpportunity {
    Allowed,    // Can break here (e.g., after space)
    Mandatory,  // Must break here (e.g., newline)
}
```

### Text Fragments

Fragments are units between break opportunities:

```rust
pub struct TextFragment<Length> {
    pub byte_range: Range<usize>,
    pub glyph_range: Range<usize>,
    pub width: Length,
    pub trailing_whitespace_width: Length,
    pub trailing_whitespace_bytes: usize,
    pub trailing_mandatory_break: bool,
}
```

**Whitespace handling:**
- Trailing whitespace width tracked separately
- Allows line to exceed width by trailing whitespace
- Whitespace at line end not counted for alignment

### TextLine

Represents a laid-out line:

```rust
pub struct TextLine<Length> {
    pub byte_range: Range<usize>,        // Source text (excluding trailing WS)
    pub trailing_whitespace_bytes: usize,
    pub(crate) glyph_range: Range<usize>,
    trailing_whitespace: Length,
    pub(crate) text_width: Length,
}

impl TextLine {
    pub fn width_including_trailing_whitespace(&self) -> Length;
    pub fn line_text<'a>(&self, paragraph: &'a str) -> &'a str;
    pub fn is_empty(&self) -> bool;
}
```

### TextLineBreaker

Iterator that breaks text into lines:

```rust
pub struct TextLineBreaker<'a, Font: TextShaper> {
    fragments: TextFragmentIterator<'a, Font::Length>,
    available_width: Option<Font::Length>,
    current_line: TextLine<Font::Length>,
    num_emitted_lines: usize,
    mandatory_line_break_on_next_iteration: bool,
    max_lines: Option<usize>,
    text_wrap: TextWrap,
}
```

**Wrap modes:**
- `TextWrap::NoWrap`: Single line, no wrapping
- `TextWrap::WordWrap`: Break at word boundaries, fallback to anywhere
- `TextWrap::CharWrap`: Break anywhere (character boundaries)

**Break anywhere fallback:**
When a word doesn't fit even on its own line, WordWrap falls back to breaking anywhere.

## Paragraph Layout

### TextParagraphLayout

Full paragraph layout with alignment:

```rust
pub struct TextParagraphLayout<'a, Font: AbstractFont> {
    pub string: &'a str,
    pub layout: TextLayout<'a, Font>,
    pub max_width: Font::Length,
    pub max_height: Font::Length,
    pub horizontal_alignment: TextHorizontalAlignment,
    pub vertical_alignment: TextVerticalAlignment,
    pub wrap: TextWrap,
    pub overflow: TextOverflow,
    pub single_line: bool,
}
```

### layout_lines()

Main layout function - iterates over positioned glyphs:

```rust
pub fn layout_lines<R>(
    &self,
    mut line_callback: impl FnMut(
        &mut dyn Iterator<Item = PositionedGlyph<Font::Length>>,
        Font::Length,     // line_x
        Font::Length,     // line_y
        &TextLine<Font::Length>,
        Option<Range<Font::Length>>,  // selection
    ) -> ControlFlow<R>,
    selection: Option<Range<usize>>,  // byte range
) -> Result<Font::Length, R>;  // Returns baseline_y
```

### PositionedGlyph

Final glyph with absolute position:

```rust
pub struct PositionedGlyph<Length> {
    pub x: Length,              // X position relative to line
    pub y: Length,              // Y position (usually 0)
    pub advance: Length,
    pub glyph_id: NonZeroU16,
    pub text_byte_offset: usize,
}
```

### Alignment

**Horizontal:**
- `Left`: x = 0
- `Center`: x = (max_width - text_width) / 2
- `Right`: x = max_width - text_width

**Vertical:**
- `Top`: baseline_y = 0
- `Center`: baseline_y = (max_height - text_height) / 2
- `Bottom`: baseline_y = max_height - text_height

### Text Overflow

**Clip:** Text is simply clipped at boundaries

**Elide:** Ellipsis (…) replaces truncated text:
```rust
// Elision logic:
// 1. Get ellipsis glyph width
// 2. When line width + next glyph > max_width - ellipsis_width:
//    - Replace remaining with ellipsis
// 3. Also elide last visible line when more lines exist
```

## Cursor Positioning

### cursor_pos_for_byte_offset()

Get cursor position for text offset:

```rust
pub fn cursor_pos_for_byte_offset(
    &self,
    byte_offset: usize,
) -> (Font::Length, Font::Length)  // (x, y)
```

### byte_offset_for_position()

Get text offset for click position:

```rust
pub fn byte_offset_for_position(
    &self,
    (pos_x, pos_y): (Font::Length, Font::Length),
) -> usize
```

**Click position logic:**
- Find line by y position
- Iterate glyphs to find x position
- If click is in left half of glyph → return glyph offset
- If click is in right half → return next glyph offset

## Styled Text

### Style Types

```rust
pub enum Style {
    Emphasis,       // *italic*
    Strong,         // **bold**
    Strikethrough,  // ~~strikethrough~~
    Code,           // `code`
    Link,           // [text](url)
    Underline,      // <u>underline</u>
    Color(Color),   // <span style="color:...">
}
```

### StyledTextParagraph

```rust
pub struct StyledTextParagraph {
    pub text: String,                              // Raw text
    pub formatting: Vec<FormattedSpan>,            // Style ranges
    pub links: Vec<(Range<usize>, String)>,        // Link destinations
}

pub struct FormattedSpan {
    pub range: Range<usize>,  // Byte range in text
    pub style: Style,
}
```

### StyledText

```rust
pub struct StyledText {
    pub paragraphs: SharedVector<StyledTextParagraph>,
}

impl StyledText {
    /// Parse markdown string
    pub fn parse(string: &str) -> Result<Self, StyledTextError>;
}
```

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
let layout = TextLayout { font: &font, letter_spacing: None };
let (width, height) = layout.text_size(
    "Hello World",
    Some(max_width),  // None for unconstrained
    TextWrap::WordWrap,
);
```

### Rendering Text

```rust
let paragraph = TextParagraphLayout {
    string: text,
    layout: TextLayout { font: &font, letter_spacing: None },
    max_width: 200.0,
    max_height: 100.0,
    horizontal_alignment: TextHorizontalAlignment::Left,
    vertical_alignment: TextVerticalAlignment::Top,
    wrap: TextWrap::WordWrap,
    overflow: TextOverflow::Elide,
    single_line: false,
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

    fn max_lines(&self, max_height: f32) -> usize {
        (max_height / self.height()).floor() as usize
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
