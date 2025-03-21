// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore rrect

use std::pin::Pin;

use super::{PhysicalBorderRadius, PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalSize};
use i_slint_core::graphics::boxshadowcache::BoxShadowCache;
use i_slint_core::graphics::euclid::num::Zero;
use i_slint_core::graphics::euclid::{self, Vector2D};
use i_slint_core::graphics::ApproxEq;
use i_slint_core::item_rendering::{
    CachedRenderingData, ItemCache, ItemRenderer, ItemRendererFeatures, RenderImage, RenderText,
};
use i_slint_core::items::{
    ImageFit, ImageRendering, ItemRc, Layer, Opacity, RenderingResult, TextStrokeStyle,
};
use i_slint_core::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalPx, LogicalRect, LogicalSize,
    LogicalVector, PhysicalPx, RectLengths, ScaleFactor, SizeLengths,
};
use i_slint_core::window::WindowInner;
use i_slint_core::{Brush, Color};
use skia_safe::{Matrix, TileMode};

pub type SkiaBoxShadowCache = BoxShadowCache<skia_safe::Image>;

#[derive(Clone, Copy)]
struct RenderState {
    alpha: f32,
    translation: LogicalVector,
}

pub struct SkiaItemRenderer<'a> {
    pub canvas: &'a skia_safe::Canvas,
    pub scale_factor: ScaleFactor,
    pub window: &'a i_slint_core::api::Window,
    state_stack: Vec<RenderState>,
    current_state: RenderState,
    image_cache: &'a ItemCache<Option<skia_safe::Image>>,
    path_cache: &'a ItemCache<Option<(Vector2D<f32, PhysicalPx>, skia_safe::Path)>>,
    box_shadow_cache: &'a mut SkiaBoxShadowCache,
}

impl<'a> SkiaItemRenderer<'a> {
    pub fn new(
        canvas: &'a skia_safe::Canvas,
        window: &'a i_slint_core::api::Window,
        image_cache: &'a ItemCache<Option<skia_safe::Image>>,
        path_cache: &'a ItemCache<Option<(Vector2D<f32, PhysicalPx>, skia_safe::Path)>>,
        box_shadow_cache: &'a mut SkiaBoxShadowCache,
    ) -> Self {
        Self {
            canvas,
            scale_factor: ScaleFactor::new(window.scale_factor()),
            window,
            state_stack: vec![],
            current_state: RenderState { alpha: 1.0, translation: Default::default() },
            image_cache,
            path_cache,
            box_shadow_cache,
        }
    }

    fn default_paint(&self) -> Option<skia_safe::Paint> {
        if self.current_state.alpha.approx_eq(&1.0) {
            None
        } else {
            let mut paint = skia_safe::Paint::default();
            paint.set_alpha_f(self.current_state.alpha);
            Some(paint)
        }
    }

    fn brush_to_paint(
        &self,
        brush: Brush,
        width: PhysicalLength,
        height: PhysicalLength,
    ) -> Option<skia_safe::Paint> {
        let (mut paint, shader) =
            Self::brush_to_shader(self.default_paint().unwrap_or_default(), brush, width, height)?;
        paint.set_shader(Some(shader));

        Some(paint)
    }

    fn brush_to_shader(
        mut paint: skia_safe::Paint,
        brush: Brush,
        width: PhysicalLength,
        height: PhysicalLength,
    ) -> Option<(skia_safe::Paint, skia_safe::Shader)> {
        if brush.is_transparent() {
            return None;
        }

        match brush {
            Brush::SolidColor(color) => Some(skia_safe::shaders::color(to_skia_color(&color))),

            Brush::LinearGradient(g) => {
                let (start, end) = i_slint_core::graphics::line_for_angle(
                    g.angle(),
                    [width.get(), height.get()].into(),
                );
                let (colors, pos): (Vec<_>, Vec<_>) =
                    g.stops().map(|s| (to_skia_color(&s.color), s.position)).unzip();

                paint.set_dither(true);

                skia_safe::gradient_shader::linear(
                    (skia_safe::Point::new(start.x, start.y), skia_safe::Point::new(end.x, end.y)),
                    skia_safe::gradient_shader::GradientShaderColors::Colors(&colors),
                    Some(&*pos),
                    TileMode::Clamp,
                    skia_safe::gradient_shader::Flags::INTERPOLATE_COLORS_IN_PREMUL,
                    &skia_safe::Matrix::new_identity(),
                )
            }
            Brush::RadialGradient(g) => {
                let (colors, pos): (Vec<_>, Vec<_>) =
                    g.stops().map(|s| (to_skia_color(&s.color), s.position)).unzip();
                let circle_scale =
                    0.5 * (width.get() * width.get() + height.get() * height.get()).sqrt();

                paint.set_dither(true);

                skia_safe::gradient_shader::radial(
                    skia_safe::Point::new(0., 0.),
                    1.,
                    skia_safe::gradient_shader::GradientShaderColors::Colors(&colors),
                    Some(&*pos),
                    TileMode::Clamp,
                    skia_safe::gradient_shader::Flags::INTERPOLATE_COLORS_IN_PREMUL,
                    skia_safe::Matrix::scale((circle_scale, circle_scale))
                        .post_translate((width.get() / 2., height.get() / 2.))
                        as &skia_safe::Matrix,
                )
            }
            _ => None,
        }
        .map(|shader| (paint, shader))
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

        Self::brush_to_shader(
            skia_safe::Paint::default(), // Don't use the renderer's default paint because alpha is applied later
            colorize_brush,
            PhysicalLength::new(image.width() as f32),
            PhysicalLength::new(image.height() as f32),
        )
        .map(|(mut paint, colorize_shader)| {
            let mut surface = self.canvas.new_surface(&image_info, None)?;
            let canvas = surface.canvas();
            canvas.clear(skia_safe::Color::TRANSPARENT);

            paint.set_image_filter(skia_safe::image_filters::blend(
                skia_safe::BlendMode::SrcIn,
                skia_safe::image_filters::image(image, None, None, None),
                skia_safe::image_filters::shader(colorize_shader, None),
                None,
            ));
            canvas.draw_paint(&paint);
            Some(surface.image_snapshot())
        })?
    }

    fn draw_image_impl(
        &mut self,
        item_rc: &ItemRc,
        item: Pin<&dyn RenderImage>,
        dest_rect: PhysicalRect,
    ) {
        let tiling = item.tiling();

        // TODO: avoid doing creating an SkImage multiple times when the same source is used in multiple image elements
        let skia_image = self.image_cache.get_or_update_cache_entry(item_rc, || {
            let image = item.source();
            super::cached_image::as_skia_image(
                image,
                &|| item.target_size(),
                if tiling != Default::default() { ImageFit::Preserve } else { item.image_fit() },
                self.scale_factor,
                self.canvas,
            )
            .and_then(|skia_image| {
                let brush = item.colorize();
                if !brush.is_transparent() {
                    self.colorize_image(skia_image, brush)
                } else {
                    Some(skia_image)
                }
            })
        });

        let Some(skia_image) = skia_image else { return };
        let source = item.source();
        let source_size = source.size();
        if source_size.is_empty() {
            // Not sure how this can happen, but we've seen with #6280
            // that somehow we end up with a `skia_safe::Image` but a zero
            // source size.
            return;
        }
        let fits = if let &i_slint_core::ImageInner::NineSlice(ref nine) = (&source).into() {
            i_slint_core::graphics::fit9slice(
                source_size.cast(),
                nine.1,
                dest_rect.size,
                self.scale_factor,
                item.alignment(),
                tiling,
            )
            .collect::<Vec<_>>()
        } else {
            vec![i_slint_core::graphics::fit(
                item.image_fit(),
                dest_rect.size,
                item.source_clip().unwrap_or_else(|| euclid::Rect::from_size(source_size.cast())),
                self.scale_factor,
                item.alignment(),
                tiling,
            )]
        };

        for fit in fits {
            self.canvas.save();

            let dst = to_skia_rect(&PhysicalRect::new(fit.offset, fit.size));
            self.canvas.clip_rect(dst, None, None);
            let src = skia_safe::IRect::from_xywh(
                skia_image.width() * fit.clip_rect.origin.x / source_size.width as i32,
                skia_image.height() * fit.clip_rect.origin.y / source_size.height as i32,
                skia_image.width() * fit.clip_rect.size.width / source_size.width as i32,
                skia_image.height() * fit.clip_rect.size.height / source_size.height as i32,
            );

            let filter_mode: skia_safe::sampling_options::SamplingOptions =
                match item.rendering() {
                    ImageRendering::Smooth => skia_safe::sampling_options::FilterMode::Linear,
                    ImageRendering::Pixelated => skia_safe::sampling_options::FilterMode::Nearest,
                }
                .into();

            if let Some(tiled_offset) = fit.tiled {
                let matrix = Matrix::translate(((fit.offset.x as i32), (fit.offset.y as i32)))
                    * Matrix::scale((
                        fit.source_to_target_x * source_size.width as f32
                            / skia_image.width() as f32,
                        fit.source_to_target_y * source_size.height as f32
                            / skia_image.height() as f32,
                    ))
                    * Matrix::translate((-(tiled_offset.x as i32), -(tiled_offset.y as i32)));
                if let Some(shader) =
                    skia_image.make_subset(self.canvas.direct_context().as_mut(), &src).and_then(
                        |i| i.to_shader((TileMode::Repeat, TileMode::Repeat), filter_mode, &matrix),
                    )
                {
                    let mut paint = self.default_paint().unwrap_or_default();
                    paint.set_shader(shader);
                    self.canvas.draw_paint(&paint);
                }
            } else {
                let transform =
                    skia_safe::Matrix::rect_to_rect(skia_safe::Rect::from(src), dst, None)
                        .unwrap_or_default();
                self.canvas.concat(&transform);
                self.canvas.draw_image_with_sampling_options(
                    skia_image.clone(),
                    skia_safe::Point::default(),
                    filter_mode,
                    self.default_paint().as_ref(),
                );
            }

            self.canvas.restore();
        }
    }

    fn render_and_blend_layer(&mut self, item_rc: &ItemRc) -> RenderingResult {
        let current_clip = self.get_current_clip();
        if let Some(layer_image) = self.render_layer(item_rc, &|| {
            // We don't need to include the size of the "layer" item itself, since it has no content.
            let children_rect = i_slint_core::properties::evaluate_no_tracking(|| {
                item_rc.geometry().union(
                    &i_slint_core::item_rendering::item_children_bounding_rect(
                        item_rc.item_tree(),
                        item_rc.index() as isize,
                        &current_clip,
                    ),
                )
            });
            children_rect.size_length()
        }) {
            let _saved_canvas = self.pixel_align_origin();
            self.canvas.draw_image_with_sampling_options(
                layer_image,
                skia_safe::Point::default(),
                skia_safe::sampling_options::FilterMode::Linear,
                self.default_paint().as_ref(),
            );
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

            let mut sub_renderer = SkiaItemRenderer::new(
                canvas,
                self.window,
                self.image_cache,
                self.path_cache,
                self.box_shadow_cache,
            );

            i_slint_core::item_rendering::render_item_children(
                &mut sub_renderer,
                item_rc.item_tree(),
                item_rc.index() as isize,
                &WindowInner::from_pub(self.window).window_adapter(),
            );

            Some(surface.image_snapshot())
        })
    }

    fn pixel_align_origin(&self) -> Option<skia_safe::canvas::AutoRestoredCanvas<'_>> {
        let local_to_device = self.canvas.local_to_device_as_3x3();
        let Some(device_to_local) = local_to_device.invert() else {
            return None;
        };
        let mut target_point = local_to_device.map_point(skia_safe::Point::default());

        target_point.x = target_point.x.round();
        target_point.y = target_point.y.round();

        let restore_point = skia_safe::AutoCanvasRestore::guard(self.canvas, true);

        self.canvas.translate(device_to_local.map_point(target_point));

        Some(restore_point)
    }
}

impl ItemRenderer for SkiaItemRenderer<'_> {
    fn draw_rectangle(
        &mut self,
        rect: Pin<&dyn i_slint_core::item_rendering::RenderRectangle>,
        _self_rc: &i_slint_core::items::ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let geometry = PhysicalRect::from(size * self.scale_factor);
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
        rect: Pin<&dyn i_slint_core::item_rendering::RenderBorderRectangle>,
        _self_rc: &i_slint_core::items::ItemRc,
        size: LogicalSize,
        _: &CachedRenderingData,
    ) {
        let mut geometry = PhysicalRect::from(size * self.scale_factor);
        if geometry.is_empty() {
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
        // Skia's border radius on stroke is in the middle of the border. But we want it to be the radius of the rectangle itself.
        // This is incorrect if fill_radius < border_width/2, but this can't be fixed. Better to have a radius a bit too big than no radius at all
        fill_radius = fill_radius.outer(border_width / 2. + PhysicalLength::new(0.01));
        let stroke_border_radius = fill_radius.inner(border_width / 2.);

        let (background_rect, border_rect) = if opaque_border {
            // In CSS the border is entirely towards the inside of the boundary
            // geometry, while in femtovg the line with for a stroke is 50% in-
            // and 50% outwards. We choose the CSS model, so the inner rectangle
            // is adjusted accordingly.
            adjust_rect_and_border_for_inner_drawing(&mut geometry, &mut border_width);

            let rounded_rect = to_skia_rrect(&geometry, &stroke_border_radius);

            (rounded_rect.clone(), rounded_rect)
        } else {
            let background_rect = to_skia_rrect(&geometry, &fill_radius);

            // In CSS the border is entirely towards the inside of the boundary
            // geometry, while in femtovg the line with for a stroke is 50% in-
            // and 50% outwards. We choose the CSS model, so the inner rectangle
            // is adjusted accordingly.
            adjust_rect_and_border_for_inner_drawing(&mut geometry, &mut border_width);

            let border_rect = to_skia_rrect(&geometry, &stroke_border_radius);

            (background_rect, border_rect)
        };

        if let Some(mut fill_paint) = self.brush_to_paint(
            rect.background(),
            geometry.width_length(),
            geometry.height_length(),
        ) {
            fill_paint.set_style(skia_safe::PaintStyle::Fill);
            if !background_rect.is_rect() {
                fill_paint.set_anti_alias(true);
            }
            self.canvas.draw_rrect(background_rect, &fill_paint);
        }

        if border_width.get() > 0.0 {
            if let Some(mut border_paint) =
                self.brush_to_paint(border_color, geometry.width_length(), geometry.height_length())
            {
                border_paint.set_style(skia_safe::PaintStyle::Stroke);
                border_paint.set_stroke_width(border_width.get());
                if !border_rect.is_rect() {
                    border_paint.set_anti_alias(true);
                }
                self.canvas.draw_rrect(border_rect, &border_paint);
            }
        }
    }

    fn draw_window_background(
        &mut self,
        _rect: Pin<&dyn i_slint_core::item_rendering::RenderRectangle>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        // The background is drawn directly by FemtoVG renderer (via clear_color, if necessary).
    }

    fn draw_image(
        &mut self,
        image: Pin<&dyn RenderImage>,
        self_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let geometry = PhysicalRect::from(size * self.scale_factor);
        if geometry.is_empty() {
            return;
        }
        self.draw_image_impl(self_rc, image, geometry);
    }

    fn draw_text(
        &mut self,
        text: Pin<&dyn RenderText>,
        _self_rc: &i_slint_core::items::ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let max_width = size.width_length() * self.scale_factor;
        let max_height = size.height_length() * self.scale_factor;

        if max_width.get() <= 0. || max_height.get() <= 0. {
            return;
        }

        let string = text.text();
        let string = string.as_str();
        let font_request = text.font_request(WindowInner::from_pub(self.window));

        let paint = match self.brush_to_paint(text.color(), max_width, max_height) {
            Some(paint) => paint,
            None => return,
        };

        let mut text_style = skia_safe::textlayout::TextStyle::new();
        text_style.set_foreground_paint(&paint);

        let (stroke_brush, stroke_width, stroke_style) = text.stroke();
        let (horizontal_alignment, vertical_alignment) = text.alignment();
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

        let mut text_stroke_style = skia_safe::textlayout::TextStyle::new();
        let stroke_layout = match self.brush_to_paint(stroke_brush.clone(), max_width, max_height) {
            Some(mut stroke_paint) => {
                if stroke_brush.is_transparent() {
                    None
                } else {
                    stroke_paint.set_style(skia_safe::PaintStyle::Stroke);
                    stroke_paint.set_stroke_width(stroke_width);
                    // Set stroke cap/join/miter to match FemtoVG
                    stroke_paint.set_stroke_cap(skia_safe::PaintCap::Butt);
                    stroke_paint.set_stroke_join(skia_safe::PaintJoin::Miter);
                    stroke_paint.set_stroke_miter(10.0);
                    text_stroke_style.set_foreground_paint(&stroke_paint);
                    Some(super::textlayout::create_layout(
                        font_request.clone(),
                        self.scale_factor,
                        string,
                        Some(text_stroke_style),
                        Some(max_width),
                        max_height,
                        horizontal_alignment,
                        vertical_alignment,
                        text.wrap(),
                        text.overflow(),
                        None,
                    ))
                }
            }
            None => None,
        };

        let (layout, layout_top_left) = super::textlayout::create_layout(
            font_request,
            self.scale_factor,
            string,
            Some(text_style),
            Some(max_width),
            max_height,
            horizontal_alignment,
            vertical_alignment,
            text.wrap(),
            text.overflow(),
            None,
        );

        match (stroke_style, stroke_layout) {
            (TextStrokeStyle::Outside, Some((stroke_layout, stroke_layout_top_left))) => {
                stroke_layout.paint(self.canvas, to_skia_point(stroke_layout_top_left));
                layout.paint(self.canvas, to_skia_point(layout_top_left));
            }
            (TextStrokeStyle::Center, Some((stroke_layout, stroke_layout_top_left))) => {
                layout.paint(self.canvas, to_skia_point(layout_top_left));
                stroke_layout.paint(self.canvas, to_skia_point(stroke_layout_top_left));
            }
            _ => {
                layout.paint(self.canvas, to_skia_point(layout_top_left));
            }
        };
    }

    fn draw_text_input(
        &mut self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        _self_rc: &i_slint_core::items::ItemRc,
        size: LogicalSize,
    ) {
        let max_width = size.width_length() * self.scale_factor;
        let max_height = size.height_length() * self.scale_factor;

        if max_width.get() <= 0. || max_height.get() <= 0. {
            return;
        }

        let font_request =
            text_input.font_request(&WindowInner::from_pub(self.window).window_adapter());

        let visual_representation = text_input.visual_representation(None);
        let paint =
            match self.brush_to_paint(visual_representation.text_color, max_width, max_height) {
                Some(paint) => paint,
                None => return,
            };

        let mut text_style = skia_safe::textlayout::TextStyle::new();
        text_style.set_foreground_paint(&paint);

        let selection = if !visual_representation.preedit_range.is_empty() {
            Some(super::textlayout::Selection {
                range: visual_representation.preedit_range,
                foreground: None,
                background: None,
                underline: true,
            })
        } else if !visual_representation.selection_range.is_empty() {
            Some(super::textlayout::Selection {
                range: visual_representation.selection_range,
                foreground: text_input.selection_foreground_color().into(),
                background: text_input.selection_background_color().into(),
                underline: false,
            })
        } else {
            None
        };

        let (layout, layout_top_left) = super::textlayout::create_layout(
            font_request,
            self.scale_factor,
            &visual_representation.text,
            Some(text_style),
            Some(max_width),
            max_height,
            text_input.horizontal_alignment(),
            text_input.vertical_alignment(),
            text_input.wrap(),
            i_slint_core::items::TextOverflow::Clip,
            selection.as_ref(),
        );

        layout.paint(self.canvas, to_skia_point(layout_top_left));

        if let Some(cursor_position) = visual_representation.cursor_position {
            let cursor_rect = super::textlayout::cursor_rect(
                &visual_representation.text,
                cursor_position,
                layout,
                text_input.text_cursor_width() * self.scale_factor,
                text_input.horizontal_alignment(),
            )
            .translate(layout_top_left.to_vector());

            let cursor_paint = match self.brush_to_paint(
                Brush::SolidColor(visual_representation.cursor_color),
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
        path: Pin<&i_slint_core::items::Path>,
        item_rc: &i_slint_core::items::ItemRc,
        size: LogicalSize,
    ) {
        let geometry = PhysicalRect::from(size * self.scale_factor);

        let (physical_offset, skpath): (crate::euclid::Vector2D<f32, PhysicalPx>, _) =
            match self.path_cache.get_or_update_cache_entry(item_rc, || {
                let (logical_offset, path_events): (crate::euclid::Vector2D<f32, LogicalPx>, _) =
                    path.fitted_path_events(item_rc)?;

                let mut skpath = skia_safe::Path::new();

                for x in path_events.iter() {
                    match x {
                        lyon_path::Event::Begin { at } => {
                            skpath.move_to(to_skia_point(
                                LogicalPoint::from_untyped(at) * self.scale_factor,
                            ));
                        }
                        lyon_path::Event::Line { from: _, to } => {
                            skpath.line_to(to_skia_point(
                                LogicalPoint::from_untyped(to) * self.scale_factor,
                            ));
                        }
                        lyon_path::Event::Quadratic { from: _, ctrl, to } => {
                            skpath.quad_to(
                                to_skia_point(LogicalPoint::from_untyped(ctrl) * self.scale_factor),
                                to_skia_point(LogicalPoint::from_untyped(to) * self.scale_factor),
                            );
                        }

                        lyon_path::Event::Cubic { from: _, ctrl1, ctrl2, to } => {
                            skpath.cubic_to(
                                to_skia_point(
                                    LogicalPoint::from_untyped(ctrl1) * self.scale_factor,
                                ),
                                to_skia_point(
                                    LogicalPoint::from_untyped(ctrl2) * self.scale_factor,
                                ),
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

                (logical_offset * self.scale_factor, skpath).into()
            }) {
                Some(offset_and_path) => offset_and_path,
                None => return,
            };

        self.canvas.translate((physical_offset.x, physical_offset.y));

        let anti_alias = path.anti_alias();

        if let Some(mut fill_paint) =
            self.brush_to_paint(path.fill(), geometry.width_length(), geometry.height_length())
        {
            fill_paint.set_anti_alias(anti_alias);
            self.canvas.draw_path(&skpath, &fill_paint);
        }
        if let Some(mut border_paint) =
            self.brush_to_paint(path.stroke(), geometry.width_length(), geometry.height_length())
        {
            border_paint.set_anti_alias(anti_alias);
            border_paint.set_stroke_width((path.stroke_width() * self.scale_factor).get());
            border_paint.set_stroke_cap(match path.stroke_line_cap() {
                i_slint_core::items::LineCap::Butt => skia_safe::PaintCap::Butt,
                i_slint_core::items::LineCap::Round => skia_safe::PaintCap::Round,
                i_slint_core::items::LineCap::Square => skia_safe::PaintCap::Square,
            });
            border_paint.set_stroke(true);
            self.canvas.draw_path(&skpath, &border_paint);
        }
    }

    fn draw_box_shadow(
        &mut self,
        box_shadow: Pin<&i_slint_core::items::BoxShadow>,
        self_rc: &i_slint_core::items::ItemRc,
        _size: LogicalSize,
    ) {
        let offset = LogicalPoint::from_lengths(box_shadow.offset_x(), box_shadow.offset_y())
            * self.scale_factor;

        if offset.x == 0. && offset.y == 0. && box_shadow.blur() == LogicalLength::zero() {
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

                let mut surface = self.canvas.new_surface(&image_info, None)?;
                let canvas = surface.canvas();
                canvas.clear(skia_safe::Color::TRANSPARENT);
                canvas.draw_rrect(rounded_rect, &paint);
                Some(surface.image_snapshot())
            },
        );

        let cached_shadow_image = match cached_shadow_image {
            Some(img) => img,
            None => return,
        };

        let blur = box_shadow.blur() * self.scale_factor;
        self.canvas.draw_image(
            cached_shadow_image,
            to_skia_point(offset - PhysicalPoint::from_lengths(blur, blur).to_vector()),
            self.default_paint().as_ref(),
        );
    }

    fn combine_clip(
        &mut self,
        rect: LogicalRect,
        radius: LogicalBorderRadius,
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
        let rounded_rect = to_skia_rrect(&rect, &radius);
        self.canvas.clip_rrect(rounded_rect, None, true);
        self.canvas.local_clip_bounds().is_some()
    }

    fn get_current_clip(&self) -> LogicalRect {
        from_skia_rect(&self.canvas.local_clip_bounds().unwrap_or_default()) / self.scale_factor
    }

    fn translate(&mut self, distance: LogicalVector) {
        self.current_state.translation += distance;
        let distance = distance * self.scale_factor;
        self.canvas.translate(skia_safe::Vector::from((distance.x, distance.y)));
    }

    fn translation(&self) -> LogicalVector {
        self.current_state.translation
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
                cached_image = skia_safe::images::raster_from_data(
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
        let _saved_canvas = self.pixel_align_origin();
        self.canvas.draw_image(skia_image, skia_safe::Point::default(), None);
    }

    fn draw_string(&mut self, string: &str, color: i_slint_core::Color) {
        let mut paint = self.default_paint().unwrap_or_default();
        paint.set_color(to_skia_color(&color));
        if let Some(font) = super::textlayout::default_font(self.scale_factor.get()) {
            self.canvas.draw_str(
                string,
                skia_safe::Point::new(0., 12. * self.scale_factor.get()), // Default text size is 12 pixels
                &font,
                &paint,
            );
        }
    }

    fn draw_image_direct(&mut self, image: i_slint_core::graphics::Image) {
        let skia_image = super::cached_image::as_skia_image(
            image.clone(),
            &|| LogicalSize::from_untyped(image.size().cast()),
            ImageFit::Fill,
            self.scale_factor,
            self.canvas,
        );

        let skia_image = match skia_image {
            Some(img) => img,
            None => return,
        };

        self.canvas.draw_image(
            skia_image,
            skia_safe::Point::default(),
            self.default_paint().as_ref(),
        );
    }

    fn window(&self) -> &i_slint_core::window::WindowInner {
        i_slint_core::window::WindowInner::from_pub(self.window)
    }

    fn as_any(&mut self) -> Option<&mut dyn core::any::Any> {
        None
    }

    fn visit_opacity(
        &mut self,
        opacity_item: Pin<&Opacity>,
        item_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        let opacity = opacity_item.opacity();
        if Opacity::need_layer(item_rc, opacity) {
            self.canvas.save_layer_alpha(None, (opacity * 255.) as u32);
            self.state_stack.push(self.current_state);
            self.current_state.alpha = 1.0;

            let window_adapter = WindowInner::from_pub(self.window).window_adapter();

            i_slint_core::item_rendering::render_item_children(
                self,
                item_rc.item_tree(),
                item_rc.index() as isize,
                &window_adapter,
            );

            self.current_state = self.state_stack.pop().unwrap();
            self.canvas.restore();
            RenderingResult::ContinueRenderingWithoutChildren
        } else {
            self.apply_opacity(opacity);
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

pub fn to_skia_rrect(rect: &PhysicalRect, radius: &PhysicalBorderRadius) -> skia_safe::RRect {
    if let Some(radius) = radius.as_uniform() {
        skia_safe::RRect::new_rect_xy(to_skia_rect(rect), radius, radius)
    } else {
        skia_safe::RRect::new_rect_radii(
            to_skia_rect(rect),
            &[
                skia_safe::Point::new(radius.top_left, radius.top_left),
                skia_safe::Point::new(radius.top_right, radius.top_right),
                skia_safe::Point::new(radius.bottom_right, radius.bottom_right),
                skia_safe::Point::new(radius.bottom_left, radius.bottom_left),
            ],
        )
    }
}

impl ItemRendererFeatures for SkiaItemRenderer<'_> {
    const SUPPORTS_TRANSFORMATIONS: bool = true;
}

pub fn to_skia_point(point: PhysicalPoint) -> skia_safe::Point {
    skia_safe::Point::new(point.x, point.y)
}

pub fn to_skia_size(size: &PhysicalSize) -> skia_safe::Size {
    skia_safe::Size::new(size.width, size.height)
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

    rect.origin += PhysicalSize::from_lengths(*border_width / 2., *border_width / 2.);
    rect.size -= PhysicalSize::from_lengths(*border_width, *border_width);
}
