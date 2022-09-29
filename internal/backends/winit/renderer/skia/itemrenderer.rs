// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::pin::Pin;

use super::{PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalSize};
use i_slint_core::graphics::euclid;
use i_slint_core::item_rendering::{ItemCache, ItemRenderer};
use i_slint_core::items::{ImageFit, ImageRendering, ItemRc, Layer, Opacity, RenderingResult};
use i_slint_core::lengths::{
    LogicalLength, LogicalPoint, LogicalRect, LogicalSize, RectLengths, ScaleFactor,
};
use i_slint_core::window::WindowInner;
use i_slint_core::{items, Brush, Color, Property};

use super::super::boxshadowcache::BoxShadowCache;

pub type SkiaBoxShadowCache = BoxShadowCache<skia_safe::Image>;

#[derive(Clone, Copy)]
struct RenderState {
    alpha: f32,
}

pub struct SkiaRenderer<'a> {
    pub canvas: &'a mut skia_safe::Canvas,
    pub scale_factor: ScaleFactor,
    pub window: &'a i_slint_core::api::Window,
    state_stack: Vec<RenderState>,
    current_state: RenderState,
    image_cache: &'a ItemCache<Option<skia_safe::Image>>,
    box_shadow_cache: &'a mut SkiaBoxShadowCache,
}

impl<'a> SkiaRenderer<'a> {
    pub fn new(
        canvas: &'a mut skia_safe::Canvas,
        window: &'a i_slint_core::api::Window,
        image_cache: &'a ItemCache<Option<skia_safe::Image>>,
        box_shadow_cache: &'a mut SkiaBoxShadowCache,
    ) -> Self {
        Self {
            canvas,
            scale_factor: ScaleFactor::new(window.scale_factor()),
            window,
            state_stack: vec![],
            current_state: RenderState { alpha: 1.0 },
            image_cache,
            box_shadow_cache,
        }
    }

    fn brush_to_paint(
        &self,
        brush: Brush,
        width: PhysicalLength,
        height: PhysicalLength,
    ) -> Option<skia_safe::Paint> {
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
                    (skia_safe::Point::new(start.x, start.y), skia_safe::Point::new(end.x, end.y)),
                    skia_safe::gradient_shader::GradientShaderColors::Colors(&colors),
                    Some(&*pos),
                    skia_safe::TileMode::Clamp,
                    None,
                    &skia_safe::Matrix::scale((width.get(), height.get())),
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
                    skia_safe::Matrix::scale((circle_scale.get(), circle_scale.get()))
                        .post_translate((width.get() / 2., height.get() / 2.))
                        as &skia_safe::Matrix,
                ))
            }
            _ => return None,
        };

        paint.set_alpha_f(paint.alpha_f() * self.current_state.alpha);

        Some(paint)
    }

    fn colorize_image(
        &mut self,
        image: skia_safe::Image,
        colorize_brush: Brush,
    ) -> Option<skia_safe::Image> {
        let image_info = skia_safe::ImageInfo::new(
            image.dimensions(),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Premul,
            None,
        );

        let mut surface = self.canvas.new_surface(&image_info, None)?;
        let canvas = surface.canvas();
        canvas.clear(skia_safe::Color::TRANSPARENT);

        let colorize_paint = self.brush_to_paint(
            colorize_brush,
            PhysicalLength::new(image.width() as f32),
            PhysicalLength::new(image.height() as f32),
        )?;

        let mut paint = skia_safe::Paint::default();
        paint.set_image_filter(skia_safe::image_filters::blend(
            skia_safe::BlendMode::SrcIn,
            skia_safe::image_filters::image(image, None, None, None),
            skia_safe::image_filters::paint(&colorize_paint, None),
            None,
        ));
        canvas.draw_paint(&paint);
        Some(surface.image_snapshot())
    }

    fn draw_image_impl(
        &mut self,
        item_rc: &ItemRc,
        source_property: Pin<&Property<i_slint_core::graphics::Image>>,
        mut dest_rect: PhysicalRect,
        source_rect: Option<skia_safe::Rect>,
        target_width: std::pin::Pin<&Property<f32>>,
        target_height: std::pin::Pin<&Property<f32>>,
        image_fit: ImageFit,
        rendering: ImageRendering,
        colorize_property: Option<Pin<&Property<Brush>>>,
    ) {
        // TODO: avoid doing creating an SkImage multiple times when the same source is used in multiple image elements
        let skia_image = self.image_cache.get_or_update_cache_entry(item_rc, || {
            let image = source_property.get();
            super::cached_image::as_skia_image(
                image,
                target_width,
                target_height,
                self.scale_factor,
            )
            .and_then(|skia_image| {
                match colorize_property.map(|p| p.get()).filter(|c| !c.is_transparent()) {
                    Some(color) => self.colorize_image(skia_image, color),
                    None => Some(skia_image),
                }
            })
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

        self.canvas.clip_rect(to_skia_rect(&dest_rect), None, None);

        let transform =
            skia_safe::Matrix::rect_to_rect(source_rect, to_skia_rect(&dest_rect), None)
                .unwrap_or_default();
        self.canvas.concat(&transform);

        let filter_mode: skia_safe::sampling_options::SamplingOptions = match rendering {
            ImageRendering::Smooth => skia_safe::sampling_options::FilterMode::Linear,
            ImageRendering::Pixelated => skia_safe::sampling_options::FilterMode::Nearest,
        }
        .into();

        self.canvas.draw_image_with_sampling_options(
            skia_image,
            skia_safe::Point::default(),
            filter_mode,
            None,
        );

        self.canvas.restore();
    }

    fn render_and_blend_layer(&mut self, item_rc: &ItemRc) -> RenderingResult {
        let current_clip = self.get_current_clip();
        if let Some(layer_image) = self.render_layer(item_rc, &|| {
            // We don't need to include the size of the "layer" item itself, since it has no content.
            let children_rect = i_slint_core::properties::evaluate_no_tracking(|| {
                let self_ref = item_rc.borrow();
                LogicalRect::from_untyped(&self_ref.as_ref().geometry()).union(
                    &i_slint_core::item_rendering::item_children_bounding_rect(
                        &item_rc.component(),
                        item_rc.index() as isize,
                        &current_clip,
                    ),
                )
            });
            children_rect.size_length()
        }) {
            let mut tint = skia_safe::Paint::default();
            tint.set_alpha_f(self.current_state.alpha);
            self.canvas.draw_image(layer_image, skia_safe::Point::default(), Some(&tint));
        }
        RenderingResult::ContinueRenderingWithoutChildren
    }

    fn render_layer(
        &mut self,
        item_rc: &ItemRc,
        layer_logical_size_fn: &dyn Fn() -> LogicalSize,
    ) -> Option<skia_safe::Image> {
        self.image_cache.get_or_update_cache_entry(item_rc, || {
            let layer_size = layer_logical_size_fn() * self.scale_factor;

            let image_info = skia_safe::ImageInfo::new(
                to_skia_size(&layer_size).to_ceil(),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Premul,
                None,
            );
            let mut surface = self.canvas.new_surface(&image_info, None)?;
            let canvas = surface.canvas();
            canvas.clear(skia_safe::Color::TRANSPARENT);

            let mut sub_renderer =
                SkiaRenderer::new(canvas, &self.window, self.image_cache, self.box_shadow_cache);

            i_slint_core::item_rendering::render_item_children(
                &mut sub_renderer,
                &item_rc.component(),
                item_rc.index() as isize,
            );

            Some(surface.image_snapshot())
        })
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

        let paint = match self.brush_to_paint(
            rect.background(),
            geometry.width_length(),
            geometry.height_length(),
        ) {
            Some(paint) => paint,
            None => return,
        };
        self.canvas.draw_rect(to_skia_rect(&geometry), &paint);
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

        let mut border_width = LogicalLength::new(rect.border_width()) * self.scale_factor;
        // In CSS the border is entirely towards the inside of the boundary
        // geometry, while in femtovg the line with for a stroke is 50% in-
        // and 50% outwards. We choose the CSS model, so the inner rectangle
        // is adjusted accordingly.
        adjust_rect_and_border_for_inner_drawing(&mut geometry, &mut border_width);

        let radius = LogicalLength::new(rect.border_radius()) * self.scale_factor;
        let rounded_rect =
            skia_safe::RRect::new_rect_xy(to_skia_rect(&geometry), radius.get(), radius.get());

        if let Some(mut fill_paint) = self.brush_to_paint(
            rect.background(),
            geometry.width_length(),
            geometry.height_length(),
        ) {
            fill_paint.set_style(skia_safe::PaintStyle::Fill);
            self.canvas.draw_rrect(rounded_rect, &fill_paint);
        }

        if border_width.get() > 0.0 {
            if let Some(mut border_paint) = self.brush_to_paint(
                rect.border_color(),
                geometry.width_length(),
                geometry.height_length(),
            ) {
                border_paint.set_style(skia_safe::PaintStyle::Stroke);
                border_paint.set_stroke_width(border_width.get());
                self.canvas.draw_rrect(rounded_rect, &border_paint);
            }
        }
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
            items::ImageItem::FIELD_OFFSETS.width.apply_pin(image),
            items::ImageItem::FIELD_OFFSETS.height.apply_pin(image),
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
            items::ClippedImage::FIELD_OFFSETS.width.apply_pin(image),
            items::ClippedImage::FIELD_OFFSETS.height.apply_pin(image),
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
        let max_width = LogicalLength::new(text.width());
        let max_height = LogicalLength::new(text.height());

        if max_width.get() <= 0. || max_height.get() <= 0. {
            return;
        }

        let string = text.text();
        let string = string.as_str();
        let font_request = text.font_request(WindowInner::from_pub(&self.window));

        let paint = match self.brush_to_paint(
            text.color(),
            max_width * self.scale_factor,
            max_height * self.scale_factor,
        ) {
            Some(paint) => paint,
            None => return,
        };

        let mut text_style = skia_safe::textlayout::TextStyle::new();
        text_style.set_foreground_color(paint);

        let (layout, layout_top_left) = super::textlayout::create_layout(
            font_request,
            self.scale_factor,
            string,
            Some(text_style),
            Some(max_width),
            max_height,
            text.horizontal_alignment(),
            text.vertical_alignment(),
            text.overflow(),
            None,
        );

        layout.paint(&mut self.canvas, to_skia_point(layout_top_left));
    }

    fn draw_text_input(
        &mut self,
        text_input: std::pin::Pin<&i_slint_core::items::TextInput>,
        _self_rc: &i_slint_core::items::ItemRc,
    ) {
        let max_width = LogicalLength::new(text_input.width());
        let max_height = LogicalLength::new(text_input.height());

        if max_width.get() <= 0. || max_height.get() <= 0. {
            return;
        }

        let string = text_input.text();
        let string = string.as_str();
        let font_request =
            text_input.font_request(&WindowInner::from_pub(&self.window).window_adapter());

        let paint = match self.brush_to_paint(
            text_input.color(),
            max_width * self.scale_factor,
            max_height * self.scale_factor,
        ) {
            Some(paint) => paint,
            None => return,
        };

        let mut text_style = skia_safe::textlayout::TextStyle::new();
        text_style.set_foreground_color(paint);

        let (selection_anchor_pos, selection_cursor_pos) = text_input.selection_anchor_and_cursor();
        let selection = if selection_anchor_pos != selection_cursor_pos {
            Some(super::textlayout::Selection {
                range: (selection_anchor_pos..selection_cursor_pos),
                foreground: text_input.selection_foreground_color(),
                background: text_input.selection_background_color(),
            })
        } else {
            None
        };

        let (layout, layout_top_left) = super::textlayout::create_layout(
            font_request,
            self.scale_factor,
            string,
            Some(text_style),
            Some(max_width),
            max_height,
            text_input.horizontal_alignment(),
            text_input.vertical_alignment(),
            i_slint_core::items::TextOverflow::Clip,
            selection.as_ref(),
        );

        layout.paint(&mut self.canvas, to_skia_point(layout_top_left));

        let cursor_pos = text_input.cursor_position();

        let cursor_visible = cursor_pos >= 0
            && text_input.cursor_visible()
            && text_input.enabled()
            && !text_input.read_only();

        if cursor_visible {
            let cursor_rect = super::textlayout::cursor_rect(
                string,
                cursor_pos as usize,
                layout,
                LogicalLength::new(text_input.text_cursor_width()) * self.scale_factor,
            )
            .translate(layout_top_left.to_vector());

            let cursor_paint = match self.brush_to_paint(
                text_input.color(),
                cursor_rect.width_length(),
                cursor_rect.height_length(),
            ) {
                Some(paint) => paint,
                None => return,
            };

            self.canvas.draw_rect(to_skia_rect(&cursor_rect), &cursor_paint);
        }
    }

    fn draw_path(
        &mut self,
        path: std::pin::Pin<&i_slint_core::items::Path>,
        _self_rc: &i_slint_core::items::ItemRc,
    ) {
        let geometry = item_rect(path, self.scale_factor);

        let (offset, path_events) = path.fitted_path_events();

        let mut skpath = skia_safe::Path::new();

        for x in path_events.iter() {
            match x {
                lyon_path::Event::Begin { at } => {
                    skpath
                        .move_to(to_skia_point(LogicalPoint::from_untyped(at) * self.scale_factor));
                }
                lyon_path::Event::Line { from: _, to } => {
                    skpath
                        .line_to(to_skia_point(LogicalPoint::from_untyped(to) * self.scale_factor));
                }
                lyon_path::Event::Quadratic { from: _, ctrl, to } => {
                    skpath.quad_to(
                        to_skia_point(LogicalPoint::from_untyped(ctrl) * self.scale_factor),
                        to_skia_point(LogicalPoint::from_untyped(to) * self.scale_factor),
                    );
                }

                lyon_path::Event::Cubic { from: _, ctrl1, ctrl2, to } => {
                    skpath.cubic_to(
                        to_skia_point(LogicalPoint::from_untyped(ctrl1) * self.scale_factor),
                        to_skia_point(LogicalPoint::from_untyped(ctrl2) * self.scale_factor),
                        to_skia_point(LogicalPoint::from_untyped(to) * self.scale_factor),
                    );
                }
                lyon_path::Event::End { last: _, first: _, close } => {
                    if close {
                        skpath.close();
                    }
                }
            }
        }

        self.canvas.translate((offset.x, offset.y));

        if let Some(mut fill_paint) =
            self.brush_to_paint(path.fill(), geometry.width_length(), geometry.height_length())
        {
            fill_paint.set_anti_alias(true);
            self.canvas.draw_path(&skpath, &fill_paint);
        }
        if let Some(mut border_paint) =
            self.brush_to_paint(path.stroke(), geometry.width_length(), geometry.height_length())
        {
            border_paint.set_anti_alias(true);
            border_paint.set_stroke_width(path.stroke_width());
            border_paint.set_stroke(true);
            self.canvas.draw_path(&skpath, &border_paint);
        }
    }

    fn draw_box_shadow(
        &mut self,
        box_shadow: std::pin::Pin<&i_slint_core::items::BoxShadow>,
        self_rc: &i_slint_core::items::ItemRc,
    ) {
        let offset =
            LogicalPoint::new(box_shadow.offset_x(), box_shadow.offset_y()) * self.scale_factor;

        if offset.x == 0. && offset.y == 0. && box_shadow.blur() == 0.0 {
            return;
        }

        let cached_shadow_image = self.box_shadow_cache.get_box_shadow(
            self_rc,
            self.image_cache,
            box_shadow,
            self.scale_factor,
            |shadow_options| {
                let shadow_size: skia_safe::Size = (
                    shadow_options.width.get() + shadow_options.blur.get() * 2.,
                    shadow_options.height.get() + shadow_options.blur.get() * 2.,
                )
                    .into();

                let image_info = skia_safe::ImageInfo::new(
                    shadow_size.to_ceil(),
                    skia_safe::ColorType::RGBA8888,
                    skia_safe::AlphaType::Premul,
                    None,
                );

                let rounded_rect = skia_safe::RRect::new_rect_xy(
                    skia_safe::Rect::from_xywh(
                        shadow_options.blur.get(),
                        shadow_options.blur.get(),
                        shadow_options.width.get(),
                        shadow_options.height.get(),
                    ),
                    shadow_options.radius.get(),
                    shadow_options.radius.get(),
                );

                let mut paint = skia_safe::Paint::default();
                paint.set_color(to_skia_color(&shadow_options.color));
                paint.set_anti_alias(true);
                paint.set_mask_filter(skia_safe::MaskFilter::blur(
                    skia_safe::BlurStyle::Normal,
                    shadow_options.blur.get() / 2.,
                    None,
                ));

                let mut surface = self.canvas.new_surface(&image_info, None).unwrap();
                let canvas = surface.canvas();
                canvas.clear(skia_safe::Color::TRANSPARENT);
                canvas.draw_rrect(rounded_rect, &paint);
                surface.image_snapshot()
            },
        );

        let cached_shadow_image = match cached_shadow_image {
            Some(img) => img,
            None => return,
        };

        let blur = LogicalLength::new(box_shadow.blur()) * self.scale_factor;
        self.canvas.draw_image(
            cached_shadow_image,
            to_skia_point(offset - PhysicalPoint::from_lengths(blur, blur).to_vector()),
            None,
        );
    }

    fn combine_clip(
        &mut self,
        rect: LogicalRect,
        radius: LogicalLength,
        border_width: LogicalLength,
    ) -> bool {
        let mut rect = rect * self.scale_factor;
        let mut border_width = border_width * self.scale_factor;
        // In CSS the border is entirely towards the inside of the boundary
        // geometry, while in femtovg the line with for a stroke is 50% in-
        // and 50% outwards. We choose the CSS model, so the inner rectangle
        // is adjusted accordingly.
        adjust_rect_and_border_for_inner_drawing(&mut rect, &mut border_width);

        let radius = radius * self.scale_factor;
        let rounded_rect =
            skia_safe::RRect::new_rect_xy(to_skia_rect(&rect), radius.get(), radius.get());
        self.canvas.clip_rrect(rounded_rect, None, true);
        self.canvas.local_clip_bounds().is_some()
    }

    fn get_current_clip(&self) -> LogicalRect {
        from_skia_rect(&self.canvas.local_clip_bounds().unwrap_or_default()) / self.scale_factor
    }

    fn translate(&mut self, x: LogicalLength, y: LogicalLength) {
        self.canvas.translate(skia_safe::Vector::from((
            (x * self.scale_factor).get(),
            (y * self.scale_factor).get(),
        )));
    }

    fn rotate(&mut self, angle_in_degrees: f32) {
        self.canvas.rotate(angle_in_degrees, None);
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
        self.scale_factor.get()
    }

    fn draw_cached_pixmap(
        &mut self,
        item_rc: &i_slint_core::items::ItemRc,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        let skia_image = self.image_cache.get_or_update_cache_entry(item_rc, || {
            let mut cached_image = None;
            update_fn(&mut |width: u32, height: u32, data: &[u8]| {
                let image_info = skia_safe::ImageInfo::new(
                    skia_safe::ISize::new(width as i32, height as i32),
                    skia_safe::ColorType::RGBA8888,
                    skia_safe::AlphaType::Premul,
                    None,
                );
                cached_image = skia_safe::image::Image::from_raster_data(
                    &image_info,
                    skia_safe::Data::new_copy(data),
                    width as usize * 4,
                );
            });
            cached_image
        });
        let skia_image = match skia_image {
            Some(img) => img,
            None => return,
        };
        self.canvas.draw_image(skia_image, skia_safe::Point::default(), None);
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

    fn window(&self) -> &i_slint_core::api::Window {
        self.window
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

    fn visit_layer(&mut self, layer_item: Pin<&Layer>, self_rc: &ItemRc) -> RenderingResult {
        if layer_item.cache_rendering_hint() {
            self.render_and_blend_layer(self_rc)
        } else {
            self.image_cache.release(self_rc);
            RenderingResult::ContinueRenderingChildren
        }
    }
}

pub fn from_skia_rect(rect: &skia_safe::Rect) -> PhysicalRect {
    let top_left = euclid::Point2D::new(rect.left, rect.top);
    let bottom_right = euclid::Point2D::new(rect.right, rect.bottom);
    euclid::Box2D::new(top_left, bottom_right).to_rect()
}

pub fn to_skia_rect(rect: &PhysicalRect) -> skia_safe::Rect {
    skia_safe::Rect::from_xywh(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height)
}

pub fn to_skia_point(point: PhysicalPoint) -> skia_safe::Point {
    skia_safe::Point::new(point.x, point.y)
}

pub fn to_skia_size(size: &PhysicalSize) -> skia_safe::Size {
    skia_safe::Size::new(size.width, size.height)
}

fn item_rect<Item: items::Item>(item: Pin<&Item>, scale_factor: ScaleFactor) -> PhysicalRect {
    let geometry = LogicalRect::from_untyped(&item.geometry());
    PhysicalRect::new(
        PhysicalPoint::default(),
        PhysicalSize::from_lengths(
            geometry.width_length() * scale_factor,
            geometry.height_length() * scale_factor,
        ),
    )
}

pub fn to_skia_color(col: &Color) -> skia_safe::Color {
    skia_safe::Color::from_argb(col.alpha(), col.red(), col.green(), col.blue())
}

fn adjust_rect_and_border_for_inner_drawing(
    rect: &mut PhysicalRect,
    border_width: &mut PhysicalLength,
) {
    // If the border width exceeds the width, just fill the rectangle.
    *border_width = border_width.min(rect.width_length() / 2.);
    // adjust the size so that the border is drawn within the geometry

    let half_border_size = PhysicalSize::from_lengths(*border_width / 2., *border_width / 2.);
    rect.origin += half_border_size;
    rect.size -= half_border_size;
}

/// Changes the source or the destination rectangle to respect the image fit
fn adjust_to_image_fit(
    image_fit: ImageFit,
    source_rect: &mut skia_safe::Rect,
    dest_rect: &mut PhysicalRect,
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
                dest_rect.origin.x += (dest_rect.width() - source_rect.width() * ratio) / 2.;
                dest_rect.size.width = source_rect.width() * ratio;
            }
            if dest_rect.height() > source_rect.height() * ratio {
                dest_rect.origin.y += (dest_rect.height() - source_rect.height() * ratio) / 2.;
                dest_rect.size.height = source_rect.height() * ratio;
            }
        }
    };
}
