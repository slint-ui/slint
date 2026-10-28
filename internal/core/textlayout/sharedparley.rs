// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore unshareable
pub use parley;
pub use parley::fontique;

use crate::{
    Color,
    graphics::FontRequest,
    item_rendering::PlainOrStyledText,
    items::TextStrokeStyle,
    lengths::{
        LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalRect, LogicalSize, PhysicalPx,
        PointLengths, RectLengths, ScaleFactor, SizeLengths,
    },
    renderer::RendererSealed,
    textlayout::{TextHorizontalAlignment, TextOverflow, TextVerticalAlignment, TextWrap},
    window::WindowAdapter,
};
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::ops::Range;
use core::pin::Pin;
use euclid::num::Zero;
use i_slint_common::sharedfontique;
use skrifa::MetadataProvider as _;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(derive_more::Deref, derive_more::DerefMut)]
pub struct FontContext {
    #[deref]
    #[deref_mut]
    pub inner: parley::FontContext,
    /// `(ptr, len)` of each `&'static [u8]` already handed to fontique, so repeat
    /// `register_static_font` calls for the same embedded font are skipped.
    registered_static_fonts: HashSet<(usize, usize)>,
    /// See [`Self::query_font`].
    resolved_fonts: HashMap<FontQueryKey, Option<ResolvedFont>>,
}

/// The fields of a [`FontRequest`] that [`FontRequest::query_fontique`] matches on.
#[derive(PartialEq, Eq, Hash)]
struct FontQueryKey {
    family: Option<crate::SharedString>,
    weight: Option<i32>,
    italic: bool,
}

/// What [`FontContext::query_font`] memoizes per font request.
pub struct ResolvedFont {
    pub font: fontique::QueryFont,
    /// `(ascent - descent + leading) / units_per_em`, or `None` without a valid `units_per_em`.
    pub natural_line_height_ratio: Option<f32>,
}

impl ResolvedFont {
    fn new(font: fontique::QueryFont) -> Self {
        let natural_line_height_ratio =
            skrifa::FontRef::from_index(font.blob.data(), font.index).ok().and_then(|face| {
                let location = face.axes().location(font.synthesis.variation_settings());
                let metrics = face.metrics(skrifa::instance::Size::unscaled(), &location);
                let units_per_em = metrics.units_per_em as f32;
                (units_per_em > 0.0)
                    .then(|| (metrics.ascent - metrics.descent + metrics.leading) / units_per_em)
            });
        Self { font, natural_line_height_ratio }
    }
}

impl FontContext {
    pub fn new(inner: parley::FontContext) -> Self {
        Self {
            inner,
            registered_static_fonts: HashSet::default(),
            resolved_fonts: HashMap::default(),
        }
    }

    /// The font [`FontRequest::query_fontique`] resolves `request` to, memoized until
    /// [`Self::begin_frame`].
    ///
    /// A layout pass resolves the same few requests once per text item, and each fontique
    /// query rebuilds its candidate list from the family names.
    pub fn query_font(&mut self, request: &FontRequest) -> Option<&ResolvedFont> {
        let key = FontQueryKey {
            family: request.family.clone(),
            weight: request.weight,
            italic: request.italic,
        };
        let inner = &mut self.inner;
        self.resolved_fonts
            .entry(key)
            .or_insert_with(|| {
                request
                    .query_fontique(&mut inner.collection, &mut inner.source_cache)
                    .map(ResolvedFont::new)
            })
            .as_ref()
    }

    /// Forgets the memoized fonts, so that fonts registered through a clone of the shared
    /// collection, which the collection doesn't report, are seen from the next frame on.
    pub fn begin_frame(&mut self) {
        self.resolved_fonts.clear();
    }

    pub fn register_fonts(
        &mut self,
        data: fontique::Blob<u8>,
    ) -> Vec<(fontique::FamilyId, Vec<fontique::FontInfo>)> {
        self.resolved_fonts.clear();
        self.inner.collection.register_fonts(data, None)
    }

    pub fn register_static_font(&mut self, data: &'static [u8]) {
        let key = (data.as_ptr() as usize, data.len());
        if self.registered_static_fonts.insert(key) {
            self.register_fonts(fontique::Blob::new(Arc::new(data)));
        }
    }

    /// Forgets everything registered with and resolved from the previous `collection`.
    /// Call after replacing it.
    pub fn clear_registered_static_fonts(&mut self) {
        self.registered_static_fonts.clear();
        self.resolved_fonts.clear();
    }
}

pub type PhysicalLength = euclid::Length<f32, PhysicalPx>;
pub type PhysicalRect = euclid::Rect<f32, PhysicalPx>;
type PhysicalSize = euclid::Size2D<f32, PhysicalPx>;
type PhysicalPoint = euclid::Point2D<f32, PhysicalPx>;

pub use super::DEFAULT_FONT_SIZE;

mod cache;
mod draw;
mod layout;
mod selection;
mod shaping;
#[cfg(test)]
mod tests;

pub use cache::TextLayoutCache;
pub use draw::{GlyphRenderer, RectangleBorder};

use cache::cached_paragraphs;
use layout::{Layout, LayoutOptions, layout};
use selection::{SelectionRendering, SelectionSpans};
use shaping::{
    Brush, LayoutWithoutLineBreaksBuilder, create_text_paragraphs, shape_paragraphs,
    shaping_builder,
};

/// Lays out the shaped text of an item and runs `f` over the result.
///
/// This is the one place that checks paragraphs out of a [`TextLayoutCache`] entry and hands them
/// back: `f` only borrows the [`Layout`], so no caller can lose the shaped paragraphs and cost the
/// next use of the entry a reshape.
///
/// The font context is borrowed from `window`'s Slint context for shaping and layout only,
/// and released before `f` runs, as glyph drawing inside `f` re-enters it.
/// `f` runs with the cache entry checked out, which keeps the whole [`TextLayoutCache`] borrowed.
/// So `f` must not read properties: a binding that measures text would panic.
/// The wrap mode, line-height factor and scale factor the cache entry is keyed on come from
/// `layout_builder`, so shaping and layout cannot disagree about them.
///
/// Returns `None` only when `window` has no Slint context yet.
fn with_text_layout<R>(
    cache: Option<&TextLayoutCache>,
    item_rc: Option<&crate::item_tree::ItemRc>,
    text: Pin<&dyn crate::item_rendering::RenderString>,
    layout_builder: &LayoutWithoutLineBreaksBuilder,
    options: LayoutOptions,
    window: &crate::api::Window,
    f: impl FnOnce(&Layout) -> R,
) -> Option<R> {
    let ctx = crate::window::WindowInner::from_pub(window).try_context()?;
    let font_context = ctx.font_context();

    let text_wrap = layout_builder.text_wrap;
    let scale_factor = layout_builder.scale_factor;
    let mut guard = cached_paragraphs(
        cache,
        item_rc,
        text_wrap,
        layout_builder.line_height_factor,
        text.measures_width_only(),
        window,
        &|| shape_paragraphs(text, item_rc, text_wrap, scale_factor, font_context),
    );

    let line_breaking = guard.take_line_breaking();
    let layout = layout(
        layout_builder,
        &mut font_context.borrow_mut(),
        guard.take(),
        scale_factor,
        options,
        line_breaking,
    );

    if layout.broke_lines {
        #[cfg(feature = "testing")]
        if let Some(cache) = cache {
            cache.count_layout_miss();
        }
    }

    let result = f(&layout);
    let (paragraphs, line_breaking) = layout.dismantle();
    guard.restore(paragraphs, line_breaking);
    Some(result)
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

    let scale_factor = item_renderer.scale_factor();

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

    let window_adapter = item_renderer.window().window_adapter();

    let text_overflow = text.overflow();
    let text_color = text.color().color();
    let text_selection = text.selection();

    let _ = with_render_text_layout(
        scale_factor,
        text,
        item_rc,
        size,
        window_adapter.window(),
        cache,
        |layout| {
            // When `overflow: elide` can't even fit the first line, the line is still drawn
            // (rather than dropped, which would render nothing) but its vertical overflow needs
            // to be clipped like `overflow: clip` would. Horizontal elision still applies, so a
            // line that is both too tall and too wide is clipped vertically and gets an ellipsis
            // horizontally.
            let clip_overflowing_first_line =
                text_overflow == TextOverflow::Elide && layout.first_line_exceeds_height();

            let render = if text_overflow == TextOverflow::Clip || clip_overflowing_first_line {
                item_renderer.save_state();

                item_renderer.combine_clip(
                    LogicalRect::new(LogicalPoint::default(), size),
                    LogicalBorderRadius::zero(),
                )
            } else {
                true
            };

            if render {
                let mut selection_spans = SelectionSpans::default();
                let selection = text_selection.and_then(|selection| {
                    draw_selection_background(
                        item_renderer,
                        layout,
                        selection.range,
                        selection.background,
                        selection.foreground,
                        &mut selection_spans,
                    )
                });
                layout.draw(
                    item_renderer,
                    platform_fill_brush,
                    platform_stroke_brush,
                    text_color,
                    selection.as_ref(),
                );
            }

            if text_overflow == TextOverflow::Clip || clip_overflowing_first_line {
                item_renderer.restore_state();
            }
        },
    );
}

/// Lays a `Text` or `StyledText` item out as it is drawn, `size` being the item's.
fn with_render_text_layout<R>(
    scale_factor: ScaleFactor,
    text: Pin<&dyn crate::item_rendering::RenderText>,
    item_rc: Option<&crate::item_tree::ItemRc>,
    size: LogicalSize,
    window: &crate::api::Window,
    cache: Option<&TextLayoutCache>,
    f: impl FnOnce(&Layout) -> R,
) -> Option<R> {
    let layout_builder = shaping_builder(text, item_rc, text.wrap(), scale_factor);
    let (horizontal_align, vertical_align) = text.alignment();
    with_text_layout(
        cache,
        item_rc,
        text,
        &layout_builder,
        LayoutOptions {
            horizontal_align,
            vertical_align,
            max_height: Some(size.height_length()),
            max_width: Some(size.width_length()),
            max_lines: text.line_limit(),
            text_overflow: text.overflow(),
        },
        window,
        f,
    )
}

#[cfg(feature = "std")]
pub fn link_under_cursor(
    scale_factor: ScaleFactor,
    text: Pin<&dyn crate::item_rendering::RenderText>,
    item_rc: &crate::item_tree::ItemRc,
    size: LogicalSize,
    cursor: PhysicalPoint,
    window: &crate::api::Window,
    cache: Option<&TextLayoutCache>,
) -> Option<std::string::String> {
    with_render_text_layout(scale_factor, text, Some(item_rc), size, window, cache, |layout| {
        link_in_layout(layout, cursor)
    })
    .flatten()
}

/// The byte offset into the raw text of the character under `cursor`,
/// or of the nearest one on that line.
/// `None` when nothing is laid out there.
#[cfg(feature = "std")]
pub fn byte_offset_under_cursor(
    scale_factor: ScaleFactor,
    text: Pin<&dyn crate::item_rendering::RenderText>,
    item_rc: &crate::item_tree::ItemRc,
    size: LogicalSize,
    cursor: PhysicalPoint,
    window: &crate::api::Window,
    cache: Option<&TextLayoutCache>,
) -> Option<usize> {
    with_render_text_layout(scale_factor, text, Some(item_rc), size, window, cache, |layout| {
        layout.byte_offset_from_point(cursor).0
    })
}

/// Fills the highlight behind `range` and returns what recolors its glyphs.
/// `None` when nothing of the range is on a drawn line.
fn draw_selection_background<'a, R: GlyphRenderer>(
    item_renderer: &mut R,
    layout: &Layout,
    range: core::ops::Range<usize>,
    background: Color,
    foreground: Color,
    spans: &'a mut SelectionSpans,
) -> Option<SelectionRendering<'a, R>> {
    // When a piece of text is first selected, it gets an empty range like `1..1`. If the text
    // starts with a multi-byte character then this selection would be within that character and
    // parley would panic, so empty ranges are filtered out. The spans only feed drawing (the
    // highlight fill and the glyph clip), so only the lines the clip lets through need any.
    if range.is_empty() {
        return None;
    }
    *spans = layout.selection_geometry(range, &draw::visible_band(item_renderer), |x| {
        item_renderer.snap_selection_x(x)
    });
    if spans.is_empty() {
        return None;
    }
    // Inside the clip, like the glyphs it sits under: a line box taller than the item would
    // otherwise paint the highlight over whatever follows the item.
    for rect in spans.backgrounds() {
        item_renderer.fill_rectangle_with_color(rect, background);
    }
    // Selected glyphs are recolored by clipping, not by restyling the layout, so that a boundary
    // landing inside a ligature cuts the glyph instead of recoloring all of it.
    let foreground = item_renderer.platform_brush_for_color(&foreground)?;
    Some(SelectionRendering { spans, foreground })
}

fn link_in_layout(layout: &Layout, cursor: PhysicalPoint) -> Option<std::string::String> {
    layout.paragraph_by_y(cursor.y_length()).and_then(|paragraph| {
        // Where the paragraph is drawn: `y_offset` carries the vertical alignment.
        let paragraph_y: f64 = (layout.y_offset + paragraph.y).cast::<f64>().get();

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
                    clicked |= bounding_box.union(parley::BoundingBox::new(
                        cursor.x.into(),
                        cursor.y.into(),
                        cursor.x.into(),
                        cursor.y.into(),
                    )) == bounding_box;
                });
                clicked
            })
            .map(|(_, link)| link.clone())
    })
}

pub fn draw_text_input(
    item_renderer: &mut impl GlyphRenderer,
    text_input: Pin<&crate::items::TextInput>,
    item_rc: &crate::item_tree::ItemRc,
    size: LogicalSize,
    cache: &TextLayoutCache,
) {
    let width = size.width_length();
    let height = size.height_length();
    if width.get() <= 0. || height.get() <= 0. {
        return;
    }

    let visual_representation = text_input.visual_representation();

    let text_color = visual_representation.text_color.color();
    let Some(platform_fill_brush) =
        item_renderer.platform_text_fill_brush(visual_representation.text_color.clone(), size)
    else {
        return;
    };

    let selection_range = if !visual_representation.preedit_range.is_empty() {
        visual_representation.preedit_range.start..visual_representation.preedit_range.end
    } else {
        visual_representation.selection_range.start..visual_representation.selection_range.end
    };

    let scale_factor = item_renderer.scale_factor();

    let selection_background = text_input.selection_background_color();
    let selection_foreground = text_input.selection_foreground_color();
    let cursor = visual_representation
        .cursor_position
        .map(|cursor_pos| (cursor_pos, text_input.text_cursor_width() * scale_factor));

    let layout_builder =
        shaping_builder(text_input, Some(item_rc), text_input.wrap(), scale_factor);

    let window_adapter = item_renderer.window().window_adapter();

    // The visual text shapes through the shared cache entry like any other text: a selection
    // doesn't make the entry unshareable, because it is applied when drawing, by clipping the
    // runs it cuts across, and never reaches shaping. Cluster ranges and advances are identical
    // with and without one, so a selected `TextInput` hits the same entry as an unselected one --
    // which is what keeps dragging a selection, or composing with an IME, from re-shaping the
    // document on every event. A password field shapes a substituted text, but the substitution
    // is the same everywhere, so it is cacheable too.
    let _ = with_text_layout(
        Some(cache),
        Some(item_rc),
        text_input,
        &layout_builder,
        LayoutOptions::new_from_textinput(text_input, Some(width), Some(height)),
        window_adapter.window(),
        |layout| {
            item_renderer.save_state();

            let render = item_renderer.combine_clip(
                LogicalRect::new(LogicalPoint::default(), size),
                LogicalBorderRadius::zero(),
            );

            if render {
                let mut selection_spans = SelectionSpans::default();
                let selection = draw_selection_background(
                    item_renderer,
                    layout,
                    selection_range,
                    selection_background,
                    selection_foreground,
                    &mut selection_spans,
                );

                layout.draw(
                    item_renderer,
                    platform_fill_brush,
                    None,
                    text_color,
                    selection.as_ref(),
                );

                if let Some((cursor_pos, cursor_width)) = cursor {
                    let cursor_rect = layout.cursor_rect_for_byte_offset(
                        cursor_pos,
                        visual_representation.cursor_affinity,
                        cursor_width,
                    );
                    item_renderer
                        .fill_rectangle_with_color(cursor_rect, visual_representation.cursor_color);
                }
            }

            item_renderer.restore_state();
        },
    );
}

// The public entry points taking a renderer are generic so that RendererSealed's default
// implementations can pass self. Each is a thin shim that extracts what it needs from the
// renderer and forwards to a monomorphic inner function, so that the layout code is not
// instantiated (and duplicated in the binary) once per renderer type.
pub fn text_size(
    renderer: &(impl RendererSealed + ?Sized),
    text_item: Pin<&dyn crate::item_rendering::RenderString>,
    item_rc: &crate::item_tree::ItemRc,
    max_width: Option<LogicalLength>,
    text_wrap: TextWrap,
    cache: Option<&TextLayoutCache>,
) -> Option<LogicalSize> {
    text_size_impl(
        renderer.scale_factor(),
        renderer.window_adapter(),
        text_item,
        item_rc,
        max_width,
        text_wrap,
        cache,
    )
}

fn text_size_impl(
    scale_factor: Option<ScaleFactor>,
    window_adapter: Option<Rc<dyn WindowAdapter>>,
    text_item: Pin<&dyn crate::item_rendering::RenderString>,
    item_rc: &crate::item_tree::ItemRc,
    max_width: Option<LogicalLength>,
    text_wrap: TextWrap,
    cache: Option<&TextLayoutCache>,
) -> Option<LogicalSize> {
    let scale_factor = scale_factor?;
    let window_adapter = window_adapter?;

    // Only `layout()`'s elision glyph reads this, and `TextOverflow::Clip` never asks for one.
    let layout_builder = shaping_builder(text_item, Some(item_rc), text_wrap, scale_factor);

    with_text_layout(
        cache,
        Some(item_rc),
        text_item,
        &layout_builder,
        LayoutOptions {
            max_width,
            max_height: None,
            max_lines: text_item.line_limit(),
            horizontal_align: TextHorizontalAlignment::Left,
            vertical_align: TextVerticalAlignment::Top,
            text_overflow: TextOverflow::Clip,
        },
        window_adapter.window(),
        |layout| PhysicalSize::from_lengths(layout.max_width, layout.height) / scale_factor,
    )
}

/// The content widths of the text. See [`crate::renderer::ContentWidths`].
pub fn text_content_widths(
    renderer: &(impl RendererSealed + ?Sized),
    text_item: Pin<&dyn crate::item_rendering::RenderString>,
    item_rc: &crate::item_tree::ItemRc,
    cache: Option<&TextLayoutCache>,
) -> Option<crate::renderer::ContentWidths> {
    text_content_widths_impl(
        renderer.scale_factor(),
        renderer.window_adapter(),
        renderer.slint_context(),
        text_item,
        item_rc,
        cache,
    )
}

fn text_content_widths_impl(
    scale_factor: Option<ScaleFactor>,
    window_adapter: Option<Rc<dyn WindowAdapter>>,
    ctx: Option<crate::SlintContext>,
    text_item: Pin<&dyn crate::item_rendering::RenderString>,
    item_rc: &crate::item_tree::ItemRc,
    cache: Option<&TextLayoutCache>,
) -> Option<crate::renderer::ContentWidths> {
    let scale_factor = scale_factor?;
    let ctx = ctx?;

    // Reads the text, the font request and the line limit, so the cache entry depends on all
    // three. Nothing else feeds the widths: the wrap mode, the width and the color don't.
    // Reads them before borrowing the font context; see `shape_paragraphs`.
    let compute = || {
        let layout_builder =
            shaping::content_widths_builder(text_item.font_request(item_rc), scale_factor);
        let text = text_item.text();
        let line_limit = text_item.line_limit();

        let paragraphs_without_linebreaks = create_text_paragraphs(
            &layout_builder,
            &mut ctx.font_context().borrow_mut(),
            text,
            Color::default(),
        );

        // No line breaking needed: parley derives the content widths from the break
        // opportunities. Paragraphs stack vertically, so both widths are the widest.
        // Without wrapping each paragraph is one line, so a line limit drops the paragraphs
        // that are not drawn, from both widths.
        let (min, max) = paragraphs_without_linebreaks
            .iter()
            .take(line_limit.unwrap_or(usize::MAX))
            .fold((0., 0.), |(min, max), p| {
                let w = p.layout.calculate_content_widths();
                (f32::max(min, w.min), f32::max(max, w.max))
            });
        crate::renderer::ContentWidths {
            min: PhysicalLength::new(min) / scale_factor,
            max: PhysicalLength::new(max) / scale_factor,
        }
    };

    // Without a window there is no way to notice a scale factor change, and the shaped
    // advances are in physical pixels, so an entry kept across one would be wrong.
    let Some((cache, window_adapter)) = cache.zip(window_adapter) else {
        return Some(compute());
    };
    cache.clear_if_scale_factor_changed(window_adapter.window());
    Some(cache.content_widths(item_rc, compute))
}

pub fn char_size(
    font_ctx: &mut FontContext,
    text_item: Pin<&dyn crate::item_rendering::RenderString>,
    item_rc: &crate::item_tree::ItemRc,
    ch: char,
) -> Option<LogicalSize> {
    let font_request = text_item.font_request(item_rc);
    let font = &font_ctx.query_font(&font_request)?.font;

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
    let natural_line_height = font_metrics.ascent - font_metrics.descent;
    let line_height = natural_line_height * text_item.line_height_factor().unwrap_or(1.0);

    Some(LogicalSize::from_lengths(advance_width, LogicalLength::new(line_height)))
}

/// The height of one line of text: what a shaped single-line layout reports.
pub fn text_line_height(
    font_ctx: &mut FontContext,
    font_request: &FontRequest,
    line_height_factor: Option<f32>,
) -> Option<LogicalLength> {
    let pixel_size = font_request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE);
    shaping::line_height_ratio(font_ctx, font_request, line_height_factor)
        .map(|ratio| pixel_size * ratio)
}

pub fn font_metrics(
    font_ctx: &mut FontContext,
    font_request: FontRequest,
) -> crate::items::FontMetrics {
    let logical_pixel_size = font_request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE).get();

    let Some(font) = font_ctx.query_font(&font_request) else {
        return crate::items::FontMetrics::default();
    };
    let font = &font.font;

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
    renderer: &(impl RendererSealed + ?Sized),
    text_input: Pin<&crate::items::TextInput>,
    item_rc: &crate::item_tree::ItemRc,
    pos: LogicalPoint,
    cache: Option<&TextLayoutCache>,
) -> (usize, crate::items::TextCursorAffinity) {
    text_input_byte_offset_for_position_impl(
        renderer.scale_factor(),
        renderer.window_adapter(),
        text_input,
        item_rc,
        pos,
        cache,
    )
}

fn text_input_byte_offset_for_position_impl(
    scale_factor: Option<ScaleFactor>,
    window_adapter: Option<Rc<dyn WindowAdapter>>,
    text_input: Pin<&crate::items::TextInput>,
    item_rc: &crate::item_tree::ItemRc,
    pos: LogicalPoint,
    cache: Option<&TextLayoutCache>,
) -> (usize, crate::items::TextCursorAffinity) {
    let no_hit = (0, crate::items::TextCursorAffinity::NextCharacter);
    let Some(scale_factor) = scale_factor else {
        return no_hit;
    };
    let pos: PhysicalPoint = pos * scale_factor;

    let width = text_input.width();
    let height = text_input.height();
    if width.get() <= 0. || height.get() <= 0. || pos.y < 0. {
        return no_hit;
    }

    let layout_builder =
        shaping_builder(text_input, Some(item_rc), text_input.wrap(), scale_factor);
    let visual_representation = text_input.visual_representation();

    let Some(window_adapter) = window_adapter else {
        return no_hit;
    };

    let (byte_offset, affinity) = with_text_layout(
        cache,
        Some(item_rc),
        text_input,
        &layout_builder,
        LayoutOptions::new_from_textinput(text_input, Some(width), Some(height)),
        window_adapter.window(),
        |layout| layout.byte_offset_from_point(pos),
    )
    .unwrap_or(no_hit);
    (visual_representation.map_byte_offset_from_visual_text_to_actual_text(byte_offset), affinity)
}

pub fn text_input_cursor_rect_for_byte_offset(
    renderer: &(impl RendererSealed + ?Sized),
    text_input: Pin<&crate::items::TextInput>,
    item_rc: &crate::item_tree::ItemRc,
    byte_offset: usize,
    affinity: crate::items::TextCursorAffinity,
    cache: Option<&TextLayoutCache>,
) -> LogicalRect {
    text_input_cursor_rect_for_byte_offset_impl(
        renderer.scale_factor(),
        renderer.window_adapter(),
        text_input,
        item_rc,
        byte_offset,
        affinity,
        cache,
    )
}

fn text_input_cursor_rect_for_byte_offset_impl(
    scale_factor: Option<ScaleFactor>,
    window_adapter: Option<Rc<dyn WindowAdapter>>,
    text_input: Pin<&crate::items::TextInput>,
    item_rc: &crate::item_tree::ItemRc,
    byte_offset: usize,
    affinity: crate::items::TextCursorAffinity,
    cache: Option<&TextLayoutCache>,
) -> LogicalRect {
    let Some(scale_factor) = scale_factor else {
        return LogicalRect::default();
    };

    let layout_builder =
        shaping_builder(text_input, Some(item_rc), text_input.wrap(), scale_factor);

    let width = text_input.width();
    let height = text_input.height();
    if width.get() <= 0. || height.get() <= 0. {
        return LogicalRect::new(
            LogicalPoint::default(),
            LogicalSize::from_lengths(LogicalLength::new(1.0), layout_builder.pixel_size),
        );
    }

    let visual_representation = text_input.visual_representation();
    let cursor_width = text_input.text_cursor_width() * scale_factor;

    let Some(window_adapter) = window_adapter else {
        return LogicalRect::default();
    };

    let byte_offset = visual_representation.map_byte_offset_from_actual_to_visual_text(byte_offset);

    with_text_layout(
        cache,
        Some(item_rc),
        text_input,
        &layout_builder,
        LayoutOptions::new_from_textinput(text_input, Some(width), Some(height)),
        window_adapter.window(),
        |layout| {
            layout.cursor_rect_for_byte_offset(byte_offset, affinity, cursor_width) / scale_factor
        },
    )
    .unwrap_or_default()
}

/// A `TextInput`'s laid-out text, lent to [`with_text_input_layout`]'s callback for one call.
#[allow(dead_code)]
pub struct TextInputLayout<'a> {
    layout: &'a Layout,
    /// The string the paragraphs were shaped from, which [`TextInputParagraph::range`] indexes.
    text: &'a str,
}

#[allow(dead_code)]
impl<'a> TextInputLayout<'a> {
    /// The paragraphs, top to bottom. A hard line break separates two of them and belongs to
    /// neither, since Slint splits the text at `\n` before shaping.
    pub(crate) fn paragraphs(&self) -> impl Iterator<Item = TextInputParagraph<'a>> {
        let (text, y_offset) = (self.text, self.layout.y_offset);
        self.layout.paragraphs.iter().map(move |para| TextInputParagraph {
            range: para.range.clone(),
            text: &text[para.range.clone()],
            layout: &para.layout,
            y: y_offset + para.y,
        })
    }
}

/// One paragraph of a [`TextInputLayout`].
#[allow(dead_code)]
pub(crate) struct TextInputParagraph<'a> {
    /// Byte range within [`TextInputLayout::text`].
    range: Range<usize>,
    /// The slice of that text this paragraph covers.
    text: &'a str,
    /// Its shaped, line-broken and aligned glyphs.
    layout: &'a parley::Layout<Brush>,
    /// Physical y of its top edge, relative to the item's.
    y: PhysicalLength,
}

/// Lays `text_input` out the way `renderer` draws it and lends the result to `f`.
///
/// `f` must not lay text out itself: the cache entry stays checked out for the call, and
/// re-entering it panics.
///
/// Returns `None` if the renderer lays no text out through parley, so the caller can tell that
/// apart from an empty layout.
pub fn with_text_input_layout<R>(
    renderer: &(impl RendererSealed + ?Sized),
    text_input: Pin<&crate::items::TextInput>,
    item_rc: &crate::item_tree::ItemRc,
    size: LogicalSize,
    f: impl FnOnce(TextInputLayout<'_>) -> R,
) -> Option<R> {
    if !renderer.text_input_has_parley_layout(text_input, item_rc) {
        return None;
    }
    with_text_input_layout_impl(
        renderer.scale_factor(),
        renderer.window_adapter(),
        renderer.text_layout_cache(),
        text_input,
        item_rc,
        size,
        f,
    )
}

fn with_text_input_layout_impl<R>(
    scale_factor: Option<ScaleFactor>,
    window_adapter: Option<Rc<dyn WindowAdapter>>,
    cache: Option<&TextLayoutCache>,
    text_input: Pin<&crate::items::TextInput>,
    item_rc: &crate::item_tree::ItemRc,
    size: LogicalSize,
    f: impl FnOnce(TextInputLayout<'_>) -> R,
) -> Option<R> {
    let scale_factor = scale_factor?;
    let window_adapter = window_adapter?;

    let width = size.width_length();
    let height = size.height_length();
    if width.get() <= 0. || height.get() <= 0. {
        return None;
    }

    let layout_builder =
        shaping_builder(text_input, Some(item_rc), text_input.wrap(), scale_factor);

    // `RenderString for TextInput` yields plain text; a styled input doesn't exist.
    let PlainOrStyledText::Plain(text) = crate::item_rendering::RenderString::text(text_input)
    else {
        return None;
    };

    with_text_layout(
        cache,
        Some(item_rc),
        text_input,
        &layout_builder,
        LayoutOptions::new_from_textinput(text_input, Some(width), Some(height)),
        window_adapter.window(),
        |layout| f(TextInputLayout { layout, text: &text }),
    )
}

#[cfg(feature = "accessibility-text")]
mod accessibility;
#[cfg(feature = "accessibility-text")]
pub use accessibility::CachedTextInputAccessibilityState;
