// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use euclid::approxeq::ApproxEq;
use i_slint_core::graphics::boxshadowcache::BoxShadowCache;
use i_slint_core::graphics::euclid::num::Zero;
use i_slint_core::graphics::euclid::{self};
use i_slint_core::graphics::rendering_metrics_collector::RenderingMetrics;
use i_slint_core::graphics::{
    Image, ImageCacheKey, IntRect, IntSize, OpaqueImage, OpaqueImageVTable, Point,
    SharedImageBuffer, SharedPixelBuffer, Size,
};
use i_slint_core::item_rendering::{
    CachedRenderingData, ItemCache, ItemRenderer, RenderBorderRectangle, RenderImage,
    RenderRectangle, RenderText,
};
use i_slint_core::items::{
    self, Clip, FillRule, ImageFit, ImageRendering, ImageTiling, ItemRc, Layer, Opacity,
    RenderingResult,
};
use i_slint_core::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalRect, LogicalSize, LogicalVector,
    RectLengths, ScaleFactor,
};
use i_slint_core::textlayout::sharedparley::{self, GlyphRenderer, parley};
use i_slint_core::{Brush, Color, ImageInner, SharedString};
use vello::kurbo::Shape;
use vello::peniko;

use super::PhysicalSize;
use super::{PhysicalBorderRadius, PhysicalLength, PhysicalPoint, PhysicalRect};

#[derive(Clone, Copy)]
struct RenderState {
    alpha: f32,
    translation: LogicalVector,
    clip_rect: LogicalRect,
    transform: vello::kurbo::Affine,
    layer_count: usize,
}

pub struct VelloItemRenderer<'a> {
    window: &'a i_slint_core::api::Window,
    scale_factor: ScaleFactor,
    scene: &'a mut vello::Scene,
    state_stack: Vec<RenderState>,
    current_state: RenderState,
}

impl<'a> VelloItemRenderer<'a> {
    pub fn new(
        scene: &'a mut vello::Scene,
        width: u32,
        height: u32,
        window: &'a i_slint_core::api::Window,
    ) -> Self {
        let scale_factor = ScaleFactor::new(window.scale_factor());
        Self {
            window,
            scale_factor,
            scene,
            state_stack: vec![],
            current_state: RenderState {
                alpha: 1.0,
                translation: Default::default(),
                clip_rect: LogicalRect::new(
                    LogicalPoint::default(),
                    PhysicalSize::new(width as f32, height as f32) / scale_factor,
                ),
                transform: Default::default(),
                layer_count: 0,
            },
        }
    }
}

impl<'a> ItemRenderer for VelloItemRenderer<'a> {
    fn draw_rectangle(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        _: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let geometry = LogicalRect::new(LogicalPoint::default(), size);
        if size.width <= 0. || size.height <= 0. {
            return;
        }
        let Some((brush, brush_transform)) =
            self.brush(rect.background(), self.size(geometry.size))
        else {
            return;
        };

        let shape = self.rect(geometry);
        self.scene.fill(
            vello::peniko::Fill::default(),
            self.current_state.transform,
            &brush,
            brush_transform,
            &shape,
        );
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

        let border_color = rect.border_color();
        let opaque_border = border_color.is_opaque();
        let mut border_width = if border_color.is_transparent() {
            PhysicalLength::new(0.)
        } else {
            rect.border_width() * self.scale_factor
        };

        let mut fill_radius = rect.border_radius() * self.scale_factor;
        // Vello's stroke is centered on the path (50% inside, 50% outside). We want
        // the CSS model where the border is entirely inside. Adjust the outer radius
        // so that corners with a positive radius are at least border_width/2.
        fill_radius = fill_radius.outer(border_width / 2. + PhysicalLength::new(0.01));
        let stroke_border_radius = fill_radius.inner(border_width / 2.);

        let (background_shape, border_shape) = if opaque_border {
            // When the border is opaque, the fill doesn't need to extend under it,
            // so both fill and stroke use the same adjusted (inset) geometry.
            adjust_rect_and_border_for_inner_drawing(&mut geometry, &mut border_width);
            let shape = self.phys_rounded_rect(geometry, stroke_border_radius);
            (shape, shape)
        } else {
            // When the border is transparent/semi-transparent, the fill must cover
            // the full outer rectangle so the background shows through.
            let background_shape = self.phys_rounded_rect(geometry, fill_radius);
            adjust_rect_and_border_for_inner_drawing(&mut geometry, &mut border_width);
            let border_shape = self.phys_rounded_rect(geometry, stroke_border_radius);
            (background_shape, border_shape)
        };

        let shape_size = vello::kurbo::Size::new(geometry.width() as f64, geometry.height() as f64);

        if let Some((brush, brush_transform)) = self.brush(rect.background(), shape_size) {
            self.scene.fill(
                vello::peniko::Fill::default(),
                self.current_state.transform,
                &brush,
                brush_transform,
                &background_shape,
            );
        }

        if border_width.get() > 0.0 {
            if let Some((brush, brush_transform)) = self.brush(border_color, shape_size) {
                self.scene.stroke(
                    &vello::kurbo::Stroke::new(border_width.get() as f64),
                    self.current_state.transform,
                    &brush,
                    brush_transform,
                    &border_shape,
                );
            }
        }
    }

    fn draw_window_background(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        _self_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        // Solid color backgrounds are handled as the base_color in VelloRenderer::render().
        // Only draw here for gradient backgrounds.
        let background = rect.background();
        if matches!(background, Brush::SolidColor(..)) {
            return;
        }
        let geometry = LogicalRect::new(LogicalPoint::default(), size);
        let Some((brush, brush_transform)) = self.brush(background, self.size(geometry.size))
        else {
            return;
        };
        let shape = self.rect(geometry);
        self.scene.fill(
            vello::peniko::Fill::default(),
            self.current_state.transform,
            &brush,
            brush_transform,
            &shape,
        );
    }

    fn draw_image(
        &mut self,
        image: Pin<&dyn RenderImage>,
        _item_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        if size.width <= 0. || size.height <= 0. {
            return;
        }

        let tiling = image.tiling();
        let image_fit =
            if tiling != Default::default() { ImageFit::Preserve } else { image.image_fit() };

        let source = image.source();
        let Some(image_data) =
            load_image(source.clone(), &|| image.target_size(), image_fit, self.scale_factor)
        else {
            return;
        };

        let source_size = source.size();
        if source_size.is_empty() {
            return;
        }

        let dest_size = size * self.scale_factor;

        let image_inner: &ImageInner = (&source).into();
        let fits: Vec<_> = if let ImageInner::NineSlice(nine) = image_inner {
            i_slint_core::graphics::fit9slice(
                source_size.cast(),
                nine.1,
                dest_size,
                self.scale_factor,
                image.alignment(),
                tiling,
            )
            .collect()
        } else {
            vec![i_slint_core::graphics::fit(
                image_fit,
                dest_size,
                image.source_clip().unwrap_or_else(|| euclid::Rect::from_size(source_size.cast())),
                self.scale_factor,
                image.alignment(),
                tiling,
            )]
        };

        let quality = match image.rendering() {
            ImageRendering::Pixelated => peniko::ImageQuality::Low,
            _ => peniko::ImageQuality::Medium,
        };

        // Ratio to convert from source coordinates to image data coordinates.
        // This differs from 1.0 for SVGs which are rendered at a different resolution.
        let ratio_x = image_data.width as f64 / source_size.width as f64;
        let ratio_y = image_data.height as f64 / source_size.height as f64;

        let colorize_brush = image.colorize();
        let has_colorize = !colorize_brush.is_transparent();
        if has_colorize {
            // Isolate the image in a compositing group so SrcIn only affects the image
            let clip = vello::kurbo::Rect::new(0., 0., 1e9, 1e9);
            self.scene.push_layer(
                peniko::Fill::default(),
                peniko::BlendMode::default(),
                1.0,
                vello::kurbo::Affine::IDENTITY,
                &clip,
            );
        }

        for fit in fits {
            let mut image_brush = peniko::ImageBrush::new(image_data.clone()).with_quality(quality);

            // Clip rect coordinates in image data space
            let clip_x = fit.clip_rect.origin.x as f64 * ratio_x;
            let clip_y = fit.clip_rect.origin.y as f64 * ratio_y;
            let clip_w = fit.clip_rect.size.width as f64 * ratio_x;
            let clip_h = fit.clip_rect.size.height as f64 * ratio_y;

            let brush_transform = if let Some(tiled_offset) = fit.tiled {
                image_brush = image_brush.with_extend(peniko::Extend::Repeat);

                // Scale from image data pixels to target pixels
                let scale_x = fit.source_to_target_x as f64 / ratio_x;
                let scale_y = fit.source_to_target_y as f64 / ratio_y;

                vello::kurbo::Affine::translate((
                    -(tiled_offset.x as f64 * ratio_x + clip_x),
                    -(tiled_offset.y as f64 * ratio_y + clip_y),
                ))
                .then_scale_non_uniform(scale_x, scale_y)
            } else {
                vello::kurbo::Affine::translate((-clip_x, -clip_y)).then_scale_non_uniform(
                    fit.size.width as f64 / clip_w,
                    fit.size.height as f64 / clip_h,
                )
            };

            let shape =
                vello::kurbo::Rect::new(0., 0., fit.size.width as f64, fit.size.height as f64);

            let transform = self
                .current_state
                .transform
                .then_translate(vello::kurbo::Vec2::new(fit.offset.x as f64, fit.offset.y as f64));

            self.scene.fill(
                peniko::Fill::default(),
                transform,
                &peniko::Brush::Image(image_brush),
                Some(brush_transform),
                &shape,
            );
        }

        if has_colorize {
            // Apply colorize: push a SrcIn layer and fill with the colorize brush.
            // SrcIn keeps the image's alpha but replaces the color.
            let dest_rect =
                vello::kurbo::Rect::new(0., 0., dest_size.width as f64, dest_size.height as f64);
            let src_in_blend = peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::SrcIn);
            if let Some((brush, brush_transform)) = self.brush(colorize_brush, dest_rect.size()) {
                self.scene.push_layer(
                    peniko::Fill::default(),
                    src_in_blend,
                    1.0,
                    vello::kurbo::Affine::IDENTITY,
                    &vello::kurbo::Rect::new(0., 0., 1e9, 1e9),
                );
                self.scene.fill(
                    peniko::Fill::default(),
                    self.current_state.transform,
                    &brush,
                    brush_transform,
                    &dest_rect,
                );
                self.scene.pop_layer(); // pop SrcIn layer
            }
            self.scene.pop_layer(); // pop isolation layer
        }
    }

    fn draw_text(
        &mut self,
        text: Pin<&dyn RenderText>,
        self_rc: &i_slint_core::items::ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        sharedparley::draw_text(self, text, Some(self_rc), size);
    }

    fn draw_text_input(
        &mut self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        self_rc: &i_slint_core::items::ItemRc,
        size: LogicalSize,
    ) {
        sharedparley::draw_text_input(self, text_input, self_rc, size, None);
    }

    fn draw_path(&mut self, path: Pin<&items::Path>, item_rc: &ItemRc, size: LogicalSize) {
        let (offset, path_events) = match path.fitted_path_events(item_rc) {
            Some(val) => val,
            None => return,
        };

        let sf = self.scale_factor;

        let mut bezpath = vello::kurbo::BezPath::new();
        for event in path_events.iter() {
            match event {
                lyon_path::Event::Begin { at } => {
                    let p = LogicalPoint::from_untyped(at) * sf;
                    bezpath.move_to((p.x as f64, p.y as f64));
                }
                lyon_path::Event::Line { to, .. } => {
                    let p = LogicalPoint::from_untyped(to) * sf;
                    bezpath.line_to((p.x as f64, p.y as f64));
                }
                lyon_path::Event::Quadratic { ctrl, to, .. } => {
                    let c = LogicalPoint::from_untyped(ctrl) * sf;
                    let p = LogicalPoint::from_untyped(to) * sf;
                    bezpath.quad_to((c.x as f64, c.y as f64), (p.x as f64, p.y as f64));
                }
                lyon_path::Event::Cubic { ctrl1, ctrl2, to, .. } => {
                    let c1 = LogicalPoint::from_untyped(ctrl1) * sf;
                    let c2 = LogicalPoint::from_untyped(ctrl2) * sf;
                    let p = LogicalPoint::from_untyped(to) * sf;
                    bezpath.curve_to(
                        (c1.x as f64, c1.y as f64),
                        (c2.x as f64, c2.y as f64),
                        (p.x as f64, p.y as f64),
                    );
                }
                lyon_path::Event::End { close, .. } => {
                    if close {
                        bezpath.close_path();
                    }
                }
            }
        }

        let phys_offset = offset * sf;
        let transform = self
            .current_state
            .transform
            .then_translate(vello::kurbo::Vec2::new(phys_offset.x as f64, phys_offset.y as f64));

        let phys_size = size * sf;
        let brush_size = vello::kurbo::Size::new(phys_size.width as f64, phys_size.height as f64);

        // Fill
        if !path.fill().is_transparent() {
            let fill_rule = match path.fill_rule() {
                FillRule::Evenodd => peniko::Fill::EvenOdd,
                _ => peniko::Fill::NonZero,
            };
            if let Some((brush, brush_transform)) = self.brush(path.fill(), brush_size) {
                self.scene.fill(fill_rule, transform, &brush, brush_transform, &bezpath);
            }
        }

        // Stroke
        if !path.stroke().is_transparent() {
            let stroke_width = (path.stroke_width() * sf).get() as f64;
            let cap = match path.stroke_line_cap() {
                items::LineCap::Round => vello::kurbo::Cap::Round,
                items::LineCap::Square => vello::kurbo::Cap::Square,
                _ => vello::kurbo::Cap::Butt,
            };
            let join = match path.stroke_line_join() {
                items::LineJoin::Round => vello::kurbo::Join::Round,
                items::LineJoin::Bevel => vello::kurbo::Join::Bevel,
                _ => vello::kurbo::Join::Miter,
            };
            let stroke = vello::kurbo::Stroke::new(stroke_width).with_caps(cap).with_join(join);
            if let Some((brush, brush_transform)) = self.brush(path.stroke(), brush_size) {
                self.scene.stroke(&stroke, transform, &brush, brush_transform, &bezpath);
            }
        }
    }

    fn draw_box_shadow(
        &mut self,
        box_shadow: Pin<&items::BoxShadow>,
        item_rc: &ItemRc,
        _size: LogicalSize,
    ) {
    }

    fn visit_opacity(
        &mut self,
        opacity_item: Pin<&Opacity>,
        _item_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        let opacity = opacity_item.opacity();
        if opacity < 1.0 {
            let clip = vello::kurbo::Rect::new(0., 0., 1e9, 1e9);
            self.scene.push_layer(
                peniko::Fill::default(),
                peniko::BlendMode::default(),
                opacity,
                vello::kurbo::Affine::IDENTITY,
                &clip,
            );
            self.current_state.layer_count += 1;
        }
        RenderingResult::ContinueRenderingChildren
    }

    fn visit_layer(
        &mut self,
        layer_item: Pin<&Layer>,
        self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        RenderingResult::ContinueRenderingChildren

        /*
        if layer_item.cache_rendering_hint() {
            self.render_and_blend_layer(1.0, self_rc)
        } else {
            self.graphics_cache.release(self_rc);
            RenderingResult::ContinueRenderingChildren
        }
        */
    }

    fn visit_clip(
        &mut self,
        clip_item: Pin<&Clip>,
        _item_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        if !clip_item.clip() {
            return RenderingResult::ContinueRenderingChildren;
        }

        let geometry = LogicalRect::from(size);

        if !self.get_current_clip().intersects(&geometry) {
            return RenderingResult::ContinueRenderingWithoutChildren;
        }

        let radius = clip_item.logical_border_radius();
        let border_width = clip_item.border_width();
        self.combine_clip(geometry, radius, border_width);
        RenderingResult::ContinueRenderingChildren
    }

    fn combine_clip(
        &mut self,
        clip_rect: LogicalRect,
        radius: LogicalBorderRadius,
        border_width: LogicalLength,
    ) -> bool {
        let clip = &mut self.current_state.clip_rect;
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

        let clip_shape = self.rounded_rect(clip_rect, radius);

        self.scene.push_layer(
            peniko::Fill::default(),
            peniko::BlendMode::default(),
            1.0,
            self.current_state.transform,
            &clip_shape,
        );
        self.current_state.layer_count += 1;

        clip_region_valid
    }

    fn get_current_clip(&self) -> LogicalRect {
        self.current_state.clip_rect
    }

    fn save_state(&mut self) {
        self.state_stack.push(self.current_state);
        self.current_state.layer_count = 0;
    }

    fn restore_state(&mut self) {
        for _ in 0..self.current_state.layer_count {
            self.scene.pop_layer();
        }
        self.current_state = self.state_stack.pop().unwrap();
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor.get()
    }

    fn draw_cached_pixmap(
        &mut self,
        item_rc: &ItemRc,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        todo!()
    }

    fn draw_string(&mut self, string: &str, color: Color) {
        sharedparley::draw_text(
            self,
            std::pin::pin!((SharedString::from(string), Brush::from(color))),
            None,
            LogicalSize::new(1., 1.), // Non-zero size to avoid an early return
        );
    }

    fn draw_image_direct(&mut self, image: i_slint_core::graphics::Image) {
        let Some(image_data) = load_image(
            image.clone(),
            &|| LogicalSize::from_untyped(image.size().cast()),
            ImageFit::Fill,
            self.scale_factor,
        ) else {
            return;
        };

        let shape =
            vello::kurbo::Rect::new(0., 0., image_data.width as f64, image_data.height as f64);

        self.scene.fill(
            peniko::Fill::default(),
            self.current_state.transform,
            &peniko::Brush::Image(peniko::ImageBrush::new(image_data)),
            None,
            &shape,
        );
    }

    fn window(&self) -> &i_slint_core::window::WindowInner {
        i_slint_core::window::WindowInner::from_pub(self.window)
    }

    fn as_any(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }

    fn translate(&mut self, distance: LogicalVector) {
        self.current_state.translation += distance;
        self.current_state.clip_rect = self.current_state.clip_rect.translate(-distance);
        let distance = distance * self.scale_factor;
        self.current_state.transform = self
            .current_state
            .transform
            .then_translate(vello::kurbo::Vec2::new(distance.x as f64, distance.y as f64));
    }

    fn rotate(&mut self, angle_in_degrees: f32) {
        self.current_state.transform =
            self.current_state.transform.then_rotate(angle_in_degrees.to_radians().into());
    }

    fn scale(&mut self, x_factor: f32, y_factor: f32) {
        self.current_state.transform =
            self.current_state.transform.then_scale_non_uniform(x_factor as f64, y_factor as f64)
    }

    fn apply_opacity(&mut self, opacity: f32) {
        self.current_state.alpha *= opacity;
    }
}

#[derive(Clone)]
pub struct GlyphBrush {
    peniko_brush: vello::peniko::Brush,
    brush_transform: Option<vello::kurbo::Affine>,
    style: vello::peniko::Style,
}

impl<'a> GlyphRenderer for VelloItemRenderer<'a> {
    type PlatformBrush = GlyphBrush;

    fn platform_text_fill_brush(
        &mut self,
        brush: Brush,
        size: LogicalSize,
    ) -> Option<Self::PlatformBrush> {
        let (peniko_brush, brush_transform) = self.brush(brush, self.size(size))?;
        Some(GlyphBrush {
            peniko_brush,
            brush_transform,
            style: vello::peniko::Style::Fill(vello::peniko::Fill::default()),
        })
    }

    fn platform_brush_for_color(
        &mut self,
        color: &i_slint_core::Color,
    ) -> Option<Self::PlatformBrush> {
        self.platform_text_fill_brush(Brush::SolidColor(color.clone()), LogicalSize::default())
    }

    fn platform_text_stroke_brush(
        &mut self,
        stroke_brush: Brush,
        physical_stroke_width: f32,
        size: LogicalSize,
    ) -> Option<Self::PlatformBrush> {
        let (peniko_brush, brush_transform) = self.brush(stroke_brush, self.size(size))?;

        Some(GlyphBrush {
            peniko_brush,
            brush_transform,
            style: vello::peniko::Style::Stroke(vello::kurbo::Stroke::new(
                physical_stroke_width as f64,
            )),
        })
    }

    fn draw_glyph_run(
        &mut self,
        font: &parley::FontData,
        font_size: PhysicalLength,
        brush: Self::PlatformBrush,
        y_offset: sharedparley::PhysicalLength,
        glyphs_it: &mut dyn Iterator<Item = parley::layout::Glyph>,
    ) {
        self.scene
            .draw_glyphs(font)
            .transform(
                self.current_state
                    .transform
                    .then_translate(peniko::kurbo::Vec2::new(0., y_offset.get() as f64)),
            )
            .font_size(font_size.get())
            .brush(&brush.peniko_brush)
            .draw(&brush.style, glyphs_it.map(|g| vello::Glyph { id: g.id, x: g.x, y: g.y }));
    }

    fn fill_rectangle(
        &mut self,
        physical_rect: sharedparley::PhysicalRect,
        brush: Self::PlatformBrush,
    ) {
        let shape = vello::kurbo::Rect::new(
            physical_rect.min_x() as f64,
            physical_rect.min_y() as f64,
            physical_rect.max_x() as f64,
            physical_rect.max_y() as f64,
        );

        self.scene.fill(
            vello::peniko::Fill::default(),
            self.current_state.transform,
            &brush.peniko_brush,
            brush.brush_transform,
            &shape,
        );
    }
}

impl<'a> VelloItemRenderer<'a> {
    fn rounded_rect(
        &self,
        rect: LogicalRect,
        radius: LogicalBorderRadius,
    ) -> vello::kurbo::RoundedRect {
        let phys_rect = rect * self.scale_factor;
        let phys_radius = radius * self.scale_factor;
        Self::phys_rounded_rect_impl(phys_rect, phys_radius)
    }

    fn phys_rounded_rect(
        &self,
        rect: PhysicalRect,
        radius: PhysicalBorderRadius,
    ) -> vello::kurbo::RoundedRect {
        Self::phys_rounded_rect_impl(rect, radius)
    }

    fn phys_rounded_rect_impl(
        rect: PhysicalRect,
        radius: PhysicalBorderRadius,
    ) -> vello::kurbo::RoundedRect {
        vello::kurbo::RoundedRect::new(
            rect.min_x() as f64,
            rect.min_y() as f64,
            rect.max_x() as f64,
            rect.max_y() as f64,
            vello::kurbo::RoundedRectRadii::new(
                radius.top_left as f64,
                radius.top_right as f64,
                radius.bottom_right as f64,
                radius.bottom_left as f64,
            ),
        )
    }

    fn rect(&self, rect: LogicalRect) -> vello::kurbo::Rect {
        let phys_rect = rect * self.scale_factor;
        vello::kurbo::Rect::new(
            phys_rect.min_x() as f64,
            phys_rect.min_y() as f64,
            phys_rect.max_x() as f64,
            phys_rect.max_y() as f64,
        )
    }

    fn size(&self, size: LogicalSize) -> vello::kurbo::Size {
        let phys_size = size * self.scale_factor;
        vello::kurbo::Size::new(phys_size.width as f64, phys_size.height as f64)
    }

    fn brush(
        &self,
        brush: Brush,
        shape_size: vello::kurbo::Size,
    ) -> Option<(vello::peniko::Brush, Option<vello::kurbo::Affine>)> {
        if brush.is_transparent() {
            return None;
        }

        fn convert_color_stops<'a>(
            stops: impl Iterator<Item = &'a i_slint_core::graphics::GradientStop>,
        ) -> vello::peniko::ColorStops {
            vello::peniko::ColorStops(
                stops
                    .map(|stop| vello::peniko::ColorStop {
                        offset: stop.position,
                        color: vello::peniko::color::DynamicColor::from_alpha_color(
                            to_peniko_color(stop.color),
                        ),
                    })
                    .collect(),
            )
        }

        Some(match brush {
            Brush::SolidColor(color) => {
                let color = color.to_argb_u8();
                (
                    vello::peniko::color::AlphaColor::<vello::peniko::color::Srgb>::from_rgba8(
                        color.red,
                        color.green,
                        color.blue,
                        color.alpha,
                    )
                    .into(),
                    None,
                )
            }
            Brush::LinearGradient(gradient) => {
                let (start, end) = i_slint_core::graphics::line_for_angle(
                    gradient.angle(),
                    [shape_size.width as f32, shape_size.height as f32].into(),
                );
                let start = to_kurbo_point(start);
                let end = to_kurbo_point(end);

                let mut vello_gradient = vello::peniko::Gradient::new_linear(start, end);
                vello_gradient.stops = convert_color_stops(gradient.stops());

                (vello_gradient.into(), None)
            }
            Brush::RadialGradient(gradient) => {
                let circle_scale = 0.5
                    * (shape_size.width * shape_size.width + shape_size.height * shape_size.height)
                        .sqrt();

                let mut vello_gradient =
                    vello::peniko::Gradient::new_radial(vello::kurbo::Point::new(0., 0.), 1.0);
                vello_gradient.stops = convert_color_stops(gradient.stops());

                (
                    vello_gradient.into(),
                    Some(vello::kurbo::Affine::scale(circle_scale).then_translate(
                        vello::kurbo::Vec2::new(shape_size.width / 2., shape_size.height / 2.),
                    )),
                )
            }
            Brush::ConicGradient(gradient) => {
                let center =
                    vello::kurbo::Point::new(shape_size.width / 2., shape_size.height / 2.);

                let mut vello_gradient = vello::peniko::Gradient::new_sweep(
                    center,
                    0f32.to_radians(),
                    360f32.to_radians(),
                );
                vello_gradient.stops = convert_color_stops(gradient.stops());

                (vello_gradient.into(), None)
            }
            _ => todo!(),
        })
    }
}

fn adjust_rect_and_border_for_inner_drawing(
    rect: &mut PhysicalRect,
    border_width: &mut PhysicalLength,
) {
    // If the border width exceeds the width, just fill the rectangle.
    *border_width = border_width.min(rect.width_length() / 2.);
    // Adjust the size so that the border is drawn within the geometry.
    rect.origin += PhysicalSize::from_lengths(*border_width / 2., *border_width / 2.);
    rect.size -= PhysicalSize::from_lengths(*border_width, *border_width);
}

fn to_kurbo_point(p: euclid::default::Point2D<f32>) -> vello::kurbo::Point {
    vello::kurbo::Point::new(p.x as f64, p.y as f64)
}

fn to_peniko_color(color: Color) -> vello::peniko::color::AlphaColor<vello::peniko::color::Srgb> {
    let color = color.to_argb_u8();
    vello::peniko::color::AlphaColor::<vello::peniko::color::Srgb>::from_rgba8(
        color.red,
        color.green,
        color.blue,
        color.alpha,
    )
}

struct VelloCachedImage {
    image_data: peniko::ImageData,
    size: IntSize,
    cache_key: ImageCacheKey,
}

i_slint_core::OpaqueImageVTable_static! {
    static VELLO_CACHED_IMAGE_VT for VelloCachedImage
}

impl OpaqueImage for VelloCachedImage {
    fn size(&self) -> IntSize {
        self.size
    }

    fn cache_key(&self) -> ImageCacheKey {
        self.cache_key.clone()
    }
}

fn load_image(
    image: Image,
    target_size_fn: &dyn Fn() -> LogicalSize,
    image_fit: ImageFit,
    scale_factor: ScaleFactor,
) -> Option<peniko::ImageData> {
    let image_inner: &ImageInner = (&image).into();
    match image_inner {
        ImageInner::None => None,
        ImageInner::EmbeddedImage { buffer, cache_key } => {
            let image_data = image_buffer_to_peniko_image(buffer)?;
            i_slint_core::graphics::cache::replace_cached_image(
                cache_key.clone(),
                ImageInner::BackendStorage(vtable::VRc::into_dyn(vtable::VRc::new(
                    VelloCachedImage {
                        image_data: image_data.clone(),
                        size: IntSize::new(image_data.width, image_data.height),
                        cache_key: cache_key.clone(),
                    },
                ))),
            );
            Some(image_data)
        }
        ImageInner::Svg(svg) => {
            // Query target_width/height here again to ensure that changes will invalidate the item rendering cache.
            let svg_size = svg.size();
            let fit = i_slint_core::graphics::fit(
                image_fit,
                target_size_fn() * scale_factor,
                IntRect::from_size(svg_size.cast()),
                scale_factor,
                Default::default(), // We only care about the size, so alignments don't matter
                Default::default(),
            );
            let target_size = PhysicalSize::new(
                svg_size.cast::<f32>().width * fit.source_to_target_x,
                svg_size.cast::<f32>().height * fit.source_to_target_y,
            );
            let pixels = match svg.render(Some(target_size.cast())).ok()? {
                SharedImageBuffer::RGB8(_) => unreachable!(),
                SharedImageBuffer::RGBA8(_) => unreachable!(),
                SharedImageBuffer::RGBA8Premultiplied(pixels) => pixels,
            };

            let width = pixels.width();
            let height = pixels.height();

            let data = peniko::Blob::new(Arc::new(PixelBufferWrap(pixels)));

            let image_data = peniko::ImageData {
                data,
                format: peniko::ImageFormat::Rgba8,
                alpha_type: peniko::ImageAlphaType::AlphaPremultiplied,
                width,
                height,
            };

            Some(image_data)
        }
        ImageInner::StaticTextures(_) => {
            let buffer = image_inner.render_to_buffer(None)?;
            image_buffer_to_peniko_image(&buffer)
        }
        ImageInner::BackendStorage(x) => {
            vtable::VRc::borrow(x).downcast::<VelloCachedImage>().map(|x| x.image_data.clone())
        }
        ImageInner::BorrowedOpenGLTexture(texture) => {
            return None;
        }
        ImageInner::NineSlice(n) => {
            load_image(n.image(), target_size_fn, ImageFit::Preserve, scale_factor)
        }
        ImageInner::WGPUTexture(any_wgpu_texture) => {
            //surface.and_then(|surface| surface.import_wgpu_texture(canvas, any_wgpu_texture))
            todo!()
        }
    }
}

fn image_buffer_to_peniko_image(buffer: &SharedImageBuffer) -> Option<peniko::ImageData> {
    let (data, format, alpha_type) = match buffer {
        SharedImageBuffer::RGB8(shared_pixel_buffer) => {
            let rgba: Vec<u8> = shared_pixel_buffer
                .as_bytes()
                .chunks_exact(3)
                .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
                .collect();
            let width = shared_pixel_buffer.width();
            let height = shared_pixel_buffer.height();
            return Some(peniko::ImageData {
                data: peniko::Blob::new(Arc::new(rgba)),
                format: peniko::ImageFormat::Rgba8,
                alpha_type: peniko::ImageAlphaType::Alpha,
                width,
                height,
            });
        }
        SharedImageBuffer::RGBA8(shared_pixel_buffer) => (
            Arc::new(PixelBufferWrap(shared_pixel_buffer.clone())),
            peniko::ImageFormat::Rgba8,
            peniko::ImageAlphaType::Alpha,
        ),
        SharedImageBuffer::RGBA8Premultiplied(shared_pixel_buffer) => (
            Arc::new(PixelBufferWrap(shared_pixel_buffer.clone())),
            peniko::ImageFormat::Rgba8,
            peniko::ImageAlphaType::AlphaPremultiplied,
        ),
    };

    Some(peniko::ImageData {
        data: peniko::Blob::new(data),
        format,
        alpha_type,
        width: buffer.width(),
        height: buffer.height(),
    })
}

pub struct PixelBufferWrap<Pixel>(SharedPixelBuffer<Pixel>);
impl<Pixel: Clone + rgb::Pod> AsRef<[u8]> for PixelBufferWrap<Pixel>
where
    [Pixel]: rgb::ComponentBytes<u8>,
{
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}
