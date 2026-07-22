// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore RAII
pub use parley;
pub use parley::fontique;

use crate::item_rendering::HasFont;
use crate::{
    Color,
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
use alloc::vec::Vec;
use core::ops::Range;
use core::pin::Pin;
use euclid::num::Zero;
use i_slint_common::sharedfontique;
use skrifa::MetadataProvider as _;
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(derive_more::Deref, derive_more::DerefMut)]
pub struct FontContext {
    #[deref]
    #[deref_mut]
    pub inner: parley::FontContext,
    /// `(ptr, len)` of each `&'static [u8]` already handed to fontique, so repeat
    /// `register_static_font` calls for the same embedded font are skipped.
    registered_static_fonts: HashSet<(usize, usize)>,
}

impl FontContext {
    pub fn new(inner: parley::FontContext) -> Self {
        Self { inner, registered_static_fonts: HashSet::default() }
    }

    pub fn register_static_font(&mut self, data: &'static [u8]) {
        let key = (data.as_ptr() as usize, data.len());
        if self.registered_static_fonts.insert(key) {
            self.inner.collection.register_fonts(fontique::Blob::new(Arc::new(data)), None);
        }
    }

    pub fn clear_registered_static_fonts(&mut self) {
        self.registered_static_fonts.clear();
    }

    pub fn set_default_font_family(&mut self, family_name: &str) -> bool {
        sharedfontique::set_default_font_family(&mut self.inner.collection, family_name)
    }
}

type InnerTextLayoutCache = crate::item_rendering::ItemCache<Vec<TextParagraph>>;

/// Cache for shaped text paragraphs (before line breaking), keyed by ItemRc.
pub struct TextLayoutCache {
    inner: InnerTextLayoutCache,
    #[cfg(feature = "testing")]
    cache_miss_count: std::cell::Cell<u64>,
}

#[allow(clippy::derivable_impls)] // clippy doesn't see the feature = "testing" code
impl Default for TextLayoutCache {
    fn default() -> Self {
        Self {
            inner: Default::default(),
            #[cfg(feature = "testing")]
            cache_miss_count: std::cell::Cell::new(0),
        }
    }
}

impl TextLayoutCache {
    pub fn clear_cache_if_scale_factor_changed(&self, window: &crate::api::Window) {
        self.inner.clear_cache_if_scale_factor_changed(window);
    }
    pub fn component_destroyed(&self, component: crate::item_tree::ItemTreeRef) {
        self.inner.component_destroyed(component);
    }
    pub fn clear_all(&self) {
        self.inner.clear_all();
    }
}

#[cfg(feature = "testing")]
impl TextLayoutCache {
    pub fn cache_miss_count(&self) -> u64 {
        self.cache_miss_count.get()
    }
    pub fn reset_cache_miss_count(&self) {
        self.cache_miss_count.set(0);
    }
}

pub type PhysicalLength = euclid::Length<f32, PhysicalPx>;
pub type PhysicalRect = euclid::Rect<f32, PhysicalPx>;
type PhysicalSize = euclid::Size2D<f32, PhysicalPx>;
type PhysicalPoint = euclid::Point2D<f32, PhysicalPx>;

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

pub use super::DEFAULT_FONT_SIZE;

/// Font size of inline `code` runs, as a fraction of the surrounding body
/// text. Matches the convention used by GitHub-style markdown renderers — the
/// glyphs sit a little smaller than body text, inside a translucent capsule
/// that visually marks them as code.
const INLINE_CODE_FONT_SCALE: f32 = 0.85;

std::thread_local! {
    static LAYOUT_CONTEXT: RefCell<parley::LayoutContext<Brush>> = Default::default();
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
struct Brush {
    /// When set, this overrides the fill/stroke to use this color.
    override_fill_color: Option<Color>,
    stroke: Option<TextStrokeStyle>,
    link_color: Option<Color>,
}

#[derive(Default)]
struct LayoutOptions {
    max_width: Option<LogicalLength>,
    max_height: Option<LogicalLength>,
    /// Maximum number of visible lines across all paragraphs.
    max_lines: Option<usize>,
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
            max_lines: None,
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
        layout_ctx: &'a mut parley::LayoutContext<Brush>,
        font_ctx: &'a mut parley::FontContext,
        text: &'a str,
    ) -> parley::RangedBuilder<'a, Brush> {
        // Pin the line height to the requested font's metrics so fallback runs (e.g.
        // password chars rendered by a wider-descender symbol font) don't grow the
        // line box. FontSizeRelative makes the ratio scale with any per-span FontSize.
        let line_height_ratio = self.font_request.as_ref().and_then(|font_request| {
            let font = font_request
                .clone()
                .query_fontique(&mut font_ctx.collection, &mut font_ctx.source_cache)?;
            let face = skrifa::FontRef::from_index(font.blob.data(), font.index).ok()?;
            let location = face.axes().location(font.synthesis.variation_settings());
            let metrics = face.metrics(skrifa::instance::Size::unscaled(), &location);
            let units_per_em = metrics.units_per_em as f32;
            (units_per_em > 0.0)
                .then(|| (metrics.ascent - metrics.descent + metrics.leading) / units_per_em)
        });

        let mut builder = layout_ctx.ranged_builder(font_ctx, text, self.scale_factor.get(), false);

        if let Some(ratio) = line_height_ratio {
            builder.push_default(parley::StyleProperty::LineHeight(
                parley::style::LineHeight::FontSizeRelative(ratio),
            ));
        }

        if let Some(ref font_request) = self.font_request {
            let mut fallback_family_iter = sharedfontique::FALLBACK_FAMILIES
                .into_iter()
                .map(parley::style::FontFamilyName::Generic);

            let font_families: &[parley::style::FontFamilyName] = if let Some(family) =
                &font_request.family
            {
                let mut iter =
                    core::iter::once(parley::style::FontFamilyName::named(family.as_str()))
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

            builder.push_default(parley::style::FontFamily::List(std::borrow::Cow::Borrowed(
                font_families,
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
            TextWrap::NoWrap => parley::style::WordBreak::KeepAll,
            TextWrap::WordWrap => parley::style::WordBreak::Normal,
            TextWrap::CharWrap => parley::style::WordBreak::BreakAll,
        }));
        builder.push_default(parley::StyleProperty::OverflowWrap(match self.text_wrap {
            TextWrap::NoWrap => parley::style::OverflowWrap::Normal,
            TextWrap::WordWrap | TextWrap::CharWrap => parley::style::OverflowWrap::Anywhere,
        }));
        if self.text_wrap == TextWrap::NoWrap {
            // Parley 0.9 removed the width parameter from `Layout::align()` and instead
            // uses the `max_advance` set by `break_all_lines()` as the alignment container
            // width. To allow passing `max_physical_width` to `break_all_lines` for alignment
            // purposes without triggering actual line wrapping, we must set `TextWrapMode::NoWrap`.
            builder.push_default(parley::StyleProperty::TextWrapMode(
                parley::style::TextWrapMode::NoWrap,
            ));
        }

        builder.push_default(parley::StyleProperty::Brush(Brush {
            override_fill_color: None,
            stroke: self.stroke,
            link_color: None,
        }));

        builder
    }

    fn build(
        &self,
        font_context: &mut parley::FontContext,
        text: &str,
        selection: Option<(Range<usize>, Color)>,
        formatting: impl IntoIterator<Item = i_slint_common::styled_text::FormattedSpan>,
        link_color: Option<Color>,
    ) -> parley::Layout<Brush> {
        use i_slint_common::styled_text::Style;

        LAYOUT_CONTEXT.with_borrow_mut(|layout_ctx| {
            let mut builder = self.ranged_builder(layout_ctx, font_context, text);

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

            // filter empty ranges otherwise parley will panic on assert
            for span in formatting.into_iter().filter(|s| !s.range.is_empty()) {
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
                            parley::StyleProperty::FontFamily(parley::style::FontFamily::Single(
                                parley::style::FontFamilyName::Generic(
                                    parley::style::GenericFamily::Monospace,
                                ),
                            )),
                            span.range.clone(),
                        );
                        // Inline `code` reads as slightly smaller text on top of a
                        // translucent capsule (drawn separately in `TextParagraph::draw`),
                        // matching the convention used by common markdown renderers.
                        builder.push(
                            parley::StyleProperty::FontSize(
                                self.pixel_size.get() * INLINE_CODE_FONT_SCALE,
                            ),
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
                                link_color,
                            }),
                            span.range,
                        );
                    }
                    Style::Color(color) => {
                        builder.push(
                            parley::StyleProperty::Brush(Brush {
                                override_fill_color: Some(crate::Color::from_argb_encoded(color)),
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

/// Splits plain text into paragraph byte ranges at `'\n'`. The `'\n'` and any preceding `'\r'`
/// are excluded from the range: parley treats a lone CR as a mandatory line break, so a CRLF
/// left in the paragraph would render an extra empty line.
fn paragraph_ranges(text: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut start = 0;
    text.split('\n').map(move |paragraph| {
        let end = start + paragraph.len();
        let range = if paragraph.ends_with('\r') { start..end - 1 } else { start..end };
        start = end + 1;
        range
    })
}

fn create_text_paragraphs(
    layout_builder: &LayoutWithoutLineBreaksBuilder,
    font_context: &mut parley::FontContext,
    text: PlainOrStyledText,
    selection: Option<(Range<usize>, Color)>,
    link_color: Color,
) -> Vec<TextParagraph> {
    let paragraph_from_text =
        |font_context: &mut parley::FontContext,
         text: &str,
         range: std::ops::Range<usize>,
         formatting: Vec<i_slint_common::styled_text::FormattedSpan>,
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

            let code_ranges: alloc::vec::Vec<Range<usize>> = formatting
                .iter()
                .filter(|s| matches!(s.style, i_slint_common::styled_text::Style::Code))
                .map(|s| s.range.clone())
                .collect();

            let layout =
                layout_builder.build(font_context, text, selection, formatting, Some(link_color));

            TextParagraph { range, y: PhysicalLength::default(), layout, links, code_ranges }
        };

    let mut paragraphs = Vec::with_capacity(1);

    match text {
        PlainOrStyledText::Plain(ref text) => {
            for range in paragraph_ranges(text) {
                paragraphs.push(paragraph_from_text(
                    font_context,
                    &text[range.clone()],
                    range,
                    Default::default(),
                    Default::default(),
                ));
            }
        }
        PlainOrStyledText::Styled(rich_text) => {
            for paragraph in rich_text.paragraphs {
                paragraphs.push(paragraph_from_text(
                    font_context,
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

/// Note: parley currently uses `WordBreak` while shaping via `analyze_text()`,
/// so shaped paragraphs aren't identical across wrap modes. This is why `text_size()`
/// doesn't use the `TextLayoutCache` — it would be incorrect to share cached paragraphs
/// shaped with one wrap mode and reuse them with another.
fn layout(
    layout_builder: &LayoutWithoutLineBreaksBuilder,
    font_context: &mut parley::FontContext,
    mut paragraphs: Vec<TextParagraph>,
    scale_factor: ScaleFactor,
    options: LayoutOptions,
) -> Layout {
    let max_physical_width = options.max_width.map(|max_width| max_width * scale_factor);
    let max_physical_height = options.max_height.map(|max_height| max_height * scale_factor);

    // Returned None if failed to get the ellipsis glyph for some rare reason.
    let get_ellipsis_glyph = |font_context: &mut parley::FontContext| {
        let mut layout = layout_builder.build(font_context, "…", None, None, None);
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
        para.layout.align(
            match options.horizontal_align {
                TextHorizontalAlignment::Start | TextHorizontalAlignment::Left => {
                    parley::Alignment::Left
                }
                TextHorizontalAlignment::Center => parley::Alignment::Center,
                TextHorizontalAlignment::End | TextHorizontalAlignment::Right => {
                    parley::Alignment::Right
                }
            },
            parley::AlignmentOptions::default(),
        );

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

    let y_offset = match (max_physical_height, options.vertical_align) {
        (Some(max_height), TextVerticalAlignment::Center) => (max_height - height) / 2.0,
        (Some(max_height), TextVerticalAlignment::Bottom) => max_height - height,
        (None, _) | (Some(_), TextVerticalAlignment::Top) => PhysicalLength::new(0.0),
    };

    Layout {
        paragraphs,
        y_offset,
        elision_info,
        max_width,
        height,
        max_physical_height,
        line_limit_cut,
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

/// RAII guard: takes Vec out of the cache on creation, puts it back on drop.
struct CachedParagraphsGuard<'a> {
    paragraphs: Option<Vec<TextParagraph>>,
    container: Option<std::cell::RefMut<'a, Vec<TextParagraph>>>,
}

impl Drop for CachedParagraphsGuard<'_> {
    fn drop(&mut self) {
        if let (Some(paragraphs), Some(container)) = (self.paragraphs.take(), &mut self.container) {
            **container = paragraphs;
        }
    }
}

fn shape_paragraphs(
    text: Pin<&dyn crate::item_rendering::RenderText>,
    item_rc: Option<&crate::item_tree::ItemRc>,
    scale_factor: ScaleFactor,
    font_context: &mut parley::FontContext,
) -> Vec<TextParagraph> {
    let (stroke_brush, _, stroke_style) = text.stroke();
    let has_stroke = !stroke_brush.is_transparent();
    let builder = LayoutWithoutLineBreaksBuilder::new(
        item_rc.map(|irc| text.font_request(irc)),
        text.wrap(),
        has_stroke.then_some(stroke_style),
        scale_factor,
    );
    create_text_paragraphs(&builder, font_context, text.text(), None, text.link_color())
}

fn get_or_create_text_paragraphs<'a>(
    cache: Option<&'a TextLayoutCache>,
    item_rc: Option<&crate::item_tree::ItemRc>,
    text: Pin<&dyn crate::item_rendering::RenderText>,
    scale_factor: ScaleFactor,
    font_context: &mut parley::FontContext,
) -> CachedParagraphsGuard<'a> {
    if let (Some(cache), Some(item_rc)) = (cache, item_rc) {
        let mut entry = cache.inner.get_or_update_cache_entry_ref(item_rc, || {
            #[cfg(feature = "testing")]
            cache.cache_miss_count.set(cache.cache_miss_count.get() + 1);
            shape_paragraphs(text, Some(item_rc), scale_factor, font_context)
        });
        let paragraphs = std::mem::take(&mut *entry);
        CachedParagraphsGuard { paragraphs: Some(paragraphs), container: Some(entry) }
    } else {
        CachedParagraphsGuard {
            paragraphs: Some(shape_paragraphs(text, item_rc, scale_factor, font_context)),
            container: None,
        }
    }
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

struct TextParagraph {
    range: Range<usize>,
    y: PhysicalLength,
    layout: parley::Layout<Brush>,
    links: std::vec::Vec<(Range<usize>, std::string::String)>,
    /// Byte ranges within the paragraph's text that carry `Style::Code`. Drawn with a
    /// translucent rounded background by `draw` for visual parity with common markdown
    /// renderers.
    code_ranges: std::vec::Vec<Range<usize>>,
}

impl TextParagraph {
    fn draw<R: GlyphRenderer>(
        &self,
        layout: &Layout,
        paragraph_index: usize,
        visible_extent: Option<ElisionCut>,
        item_renderer: &mut R,
        default_fill_brush: &<R as GlyphRenderer>::PlatformBrush,
        default_stroke_brush: &Option<<R as GlyphRenderer>::PlatformBrush>,
        default_text_color: Color,
        draw_glyphs: &mut dyn FnMut(
            &mut R,
            &parley::FontData,
            PhysicalLength,
            &[i16],               // normalized variation coords
            &fontique::Synthesis, // design-space variation settings
            <R as GlyphRenderer>::PlatformBrush,
            PhysicalLength, // y offset for paragraph
            &mut dyn Iterator<Item = parley::layout::Glyph>,
        ),
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
            for item in line.items() {
                match item {
                    parley::PositionedLayoutItem::GlyphRun(glyph_run) => {
                        let ellipsis = if last_line {
                            let (truncated_glyphs, ellipsis) = layout.glyphs_with_elision(
                                &glyph_run,
                                vertically_truncated,
                                metrics.trailing_whitespace,
                            );

                            Self::draw_glyph_run(
                                &glyph_run,
                                item_renderer,
                                default_fill_brush,
                                default_stroke_brush,
                                para_y,
                                &mut truncated_glyphs.into_iter(),
                                draw_glyphs,
                            );
                            ellipsis
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

                        if let Some((ellipsis_glyph, ellipsis_font, font_size)) = ellipsis {
                            let run = glyph_run.run();
                            draw_glyphs(
                                item_renderer,
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

        let scale_factor = ScaleFactor::new(item_renderer.scale_factor());
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
            &[i16],               // normalized variation coords
            &fontique::Synthesis, // design-space variation settings
            <R as GlyphRenderer>::PlatformBrush,
            PhysicalLength,
            &mut dyn Iterator<Item = parley::layout::Glyph>,
        ),
    ) {
        let run = glyph_run.run();
        let normalized_coords = run.normalized_coords();
        let synthesis = run.synthesis();
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
                        normalized_coords,
                        &synthesis,
                        stroke_brush,
                        para_y,
                        &mut glyphs.iter().cloned(),
                    );
                }

                draw_glyphs(
                    item_renderer,
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

                draw_glyphs(
                    item_renderer,
                    run.font(),
                    PhysicalLength::new(run.font_size()),
                    normalized_coords,
                    &synthesis,
                    fill_brush.clone(),
                    para_y,
                    &mut glyphs.iter().cloned(),
                );

                if let Some(stroke_brush) = default_stroke_brush.clone() {
                    draw_glyphs(
                        item_renderer,
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
                draw_glyphs(
                    item_renderer,
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

/// Where `overflow: elide` cuts text off, computed across all paragraphs (each explicit `\n`
/// produces one paragraph). See [`Layout::elision_extent`].
#[derive(Clone, Copy)]
struct ElisionCut {
    /// Paragraph holding the last kept line.
    last_paragraph: usize,
    /// Last kept line within `last_paragraph`.
    last_line: usize,
    /// A line below the kept one was dropped for the height, so the kept line shows an ellipsis.
    needs_ellipsis: bool,
}

struct Layout {
    paragraphs: Vec<TextParagraph>,
    y_offset: PhysicalLength,
    max_width: PhysicalLength,
    height: PhysicalLength,
    max_physical_height: Option<PhysicalLength>,
    elision_info: Option<ElisionInfo>,
    /// Where an active `max-lines` limit drops lines, in the same coordinates as [`ElisionCut`]:
    /// the (paragraph index, line index) of the last kept line. See [`line_limit_cut`].
    line_limit_cut: Option<(usize, usize)>,
}

impl Layout {
    /// The paragraphs that have at least one line to show. Only differs from `paragraphs` when a
    /// `max-lines` limit drops lines: paragraphs entirely below the cut don't take part in
    /// hit-testing or selection.
    fn visible_paragraphs(&self) -> &[TextParagraph] {
        match self.line_limit_cut {
            Some((last_paragraph, _)) => &self.paragraphs[..=last_paragraph],
            None => &self.paragraphs,
        }
    }

    /// True when an active line limit dropped lines and `y` (in item coordinates) falls below
    /// the last kept line, i.e. into the item region where the dropped lines would have been.
    /// Nothing is shown there, so nothing there should hit-test. With an active cut, `height`
    /// is the bottom of the last kept line.
    fn below_line_limit(&self, y: PhysicalLength) -> bool {
        self.line_limit_cut.is_some() && y >= self.y_offset + self.height
    }

    /// The last line to draw, combining the height-based elision cut with the `max-lines` limit:
    /// whichever cuts earlier wins. Unlike the elision cut, the line limit also applies with
    /// `overflow: clip` -- just without the ellipsis.
    fn visible_extent(&self) -> Option<ElisionCut> {
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
    fn first_line_exceeds_height(&self) -> bool {
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
    fn paragraph_line_within_box(
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

    fn paragraph_by_y(&self, y: PhysicalLength) -> Option<&TextParagraph> {
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

    fn selection_geometry(
        &self,
        selection_range: Range<usize>,
        mut callback: impl FnMut(PhysicalRect),
    ) {
        for paragraph in self.visible_paragraphs() {
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
    fn glyphs_with_elision<'a>(
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

    fn draw<R: GlyphRenderer>(
        &self,
        item_renderer: &mut R,
        default_fill_brush: <R as GlyphRenderer>::PlatformBrush,
        default_stroke_brush: Option<<R as GlyphRenderer>::PlatformBrush>,
        default_text_color: Color,
        draw_glyphs: &mut dyn FnMut(
            &mut R,
            &parley::FontData,
            PhysicalLength,
            &[i16],               // normalized variation coords
            &fontique::Synthesis, // design-space variation settings
            <R as GlyphRenderer>::PlatformBrush,
            PhysicalLength, // y offset for paragraph
            &mut dyn Iterator<Item = parley::layout::Glyph>,
        ),
    ) {
        // Compute the cut once: explicit `\n` breaks produce one paragraph each, but they must
        // elide as a single block (drop lines below the box, ellipsis on the last visible one).
        let visible_extent = self.visible_extent();
        for (paragraph_index, paragraph) in self.paragraphs.iter().enumerate() {
            paragraph.draw(
                self,
                paragraph_index,
                visible_extent,
                item_renderer,
                &default_fill_brush,
                &default_stroke_brush,
                default_text_color,
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
    cache: Option<&TextLayoutCache>,
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

    // The layout_builder is still needed for the elision glyph in layout().
    let layout_builder = LayoutWithoutLineBreaksBuilder::new(
        item_rc.map(|item_rc| text.font_request(item_rc)),
        text.wrap(),
        platform_stroke_brush.is_some().then_some(stroke_style),
        scale_factor,
    );

    let mut font_ctx = item_renderer.window().context().font_context().borrow_mut();

    let mut guard =
        get_or_create_text_paragraphs(cache, item_rc, text, scale_factor, &mut font_ctx);

    let (horizontal_align, vertical_align) = text.alignment();
    let text_overflow = text.overflow();

    let layout = layout(
        &layout_builder,
        &mut font_ctx,
        guard.paragraphs.take().unwrap_or_default(),
        scale_factor,
        LayoutOptions {
            horizontal_align,
            vertical_align,
            max_height: Some(max_height),
            max_width: Some(max_width),
            max_lines: text.line_limit(),
            text_overflow: text.overflow(),
        },
    );

    drop(font_ctx);

    // When `overflow: elide` can't even fit the first line, the line is still drawn (rather than
    // dropped, which would render nothing) but its vertical overflow needs to be clipped like
    // `overflow: clip` would. Horizontal elision still applies, so a line that is both too tall
    // and too wide is clipped vertically and gets an ellipsis horizontally.
    let clip_overflowing_first_line =
        text_overflow == TextOverflow::Elide && layout.first_line_exceeds_height();

    let render = if text_overflow == TextOverflow::Clip || clip_overflowing_first_line {
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
            text.color().color(),
            &mut |item_renderer: &mut _,
                  font,
                  font_size,
                  normalized_coords,
                  synthesis,
                  brush,
                  y_offset,
                  glyphs_it| {
                item_renderer.draw_glyph_run(
                    font,
                    font_size,
                    normalized_coords,
                    synthesis,
                    brush,
                    y_offset,
                    glyphs_it,
                );
            },
        );
    }

    if text_overflow == TextOverflow::Clip || clip_overflowing_first_line {
        item_renderer.restore_state();
    }

    // Put paragraphs back into the cache guard for reuse.
    // break_all_lines replaces line data each time, so the state is ready for the next call.
    guard.paragraphs = Some(layout.paragraphs);
}

#[cfg(feature = "std")]
pub fn link_under_cursor(
    font_context: &mut parley::FontContext,
    scale_factor: ScaleFactor,
    text: Pin<&dyn crate::item_rendering::RenderText>,
    item_rc: &crate::item_tree::ItemRc,
    size: LogicalSize,
    cursor: PhysicalPoint,
    cache: Option<&TextLayoutCache>,
) -> Option<std::string::String> {
    let layout_builder = LayoutWithoutLineBreaksBuilder::new(
        Some(text.font_request(item_rc)),
        text.wrap(),
        None,
        scale_factor,
    );

    let mut guard =
        get_or_create_text_paragraphs(cache, Some(item_rc), text, scale_factor, font_context);

    let (horizontal_align, vertical_align) = text.alignment();

    let layout = layout(
        &layout_builder,
        font_context,
        guard.paragraphs.take().unwrap_or_default(),
        scale_factor,
        LayoutOptions {
            horizontal_align,
            vertical_align,
            max_height: Some(size.height_length()),
            max_width: Some(size.width_length()),
            max_lines: text.line_limit(),
            text_overflow: text.overflow(),
        },
    );

    let result = layout.paragraph_by_y(cursor.y_length()).and_then(|paragraph| {
        let paragraph_y: f64 = paragraph.y.cast::<f64>().get();

        paragraph
            .links
            .iter()
            .find(|(range, _)| {
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
            })
            .map(|(_, link)| link.clone())
    });

    // Put paragraphs back into the cache guard for reuse.
    guard.paragraphs = Some(layout.paragraphs);

    result
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

    let text_color = visual_representation.text_color.color();
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

    let text = visual_representation.text.clone();

    // When a piece of text is first selected, it gets an empty range like `Some(1..1)`.
    // If the text starts with a multi-byte character then this selection will be within
    // that character and parley will panic. We just filter out empty selection ranges.
    let selection_and_color = if !selection_range.is_empty() {
        Some((selection_range.clone(), text_input.selection_foreground_color()))
    } else {
        None
    };

    let mut font_ctx = item_renderer.window().context().font_context().borrow_mut();

    let paragraphs_without_linebreaks = create_text_paragraphs(
        &layout_builder,
        &mut font_ctx,
        PlainOrStyledText::Plain(text),
        selection_and_color,
        Color::default(),
    );

    let layout = layout(
        &layout_builder,
        &mut font_ctx,
        paragraphs_without_linebreaks,
        scale_factor,
        LayoutOptions::new_from_textinput(text_input, Some(width), Some(height)),
    );

    drop(font_ctx);

    layout.selection_geometry(selection_range, |selection_rect| {
        item_renderer
            .fill_rectangle_with_color(selection_rect, text_input.selection_background_color());
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
            text_color,
            &mut |item_renderer: &mut _,
                  font,
                  font_size,
                  normalized_coords,
                  synthesis,
                  brush,
                  y_offset,
                  glyphs_it| {
                item_renderer.draw_glyph_run(
                    font,
                    font_size,
                    normalized_coords,
                    synthesis,
                    brush,
                    y_offset,
                    glyphs_it,
                );
            },
        );

        if let Some(cursor_pos) = visual_representation.cursor_position {
            let cursor_rect = layout.cursor_rect_for_byte_offset(
                cursor_pos,
                text_input.text_cursor_width() * scale_factor,
            );
            item_renderer
                .fill_rectangle_with_color(cursor_rect, visual_representation.cursor_color);
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
    _cache: Option<&TextLayoutCache>,
) -> Option<LogicalSize> {
    let scale_factor = renderer.scale_factor()?;

    // Evaluate properties before borrowing font_context: both font_request()
    // and text() can trigger property bindings that re-enter text_size for
    // other elements, which would panic on a second borrow_mut().
    let font_request = text_item.font_request(item_rc);
    let text = text_item.text();

    let ctx = renderer.slint_context()?;
    let mut font_ctx = ctx.font_context().borrow_mut();

    let layout_builder =
        LayoutWithoutLineBreaksBuilder::new(Some(font_request), text_wrap, None, scale_factor);

    let paragraphs_without_linebreaks =
        create_text_paragraphs(&layout_builder, &mut font_ctx, text, None, Color::default());

    let layout = layout(
        &layout_builder,
        &mut font_ctx,
        paragraphs_without_linebreaks,
        scale_factor,
        LayoutOptions {
            max_width,
            max_height: None,
            max_lines: text_item.line_limit(),
            horizontal_align: TextHorizontalAlignment::Left,
            vertical_align: TextVerticalAlignment::Top,
            text_overflow: TextOverflow::Clip,
        },
    );
    Some(PhysicalSize::from_lengths(layout.max_width, layout.height) / scale_factor)
}

pub fn char_size(
    font_ctx: &mut parley::FontContext,
    text_item: Pin<&dyn crate::item_rendering::HasFont>,
    item_rc: &crate::item_tree::ItemRc,
    ch: char,
) -> Option<LogicalSize> {
    let font_request = text_item.font_request(item_rc);
    let font = font_request.query_fontique(&mut font_ctx.collection, &mut font_ctx.source_cache)?;

    let char_map = font.charmap()?;

    let face = skrifa::FontRef::from_index(font.blob.data(), font.index).unwrap();

    let glyph_index = char_map.map(ch)?;

    let pixel_size = font_request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE);

    let location = face.axes().location(font.synthesis.variation_settings());

    let glyph_metrics = skrifa::metrics::GlyphMetrics::new(
        &face,
        skrifa::instance::Size::new(pixel_size.get()),
        &location,
    );

    let advance_width = LogicalLength::new(glyph_metrics.advance_width(glyph_index.into())?);

    let font_metrics = skrifa::metrics::Metrics::new(
        &face,
        skrifa::instance::Size::new(pixel_size.get()),
        &location,
    );

    Some(LogicalSize::from_lengths(
        advance_width,
        LogicalLength::new(font_metrics.ascent - font_metrics.descent),
    ))
}

pub fn font_metrics(
    font_ctx: &mut parley::FontContext,
    font_request: FontRequest,
) -> crate::items::FontMetrics {
    let logical_pixel_size = font_request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE).get();

    let Some(font) =
        font_request.query_fontique(&mut font_ctx.collection, &mut font_ctx.source_cache)
    else {
        return crate::items::FontMetrics::default();
    };

    let face = skrifa::FontRef::from_index(font.blob.data(), font.index).unwrap();
    let location = face.axes().location(font.synthesis.variation_settings());
    let metrics = face.metrics(skrifa::instance::Size::unscaled(), &location);

    let units_per_em = metrics.units_per_em as f32;

    crate::items::FontMetrics {
        ascent: metrics.ascent * logical_pixel_size / units_per_em,
        descent: metrics.descent * logical_pixel_size / units_per_em,
        x_height: metrics.x_height.unwrap_or_default() * logical_pixel_size / units_per_em,
        cap_height: metrics.cap_height.unwrap_or_default() * logical_pixel_size / units_per_em,
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
    let visual_representation = text_input.visual_representation(None);

    let Some(ctx) = renderer.slint_context() else {
        return 0;
    };
    let mut font_ctx = ctx.font_context().borrow_mut();

    let paragraphs_without_linebreaks = create_text_paragraphs(
        &layout_builder,
        &mut font_ctx,
        PlainOrStyledText::Plain(visual_representation.text.clone()),
        None,
        Color::default(),
    );

    let layout = layout(
        &layout_builder,
        &mut font_ctx,
        paragraphs_without_linebreaks,
        scale_factor,
        LayoutOptions::new_from_textinput(text_input, Some(width), Some(height)),
    );
    let byte_offset = layout.byte_offset_from_point(pos);
    visual_representation.map_byte_offset_from_visual_text_to_actual_text(byte_offset)
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

    let visual_representation = text_input.visual_representation(None);
    let cursor_width = text_input.text_cursor_width() * scale_factor;

    let Some(ctx) = renderer.slint_context() else {
        return LogicalRect::default();
    };

    let mut font_ctx = ctx.font_context().borrow_mut();

    let byte_offset = visual_representation.map_byte_offset_from_actual_to_visual_text(byte_offset);

    let paragraphs_without_linebreaks = create_text_paragraphs(
        &layout_builder,
        &mut font_ctx,
        PlainOrStyledText::Plain(visual_representation.text),
        None,
        Color::default(),
    );

    let layout = layout(
        &layout_builder,
        &mut font_ctx,
        paragraphs_without_linebreaks,
        scale_factor,
        LayoutOptions::new_from_textinput(text_input, Some(width), Some(height)),
    );
    let cursor_rect = layout.cursor_rect_for_byte_offset(byte_offset, cursor_width);
    cursor_rect / scale_factor
}

#[cfg(test)]
mod tests {
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
        let data = include_bytes!("../../common/sharedfontique/Inter-VariableFont.ttf");
        let families =
            font_ctx.collection.register_fonts(fontique::Blob::new(Arc::new(data)), None);
        font_ctx.collection.set_generic_families(
            fontique::GenericFamily::SansSerif,
            families.iter().map(|(id, _)| *id),
        );
        let builder = LayoutWithoutLineBreaksBuilder::new(
            None,
            TextWrap::NoWrap,
            None,
            ScaleFactor::new(1.0),
        );
        let paragraphs = create_text_paragraphs(
            &builder,
            &mut font_ctx,
            PlainOrStyledText::Plain(text.into()),
            None,
            Color::default(),
        );
        layout(&builder, &mut font_ctx, paragraphs, ScaleFactor::new(1.0), options)
    }

    fn layout_text(text: &str) -> Layout {
        layout_text_with_options(text, LayoutOptions::default())
    }

    fn visual_line_count(text: &str) -> usize {
        layout_text(text).paragraphs.iter().map(|p| p.layout.lines().len()).sum()
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
}
