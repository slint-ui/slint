// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::pin::Pin;
use std::rc::Rc;

use i_slint_core::graphics::{euclid, SharedImageBuffer};
use i_slint_core::item_rendering::{ItemCache, ItemRenderer};
use i_slint_core::items::{ImageFit, ImageRendering, ItemRc, Opacity, RenderingResult, TextWrap};
use i_slint_core::{items, Brush, Color, ImageInner, Property};

#[derive(Clone, Copy)]
struct RenderState {
    alpha: f32,
}

pub struct SkiaRenderer<'a> {
    pub canvas: &'a mut skia_safe::Canvas,
    pub window: Rc<i_slint_core::window::WindowInner>,
    pub scale_factor: f32,
    state_stack: Vec<RenderState>,
    current_state: RenderState,
    image_cache: &'a ItemCache<Option<skia_safe::Image>>,
}

impl<'a> SkiaRenderer<'a> {
    pub fn new(
        canvas: &'a mut skia_safe::Canvas,
        window: &Rc<i_slint_core::window::WindowInner>,
        image_cache: &'a ItemCache<Option<skia_safe::Image>>,
    ) -> Self {
        Self {
            canvas,
            window: window.clone(),
            scale_factor: window.scale_factor(),
            state_stack: vec![],
            current_state: RenderState { alpha: 1.0 },
            image_cache,
        }
    }

    fn brush_to_paint(&self, brush: Brush, width: f32, height: f32) -> Option<skia_safe::Paint> {
        if brush.is_transparent() {
            return None;
        }
        let mut paint = skia_safe::Paint::default();
        match brush {
            Brush::SolidColor(color) => paint.set_color(to_skia_color(&color)),
            Brush::LinearGradient(g) => {
                let (start, end) = i_slint_core::graphics::line_for_angle(g.angle());
                let (colors, pos): (Vec<_>, Vec<_>) =
                    g.stops().map(|s| (to_skia_color(&s.color), s.position)).unzip();
                paint.set_shader(skia_safe::gradient_shader::linear(
                    (to_skia_point(start), to_skia_point(end)),
                    skia_safe::gradient_shader::GradientShaderColors::Colors(&colors),
                    Some(&*pos),
                    skia_safe::TileMode::Clamp,
                    None,
                    &skia_safe::Matrix::scale((width, height)),
                ))
            }
            Brush::RadialGradient(g) => {
                let (colors, pos): (Vec<_>, Vec<_>) =
                    g.stops().map(|s| (to_skia_color(&s.color), s.position)).unzip();
                let circle_scale = width.max(height) / 2.;
                paint.set_shader(skia_safe::gradient_shader::radial(
                    skia_safe::Point::new(0., 0.),
                    1.,
                    skia_safe::gradient_shader::GradientShaderColors::Colors(&colors),
                    Some(&*pos),
                    skia_safe::TileMode::Clamp,
                    None,
                    skia_safe::Matrix::scale((circle_scale, circle_scale))
                        .post_translate((width / 2., height / 2.))
                        as &skia_safe::Matrix,
                ))
            }
            _ => return None,
        };

        paint.set_alpha_f(paint.alpha_f() * self.current_state.alpha);

        Some(paint)
    }

    fn draw_image_impl(
        &mut self,
        item_rc: &ItemRc,
        source_property: Pin<&Property<i_slint_core::graphics::Image>>,
        mut dest_rect: skia_safe::Rect,
        source_rect: Option<skia_safe::Rect>,
        image_fit: ImageFit,
        rendering: ImageRendering,
        colorize_property: Option<Pin<&Property<Brush>>>,
    ) {
        // TODO: avoid doing creating an SkImage multiple times when the same source is used in multiple image elements
        let skia_image = self.image_cache.get_or_update_cache_entry(item_rc, || {
            let image = source_property.get();
            let image_inner: &ImageInner = (&image).into();
            match image_inner {
                ImageInner::None => None,
                ImageInner::EmbeddedImage { buffer, .. } => {
                    let (data, bpl, size, color_type, alpha_type) = match buffer {
                        SharedImageBuffer::RGB8(pixels) => {
                            // RGB888 with one byte per component is not supported by Skia right now. Convert once to RGBA8 :-(
                            let rgba = pixels
                                .as_bytes()
                                .chunks(3)
                                .flat_map(|rgb| {
                                    IntoIterator::into_iter([rgb[0], rgb[1], rgb[2], 255])
                                })
                                .collect::<Vec<u8>>();
                            (
                                skia_safe::Data::new_copy(&*rgba),
                                pixels.stride() as usize * 4,
                                pixels.size(),
                                skia_safe::ColorType::RGBA8888,
                                skia_safe::AlphaType::Unpremul,
                            )
                        }
                        SharedImageBuffer::RGBA8(pixels) => (
                            skia_safe::Data::new_copy(pixels.as_bytes()),
                            pixels.stride() as usize * 4,
                            pixels.size(),
                            skia_safe::ColorType::RGBA8888,
                            skia_safe::AlphaType::Unpremul,
                        ),
                        SharedImageBuffer::RGBA8Premultiplied(pixels) => (
                            skia_safe::Data::new_copy(pixels.as_bytes()),
                            pixels.stride() as usize * 4,
                            pixels.size(),
                            skia_safe::ColorType::RGBA8888,
                            skia_safe::AlphaType::Premul,
                        ),
                    };

                    let image_info = skia_safe::ImageInfo::new(
                        skia_safe::ISize::new(size.width as i32, size.height as i32),
                        color_type,
                        alpha_type,
                        None,
                    );

                    skia_safe::image::Image::from_raster_data(&image_info, data, bpl)
                }
                ImageInner::Svg(_) => None, // TODO
                ImageInner::StaticTextures(_) => todo!(),
            }
        });

        let skia_image = match skia_image {
            Some(img) => img,
            None => return,
        };

        self.canvas.save();

        let mut source_rect = source_rect.filter(|r| !r.is_empty()).unwrap_or_else(|| {
            skia_safe::Rect::from_wh(skia_image.width() as _, skia_image.height() as _)
        });
        adjust_to_image_fit(image_fit, &mut source_rect, &mut dest_rect);

        self.canvas.clip_rect(dest_rect, None, None);

        let transform =
            skia_safe::Matrix::rect_to_rect(source_rect, dest_rect, None).unwrap_or_default();
        self.canvas.concat(&transform);
        self.canvas.draw_image(skia_image, skia_safe::Point::default(), None);
        self.canvas.restore();
    }
}

impl<'a> ItemRenderer for SkiaRenderer<'a> {
    fn draw_rectangle(
        &mut self,
        rect: std::pin::Pin<&i_slint_core::items::Rectangle>,
        _self_rc: &i_slint_core::items::ItemRc,
    ) {
        let geometry = item_rect(rect, self.scale_factor);
        if geometry.is_empty() {
            return;
        }

        let paint =
            match self.brush_to_paint(rect.background(), geometry.width(), geometry.height()) {
                Some(paint) => paint,
                None => return,
            };
        self.canvas.draw_rect(geometry, &paint);
    }

    fn draw_border_rectangle(
        &mut self,
        rect: std::pin::Pin<&i_slint_core::items::BorderRectangle>,
        _self_rc: &i_slint_core::items::ItemRc,
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

        let radius = rect.border_radius() * self.scale_factor;
        let rounded_rect = skia_safe::RRect::new_rect_xy(geometry, radius, radius);

        if let Some(mut fill_paint) =
            self.brush_to_paint(rect.background(), geometry.width(), geometry.height())
        {
            fill_paint.set_style(skia_safe::PaintStyle::Fill);
            self.canvas.draw_rrect(rounded_rect, &fill_paint);
        }

        if let Some(mut border_paint) =
            self.brush_to_paint(rect.border_color(), geometry.width(), geometry.height())
        {
            border_paint.set_style(skia_safe::PaintStyle::Stroke);
            border_paint.set_stroke_width(border_width);
            self.canvas.draw_rrect(rounded_rect, &border_paint);
        };
    }

    fn draw_image(
        &mut self,
        image: std::pin::Pin<&i_slint_core::items::ImageItem>,
        self_rc: &i_slint_core::items::ItemRc,
    ) {
        let geometry = item_rect(image, self.scale_factor);
        if geometry.is_empty() {
            return;
        }

        self.draw_image_impl(
            self_rc,
            i_slint_core::items::ImageItem::FIELD_OFFSETS.source.apply_pin(image),
            geometry,
            None,
            image.image_fit(),
            image.image_rendering(),
            None,
        );
    }

    fn draw_clipped_image(
        &mut self,
        image: std::pin::Pin<&i_slint_core::items::ClippedImage>,
        self_rc: &i_slint_core::items::ItemRc,
    ) {
        let geometry = item_rect(image, self.scale_factor);
        if geometry.is_empty() {
            return;
        }

        let source_rect = skia_safe::Rect::from_xywh(
            image.source_clip_x() as _,
            image.source_clip_y() as _,
            image.source_clip_width() as _,
            image.source_clip_height() as _,
        );

        self.draw_image_impl(
            self_rc,
            i_slint_core::items::ClippedImage::FIELD_OFFSETS.source.apply_pin(image),
            geometry,
            Some(source_rect),
            image.image_fit(),
            image.image_rendering(),
            Some(items::ClippedImage::FIELD_OFFSETS.colorize.apply_pin(image)),
        );
    }

    fn draw_text(
        &mut self,
        text: std::pin::Pin<&i_slint_core::items::Text>,
        _self_rc: &i_slint_core::items::ItemRc,
    ) {
        let max_width = text.width() * self.scale_factor;
        let max_height = text.height() * self.scale_factor;

        if max_width <= 0. || max_height <= 0. {
            return;
        }

        let string = text.text();
        let string = string.as_str();
        let font_request = text.font_request(&self.window);

        let paint = match self.brush_to_paint(text.color(), max_width, max_height) {
            Some(paint) => paint,
            None => return,
        };

        let mut text_style = skia_safe::textlayout::TextStyle::new();
        text_style.set_foreground_color(paint);

        let layout = super::textlayout::create_layout(
            font_request,
            self.scale_factor,
            string,
            Some(text_style),
            if text.wrap() == TextWrap::WordWrap { Some(max_width) } else { None },
        );

        layout.paint(&mut self.canvas, skia_safe::Point::default());
    }

    fn draw_text_input(
        &mut self,
        text_input: std::pin::Pin<&i_slint_core::items::TextInput>,
        _self_rc: &i_slint_core::items::ItemRc,
    ) {
        //todo!()
    }

    fn draw_path(
        &mut self,
        path: std::pin::Pin<&i_slint_core::items::Path>,
        _self_rc: &i_slint_core::items::ItemRc,
    ) {
        //todo!()
    }

    fn draw_box_shadow(
        &mut self,
        box_shadow: std::pin::Pin<&i_slint_core::items::BoxShadow>,
        _self_rc: &i_slint_core::items::ItemRc,
    ) {
        //todo!()
    }

    fn combine_clip(
        &mut self,
        rect: i_slint_core::graphics::Rect,
        radius: i_slint_core::Coord,
        border_width: i_slint_core::Coord,
    ) -> bool {
        let mut rect = to_skia_rect(&rect.scale(self.scale_factor, self.scale_factor));
        let mut border_width = border_width * self.scale_factor;
        // In CSS the border is entirely towards the inside of the boundary
        // geometry, while in femtovg the line with for a stroke is 50% in-
        // and 50% outwards. We choose the CSS model, so the inner rectangle
        // is adjusted accordingly.
        adjust_rect_and_border_for_inner_drawing(&mut rect, &mut border_width);

        let radius = radius * self.scale_factor;
        let rounded_rect = skia_safe::RRect::new_rect_xy(rect, radius, radius);
        self.canvas.clip_rrect(rounded_rect, None, true);
        self.canvas.local_clip_bounds().is_some()
    }

    fn get_current_clip(&self) -> i_slint_core::graphics::Rect {
        from_skia_rect(&self.canvas.local_clip_bounds().unwrap_or_default())
            .scale(1. / self.scale_factor, 1. / self.scale_factor)
    }

    fn translate(&mut self, x: i_slint_core::Coord, y: i_slint_core::Coord) {
        self.canvas
            .translate(skia_safe::Vector::from((x * self.scale_factor, y * self.scale_factor)));
    }

    fn rotate(&mut self, angle_in_degrees: f32) {
        //todo!()
    }

    fn apply_opacity(&mut self, opacity: f32) {
        self.current_state.alpha *= opacity;
    }

    fn save_state(&mut self) {
        self.canvas.save();
        self.state_stack.push(self.current_state);
    }

    fn restore_state(&mut self) {
        self.current_state = self.state_stack.pop().unwrap();
        self.canvas.restore();
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    fn draw_cached_pixmap(
        &mut self,
        item_cache: &i_slint_core::items::ItemRc,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        //todo!()
    }

    fn draw_string(&mut self, string: &str, color: i_slint_core::Color) {
        let mut paint = skia_safe::Paint::default();
        paint.set_color(to_skia_color(&color));
        self.canvas.draw_str(
            string,
            skia_safe::Point::new(0., 12.), // Default text size is 12 pixels
            &skia_safe::Font::default(),
            &paint,
        );
    }

    fn window(&self) -> i_slint_core::window::WindowRc {
        self.window.clone()
    }

    fn as_any(&mut self) -> Option<&mut dyn core::any::Any> {
        None
    }

    fn visit_opacity(&mut self, opacity_item: Pin<&Opacity>, item_rc: &ItemRc) -> RenderingResult {
        let opacity = opacity_item.opacity();
        if Opacity::need_layer(item_rc, opacity) {
            self.canvas.save_layer_alpha(None, (opacity * 255.) as u32);
            self.state_stack.push(self.current_state);
            self.current_state.alpha = 1.0;

            i_slint_core::item_rendering::render_item_children(
                self,
                &item_rc.component(),
                item_rc.index() as isize,
            );

            self.current_state = self.state_stack.pop().unwrap();
            self.canvas.restore();
            RenderingResult::ContinueRenderingWithoutChildren
        } else {
            self.apply_opacity(opacity);
            RenderingResult::ContinueRenderingChildren
        }
    }
}

pub fn from_skia_rect(rect: &skia_safe::Rect) -> i_slint_core::graphics::Rect {
    let top_left = euclid::Point2D::new(rect.left, rect.top);
    let bottom_right = euclid::Point2D::new(rect.right, rect.bottom);
    euclid::Box2D::new(top_left, bottom_right).to_rect()
}

pub fn to_skia_rect(rect: &i_slint_core::graphics::Rect) -> skia_safe::Rect {
    skia_safe::Rect::from_xywh(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height)
}

pub fn to_skia_point(point: i_slint_core::graphics::Point) -> skia_safe::Point {
    skia_safe::Point::new(point.x, point.y)
}

fn item_rect<Item: items::Item>(item: Pin<&Item>, scale_factor: f32) -> skia_safe::Rect {
    let geometry = item.geometry();
    skia_safe::Rect::from_xywh(
        0.,
        0.,
        geometry.width() * scale_factor,
        geometry.height() * scale_factor,
    )
}

pub fn to_skia_color(col: &Color) -> skia_safe::Color {
    skia_safe::Color::from_argb(col.alpha(), col.red(), col.green(), col.blue())
}

fn adjust_rect_and_border_for_inner_drawing(rect: &mut skia_safe::Rect, border_width: &mut f32) {
    // If the border width exceeds the width, just fill the rectangle.
    *border_width = border_width.min((rect.width() as f32) / 2.);
    // adjust the size so that the border is drawn within the geometry

    rect.left += *border_width / 2.;
    rect.top += *border_width / 2.;
    rect.right -= *border_width / 2.;
    rect.bottom -= *border_width / 2.;
}

/// Changes the source or the destination rectangle to respect the image fit
fn adjust_to_image_fit(
    image_fit: ImageFit,
    source_rect: &mut skia_safe::Rect,
    dest_rect: &mut skia_safe::Rect,
) {
    match image_fit {
        ImageFit::Fill => (),
        ImageFit::Cover => {
            let ratio = (dest_rect.width() / source_rect.width())
                .max(dest_rect.height() / source_rect.height());
            if source_rect.width() > dest_rect.width() / ratio {
                source_rect.left += (source_rect.width() - dest_rect.width() / ratio) / 2.;
                source_rect.right = source_rect.left + dest_rect.width() / ratio;
            }
            if source_rect.height() > dest_rect.height() / ratio {
                source_rect.top += (source_rect.height() - dest_rect.height() / ratio) / 2.;
                source_rect.bottom = source_rect.top + dest_rect.height() / ratio;
            }
        }
        ImageFit::Contain => {
            let ratio = (dest_rect.width() / source_rect.width())
                .min(dest_rect.height() / source_rect.height());
            if dest_rect.width() > source_rect.width() * ratio {
                dest_rect.left += (dest_rect.width() - source_rect.width() * ratio) / 2.;
                dest_rect.right = dest_rect.left + source_rect.width() * ratio;
            }
            if dest_rect.height() > source_rect.height() * ratio {
                dest_rect.top += (dest_rect.height() - source_rect.height() * ratio) / 2.;
                dest_rect.bottom = dest_rect.top + source_rect.height() * ratio;
            }
        }
    };
}
