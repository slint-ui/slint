// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;

use euclid::approxeq::ApproxEq;
use i_slint_core::graphics::boxshadowcache::BoxShadowCache;
use i_slint_core::graphics::euclid::num::Zero;
use i_slint_core::graphics::euclid::{self};
use i_slint_core::graphics::rendering_metrics_collector::RenderingMetrics;
use i_slint_core::graphics::{IntRect, Point, Size};
use i_slint_core::item_rendering::{
    CachedRenderingData, ItemCache, ItemRenderer, RenderBorderRectangle, RenderImage,
    RenderRectangle, RenderText,
};
use i_slint_core::items::{
    self, Clip, FillRule, ImageRendering, ImageTiling, ItemRc, Layer, Opacity, RenderingResult,
    TextStrokeStyle,
};
use i_slint_core::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalRect, LogicalSize, LogicalVector,
    RectLengths, ScaleFactor, SizeLengths,
};
use i_slint_core::{Brush, Color, ImageInner, SharedString};

use crate::images::TextureImporter;

use super::images::{Texture, TextureCacheKey};
use super::PhysicalSize;
use super::{fonts, PhysicalBorderRadius, PhysicalLength, PhysicalPoint, PhysicalRect};

type FemtovgBoxShadowCache<R> = BoxShadowCache<ItemGraphicsCacheEntry<R>>;

pub use femtovg::Canvas;
pub type CanvasRc<R> = Rc<RefCell<Canvas<R>>>;

pub enum ItemGraphicsCacheEntry<R: femtovg::Renderer + TextureImporter> {
    Texture(Rc<Texture<R>>),
    ColorizedImage {
        // This original image Rc is kept here to keep the image in the shared image cache, so that
        // changes to the colorization brush will not require re-uploading the image.
        _original_image: Rc<Texture<R>>,
        colorized_image: Rc<Texture<R>>,
    },
}

impl<R: femtovg::Renderer + TextureImporter> Clone for ItemGraphicsCacheEntry<R> {
    fn clone(&self) -> Self {
        match self {
            Self::Texture(arg0) => Self::Texture(arg0.clone()),
            Self::ColorizedImage { _original_image, colorized_image } => Self::ColorizedImage {
                _original_image: _original_image.clone(),
                colorized_image: colorized_image.clone(),
            },
        }
    }
}

impl<R: femtovg::Renderer + TextureImporter> ItemGraphicsCacheEntry<R> {
    fn as_texture(&self) -> &Rc<Texture<R>> {
        match self {
            ItemGraphicsCacheEntry::Texture(image) => image,
            ItemGraphicsCacheEntry::ColorizedImage { colorized_image, .. } => colorized_image,
        }
    }
    fn is_colorized_image(&self) -> bool {
        matches!(self, ItemGraphicsCacheEntry::ColorizedImage { .. })
    }
}

pub(super) type ItemGraphicsCache<R> = ItemCache<Option<ItemGraphicsCacheEntry<R>>>;

const KAPPA90: f32 = 0.55228;

#[derive(Clone)]
struct State {
    scissor: LogicalRect,
    global_alpha: f32,
    current_render_target: femtovg::RenderTarget,
}

pub struct GLItemRenderer<'a, R: femtovg::Renderer + TextureImporter> {
    graphics_cache: &'a ItemGraphicsCache<R>,
    texture_cache: &'a RefCell<super::images::TextureCache<R>>,
    box_shadow_cache: FemtovgBoxShadowCache<R>,
    canvas: CanvasRc<R>,
    // Textures from layering or tiling that were scheduled for rendering where we can't delete the femtovg::ImageId yet
    // because that can only happen after calling `flush`. Otherwise femtovg ends up processing
    // `set_render_target` commands with image ids that have been deleted.
    textures_to_delete_after_flush: RefCell<Vec<Rc<super::images::Texture<R>>>>,
    window: &'a i_slint_core::api::Window,
    scale_factor: ScaleFactor,
    /// track the state manually since femtovg don't have accessor for its state
    state: Vec<State>,
    metrics: RenderingMetrics,
}

fn rect_with_radius_to_path(
    rect: PhysicalRect,
    border_radius: PhysicalBorderRadius,
) -> femtovg::Path {
    let mut path = femtovg::Path::new();
    let x = rect.origin.x;
    let y = rect.origin.y;
    let width = rect.size.width;
    let height = rect.size.height;
    if let Some(border_radius) = border_radius.as_uniform() {
        // If we're drawing a circle, use directly connected bezier curves instead of
        // ones with intermediate LineTo verbs, as `rounded_rect` creates, to avoid
        // rendering artifacts due to those edges.
        if width.approx_eq(&height) && (border_radius * 2.).approx_eq(&width) {
            path.circle(x + border_radius, y + border_radius, border_radius);
        } else {
            path.rounded_rect(x, y, width, height, border_radius);
        }
    } else {
        path.rounded_rect_varying(
            x,
            y,
            width,
            height,
            border_radius.top_left,
            border_radius.top_right,
            border_radius.bottom_right,
            border_radius.bottom_left,
        );
    }
    path
}

fn rect_to_path(r: PhysicalRect) -> femtovg::Path {
    rect_with_radius_to_path(r, PhysicalBorderRadius::default())
}

fn adjust_rect_and_border_for_inner_drawing(
    rect: &mut PhysicalRect,
    border_width: &mut PhysicalLength,
) {
    // If the border width exceeds the width, just fill the rectangle.
    *border_width = border_width.min(rect.width_length() / 2.);
    // adjust the size so that the border is drawn within the geometry

    rect.origin += PhysicalSize::from_lengths(*border_width / 2., *border_width / 2.);
    rect.size -= PhysicalSize::from_lengths(*border_width, *border_width);
}

fn path_bounding_box<R: femtovg::Renderer>(
    canvas: &CanvasRc<R>,
    path: &femtovg::Path,
) -> euclid::default::Box2D<f32> {
    // `canvas.path_bbox()` applies the current transform. However we're not interested in that, since
    // we operate in item local coordinates with the `path` parameter as well as the resulting
    // paint.
    let mut canvas = canvas.borrow_mut();
    canvas.save();
    canvas.reset_transform();
    let bounding_box = canvas.path_bbox(path);
    canvas.restore();
    euclid::default::Box2D::new(
        [bounding_box.minx, bounding_box.miny].into(),
        [bounding_box.maxx, bounding_box.maxy].into(),
    )
}

// Return a femtovg::Path (in physical pixels) that represents the clip_rect, radius and border_width (all logical!)
fn clip_path_for_rect_alike_item(
    clip_rect: LogicalRect,
    mut radius: LogicalBorderRadius,
    mut border_width: LogicalLength,
    scale_factor: ScaleFactor,
) -> femtovg::Path {
    // Femtovg renders evenly 50% inside and 50% outside of the border width. The
    // adjust_rect_and_border_for_inner_drawing adjusts the rect so that for drawing it
    // would be entirely an *inner* border. However for clipping we want the rect that's
    // entirely inside, hence the doubling of the width and consequently radius adjustment.
    radius -= LogicalBorderRadius::new_uniform(border_width.get() * KAPPA90);
    border_width *= 2.;

    // Convert from logical to physical pixels
    let mut border_width = border_width * scale_factor;
    let radius = radius * scale_factor;
    let mut clip_rect = clip_rect * scale_factor;

    adjust_rect_and_border_for_inner_drawing(&mut clip_rect, &mut border_width);

    rect_with_radius_to_path(clip_rect, radius)
}

impl<'a, R: femtovg::Renderer + TextureImporter> GLItemRenderer<'a, R> {
    pub fn global_alpha_transparent(&self) -> bool {
        self.state.last().unwrap().global_alpha == 0.0
    }
}

impl<'a, R: femtovg::Renderer + TextureImporter> ItemRenderer for GLItemRenderer<'a, R> {
    fn draw_rectangle(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        _: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let geometry = PhysicalRect::from(size * self.scale_factor);
        if geometry.is_empty() {
            return;
        }
        if self.global_alpha_transparent() {
            return;
        }
        // TODO: cache path in item to avoid re-tesselation
        let path = rect_to_path(geometry);
        let paint = match self.brush_to_paint(rect.background(), &path) {
            Some(paint) => paint,
            None => return,
        }
        // Since we're filling a straight rectangle with either color or gradient, save
        // the extra stroke triangle strip around the edges
        .with_anti_alias(false);
        self.canvas.borrow_mut().fill_path(&path, &paint);
    }

    fn draw_border_rectangle(
        &mut self,
        rect: Pin<&dyn RenderBorderRectangle>,
        _: &ItemRc,
        size: LogicalSize,
        _: &CachedRenderingData,
    ) {
        let mut geometry = PhysicalRect::from(size * self.scale_factor);
        if geometry.is_empty() {
            return;
        }
        if self.global_alpha_transparent() {
            return;
        }

        let border_color = rect.border_color();
        let opaque_border = border_color.is_opaque();
        let mut border_width = if border_color.is_transparent() {
            PhysicalLength::new(0.)
        } else {
            rect.border_width() * self.scale_factor
        };

        // Radius of rounded rect if we were to just fill the rectangle, without a border.
        let mut fill_radius = rect.border_radius() * self.scale_factor;

        // FemtoVG's border radius on stroke is in the middle of the border. But we want it to be the radius of the rectangle itself.
        // This is incorrect if fill_radius < border_width/2, but this can't be fixed. Better to have a radius a bit too big than no radius at all
        fill_radius = fill_radius.outer(border_width / 2. + PhysicalLength::new(1.));
        let stroke_border_radius = fill_radius.inner(border_width / 2.);

        // In case of a transparent border, we want the background to cover the whole rectangle, which is
        // not how femtovg's stroke works. So fill the background separately in the else branch if the
        // border is not opaque.
        let (mut background_path, mut maybe_border_path) = if opaque_border {
            // In CSS the border is entirely towards the inside of the boundary
            // geometry, while in femtovg the line with for a stroke is 50% in-
            // and 50% outwards. We choose the CSS model, so the inner rectangle
            // is adjusted accordingly.
            adjust_rect_and_border_for_inner_drawing(&mut geometry, &mut border_width);

            (rect_with_radius_to_path(geometry, stroke_border_radius), None)
        } else {
            let background_path = rect_with_radius_to_path(geometry, fill_radius);

            // In CSS the border is entirely towards the inside of the boundary
            // geometry, while in femtovg the line with for a stroke is 50% in-
            // and 50% outwards. We choose the CSS model, so the inner rectangle
            // is adjusted accordingly.
            adjust_rect_and_border_for_inner_drawing(&mut geometry, &mut border_width);

            let border_path = rect_with_radius_to_path(geometry, stroke_border_radius);

            (background_path, Some(border_path))
        };

        let fill_paint = self.brush_to_paint(rect.background(), &background_path);

        let border_paint = self
            .brush_to_paint(
                rect.border_color(),
                maybe_border_path.as_ref().unwrap_or(&background_path),
            )
            .map(|mut paint| {
                paint.set_line_width(border_width.get());
                paint
            });

        let mut canvas = self.canvas.borrow_mut();
        if let Some(paint) = fill_paint {
            canvas.fill_path(&background_path, &paint);
        }
        if let Some(border_paint) = border_paint {
            canvas.stroke_path(
                maybe_border_path.as_mut().unwrap_or(&mut background_path),
                &border_paint,
            );
        }
    }

    fn draw_window_background(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        // register a dependency for the partial renderer's dirty tracker. The actual rendering is done earlier in SkiaRenderer.
        let _ = rect.background();
    }

    fn draw_image(
        &mut self,
        image: Pin<&dyn RenderImage>,
        item_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        self.draw_image_impl(item_rc, image, size);
    }

    fn draw_text(
        &mut self,
        text: Pin<&dyn RenderText>,
        self_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let max_width = size.width_length() * self.scale_factor;
        let max_height = size.height_length() * self.scale_factor;

        if max_width.get() <= 0. || max_height.get() <= 0. {
            return;
        }

        if self.global_alpha_transparent() {
            return;
        }

        let string = text.text();
        let string = string.as_str();
        let font = fonts::FONT_CACHE.with(|cache| {
            cache.borrow_mut().font(text.font_request(self_rc), self.scale_factor, &text.text())
        });

        let text_path = rect_to_path((size * self.scale_factor).into());
        let paint = match self.brush_to_paint(text.color(), &text_path) {
            Some(paint) => font.init_paint(text.letter_spacing() * self.scale_factor, paint),
            None => return,
        };

        let (stroke_brush, stroke_width, stroke_style) = text.stroke();
        let stroke_width = if stroke_width.get() != 0.0 {
            (stroke_width * self.scale_factor).get()
        } else {
            // Hairline stroke
            1.0
        };
        let stroke_width = match stroke_style {
            TextStrokeStyle::Outside => stroke_width * 2.0,
            TextStrokeStyle::Center => stroke_width,
        };
        let stroke_paint = match self.brush_to_paint(stroke_brush.clone(), &text_path) {
            Some(mut paint) => {
                if stroke_brush.is_transparent() {
                    None
                } else {
                    paint.set_line_width(stroke_width);
                    Some(font.init_paint(text.letter_spacing() * self.scale_factor, paint))
                }
            }
            None => None,
        };

        let mut canvas = self.canvas.borrow_mut();
        fonts::layout_text_lines(
            string,
            &font,
            PhysicalSize::from_lengths(max_width, max_height),
            text.alignment(),
            text.wrap(),
            text.overflow(),
            false,
            None,
            &paint,
            |to_draw, pos, _, _| {
                match (stroke_style, &stroke_paint) {
                    (TextStrokeStyle::Outside, Some(stroke_paint)) => {
                        canvas.stroke_text(pos.x, pos.y, to_draw.trim_end(), stroke_paint).unwrap();
                        canvas.fill_text(pos.x, pos.y, to_draw.trim_end(), &paint).unwrap();
                    }
                    (TextStrokeStyle::Center, Some(stroke_paint)) => {
                        canvas.fill_text(pos.x, pos.y, to_draw.trim_end(), &paint).unwrap();
                        canvas.stroke_text(pos.x, pos.y, to_draw.trim_end(), stroke_paint).unwrap();
                    }
                    _ => {
                        canvas.fill_text(pos.x, pos.y, to_draw.trim_end(), &paint).unwrap();
                    }
                };
            },
        );
    }

    fn draw_text_input(
        &mut self,
        text_input: Pin<&items::TextInput>,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) {
        let width = size.width_length() * self.scale_factor;
        let height = size.height_length() * self.scale_factor;
        if width.get() <= 0. || height.get() <= 0. {
            return;
        }

        if self.global_alpha_transparent() {
            return;
        }

        let font = fonts::FONT_CACHE.with(|cache| {
            cache.borrow_mut().font(
                text_input.font_request(self_rc),
                self.scale_factor,
                &text_input.text(),
            )
        });

        let visual_representation = text_input.visual_representation(None);

        let paint = match self.brush_to_paint(
            visual_representation.text_color,
            &rect_to_path((size * self.scale_factor).into()),
        ) {
            Some(paint) => font.init_paint(text_input.letter_spacing() * self.scale_factor, paint),
            None => return,
        };

        let (min_select, max_select) = if !visual_representation.preedit_range.is_empty() {
            (visual_representation.preedit_range.start, visual_representation.preedit_range.end)
        } else {
            (visual_representation.selection_range.start, visual_representation.selection_range.end)
        };

        let (cursor_visible, cursor_pos) =
            if let Some(cursor_pos) = visual_representation.cursor_position {
                (true, cursor_pos)
            } else {
                (false, 0)
            };

        let mut canvas = self.canvas.borrow_mut();
        let font_height = font.height();
        let text: SharedString = visual_representation.text.into();

        let cursor_point = fonts::layout_text_lines(
            text.as_str(),
            &font,
            PhysicalSize::from_lengths(width, height),
            (text_input.horizontal_alignment(), text_input.vertical_alignment()),
            text_input.wrap(),
            items::TextOverflow::Clip,
            text_input.single_line(),
            cursor_visible.then_some(cursor_pos),
            &paint,
            |to_draw: &str, pos: PhysicalPoint, start, metrics: &femtovg::TextMetrics| {
                let range = start..(start + to_draw.len());
                if min_select != max_select
                    && (range.contains(&min_select)
                        || range.contains(&max_select)
                        || (min_select..max_select).contains(&start))
                {
                    let mut selection_start_x = PhysicalLength::default();
                    let mut selection_end_x = PhysicalLength::default();
                    let mut after_selection_x = PhysicalLength::default();
                    // Determine the first and last (inclusive) glyph of the selection. The anchor
                    // will always be at the start of a grapheme boundary, so there's at ShapedGlyph
                    // that has a matching byte index. For the selection end we have to look for the
                    // visual end of glyph before the cursor, because due to for example ligatures
                    // (or generally glyph substitution) there may not be a dedicated glyph.
                    // FIXME: in the case of ligature, there is currently no way to know the exact
                    // position of the split. When we know it, we might need to draw in two
                    // steps with clip to draw each part of the ligature in a different color
                    for glyph in &metrics.glyphs {
                        if glyph.byte_index == min_select.saturating_sub(start) {
                            selection_start_x = PhysicalLength::new(glyph.x - glyph.bearing_x);
                        }
                        if glyph.byte_index == max_select - start
                            || glyph.byte_index >= to_draw.len()
                        {
                            after_selection_x = PhysicalLength::new(glyph.x - glyph.bearing_x);
                            break;
                        }
                        selection_end_x = PhysicalLength::new(glyph.x + glyph.advance_x);
                    }

                    let selection_rect = PhysicalRect::new(
                        pos + PhysicalPoint::from_lengths(
                            selection_start_x,
                            PhysicalLength::default(),
                        )
                        .to_vector(),
                        PhysicalSize::from_lengths(
                            selection_end_x - selection_start_x,
                            font_height,
                        ),
                    );
                    canvas.fill_path(
                        &rect_to_path(selection_rect),
                        &femtovg::Paint::color(to_femtovg_color(
                            &text_input.selection_background_color(),
                        )),
                    );
                    let mut selected_paint = paint.clone();
                    selected_paint
                        .set_color(to_femtovg_color(&text_input.selection_foreground_color()));
                    canvas
                        .fill_text(
                            pos.x,
                            pos.y,
                            to_draw[..min_select.saturating_sub(start)].trim_end(),
                            &paint,
                        )
                        .unwrap();
                    canvas
                        .fill_text(
                            pos.x + selection_start_x.get(),
                            pos.y,
                            to_draw[min_select.saturating_sub(start)
                                ..(max_select - start).min(to_draw.len())]
                                .trim_end(),
                            &selected_paint,
                        )
                        .unwrap();
                    canvas
                        .fill_text(
                            pos.x + after_selection_x.get(),
                            pos.y,
                            to_draw[(max_select - start).min(to_draw.len())..].trim_end(),
                            &paint,
                        )
                        .unwrap();
                } else {
                    // no selection on this line
                    canvas.fill_text(pos.x, pos.y, to_draw.trim_end(), &paint).unwrap();
                };
            },
        );

        if let Some(cursor_point) = cursor_point {
            let mut cursor_rect = femtovg::Path::new();
            cursor_rect.rect(
                cursor_point.x,
                cursor_point.y,
                (text_input.text_cursor_width() * self.scale_factor).get(),
                font_height.get(),
            );
            canvas.fill_path(
                &cursor_rect,
                &femtovg::Paint::color(to_femtovg_color(&visual_representation.cursor_color)),
            );
        }
    }

    fn draw_path(&mut self, path: Pin<&items::Path>, item_rc: &ItemRc, _size: LogicalSize) {
        if self.global_alpha_transparent() {
            return;
        }

        let (offset, path_events) = match path.fitted_path_events(item_rc) {
            Some(offset_and_events) => offset_and_events,
            None => return,
        };

        let mut femtovg_path = femtovg::Path::new();

        /// Contrary to the SVG spec, femtovg does not use the orientation of the path to
        /// know if it needs to fill or not some part, it uses its own Solidity enum.
        /// We must then compute ourself the orientation and set the Solidity accordingly.
        #[derive(Default)]
        struct OrientationCalculator {
            area: f32,
            prev: Point,
        }

        impl OrientationCalculator {
            fn add_point(&mut self, p: Point) {
                self.area += (p.x - self.prev.x) * (p.y + self.prev.y);
                self.prev = p;
            }
        }

        use femtovg::Solidity;

        let mut orient = OrientationCalculator::default();

        for x in path_events.iter() {
            match x {
                lyon_path::Event::Begin { at } => {
                    femtovg_path.solidity(if orient.area < 0. {
                        Solidity::Hole
                    } else {
                        Solidity::Solid
                    });
                    femtovg_path
                        .move_to(at.x * self.scale_factor.get(), at.y * self.scale_factor.get());
                    orient.area = 0.;
                    orient.prev = at;
                }
                lyon_path::Event::Line { from: _, to } => {
                    femtovg_path
                        .line_to(to.x * self.scale_factor.get(), to.y * self.scale_factor.get());
                    orient.add_point(to);
                }
                lyon_path::Event::Quadratic { from: _, ctrl, to } => {
                    femtovg_path.quad_to(
                        ctrl.x * self.scale_factor.get(),
                        ctrl.y * self.scale_factor.get(),
                        to.x * self.scale_factor.get(),
                        to.y * self.scale_factor.get(),
                    );
                    orient.add_point(to);
                }

                lyon_path::Event::Cubic { from: _, ctrl1, ctrl2, to } => {
                    femtovg_path.bezier_to(
                        ctrl1.x * self.scale_factor.get(),
                        ctrl1.y * self.scale_factor.get(),
                        ctrl2.x * self.scale_factor.get(),
                        ctrl2.y * self.scale_factor.get(),
                        to.x * self.scale_factor.get(),
                        to.y * self.scale_factor.get(),
                    );
                    orient.add_point(to);
                }
                lyon_path::Event::End { last: _, first: _, close } => {
                    femtovg_path.solidity(if orient.area < 0. {
                        Solidity::Hole
                    } else {
                        Solidity::Solid
                    });
                    if close {
                        femtovg_path.close()
                    }
                }
            }
        }

        let anti_alias = path.anti_alias();

        let fill_paint = self.brush_to_paint(path.fill(), &femtovg_path).map(|mut fill_paint| {
            fill_paint.set_fill_rule(match path.fill_rule() {
                FillRule::Nonzero => femtovg::FillRule::NonZero,
                FillRule::Evenodd => femtovg::FillRule::EvenOdd,
            });
            fill_paint.set_anti_alias(anti_alias);
            fill_paint
        });

        let border_paint = self.brush_to_paint(path.stroke(), &femtovg_path).map(|mut paint| {
            paint.set_line_width((path.stroke_width() * self.scale_factor).get());
            paint.set_line_cap(match path.stroke_line_cap() {
                items::LineCap::Butt => femtovg::LineCap::Butt,
                items::LineCap::Round => femtovg::LineCap::Round,
                items::LineCap::Square => femtovg::LineCap::Square,
            });
            paint.set_anti_alias(anti_alias);
            paint
        });

        self.canvas.borrow_mut().save_with(|canvas| {
            canvas.translate(offset.x, offset.y);
            if let Some(fill_paint) = &fill_paint {
                canvas.fill_path(&femtovg_path, fill_paint);
            }
            if let Some(border_paint) = &border_paint {
                canvas.stroke_path(&femtovg_path, border_paint);
            }
        })
    }

    /// Draws a rectangular shadow shape, which is usually placed underneath another rectangular shape
    /// with an offset (the drop-shadow-offset-x/y). The algorithm follows the HTML Canvas spec 4.12.5.1.18:
    ///  * Create a new image to cache the shadow rendering
    ///  * Fill the image with transparent "black"
    ///  * Draw the (rounded) rectangle at shadow offset_x/offset_y
    ///  * Blur the image
    ///  * Fill the image with the shadow color and SourceIn as composition mode
    ///  * Draw the shadow image
    fn draw_box_shadow(
        &mut self,
        box_shadow: Pin<&items::BoxShadow>,
        item_rc: &ItemRc,
        _size: LogicalSize,
    ) {
        if box_shadow.color().alpha() == 0
            || (box_shadow.blur() == LogicalLength::zero()
                && box_shadow.offset_x() == LogicalLength::zero()
                && box_shadow.offset_y() == LogicalLength::zero())
        {
            return;
        }

        let cache_entry = self.box_shadow_cache.get_box_shadow(
            item_rc,
            self.graphics_cache,
            box_shadow,
            self.scale_factor,
            |shadow_options| {
                let blur = shadow_options.blur;
                let width = shadow_options.width;
                let height = shadow_options.height;
                let radius = shadow_options.radius;

                let shadow_rect = PhysicalRect::new(
                    PhysicalPoint::default(),
                    PhysicalSize::from_lengths(width + blur * 2., height + blur * 2.),
                );

                let shadow_image_width = shadow_rect.width().ceil() as u32;
                let shadow_image_height = shadow_rect.height().ceil() as u32;

                let shadow_image = Texture::new_empty_on_gpu(
                    &self.canvas,
                    shadow_image_width,
                    shadow_image_height,
                )?;

                {
                    let mut canvas = self.canvas.borrow_mut();
                    canvas.save();

                    canvas.set_render_target(shadow_image.as_render_target());

                    canvas.reset();

                    canvas.clear_rect(
                        0,
                        0,
                        shadow_rect.width().ceil() as u32,
                        shadow_rect.height().ceil() as u32,
                        femtovg::Color::rgba(0, 0, 0, 0),
                    );

                    let mut shadow_path = femtovg::Path::new();
                    shadow_path.rounded_rect(
                        blur.get(),
                        blur.get(),
                        width.get(),
                        height.get(),
                        radius.get(),
                    );
                    canvas.fill_path(
                        &shadow_path,
                        &femtovg::Paint::color(femtovg::Color::rgb(255, 255, 255)),
                    );
                }

                let shadow_image = if blur.get() > 0. {
                    let blurred_image = shadow_image
                        .filter(femtovg::ImageFilter::GaussianBlur { sigma: blur.get() / 2. });

                    self.canvas.borrow_mut().set_render_target(blurred_image.as_render_target());

                    self.textures_to_delete_after_flush.borrow_mut().push(shadow_image);

                    blurred_image
                } else {
                    shadow_image
                };

                {
                    let mut canvas = self.canvas.borrow_mut();

                    canvas.global_composite_operation(femtovg::CompositeOperation::SourceIn);

                    let mut shadow_image_rect = femtovg::Path::new();
                    shadow_image_rect.rect(0., 0., shadow_rect.width(), shadow_rect.height());
                    canvas.fill_path(
                        &shadow_image_rect,
                        &femtovg::Paint::color(to_femtovg_color(&box_shadow.color())),
                    );

                    canvas.restore();

                    canvas.set_render_target(self.current_render_target());
                }

                Some(ItemGraphicsCacheEntry::Texture(shadow_image))
            },
        );

        let shadow_image = match &cache_entry {
            Some(cached_shadow_image) => cached_shadow_image.as_texture(),
            None => return, // Zero width or height shadow
        };

        let shadow_image_size = match shadow_image.size() {
            Some(size) => size,
            None => return,
        };

        // On the paint for the box shadow, we don't need anti-aliasing on the fringes,
        // since we are just blitting a texture. This saves a triangle strip for the stroke.
        let shadow_image_paint = shadow_image.as_paint().with_anti_alias(false);

        let mut shadow_image_rect = femtovg::Path::new();
        shadow_image_rect.rect(
            0.,
            0.,
            shadow_image_size.width as f32,
            shadow_image_size.height as f32,
        );

        self.canvas.borrow_mut().save_with(|canvas| {
            let blur = box_shadow.blur() * self.scale_factor;
            let offset = LogicalPoint::from_lengths(box_shadow.offset_x(), box_shadow.offset_y())
                * self.scale_factor;
            canvas.translate(offset.x - blur.get(), offset.y - blur.get());
            canvas.fill_path(&shadow_image_rect, &shadow_image_paint);
        });
    }

    fn visit_opacity(
        &mut self,
        opacity_item: Pin<&Opacity>,
        item_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        let opacity = opacity_item.opacity();
        if Opacity::need_layer(item_rc, opacity) {
            self.render_and_blend_layer(opacity, item_rc)
        } else {
            self.apply_opacity(opacity);
            self.graphics_cache.release(item_rc);
            RenderingResult::ContinueRenderingChildren
        }
    }

    fn visit_layer(
        &mut self,
        layer_item: Pin<&Layer>,
        self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        if layer_item.cache_rendering_hint() {
            self.render_and_blend_layer(1.0, self_rc)
        } else {
            self.graphics_cache.release(self_rc);
            RenderingResult::ContinueRenderingChildren
        }
    }

    fn visit_clip(
        &mut self,
        clip_item: Pin<&Clip>,
        item_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        if !clip_item.clip() {
            return RenderingResult::ContinueRenderingChildren;
        }

        // Note: This is correct, combine_clip and get_current_clip operate on logical coordinates.
        let geometry = LogicalRect::from(size);

        // If clipping is enabled but the clip element is outside the visible range, then we don't
        // need to bother doing anything, not even rendering the children.
        if !self.get_current_clip().intersects(&geometry) {
            return RenderingResult::ContinueRenderingWithoutChildren;
        }

        let radius = clip_item.logical_border_radius();
        let border_width = clip_item.border_width();

        if !radius.is_zero() {
            if let Some(layer_image) = self.render_layer(item_rc, &|| item_rc.geometry().size) {
                let layer_image_paint = layer_image.as_paint();

                let layer_path = clip_path_for_rect_alike_item(
                    geometry,
                    radius,
                    border_width,
                    self.scale_factor,
                );

                self.canvas.borrow_mut().fill_path(&layer_path, &layer_image_paint);
            }

            RenderingResult::ContinueRenderingWithoutChildren
        } else {
            self.graphics_cache.release(item_rc);
            self.combine_clip(geometry, radius, border_width);
            RenderingResult::ContinueRenderingChildren
        }
    }

    fn combine_clip(
        &mut self,
        clip_rect: LogicalRect,
        radius: LogicalBorderRadius,
        border_width: LogicalLength,
    ) -> bool {
        let clip = &mut self.state.last_mut().unwrap().scissor;
        let clip_region_valid = match clip.intersection(&clip_rect) {
            Some(r) => {
                *clip = r;
                true
            }
            None => {
                *clip = LogicalRect::default();
                false
            }
        };

        let clip_path =
            clip_path_for_rect_alike_item(clip_rect, radius, border_width, self.scale_factor);

        let clip_path_bounds = path_bounding_box(&self.canvas, &clip_path);

        self.canvas.borrow_mut().intersect_scissor(
            clip_path_bounds.min.x,
            clip_path_bounds.min.y,
            clip_path_bounds.width(),
            clip_path_bounds.height(),
        );

        // femtovg only supports rectangular clipping. Non-rectangular clips must be handled via `apply_clip`,
        // which can render children into a layer.
        debug_assert!(radius.is_zero());

        clip_region_valid
    }

    fn get_current_clip(&self) -> LogicalRect {
        self.state.last().unwrap().scissor
    }

    fn save_state(&mut self) {
        self.canvas.borrow_mut().save();
        self.state.push(self.state.last().unwrap().clone());
    }

    fn restore_state(&mut self) {
        self.state.pop();
        self.canvas.borrow_mut().restore();
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor.get()
    }

    fn draw_cached_pixmap(
        &mut self,
        item_rc: &ItemRc,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        let canvas = &self.canvas;

        let cache_entry = self.graphics_cache.get_or_update_cache_entry(item_rc, || {
            let mut cached_image = None;
            update_fn(&mut |width: u32, height: u32, data: &[u8]| {
                use rgb::FromSlice;
                let img = imgref::Img::new(data.as_rgba(), width as usize, height as usize);
                if let Ok(image_id) =
                    canvas.borrow_mut().create_image(img, femtovg::ImageFlags::PREMULTIPLIED)
                {
                    cached_image =
                        Some(ItemGraphicsCacheEntry::Texture(Texture::adopt(canvas, image_id)))
                };
            });
            cached_image
        });
        let image_id = match cache_entry {
            Some(ItemGraphicsCacheEntry::Texture(image)) => image.id,
            Some(ItemGraphicsCacheEntry::ColorizedImage { .. }) => unreachable!(),
            None => return,
        };
        let mut canvas = self.canvas.borrow_mut();

        let image_info = canvas.image_info(image_id).unwrap();
        let (width, height) = (image_info.width() as f32, image_info.height() as f32);
        let fill_paint = femtovg::Paint::image(image_id, 0., 0., width, height, 0.0, 1.0);
        let mut path = femtovg::Path::new();
        path.rect(0., 0., width, height);
        canvas.fill_path(&path, &fill_paint);
    }

    fn draw_string(&mut self, string: &str, color: Color) {
        let font = fonts::FONT_CACHE
            .with(|cache| cache.borrow_mut().font(Default::default(), self.scale_factor, string));
        let paint = font
            .init_paint(PhysicalLength::default(), femtovg::Paint::color(to_femtovg_color(&color)));
        let mut canvas = self.canvas.borrow_mut();
        canvas.fill_text(0., 0., string, &paint).unwrap();
    }

    fn draw_image_direct(&mut self, image: i_slint_core::graphics::Image) {
        let image_size = image.size();
        let target_width = LogicalLength::new(image_size.width as _);
        let target_height = LogicalLength::new(image_size.height as _);

        let target_w = target_width * self.scale_factor;
        let target_h = target_height * self.scale_factor;

        if target_w.get() <= 0. || target_h.get() <= 0. {
            return;
        }

        let image_inner: &ImageInner = (&image).into();

        let Some(cached_image) =
            TextureCacheKey::new(image_inner, None, Default::default(), Default::default())
                .and_then(|cache_key| {
                    self.texture_cache.borrow_mut().lookup_image_in_cache_or_create(
                        cache_key,
                        || {
                            Texture::new_from_image(
                                image_inner,
                                &self.canvas,
                                None,
                                Default::default(),
                                Default::default(),
                            )
                        },
                    )
                })
                .or_else(|| {
                    Texture::new_from_image(
                        image_inner,
                        &self.canvas,
                        None,
                        Default::default(),
                        Default::default(),
                    )
                })
        else {
            return;
        };

        let image_id = cached_image.id;
        let image_size = cached_image.size().unwrap_or_default().cast();

        let (source_width, source_height) = (image_size.width, image_size.height);

        let fill_paint =
            femtovg::Paint::image(image_id, 0., 0., image_size.width, image_size.height, 0.0, 1.0)
                // We preserve the rectangular shape of the image, so there's no need to apply anti-aliasing
                // at the edges
                .with_anti_alias(false);

        let mut path = femtovg::Path::new();
        path.rect(0., 0., source_width, source_height);

        self.canvas.borrow_mut().save_with(|canvas| {
            canvas.fill_path(&path, &fill_paint);
        })
    }

    fn window(&self) -> &i_slint_core::window::WindowInner {
        i_slint_core::window::WindowInner::from_pub(self.window)
    }

    fn as_any(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }

    fn translate(&mut self, distance: LogicalVector) {
        let physical_distance = distance * self.scale_factor;
        self.canvas.borrow_mut().translate(physical_distance.x, physical_distance.y);
        let clip = &mut self.state.last_mut().unwrap().scissor;
        *clip = clip.translate(-distance)
    }

    fn rotate(&mut self, angle_in_degrees: f32) {
        let angle_in_radians = angle_in_degrees.to_radians();
        self.canvas.borrow_mut().rotate(angle_in_radians);
        let clip = &mut self.state.last_mut().unwrap().scissor;
        // Compute the bounding box of the rotated rectangle
        let (sin, cos) = angle_in_radians.sin_cos();
        let rotate_point = |p: LogicalPoint| (p.x * cos - p.y * sin, p.x * sin + p.y * cos);
        let corners = [
            rotate_point(clip.origin),
            rotate_point(clip.origin + euclid::vec2(clip.width(), 0.)),
            rotate_point(clip.origin + euclid::vec2(0., clip.height())),
            rotate_point(clip.origin + clip.size),
        ];
        let origin: LogicalPoint = (
            corners.iter().fold(f32::MAX, |a, b| b.0.min(a)),
            corners.iter().fold(f32::MAX, |a, b| b.1.min(a)),
        )
            .into();
        let end: LogicalPoint = (
            corners.iter().fold(f32::MIN, |a, b| b.0.max(a)),
            corners.iter().fold(f32::MIN, |a, b| b.1.max(a)),
        )
            .into();
        *clip = LogicalRect::new(origin, (end - origin).into());
    }

    fn scale(&mut self, x_factor: f32, y_factor: f32) {
        self.canvas.borrow_mut().scale(x_factor, y_factor);
        let clip = &mut self.state.last_mut().unwrap().scissor;
        clip.size.width /= x_factor;
        clip.size.height /= y_factor;
    }

    fn apply_opacity(&mut self, opacity: f32) {
        let state = &mut self.state.last_mut().unwrap().global_alpha;
        *state *= opacity;
        self.canvas.borrow_mut().set_global_alpha(*state);
    }

    fn metrics(&self) -> RenderingMetrics {
        self.metrics.clone()
    }
}

impl<'a, R: femtovg::Renderer + TextureImporter> GLItemRenderer<'a, R> {
    pub(super) fn new(
        canvas: &CanvasRc<R>,
        graphics_cache: &'a ItemGraphicsCache<R>,
        texture_cache: &'a RefCell<super::images::TextureCache<R>>,
        window: &'a i_slint_core::api::Window,
        width: u32,
        height: u32,
    ) -> Self {
        let scale_factor = ScaleFactor::new(window.scale_factor());
        Self {
            graphics_cache,
            texture_cache,
            box_shadow_cache: Default::default(),
            canvas: canvas.clone(),
            textures_to_delete_after_flush: Default::default(),
            window,
            scale_factor,
            state: vec![State {
                scissor: LogicalRect::new(
                    LogicalPoint::default(),
                    PhysicalSize::new(width as f32, height as f32) / scale_factor,
                ),
                global_alpha: 1.,
                current_render_target: femtovg::RenderTarget::Screen,
            }],
            metrics: RenderingMetrics { layers_created: Some(0) },
        }
    }

    fn render_layer(
        &mut self,
        item_rc: &ItemRc,
        layer_logical_size_fn: &dyn Fn() -> LogicalSize,
    ) -> Option<Rc<Texture<R>>> {
        let existing_layer_texture =
            self.graphics_cache.with_entry(item_rc, |cache_entry| match cache_entry {
                Some(ItemGraphicsCacheEntry::Texture(texture)) => Some(texture.clone()),
                _ => None,
            });

        let cache_entry = self.graphics_cache.get_or_update_cache_entry(item_rc, || {
            ItemGraphicsCacheEntry::Texture({
                let size = (layer_logical_size_fn() * self.scale_factor).ceil().try_cast()?;

                let layer_image = existing_layer_texture
                    .and_then(|layer_texture| {
                        // If we have an existing layer texture, there must be only one reference from within
                        // the existing cache entry and one through the `existing_layer_texture` variable.
                        // Then it is safe to render new content into it in this callback and when we return
                        // into `get_or_update_cache_entry` the first ref is dropped.
                        debug_assert_eq!(Rc::strong_count(&layer_texture), 2);
                        if layer_texture.size() == Some(size.to_untyped()) {
                            Some(layer_texture)
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        *self.metrics.layers_created.as_mut().unwrap() += 1;
                        Texture::new_empty_on_gpu(&self.canvas, size.width, size.height)
                    })?;

                let previous_render_target = self.current_render_target();

                {
                    let mut canvas = self.canvas.borrow_mut();
                    canvas.save();

                    canvas.set_render_target(layer_image.as_render_target());

                    canvas.reset();

                    canvas.clear_rect(
                        0,
                        0,
                        size.width,
                        size.height,
                        femtovg::Color::rgba(0, 0, 0, 0),
                    );
                }

                *self.state.last_mut().unwrap() = State {
                    scissor: LogicalRect::new(
                        LogicalPoint::default(),
                        PhysicalSize::new(size.width as f32, size.height as f32)
                            / self.scale_factor,
                    ),
                    global_alpha: 1.,
                    current_render_target: layer_image.as_render_target(),
                };

                let window_adapter = self.window().window_adapter();

                i_slint_core::item_rendering::render_item_children(
                    self,
                    item_rc.item_tree(),
                    item_rc.index() as isize,
                    &window_adapter,
                );

                {
                    let mut canvas = self.canvas.borrow_mut();
                    canvas.restore();

                    canvas.set_render_target(previous_render_target);
                }

                layer_image
            })
            .into()
        });

        cache_entry.map(|item_cache_entry| item_cache_entry.as_texture().clone())
    }

    fn render_and_blend_layer(&mut self, alpha_tint: f32, item_rc: &ItemRc) -> RenderingResult {
        let current_clip = self.get_current_clip();
        if let Some((layer_image, layer_size)) = self
            .render_layer(item_rc, &|| {
                // We don't need to include the size of the opacity item itself, since it has no content.
                let children_rect = i_slint_core::properties::evaluate_no_tracking(|| {
                    item_rc.geometry().union(
                        &i_slint_core::item_rendering::item_children_bounding_rect(
                            item_rc.item_tree(),
                            item_rc.index() as isize,
                            &current_clip,
                        ),
                    )
                });
                children_rect.size
            })
            .and_then(|image| image.size().map(|size| (image, size)))
        {
            let mut layer_path = femtovg::Path::new();
            // On the paint for the layer, we don't need anti-aliasing on the fringes,
            // since we are just blitting a texture. This saves a triangle strip for the stroke.
            let layer_image_paint =
                layer_image.as_paint_with_alpha(alpha_tint).with_anti_alias(false);

            layer_path.rect(0., 0., layer_size.width as _, layer_size.height as _);
            self.canvas.borrow_mut().fill_path(&layer_path, &layer_image_paint);
        }
        RenderingResult::ContinueRenderingWithoutChildren
    }

    fn colorize_image(
        &self,
        original_cache_entry: ItemGraphicsCacheEntry<R>,
        colorize_brush: Brush,
        scaling: ImageRendering,
        tiling: (ImageTiling, ImageTiling),
    ) -> ItemGraphicsCacheEntry<R> {
        if colorize_brush.is_transparent() {
            return original_cache_entry;
        };
        let original_image = original_cache_entry.as_texture();

        let image_size = match original_image.size() {
            Some(size) => size,
            None => return original_cache_entry,
        };

        let scaling_flags = super::images::base_image_flags(scaling, tiling);

        let image_id = original_image.id;
        let colorized_image = self
            .canvas
            .borrow_mut()
            .create_image_empty(
                image_size.width as usize,
                image_size.height as usize,
                femtovg::PixelFormat::Rgba8,
                femtovg::ImageFlags::PREMULTIPLIED | scaling_flags,
            )
            .expect("internal error allocating temporary texture for image colorization");

        let image_size: Size = image_size.cast();

        let mut image_rect = femtovg::Path::new();
        image_rect.rect(0., 0., image_size.width, image_size.height);

        // We fill the entire image, there is no need to apply anti-aliasing around the edges
        let brush_paint = match self.brush_to_paint(colorize_brush, &image_rect) {
            Some(paint) => paint.with_anti_alias(false),
            None => return original_cache_entry,
        };

        self.canvas.borrow_mut().save_with(|canvas| {
            canvas.reset();
            canvas.scale(1., -1.); // Image are rendered upside down
            canvas.translate(0., -image_size.height);
            canvas.set_render_target(femtovg::RenderTarget::Image(colorized_image));

            canvas.global_composite_operation(femtovg::CompositeOperation::Copy);
            canvas.fill_path(
                &image_rect,
                &femtovg::Paint::image(
                    image_id,
                    0.,
                    0.,
                    image_size.width,
                    image_size.height,
                    0.,
                    1.0,
                ),
            );

            canvas.global_composite_operation(femtovg::CompositeOperation::SourceIn);
            canvas.fill_path(&image_rect, &brush_paint);

            canvas.set_render_target(self.current_render_target());
        });

        ItemGraphicsCacheEntry::ColorizedImage {
            _original_image: original_image.clone(),
            colorized_image: Texture::adopt(&self.canvas, colorized_image),
        }
    }

    fn draw_image_impl(
        &mut self,
        item_rc: &ItemRc,
        item: Pin<&dyn RenderImage>,
        size: LogicalSize,
    ) {
        if size.width <= 0. || size.height <= 0. {
            return;
        }

        let cached_image = loop {
            let image_cache_entry = self.graphics_cache.get_or_update_cache_entry(item_rc, || {
                let image = item.source();
                let image_inner: &ImageInner = (&image).into();
                let tiling = item.tiling();

                let target_size_for_scalable_source = if image_inner.is_svg() {
                    let image_size = image.size().cast::<f32>();
                    if image_size.is_empty() {
                        return None;
                    }
                    let t = item.target_size() * self.scale_factor;
                    let fit = i_slint_core::graphics::fit(
                        item.image_fit(),
                        t,
                        IntRect::from_size(image_size.cast()),
                        self.scale_factor,
                        Default::default(), // We only care about the size, so alignments don't matter
                        tiling,
                    );
                    Some(euclid::size2(
                        (image_size.width * fit.source_to_target_x) as u32,
                        (image_size.height * fit.source_to_target_y) as u32,
                    ))
                } else {
                    None
                };

                let image_rendering = item.rendering();

                TextureCacheKey::new(
                    image_inner,
                    target_size_for_scalable_source,
                    image_rendering,
                    tiling,
                )
                .and_then(|cache_key| {
                    self.texture_cache.borrow_mut().lookup_image_in_cache_or_create(
                        cache_key,
                        || {
                            Texture::new_from_image(
                                image_inner,
                                &self.canvas,
                                target_size_for_scalable_source,
                                image_rendering,
                                tiling,
                            )
                        },
                    )
                })
                .or_else(|| {
                    Texture::new_from_image(
                        image_inner,
                        &self.canvas,
                        target_size_for_scalable_source,
                        image_rendering,
                        tiling,
                    )
                })
                .map(ItemGraphicsCacheEntry::Texture)
                .map(|cache_entry| {
                    self.colorize_image(cache_entry, item.colorize(), image_rendering, tiling)
                })
            });

            // Check if the image in the cache is loaded. If not, don't draw any image and we'll return
            // later when the callback from load_html_image has issued a repaint
            let cached_image = match image_cache_entry {
                Some(entry) if entry.as_texture().size().is_some() => entry,
                _ => {
                    return;
                }
            };

            // It's possible that our item cache contains an image but it's not colorized yet because it was only
            // placed there via the `image_size` function (which doesn't colorize). So we may have to invalidate our
            // item cache and try again.
            if !cached_image.is_colorized_image() && !item.colorize().is_transparent() {
                self.graphics_cache.release(item_rc);
                continue;
            }

            break cached_image.as_texture().clone();
        };

        let image = item.source();
        let image_id = cached_image.id;
        let orig_size = image.size().cast::<f32>();
        let buf_size = cached_image.size().unwrap_or_default().cast::<f32>();
        let source_clip_rect = item.source_clip().unwrap_or(IntRect::from_size(orig_size.cast()));

        let image_inner: &ImageInner = (&image).into();
        let fits = if let ImageInner::NineSlice(nine) = image_inner {
            i_slint_core::graphics::fit9slice(
                image.size(),
                nine.1,
                size * self.scale_factor,
                self.scale_factor,
                item.alignment(),
                item.tiling(),
            )
            .collect::<Vec<_>>()
        } else {
            vec![i_slint_core::graphics::fit(
                item.image_fit(),
                size * self.scale_factor,
                source_clip_rect,
                self.scale_factor,
                item.alignment(),
                item.tiling(),
            )]
        };

        let scale_w = buf_size.width / orig_size.width;
        let scale_h = buf_size.height / orig_size.height;

        for fit in fits {
            let (image_id, origin, texture_size) =
                if fit.tiled.is_some() && fit.clip_rect.size.cast() != orig_size {
                    let scaling_flags = match item.rendering() {
                        ImageRendering::Smooth => femtovg::ImageFlags::empty(),
                        ImageRendering::Pixelated => {
                            femtovg::ImageFlags::empty() | femtovg::ImageFlags::NEAREST
                        }
                    };
                    let texture_size = euclid::size2(
                        scale_w * fit.clip_rect.width() as f32,
                        scale_h * fit.clip_rect.height() as f32,
                    );

                    let clipped_image = self
                        .canvas
                        .borrow_mut()
                        .create_image_empty(
                            texture_size.width as usize,
                            texture_size.height as usize,
                            femtovg::PixelFormat::Rgba8,
                            femtovg::ImageFlags::PREMULTIPLIED
                                | femtovg::ImageFlags::REPEAT_X
                                | femtovg::ImageFlags::REPEAT_Y
                                | scaling_flags,
                        )
                        .expect("internal error allocating temporary texture for image tiling");

                    let mut image_rect = femtovg::Path::new();
                    image_rect.rect(0., 0., texture_size.width, texture_size.height);
                    self.canvas.borrow_mut().save_with(|canvas| {
                        canvas.reset();
                        canvas.scale(1., -1.); // Image are rendered upside down
                        canvas.translate(0., -scale_h * (fit.clip_rect.height() as f32));
                        canvas.set_render_target(femtovg::RenderTarget::Image(clipped_image));
                        canvas.global_composite_operation(femtovg::CompositeOperation::Copy);
                        canvas.fill_path(
                            &image_rect,
                            &femtovg::Paint::image(
                                image_id,
                                -scale_w * fit.clip_rect.origin.x as f32,
                                -scale_h * fit.clip_rect.origin.y as f32,
                                buf_size.cast().width,
                                buf_size.cast().height,
                                0.,
                                1.0,
                            ),
                        );
                        canvas.set_render_target(self.current_render_target());
                    });
                    self.textures_to_delete_after_flush
                        .borrow_mut()
                        .push(Texture::adopt(&self.canvas, clipped_image));
                    (clipped_image, Default::default(), texture_size)
                } else {
                    (image_id, fit.clip_rect.origin.cast::<f32>(), buf_size)
                };
            let tiled = fit.tiled.unwrap_or_default();
            let fill_paint = femtovg::Paint::image(
                image_id,
                -origin.x - tiled.x as f32,
                -origin.y - tiled.y as f32,
                texture_size.width,
                texture_size.height,
                0.0,
                1.0,
            )
            .with_anti_alias(fit.source_to_target_x != 1. || fit.source_to_target_y != 0.);

            let mut path = femtovg::Path::new();
            path.rect(
                0.,
                0.,
                scale_w * fit.size.width / fit.source_to_target_x,
                scale_h * fit.size.height / fit.source_to_target_y,
            );

            self.canvas.borrow_mut().save_with(|canvas| {
                canvas.translate(fit.offset.x, fit.offset.y);
                canvas.scale(fit.source_to_target_x / scale_w, fit.source_to_target_y / scale_h);
                canvas.fill_path(&path, &fill_paint);
            })
        }
    }

    fn brush_to_paint(&self, brush: Brush, path: &femtovg::Path) -> Option<femtovg::Paint> {
        if brush.is_transparent() {
            return None;
        }
        Some(match brush {
            Brush::SolidColor(color) => femtovg::Paint::color(to_femtovg_color(&color)),
            Brush::LinearGradient(gradient) => {
                let path_bounds = path_bounding_box(&self.canvas, path);

                let path_width = path_bounds.width();
                let path_height = path_bounds.height();

                let (start, end) = i_slint_core::graphics::line_for_angle(
                    gradient.angle(),
                    [path_width, path_height].into(),
                );

                let mut stops: Vec<_> = gradient
                    .stops()
                    .map(|stop| (stop.position, to_femtovg_color(&stop.color)))
                    .collect();

                // Add an extra stop at 1.0 with the same color as the last stop
                if let Some(last_stop) = stops.last().cloned() {
                    if last_stop.0 != 1.0 {
                        stops.push((1.0, last_stop.1));
                    }
                }

                femtovg::Paint::linear_gradient_stops(start.x, start.y, end.x, end.y, stops)
            }
            Brush::RadialGradient(gradient) => {
                let path_bounds = path_bounding_box(&self.canvas, path);

                let path_width = path_bounds.width();
                let path_height = path_bounds.height();

                let mut stops: Vec<_> = gradient
                    .stops()
                    .map(|stop| (stop.position, to_femtovg_color(&stop.color)))
                    .collect();

                // Add an extra stop at 1.0 with the same color as the last stop
                if let Some(last_stop) = stops.last().cloned() {
                    if last_stop.0 != 1.0 {
                        stops.push((1.0, last_stop.1));
                    }
                }

                femtovg::Paint::radial_gradient_stops(
                    path_width / 2.,
                    path_height / 2.,
                    0.,
                    0.5 * (path_width * path_width + path_height * path_height).sqrt(),
                    stops,
                )
            }
            Brush::ConicGradient(gradient) => {
                let path_bounds = path_bounding_box(&self.canvas, path);

                let path_width = path_bounds.width();
                let path_height = path_bounds.height();

                let mut stops: Vec<_> = gradient
                    .stops()
                    .map(|stop| (stop.position, to_femtovg_color(&stop.color)))
                    .collect();

                // Add an extra stop at 1.0 with the same color as the last stop
                if let Some(last_stop) = stops.last().cloned() {
                    if last_stop.0 != 1.0 {
                        stops.push((1.0, last_stop.1));
                    }
                }

                femtovg::Paint::conic_gradient_stops(path_width / 2., path_height / 2., stops)
            }
            _ => return None,
        })
    }

    fn current_render_target(&self) -> femtovg::RenderTarget {
        self.state.last().unwrap().current_render_target
    }
}

pub fn to_femtovg_color(col: &Color) -> femtovg::Color {
    femtovg::Color::rgba(col.red(), col.green(), col.blue(), col.alpha())
}
