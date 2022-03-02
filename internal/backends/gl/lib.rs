// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

extern crate alloc;

use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;

use euclid::approxeq::ApproxEq;
use event_loop::WinitWindow;
use i_slint_core::graphics::{
    Brush, Color, Image, ImageInner, IntRect, IntSize, Point, Rect, RenderingCache, Size,
};
use i_slint_core::item_rendering::{CachedRenderingData, ItemRenderer};
use i_slint_core::items::{FillRule, ImageFit, ImageRendering, InputType};
use i_slint_core::properties::Property;
use i_slint_core::window::{Window, WindowRc};
use i_slint_core::SharedString;

mod glwindow;
use glwindow::*;
mod glcontext;
use glcontext::*;
pub(crate) mod event_loop;
mod images;
mod svg;
use images::*;

mod fonts;

mod stylemetrics;

type Canvas = femtovg::Canvas<femtovg::renderer::OpenGl>;
type CanvasRc = Rc<RefCell<Canvas>>;

const KAPPA90: f32 = 0.55228;

#[derive(Clone)]
enum ItemGraphicsCacheEntry {
    Image(Rc<CachedImage>),
    ColorizedImage {
        // This original image Rc is kept here to keep the image in the shared image cache, so that
        // changes to the colorization brush will not require re-uploading the image.
        _original_image: Rc<CachedImage>,
        colorized_image: Rc<CachedImage>,
    },
}

impl ItemGraphicsCacheEntry {
    fn as_image(&self) -> &Rc<CachedImage> {
        match self {
            ItemGraphicsCacheEntry::Image(image) => image,
            ItemGraphicsCacheEntry::ColorizedImage { colorized_image, .. } => colorized_image,
        }
    }
    fn is_colorized_image(&self) -> bool {
        matches!(self, ItemGraphicsCacheEntry::ColorizedImage { .. })
    }
}

type ItemGraphicsCache = RenderingCache<Option<ItemGraphicsCacheEntry>>;

// Layers are stored in the renderers State and flushed to the screen (or current rendering target)
// in restore_state() by filling the target_path.
struct Layer {
    image: CachedImage,
    target_path: femtovg::Path,
}

#[derive(Clone)]
struct State {
    scissor: Rect,
    global_alpha: f32,
    layer: Option<Rc<Layer>>,
}

pub struct GLItemRenderer {
    canvas: CanvasRc,
    // Layers that were scheduled for rendering where we can't delete the femtovg::ImageId yet
    // because that can only happen after calling `flush`. Otherwise femtovg ends up processing
    // `set_render_target` commands with image ids that have been deleted.
    layer_images_to_delete_after_flush: Vec<CachedImage>,
    graphics_window: Rc<GLWindow>,
    scale_factor: f32,
    /// track the state manually since femtovg don't have accessor for its state
    state: Vec<State>,
}

fn rect_with_radius_to_path(rect: Rect, border_radius: f32) -> femtovg::Path {
    let mut path = femtovg::Path::new();
    let x = rect.origin.x;
    let y = rect.origin.y;
    let width = rect.size.width;
    let height = rect.size.height;
    // If we're drawing a circle, use directly connected bezier curves instead of
    // ones with intermediate LineTo verbs, as `rounded_rect` creates, to avoid
    // rendering artifacts due to those edges.
    if width.approx_eq(&height) && (border_radius * 2.).approx_eq(&width) {
        path.circle(x + border_radius, y + border_radius, border_radius);
    } else {
        path.rounded_rect(x, y, width, height, border_radius);
    }
    path
}

fn rect_to_path(r: Rect) -> femtovg::Path {
    rect_with_radius_to_path(r, 0.)
}

fn adjust_rect_and_border_for_inner_drawing(rect: &mut Rect, border_width: &mut f32) {
    // If the border width exceeds the width, just fill the rectangle.
    *border_width = border_width.min((rect.size.width as f32) / 2.);
    // adjust the size so that the border is drawn within the geometry
    rect.origin.x += *border_width / 2.;
    rect.origin.y += *border_width / 2.;
    rect.size.width -= *border_width;
    rect.size.height -= *border_width;
}

fn item_rect<Item: i_slint_core::items::Item>(item: Pin<&Item>, scale_factor: f32) -> Rect {
    let geometry = item.geometry();
    euclid::rect(0., 0., geometry.width() * scale_factor, geometry.height() * scale_factor)
}

impl ItemRenderer for GLItemRenderer {
    fn draw_rectangle(&mut self, rect: std::pin::Pin<&i_slint_core::items::Rectangle>) {
        let geometry = item_rect(rect, self.scale_factor);
        if geometry.is_empty() {
            return;
        }
        // TODO: cache path in item to avoid re-tesselation
        let mut path = rect_to_path(geometry);
        let paint = match self.brush_to_paint(rect.background(), &mut path) {
            Some(paint) => paint,
            None => return,
        };
        self.canvas.borrow_mut().fill_path(&mut path, paint)
    }

    fn draw_border_rectangle(
        &mut self,
        rect: std::pin::Pin<&i_slint_core::items::BorderRectangle>,
    ) {
        let mut geometry = item_rect(rect, self.scale_factor);
        if geometry.is_empty() {
            return;
        }

        let mut border_width = rect.border_width() * self.scale_factor;
        // In CSS the border is entirely towards the inside of the boundary
        // geometry, while in femtovg the line with for a stroke is 50% in-
        // and 50% outwards. We choose the CSS model, so the inner rectangle
        // is adjusted accordingly.
        adjust_rect_and_border_for_inner_drawing(&mut geometry, &mut border_width);

        let mut path = rect_with_radius_to_path(geometry, rect.border_radius() * self.scale_factor);

        let fill_paint = self.brush_to_paint(rect.background(), &mut path);

        let border_paint = self.brush_to_paint(rect.border_color(), &mut path).map(|mut paint| {
            paint.set_line_width(border_width);
            paint
        });

        let mut canvas = self.canvas.borrow_mut();
        if let Some(paint) = fill_paint {
            canvas.fill_path(&mut path, paint);
        }
        if let Some(border_paint) = border_paint {
            canvas.stroke_path(&mut path, border_paint);
        }
    }

    fn draw_image(&mut self, image: std::pin::Pin<&i_slint_core::items::ImageItem>) {
        self.draw_image_impl(
            &image.cached_rendering_data,
            i_slint_core::items::ImageItem::FIELD_OFFSETS.source.apply_pin(image),
            IntRect::default(),
            i_slint_core::items::ImageItem::FIELD_OFFSETS.width.apply_pin(image),
            i_slint_core::items::ImageItem::FIELD_OFFSETS.height.apply_pin(image),
            image.image_fit(),
            None,
            image.image_rendering(),
        );
    }

    fn draw_clipped_image(
        &mut self,
        clipped_image: std::pin::Pin<&i_slint_core::items::ClippedImage>,
    ) {
        let source_clip_rect = IntRect::new(
            [clipped_image.source_clip_x(), clipped_image.source_clip_y()].into(),
            [clipped_image.source_clip_width(), clipped_image.source_clip_height()].into(),
        );

        self.draw_image_impl(
            &clipped_image.cached_rendering_data,
            i_slint_core::items::ClippedImage::FIELD_OFFSETS.source.apply_pin(clipped_image),
            source_clip_rect,
            i_slint_core::items::ClippedImage::FIELD_OFFSETS.width.apply_pin(clipped_image),
            i_slint_core::items::ClippedImage::FIELD_OFFSETS.height.apply_pin(clipped_image),
            clipped_image.image_fit(),
            Some(
                i_slint_core::items::ClippedImage::FIELD_OFFSETS.colorize.apply_pin(clipped_image),
            ),
            clipped_image.image_rendering(),
        );
    }

    fn draw_text(&mut self, text: std::pin::Pin<&i_slint_core::items::Text>) {
        let max_width = text.width() * self.scale_factor;
        let max_height = text.height() * self.scale_factor;

        if max_width <= 0. || max_height <= 0. {
            return;
        }

        let string = text.text();
        let string = string.as_str();
        let font = fonts::FONT_CACHE.with(|cache| {
            cache.borrow_mut().font(
                text.unresolved_font_request()
                    .merge(&self.graphics_window.default_font_properties()),
                self.scale_factor,
                &text.text(),
            )
        });

        let paint = match self
            .brush_to_paint(text.color(), &mut rect_to_path(item_rect(text, self.scale_factor)))
        {
            Some(paint) => font.init_paint(text.letter_spacing() * self.scale_factor, paint),
            None => return,
        };

        let mut canvas = self.canvas.borrow_mut();
        fonts::layout_text_lines(
            string,
            &font,
            Size::new(max_width, max_height),
            (text.horizontal_alignment(), text.vertical_alignment()),
            text.wrap(),
            text.overflow(),
            false,
            paint,
            |to_draw, pos, _, _| {
                canvas.fill_text(pos.x, pos.y, to_draw.trim_end(), paint).unwrap();
            },
        );
    }

    fn draw_text_input(&mut self, text_input: std::pin::Pin<&i_slint_core::items::TextInput>) {
        let width = text_input.width() * self.scale_factor;
        let height = text_input.height() * self.scale_factor;
        if width <= 0. || height <= 0. {
            return;
        }

        let font = fonts::FONT_CACHE.with(|cache| {
            cache.borrow_mut().font(
                text_input
                    .unresolved_font_request()
                    .merge(&self.graphics_window.default_font_properties()),
                self.scale_factor,
                &text_input.text(),
            )
        });

        let paint = match self.brush_to_paint(
            text_input.color(),
            &mut rect_to_path(item_rect(text_input, self.scale_factor)),
        ) {
            Some(paint) => font.init_paint(text_input.letter_spacing() * self.scale_factor, paint),
            None => return,
        };

        let (mut min_select, mut max_select) = text_input.selection_anchor_and_cursor();
        let cursor_pos = text_input.cursor_position();
        let cursor_visible = cursor_pos >= 0 && text_input.cursor_visible() && text_input.enabled();
        let mut cursor_pos = cursor_pos as usize;
        let mut canvas = self.canvas.borrow_mut();
        let font_height = canvas.measure_font(paint).unwrap().height();
        let mut text = text_input.text();

        if let InputType::password = text_input.input_type() {
            min_select = text[..min_select].chars().count() * PASSWORD_CHARACTER.len();
            max_select = text[..max_select].chars().count() * PASSWORD_CHARACTER.len();
            cursor_pos = text[..cursor_pos].chars().count() * PASSWORD_CHARACTER.len();
            text = SharedString::from(PASSWORD_CHARACTER.repeat(text.chars().count()));
        };

        let mut cursor_point: Option<Point> = None;

        let baseline_y = fonts::layout_text_lines(
            text.as_str(),
            &font,
            Size::new(width, height),
            (text_input.horizontal_alignment(), text_input.vertical_alignment()),
            text_input.wrap(),
            i_slint_core::items::TextOverflow::clip,
            text_input.single_line(),
            paint,
            |to_draw, pos, start, metrics| {
                let range = start..(start + to_draw.len());
                if min_select != max_select
                    && (range.contains(&min_select)
                        || range.contains(&max_select)
                        || (min_select..max_select).contains(&start))
                {
                    let mut selection_start_x = 0.;
                    let mut selection_end_x = 0.;
                    let mut after_selection_x = 0.;
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
                            selection_start_x = glyph.x - glyph.bearing_x;
                        }
                        if glyph.byte_index == max_select - start
                            || glyph.byte_index >= to_draw.len()
                        {
                            after_selection_x = glyph.x - glyph.bearing_x;
                            break;
                        }
                        selection_end_x = glyph.x + glyph.advance_x;
                    }

                    let selection_rect = Rect::new(
                        pos + euclid::vec2(selection_start_x, 0.),
                        Size::new(selection_end_x - selection_start_x, font_height),
                    );
                    canvas.fill_path(
                        &mut rect_to_path(selection_rect),
                        femtovg::Paint::color(to_femtovg_color(
                            &text_input.selection_background_color(),
                        )),
                    );
                    let mut selected_paint = paint;
                    selected_paint
                        .set_color(to_femtovg_color(&text_input.selection_foreground_color()));
                    canvas
                        .fill_text(
                            pos.x,
                            pos.y,
                            &to_draw[..min_select.saturating_sub(start)].trim_end(),
                            paint,
                        )
                        .unwrap();
                    canvas
                        .fill_text(
                            pos.x + selection_start_x,
                            pos.y,
                            &to_draw[min_select.saturating_sub(start)
                                ..(max_select - start).min(to_draw.len())]
                                .trim_end(),
                            selected_paint,
                        )
                        .unwrap();
                    canvas
                        .fill_text(
                            pos.x + after_selection_x,
                            pos.y,
                            &to_draw[(max_select - start).min(to_draw.len())..].trim_end(),
                            paint,
                        )
                        .unwrap();
                } else {
                    // no selection on this line
                    canvas.fill_text(pos.x, pos.y, to_draw.trim_end(), paint).unwrap();
                };
                if cursor_visible
                    && (range.contains(&cursor_pos)
                        || (cursor_pos == range.end && cursor_pos == text.len()))
                {
                    let cursor_x = metrics
                        .glyphs
                        .iter()
                        .find_map(|glyph| {
                            if glyph.byte_index == (cursor_pos as usize - start) {
                                Some(glyph.x)
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| metrics.width());
                    cursor_point = Some([pos.x + cursor_x, pos.y].into());
                }
            },
        );

        if let Some(cursor_point) =
            cursor_point.or_else(|| cursor_visible.then(|| [0., baseline_y].into()))
        {
            let mut cursor_rect = femtovg::Path::new();
            cursor_rect.rect(
                cursor_point.x,
                cursor_point.y,
                text_input.text_cursor_width() * self.scale_factor,
                font_height,
            );
            canvas.fill_path(&mut cursor_rect, paint);
        }
    }

    fn draw_path(&mut self, path: std::pin::Pin<&i_slint_core::items::Path>) {
        let elements = path.elements();
        if matches!(elements, i_slint_core::PathData::None) {
            return;
        }

        let (offset, path_events) = path.fitted_path_events();

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
                    femtovg_path.move_to(at.x * self.scale_factor, at.y * self.scale_factor);
                    orient.area = 0.;
                    orient.prev = at;
                }
                lyon_path::Event::Line { from: _, to } => {
                    femtovg_path.line_to(to.x * self.scale_factor, to.y * self.scale_factor);
                    orient.add_point(to);
                }
                lyon_path::Event::Quadratic { from: _, ctrl, to } => {
                    femtovg_path.quad_to(
                        ctrl.x * self.scale_factor,
                        ctrl.y * self.scale_factor,
                        to.x * self.scale_factor,
                        to.y * self.scale_factor,
                    );
                    orient.add_point(to);
                }

                lyon_path::Event::Cubic { from: _, ctrl1, ctrl2, to } => {
                    femtovg_path.bezier_to(
                        ctrl1.x * self.scale_factor,
                        ctrl1.y * self.scale_factor,
                        ctrl2.x * self.scale_factor,
                        ctrl2.y * self.scale_factor,
                        to.x * self.scale_factor,
                        to.y * self.scale_factor,
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

        let fill_paint =
            self.brush_to_paint(path.fill(), &mut femtovg_path).map(|mut fill_paint| {
                fill_paint.set_fill_rule(match path.fill_rule() {
                    FillRule::nonzero => femtovg::FillRule::NonZero,
                    FillRule::evenodd => femtovg::FillRule::EvenOdd,
                });
                fill_paint
            });

        let border_paint =
            self.brush_to_paint(path.stroke(), &mut femtovg_path).map(|mut paint| {
                paint.set_line_width(path.stroke_width() * self.scale_factor);
                paint
            });

        self.canvas.borrow_mut().save_with(|canvas| {
            canvas.translate(offset.x, offset.y);
            if let Some(fill_paint) = fill_paint {
                canvas.fill_path(&mut femtovg_path, fill_paint);
            }
            if let Some(border_paint) = border_paint {
                canvas.stroke_path(&mut femtovg_path, border_paint);
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
    fn draw_box_shadow(&mut self, box_shadow: std::pin::Pin<&i_slint_core::items::BoxShadow>) {
        if box_shadow.color().alpha() == 0
            || (box_shadow.blur() == 0.0
                && box_shadow.offset_x() == 0.
                && box_shadow.offset_y() == 0.)
        {
            return;
        }

        let cache_entry = box_shadow.cached_rendering_data.get_or_update(
            &self.graphics_window.clone().graphics_cache,
            || {
                ItemGraphicsCacheEntry::Image({
                    let blur = box_shadow.blur() * self.scale_factor;
                    let width = box_shadow.width() * self.scale_factor;
                    let height = box_shadow.height() * self.scale_factor;
                    let radius = box_shadow.border_radius() * self.scale_factor;

                    let shadow_rect: euclid::Rect<f32, euclid::UnknownUnit> =
                        euclid::rect(0., 0., width + 2. * blur, height + 2. * blur);

                    let shadow_image_width = shadow_rect.width().ceil() as u32;
                    let shadow_image_height = shadow_rect.height().ceil() as u32;

                    let shadow_image = CachedImage::new_empty_on_gpu(
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
                        shadow_path.rounded_rect(blur, blur, width, height, radius);
                        canvas.fill_path(
                            &mut shadow_path,
                            femtovg::Paint::color(femtovg::Color::rgb(255, 255, 255)),
                        );
                    }

                    let shadow_image = if blur > 0. {
                        let blurred_image = shadow_image.filter(
                            &self.canvas,
                            femtovg::ImageFilter::GaussianBlur { sigma: blur / 2. },
                        );

                        self.canvas
                            .borrow_mut()
                            .set_render_target(blurred_image.as_render_target());

                        self.layer_images_to_delete_after_flush.push(shadow_image);

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
                            &mut shadow_image_rect,
                            femtovg::Paint::color(to_femtovg_color(&box_shadow.color())),
                        );

                        canvas.restore();

                        canvas.set_render_target(self.current_render_target());
                    }

                    Rc::new(shadow_image)
                })
                .into()
            },
        );

        let shadow_image = match &cache_entry {
            Some(cached_shadow_image) => cached_shadow_image.as_image(),
            None => return, // Zero width or height shadow
        };

        let shadow_image_size = match shadow_image.size() {
            Some(size) => size,
            None => return,
        };

        let shadow_image_paint = shadow_image.as_paint();

        let mut shadow_image_rect = femtovg::Path::new();
        shadow_image_rect.rect(
            0.,
            0.,
            shadow_image_size.width as f32,
            shadow_image_size.height as f32,
        );

        self.canvas.borrow_mut().save_with(|canvas| {
            let blur = box_shadow.blur() * self.scale_factor;
            let offset_x = box_shadow.offset_x() * self.scale_factor;
            let offset_y = box_shadow.offset_y() * self.scale_factor;
            canvas.translate(offset_x - blur, offset_y - blur);
            canvas.fill_path(&mut shadow_image_rect, shadow_image_paint);
        });
    }

    fn combine_clip(&mut self, mut clip_rect: Rect, mut radius: f32, mut border_width: f32) {
        let clip = &mut self.state.last_mut().unwrap().scissor;
        match clip.intersection(&clip_rect) {
            Some(r) => {
                *clip = r;
            }
            None => {
                *clip = Rect::default();
            }
        };

        // Femtovg renders evenly 50% inside and 50% outside of the border width. The
        // adjust_rect_and_border_for_inner_drawing adjusts the rect so that for drawing it
        // would be entirely an *inner* border. However for clipping we want the rect that's
        // entirely inside, hence the doubling of the width and consequently radius adjustment.
        radius -= border_width * KAPPA90;
        border_width *= 2.;

        // Convert from logical to physical pixels
        border_width *= self.scale_factor;
        radius *= self.scale_factor;
        clip_rect *= self.scale_factor;

        adjust_rect_and_border_for_inner_drawing(&mut clip_rect, &mut border_width);
        self.canvas.borrow_mut().intersect_scissor(
            clip_rect.min_x(),
            clip_rect.min_y(),
            clip_rect.width(),
            clip_rect.height(),
        );

        // This is the very expensive clipping code path, where we change the current render target
        // to be an intermediate image and then fill the clip path with that image.
        if radius > 0. {
            let clip_path = rect_with_radius_to_path(clip_rect, radius);
            self.set_clip_path(clip_path)
        }
    }

    fn get_current_clip(&self) -> Rect {
        self.state.last().unwrap().scissor
    }

    fn save_state(&mut self) {
        self.canvas.borrow_mut().save();
        self.state.push(self.state.last().unwrap().clone());
    }

    fn restore_state(&mut self) {
        if let Some(mut layer_to_restore) = self
            .state
            .pop()
            .and_then(|state| state.layer)
            .and_then(|layer| Rc::try_unwrap(layer).ok())
        {
            let paint = layer_to_restore.image.as_paint();

            self.layer_images_to_delete_after_flush.push(layer_to_restore.image);

            let mut canvas = self.canvas.borrow_mut();

            canvas.set_render_target(self.current_render_target());

            // Balanced in set_clip_path, back to original drawing conditions when set_clip_path() was called.
            canvas.restore();
            canvas.fill_path(&mut layer_to_restore.target_path, paint);
        }
        self.canvas.borrow_mut().restore();
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    fn draw_cached_pixmap(
        &mut self,
        item_cache: &CachedRenderingData,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        let canvas = &self.canvas;

        let cache_entry = item_cache.get_or_update(&self.graphics_window.graphics_cache, || {
            let mut cached_image = None;
            update_fn(&mut |width: u32, height: u32, data: &[u8]| {
                use rgb::FromSlice;
                let img = imgref::Img::new(data.as_rgba(), width as usize, height as usize);
                if let Ok(image_id) =
                    canvas.borrow_mut().create_image(img, femtovg::ImageFlags::PREMULTIPLIED)
                {
                    cached_image = Some(ItemGraphicsCacheEntry::Image(Rc::new(
                        CachedImage::new_on_gpu(canvas, image_id),
                    )))
                };
            });
            cached_image
        });
        let image_id = match cache_entry {
            Some(ItemGraphicsCacheEntry::Image(image)) => image.ensure_uploaded_to_gpu(self, None),
            Some(ItemGraphicsCacheEntry::ColorizedImage { .. }) => unreachable!(),
            None => return,
        };
        let mut canvas = self.canvas.borrow_mut();

        let image_info = canvas.image_info(image_id).unwrap();
        let (width, height) = (image_info.width() as f32, image_info.height() as f32);
        let fill_paint = femtovg::Paint::image(image_id, 0., 0., width, height, 0.0, 1.0);
        let mut path = femtovg::Path::new();
        path.rect(0., 0., width, height);
        canvas.fill_path(&mut path, fill_paint);
    }

    fn draw_string(&mut self, string: &str, color: Color) {
        let font = fonts::FONT_CACHE.with(|cache| {
            cache.borrow_mut().font(
                self.graphics_window.default_font_properties(),
                self.scale_factor,
                string,
            )
        });
        let paint = font.init_paint(0.0, femtovg::Paint::color(to_femtovg_color(&color)));
        let mut canvas = self.canvas.borrow_mut();
        canvas.fill_text(0., 0., string, paint).unwrap();
    }

    fn window(&self) -> WindowRc {
        self.graphics_window.runtime_window()
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn translate(&mut self, x: f32, y: f32) {
        self.canvas.borrow_mut().translate(x * self.scale_factor, y * self.scale_factor);
        let clip = &mut self.state.last_mut().unwrap().scissor;
        *clip = clip.translate((-x, -y).into())
    }

    fn rotate(&mut self, angle_in_degrees: f32) {
        let angle_in_radians = angle_in_degrees.to_radians();
        self.canvas.borrow_mut().rotate(angle_in_radians);
        let clip = &mut self.state.last_mut().unwrap().scissor;
        // Compute the bounding box of the rotated rectangle
        let (sin, cos) = angle_in_radians.sin_cos();
        let rotate_point = |p: Point| (p.x * cos - p.y * sin, p.x * sin + p.y * cos);
        let corners = [
            rotate_point(clip.origin),
            rotate_point(clip.origin + euclid::vec2(clip.width(), 0.)),
            rotate_point(clip.origin + euclid::vec2(0., clip.height())),
            rotate_point(clip.origin + clip.size),
        ];
        let origin: Point = (
            corners.iter().fold(f32::MAX, |a, b| b.0.min(a)),
            corners.iter().fold(f32::MAX, |a, b| b.1.min(a)),
        )
            .into();
        let end: Point = (
            corners.iter().fold(f32::MIN, |a, b| b.0.max(a)),
            corners.iter().fold(f32::MIN, |a, b| b.1.max(a)),
        )
            .into();
        *clip = Rect::new(origin, (end - origin).into());
    }

    fn apply_opacity(&mut self, opacity: f32) {
        let state = &mut self.state.last_mut().unwrap().global_alpha;
        *state *= opacity;
        self.canvas.borrow_mut().set_global_alpha(*state);
    }
}

impl GLItemRenderer {
    fn colorize_image(
        &self,
        original_cache_entry: ItemGraphicsCacheEntry,
        colorize_property: Option<Pin<&Property<Brush>>>,
        scaling: ImageRendering,
    ) -> ItemGraphicsCacheEntry {
        let colorize_brush = colorize_property.map_or(Brush::default(), |prop| prop.get());
        if colorize_brush.is_transparent() {
            return original_cache_entry;
        };
        let original_image = original_cache_entry.as_image();

        let image_size = match original_image.size() {
            Some(size) => size,
            None => return original_cache_entry,
        };

        let scaling_flags = match scaling {
            ImageRendering::smooth => femtovg::ImageFlags::empty(),
            ImageRendering::pixelated => {
                femtovg::ImageFlags::empty() | femtovg::ImageFlags::NEAREST
            }
        };

        let image_id = original_image.ensure_uploaded_to_gpu(self, Some(scaling));
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
        let brush_paint = self.brush_to_paint(colorize_brush, &mut image_rect).unwrap();

        self.canvas.borrow_mut().save_with(|canvas| {
            canvas.reset();
            canvas.scale(1., -1.); // Image are rendered upside down
            canvas.translate(0., -image_size.height);
            canvas.set_render_target(femtovg::RenderTarget::Image(colorized_image));

            canvas.global_composite_operation(femtovg::CompositeOperation::Copy);
            canvas.fill_path(
                &mut image_rect,
                femtovg::Paint::image(
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
            canvas.fill_path(&mut image_rect, brush_paint);

            canvas.set_render_target(self.current_render_target());
        });

        ItemGraphicsCacheEntry::ColorizedImage {
            _original_image: original_image.clone(),
            colorized_image: Rc::new(CachedImage::new_on_gpu(&self.canvas, colorized_image)),
        }
    }

    fn draw_image_impl(
        &mut self,
        item_cache: &CachedRenderingData,
        source_property: std::pin::Pin<&Property<Image>>,
        source_clip_rect: IntRect,
        target_width: std::pin::Pin<&Property<f32>>,
        target_height: std::pin::Pin<&Property<f32>>,
        image_fit: ImageFit,
        colorize_property: Option<Pin<&Property<Brush>>>,
        image_rendering: ImageRendering,
    ) {
        let target_w = target_width.get() * self.scale_factor;
        let target_h = target_height.get() * self.scale_factor;

        if target_w <= 0. || target_h <= 0. {
            return;
        }

        let cached_image = loop {
            let image_cache_entry =
                item_cache.get_or_update(&self.graphics_window.graphics_cache, || {
                    let image = source_property.get();
                    let image_inner: &ImageInner = (&image).into();

                    let target_size_for_scalable_source = image_inner.is_svg().then(|| {
                        // get the scale factor as a property again, to ensure the cache is invalidated when the scale factor changes
                        let scale_factor = self.window().scale_factor();
                        [
                            (target_width.get() * scale_factor) as u32,
                            (target_height.get() * scale_factor) as u32,
                        ]
                        .into()
                    });

                    TextureCacheKey::new(
                        image_inner,
                        target_size_for_scalable_source,
                        image_rendering,
                    )
                    .and_then(|cache_key| {
                        self.graphics_window
                            .texture_cache
                            .borrow_mut()
                            .lookup_image_in_cache_or_create(cache_key, || {
                                crate::IMAGE_CACHE
                                    .with(|global_cache| {
                                        global_cache.borrow_mut().load_image_resource(image_inner)
                                    })
                                    .and_then(|image| {
                                        image
                                            .upload_to_gpu(
                                                self, // The condition at the entry of the function ensures that width/height are positive
                                                target_size_for_scalable_source,
                                                image_rendering,
                                            )
                                            .map(Rc::new)
                                    })
                            })
                    })
                    .or_else(|| CachedImage::new_from_resource(image_inner).map(Rc::new))
                    .map(ItemGraphicsCacheEntry::Image)
                    .map(|cache_entry| {
                        self.colorize_image(cache_entry, colorize_property, image_rendering)
                    })
                });

            // Check if the image in the cache is loaded. If not, don't draw any image and we'll return
            // later when the callback from load_html_image has issued a repaint
            let cached_image = match image_cache_entry {
                Some(entry) if entry.as_image().size().is_some() => entry,
                _ => {
                    return;
                }
            };

            // It's possible that our item cache contains an image but it's not colorized yet because it was only
            // placed there via the `image_size` function (which doesn't colorize). So we may have to invalidate our
            // item cache and try again.
            if colorize_property.map_or(false, |prop| !prop.get().is_transparent())
                && !cached_image.is_colorized_image()
            {
                let mut cache = self.graphics_window.graphics_cache.borrow_mut();
                item_cache.release(&mut cache);
                continue;
            }

            break cached_image.as_image().clone();
        };

        let image_id = cached_image.ensure_uploaded_to_gpu(self, Some(image_rendering));
        let image_size = cached_image.size().unwrap_or_default().cast();

        let (source_width, source_height) = if source_clip_rect.is_empty() {
            (image_size.width, image_size.height)
        } else {
            (source_clip_rect.width() as _, source_clip_rect.height() as _)
        };

        let mut source_x = source_clip_rect.min_x() as f32;
        let mut source_y = source_clip_rect.min_y() as f32;

        let mut image_fit_offset = Point::default();

        // The source_to_target scale is applied to the paint that holds the image as well as path
        // begin rendered.
        let (source_to_target_scale_x, source_to_target_scale_y) = match image_fit {
            ImageFit::fill => (target_w / source_width, target_h / source_height),
            ImageFit::cover => {
                let ratio = f32::max(target_w / source_width, target_h / source_height);

                if source_width > target_w / ratio {
                    source_x += (source_width - target_w / ratio) / 2.;
                }
                if source_height > target_h / ratio {
                    source_y += (source_height - target_h / ratio) / 2.
                }

                (ratio, ratio)
            }
            ImageFit::contain => {
                let ratio = f32::min(target_w / source_width, target_h / source_height);

                if source_width < target_w / ratio {
                    image_fit_offset.x = (target_w - source_width * ratio) / 2.;
                }
                if source_height < target_h / ratio {
                    image_fit_offset.y = (target_h - source_height * ratio) / 2.
                }

                (ratio, ratio)
            }
        };

        let fill_paint = femtovg::Paint::image(
            image_id,
            -source_x,
            -source_y,
            image_size.width,
            image_size.height,
            0.0,
            1.0,
        );

        let mut path = femtovg::Path::new();
        path.rect(0., 0., source_width, source_height);

        self.canvas.borrow_mut().save_with(|canvas| {
            canvas.translate(image_fit_offset.x, image_fit_offset.y);

            canvas.scale(source_to_target_scale_x, source_to_target_scale_y);

            canvas.fill_path(&mut path, fill_paint);
        })
    }

    fn brush_to_paint(&self, brush: Brush, path: &mut femtovg::Path) -> Option<femtovg::Paint> {
        if brush.is_transparent() {
            return None;
        }
        Some(match brush {
            Brush::SolidColor(color) => femtovg::Paint::color(to_femtovg_color(&color)),
            Brush::LinearGradient(gradient) => {
                // `canvas.path_bbox()` applies the current transform. However we're not interested in that, since
                // we operate in item local coordinates with the `path` parameter as well as the resulting
                // paint.
                let path_bounds = {
                    let mut canvas = self.canvas.borrow_mut();
                    canvas.save();
                    canvas.reset_transform();
                    let bounding_box = canvas.path_bbox(path);
                    canvas.restore();
                    bounding_box
                };

                let path_width = path_bounds.maxx - path_bounds.minx;
                let path_height = path_bounds.maxy - path_bounds.miny;

                let transform = euclid::Transform2D::scale(path_width, path_height)
                    .then_translate(euclid::Vector2D::new(path_bounds.minx, path_bounds.miny));

                let (start, end) = i_slint_core::graphics::line_for_angle(gradient.angle());

                let start: Point = transform.transform_point(start);
                let end: Point = transform.transform_point(end);

                let stops = gradient
                    .stops()
                    .map(|stop| (stop.position, to_femtovg_color(&stop.color)))
                    .collect::<Vec<_>>();
                femtovg::Paint::linear_gradient_stops(start.x, start.y, end.x, end.y, &stops)
            }
            _ => return None,
        })
    }

    // Set the specified path for clipping. This is done by redirecting rendering into
    // an intermediate image and using that to fill the clip path on the next restore_state()
    // call. Therefore this can only be called once per save_state()!
    fn set_clip_path(&mut self, mut path: femtovg::Path) {
        let path_bounds = {
            let mut canvas = self.canvas.borrow_mut();
            canvas.save();
            canvas.reset_transform();
            let bbox = canvas.path_bbox(&mut path);
            canvas.restore();
            bbox
        };

        let layer_width = path_bounds.maxx - path_bounds.minx;
        let layer_height = path_bounds.maxy - path_bounds.miny;

        let clip_buffer_img = match CachedImage::new_empty_on_gpu(
            &self.canvas,
            layer_width as _,
            layer_height as _,
        ) {
            Some(clip_buffer) => clip_buffer,
            None => return, // Zero width or height clip path
        };

        {
            let mut canvas = self.canvas.borrow_mut();

            // Balanced with the *first* restore() call in restore_state(), followed by
            // the original restore() later in restore_state().
            canvas.save();

            canvas.set_render_target(clip_buffer_img.as_render_target());

            canvas.reset();

            canvas.clear_rect(
                0,
                0,
                layer_width as _,
                layer_height as _,
                femtovg::Color::rgba(0, 0, 0, 0),
            );
            canvas.global_composite_operation(femtovg::CompositeOperation::SourceOver);
        }
        self.state.last_mut().unwrap().layer =
            Some(Rc::new(Layer { image: clip_buffer_img, target_path: path }));
    }

    fn current_render_target(&self) -> femtovg::RenderTarget {
        self.state
            .last()
            .unwrap()
            .layer
            .as_ref()
            .map_or(femtovg::RenderTarget::Screen, |layer| layer.image.as_render_target())
    }
}

fn to_femtovg_color(col: &Color) -> femtovg::Color {
    femtovg::Color::rgba(col.red(), col.green(), col.blue(), col.alpha())
}

#[cfg(target_arch = "wasm32")]
pub fn create_gl_window_with_canvas_id(canvas_id: String) -> Rc<Window> {
    i_slint_core::window::Window::new(|window| GLWindow::new(window, canvas_id))
}

#[doc(hidden)]
#[cold]
#[cfg(not(target_arch = "wasm32"))]
pub fn use_modules() {}

pub type NativeWidgets = ();
pub type NativeGlobals = (stylemetrics::NativeStyleMetrics, ());
pub mod native_widgets {
    pub use super::stylemetrics::NativeStyleMetrics;
}
pub const HAS_NATIVE_STYLE: bool = false;

pub use stylemetrics::native_style_metrics_deinit;
pub use stylemetrics::native_style_metrics_init;

// TODO: We can't connect to the wayland clipboard yet because
// it requires an external connection.
cfg_if::cfg_if! {
    if #[cfg(all(
             unix,
             not(any(
                 target_os = "macos",
                 target_os = "android",
                 target_os = "ios",
                 target_os = "emscripten"
            )),
            not(feature = "x11")
        ))] {
        type ClipboardBackend = copypasta::nop_clipboard::NopClipboardContext;
    } else {
        type ClipboardBackend = copypasta::ClipboardContext;
    }
}

thread_local!(pub(crate) static CLIPBOARD : RefCell<ClipboardBackend> = std::cell::RefCell::new(ClipboardBackend::new().unwrap()));

thread_local!(pub(crate) static IMAGE_CACHE: RefCell<images::ImageCache> = Default::default());

pub struct Backend;
impl i_slint_core::backend::Backend for Backend {
    fn create_window(&'static self) -> Rc<Window> {
        i_slint_core::window::Window::new(|window| {
            GLWindow::new(
                window,
                #[cfg(target_arch = "wasm32")]
                "canvas".into(),
            )
        })
    }

    fn run_event_loop(&'static self, behavior: i_slint_core::backend::EventLoopQuitBehavior) {
        crate::event_loop::run(behavior);
    }

    fn quit_event_loop(&'static self) {
        crate::event_loop::with_window_target(|event_loop| {
            event_loop.event_loop_proxy().send_event(crate::event_loop::CustomEvent::Exit).ok();
        })
    }

    fn register_font_from_memory(
        &'static self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        self::fonts::register_font_from_memory(data)
    }

    fn register_font_from_path(
        &'static self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self::fonts::register_font_from_path(path)
    }

    fn set_clipboard_text(&'static self, text: String) {
        use copypasta::ClipboardProvider;
        CLIPBOARD.with(|clipboard| clipboard.borrow_mut().set_contents(text).ok());
    }

    fn clipboard_text(&'static self) -> Option<String> {
        use copypasta::ClipboardProvider;
        CLIPBOARD.with(|clipboard| clipboard.borrow_mut().get_contents().ok())
    }

    fn post_event(&'static self, event: Box<dyn FnOnce() + Send>) {
        let e = crate::event_loop::CustomEvent::UserEvent(event);
        #[cfg(not(target_arch = "wasm32"))]
        crate::event_loop::GLOBAL_PROXY.get_or_init(Default::default).lock().unwrap().send_event(e);
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::closure::Closure;
            use wasm_bindgen::JsCast;

            let window = web_sys::window().expect("Failed to obtain window");
            let send_event = || {
                crate::event_loop::GLOBAL_PROXY.with(|global_proxy| {
                    global_proxy.borrow_mut().get_or_insert_with(Default::default).send_event(e)
                });
            };

            // Calling send_event is usually done by winit at the bottom of the stack,
            // in event handlers, and thus winit might decide to process the event
            // immediately within that stack.
            // To prevent re-entrancy issues that might happen by getting the application
            // event processed on top of the current stack, mimic the event handling
            // use case and make sure that our event is processed on top of a clean stack.
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    &Closure::once_into_js(send_event).as_ref().unchecked_ref(),
                    0,
                )
                .expect("Failed to set timeout");
        }
    }

    fn image_size(&'static self, image: &Image) -> IntSize {
        IMAGE_CACHE.with(|image_cache| {
            image_cache
                .borrow_mut()
                .load_image_resource(image.into())
                .and_then(|image| image.size())
                .unwrap_or_default()
        })
    }
}
