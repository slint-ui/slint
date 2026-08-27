// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::pin::Pin;
use std::sync::Arc;

use anyrender::PaintScene;
use i_slint_core::graphics::ResolvedBrush;
use i_slint_core::graphics::euclid;
use i_slint_core::graphics::{Image, ImageCacheKey, SharedImageBuffer, SharedPixelBuffer};
use i_slint_core::item_rendering::{
    BorderRectLayout, CachedRenderingData, ItemCache, ItemRenderer, RenderBorderRectangle,
    RenderImage, RenderRectangle, RenderText,
};
use i_slint_core::items::{self, FillRule, ImageFit, ImageRendering, ItemRc};
use i_slint_core::lengths::{
    LogicalBorderRadius, LogicalPoint, LogicalRect, LogicalSize, LogicalVector,
    PhysicalBorderRadius, ScaleFactor, logical_size_from_api,
};
use i_slint_core::textlayout::sharedparley::{self, GlyphRenderer, fontique, parley};
use i_slint_core::{Brush, Color, ImageInner, SharedString};

use super::{PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalSize};

/// anyrender's `push_layer` always clips; there is no "no clip", so layers
/// that should not clip use a rectangle larger than any real scene.
///
/// Only safe for non-destructive compose modes: vello_cpu <= 0.0.9 mishandles
/// layers with destructive compose modes (`SrcIn`, `DestOut`) whose bounds
/// greatly exceed the viewport, losing everything beyond the first 256px
/// wide-tile column. Bound such layers to the area they affect instead.
/// That was fixed upstream by the frontend rewrite (linebender/vello#1701),
/// but bounding destructive layers stays worthwhile on fixed versions too:
/// it spares vello_cpu from compositing the entire surface.
const UNCLIPPED: kurbo::Rect = kurbo::Rect::new(0., 0., 1e9, 1e9);

#[derive(Clone, Copy)]
struct RenderState {
    clip_rect: LogicalRect,
    transform: kurbo::Affine,
    layer_count: usize,
    alpha: f32,
}

pub struct AnyrenderItemRenderer<'a, S: PaintScene> {
    window: &'a i_slint_core::api::Window,
    scale_factor: ScaleFactor,
    scene: &'a mut S,
    image_cache: &'a std::cell::RefCell<crate::ImageConversionCache>,
    item_image_cache: &'a ItemCache<Option<crate::SharedImageData>>,
    text_layout_cache: &'a sharedparley::TextLayoutCache,
    state_stack: Vec<RenderState>,
    current_state: RenderState,
}

impl<'a, S: PaintScene> AnyrenderItemRenderer<'a, S> {
    pub fn new(
        scene: &'a mut S,
        width: u32,
        height: u32,
        window: &'a i_slint_core::api::Window,
        image_cache: &'a std::cell::RefCell<crate::ImageConversionCache>,
        item_image_cache: &'a ItemCache<Option<crate::SharedImageData>>,
        text_layout_cache: &'a sharedparley::TextLayoutCache,
    ) -> Self {
        Self::new_with_initial_transform(
            scene,
            width,
            height,
            window,
            image_cache,
            item_image_cache,
            text_layout_cache,
            kurbo::Affine::IDENTITY,
        )
    }

    /// Like [`new`](Self::new) but starts with a non-identity transform,
    /// used by linuxkms to apply a screen rotation that all subsequent
    /// items inherit.
    pub fn new_with_initial_transform(
        scene: &'a mut S,
        width: u32,
        height: u32,
        window: &'a i_slint_core::api::Window,
        image_cache: &'a std::cell::RefCell<crate::ImageConversionCache>,
        item_image_cache: &'a ItemCache<Option<crate::SharedImageData>>,
        text_layout_cache: &'a sharedparley::TextLayoutCache,
        initial_transform: kurbo::Affine,
    ) -> Self {
        let scale_factor = ScaleFactor::new(window.scale_factor());
        Self {
            window,
            scale_factor,
            scene,
            image_cache,
            item_image_cache,
            text_layout_cache,
            state_stack: vec![],
            current_state: RenderState {
                clip_rect: LogicalRect::from_size(
                    PhysicalSize::new(width as f32, height as f32) / scale_factor,
                ),
                transform: initial_transform,
                layer_count: 0,
                alpha: 1.,
            },
        }
    }
}

impl<'a, S: PaintScene> ItemRenderer for AnyrenderItemRenderer<'a, S> {
    fn global_alpha_transparent(&self) -> bool {
        self.current_state.alpha == 0.0
    }

    fn draw_rectangle(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        _: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        if size.width <= 0. || size.height <= 0. {
            return;
        }
        let shape = self.rect(LogicalRect::from_size(size));
        self.fill_with_brush(
            rect.background(),
            size * self.scale_factor,
            self.current_state.transform,
            peniko::Fill::default(),
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
        let Some(layout) = BorderRectLayout::new(rect, size, self.scale_factor) else {
            return;
        };

        let transform = self.current_state.transform;
        self.fill_with_brush(
            rect.background(),
            layout.brush_size,
            transform,
            peniko::Fill::default(),
            &phys_rect_shape(layout.background_rect, layout.background_radius),
        );

        if layout.border_width.get() > 0.0 {
            // Miter joins, not kurbo's default round ones: a round join doesn't
            // reach into sharp corners, leaving the corner tips of the border
            // uncovered.
            let stroke =
                kurbo::Stroke::new(layout.border_width.get() as f64).with_join(kurbo::Join::Miter);
            self.stroke_with_brush(
                layout.border_color,
                layout.brush_size,
                transform,
                &stroke,
                &phys_rect_shape(layout.border_rect, layout.border_radius),
            );
        }
    }

    fn draw_window_background(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        _self_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        // Solid color backgrounds are handled as the base_color in
        // AnyrenderSlintRenderer::render(). Only draw here for gradient backgrounds.
        let background = rect.background();
        if matches!(background, Brush::SolidColor(..)) {
            return;
        }
        let shape = self.rect(LogicalRect::from_size(size));
        self.fill_with_brush(
            background,
            size * self.scale_factor,
            self.current_state.transform,
            peniko::Fill::default(),
            &shape,
        );
    }

    fn draw_image(
        &mut self,
        image: Pin<&dyn RenderImage>,
        item_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        if size.width <= 0. || size.height <= 0. {
            return;
        }

        let tiling = image.tiling();
        // A tiled image repeats at its natural size, so its fit is fixed.
        let resolve_image_fit =
            || if tiling != Default::default() { ImageFit::Preserve } else { image.image_fit() };

        // The per-item cache tracks the properties read in the closure
        // (source, image-fit, target size) and invalidates on change; the
        // shared conversion cache deduplicates across items. The fit has to
        // be resolved inside the closure to be tracked: SVGs rasterize at
        // the fitted size.
        let image_data = self.item_image_cache.get_or_update_cache_entry(item_rc, || {
            load_image(
                image.source(),
                &|| image.target_size(),
                resolve_image_fit(),
                self.scale_factor,
                self.image_cache,
            )
        });
        let Some(image_data) = image_data else {
            return;
        };
        let image_fit = resolve_image_fit();
        let source = image.source();

        let source_size = source.size();
        if source_size.is_empty() {
            return;
        }

        let dest_size = size * self.scale_factor;

        let image_inner: &ImageInner = (&source).into();
        let mut nine_slice_fits;
        let mut single_fit;
        let fits: &mut dyn Iterator<Item = i_slint_core::graphics::FitResult> =
            if let ImageInner::NineSlice(nine) = image_inner {
                nine_slice_fits = i_slint_core::graphics::fit9slice(
                    source_size.cast(),
                    nine.1,
                    dest_size,
                    self.scale_factor,
                    image.alignment(),
                    tiling,
                );
                &mut nine_slice_fits
            } else {
                single_fit = std::iter::once(i_slint_core::graphics::fit(
                    image_fit,
                    dest_size,
                    image
                        .source_clip()
                        .unwrap_or_else(|| euclid::Rect::from_size(source_size.cast())),
                    self.scale_factor,
                    image.alignment(),
                    tiling,
                ));
                &mut single_fit
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
        let dest_rect = to_kurbo_size(dest_size).to_rect();
        if has_colorize {
            // Isolate the image in a compositing group so SrcIn only affects the image.
            // The layers are bounded to the image's rect: the image only draws within
            // it, and destructive compose modes must not use an unbounded layer (see
            // the doc comment on the `UNCLIPPED` constant at the top of this file).
            self.scene.push_layer(
                peniko::BlendMode::default(),
                1.0,
                self.current_state.transform,
                &dest_rect,
                None,
                None,
            );
        }

        for fit in fits {
            // Clip rect coordinates in image data space
            let clip_x = fit.clip_rect.origin.x as f64 * ratio_x;
            let clip_y = fit.clip_rect.origin.y as f64 * ratio_y;
            let clip_w = fit.clip_rect.size.width as f64 * ratio_x;
            let clip_h = fit.clip_rect.size.height as f64 * ratio_y;

            let (image_brush, brush_transform) = if let Some(tiled_offset) = fit.tiled {
                // Extend::Repeat wraps the entire brush image, but the tile
                // is only the clip_rect portion of the source, so crop it
                // out (like the skia renderer's make_subset).
                let Some((crop_x, crop_y, crop_w, crop_h)) =
                    crop_rect(&image_data, clip_x, clip_y, clip_w, clip_h)
                else {
                    continue;
                };
                let tile = if (crop_x, crop_y, crop_w, crop_h)
                    == (0, 0, image_data.width, image_data.height)
                {
                    Some(image_data.clone())
                } else {
                    self.image_cache.borrow_mut().get_or_insert(
                        ImageCacheKey::new(image_inner),
                        crate::imagecache::ImageVariant::Tile {
                            source_width: image_data.width,
                            source_height: image_data.height,
                            x: crop_x,
                            y: crop_y,
                            width: crop_w,
                            height: crop_h,
                        },
                        || Some(crop_image_data(&image_data, crop_x, crop_y, crop_w, crop_h)),
                    )
                };
                let Some(tile) = tile else {
                    continue;
                };
                let image_brush = peniko::ImageBrush::new((*tile).clone())
                    .with_quality(quality)
                    .with_extend(peniko::Extend::Repeat);

                // Scale from image data pixels to target pixels
                let scale_x = fit.source_to_target_x as f64 / ratio_x;
                let scale_y = fit.source_to_target_y as f64 / ratio_y;

                let brush_transform = kurbo::Affine::translate((
                    -(tiled_offset.x as f64 * ratio_x),
                    -(tiled_offset.y as f64 * ratio_y),
                ))
                .then_scale_non_uniform(scale_x, scale_y);
                (image_brush, brush_transform)
            } else {
                let image_brush =
                    peniko::ImageBrush::new((*image_data).clone()).with_quality(quality);
                let brush_transform = kurbo::Affine::translate((-clip_x, -clip_y))
                    .then_scale_non_uniform(
                        fit.size.width as f64 / clip_w,
                        fit.size.height as f64 / clip_h,
                    );
                (image_brush, brush_transform)
            };

            let shape = kurbo::Rect::new(0., 0., fit.size.width as f64, fit.size.height as f64);

            let mut transform = self
                .current_state
                .transform
                .then_translate(kurbo::Vec2::new(fit.offset.x as f64, fit.offset.y as f64));

            // With bilinear sampling, a fractional tile phase blends
            // adjacent texels across every tile seam and washes out the
            // pattern. The skia renderer rounds its tile shader matrix to
            // integer translations, too.
            if fit.tiled.is_some() {
                transform = snap_translation_to_pixel_grid(transform);
            }

            self.scene.fill(
                peniko::Fill::default(),
                transform,
                peniko::BrushRef::Image(image_brush.as_ref()),
                Some(brush_transform),
                &shape,
            );
        }

        if has_colorize {
            // Apply colorize: push a SrcIn layer and fill with the colorize brush.
            // SrcIn keeps the image's alpha but replaces the color.
            let src_in_blend = peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::SrcIn);
            if let Some((brush, brush_transform)) = self.brush(colorize_brush, dest_size) {
                self.scene.push_layer(
                    src_in_blend,
                    1.0,
                    self.current_state.transform,
                    &dest_rect,
                    None,
                    None,
                );
                self.scene.fill(
                    peniko::Fill::default(),
                    self.current_state.transform,
                    peniko::BrushRef::from(&brush),
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
        sharedparley::draw_text(self, text, Some(self_rc), size, Some(self.text_layout_cache));
    }

    fn draw_text_input(
        &mut self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        self_rc: &i_slint_core::items::ItemRc,
        size: LogicalSize,
    ) {
        sharedparley::draw_text_input(self, text_input, self_rc, size, self.text_layout_cache);
    }

    fn draw_path(&mut self, path: Pin<&items::Path>, item_rc: &ItemRc, size: LogicalSize) {
        let Some((offset, path_events)) = path.fitted_path_events(item_rc) else {
            return;
        };

        let sf = self.scale_factor;

        let mut bezpath = kurbo::BezPath::new();
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
            .then_translate(kurbo::Vec2::new(phys_offset.x as f64, phys_offset.y as f64));

        let brush_size = size * sf;

        let fill_rule = match path.fill_rule() {
            FillRule::Evenodd => peniko::Fill::EvenOdd,
            _ => peniko::Fill::NonZero,
        };
        self.fill_with_brush(path.fill(), brush_size, transform, fill_rule, &bezpath);

        let stroke_brush = path.stroke();
        if !stroke_brush.is_transparent() {
            let stroke_width = (path.stroke_width() * sf).get() as f64;
            let cap = match path.stroke_line_cap() {
                items::LineCap::Round => kurbo::Cap::Round,
                items::LineCap::Square => kurbo::Cap::Square,
                _ => kurbo::Cap::Butt,
            };
            let join = match path.stroke_line_join() {
                items::LineJoin::Round => kurbo::Join::Round,
                items::LineJoin::Bevel => kurbo::Join::Bevel,
                _ => kurbo::Join::Miter,
            };
            let stroke = kurbo::Stroke::new(stroke_width).with_caps(cap).with_join(join);
            self.stroke_with_brush(stroke_brush, brush_size, transform, &stroke, &bezpath);
        }
    }

    fn draw_box_shadow(
        &mut self,
        box_shadow: Pin<&items::BoxShadow>,
        _item_rc: &ItemRc,
        size: LogicalSize,
    ) {
        let color = box_shadow.color();
        if color.alpha() == 0 {
            return;
        }

        let sf = self.scale_factor;
        let offset = LogicalPoint::from_lengths(box_shadow.offset_x(), box_shadow.offset_y()) * sf;
        let spread = (box_shadow.spread() * sf).get() as f64;
        let blur = (box_shadow.blur() * sf).get().max(0.) as f64;
        let phys_size = size * sf;

        // anyrender's box shadow takes one uniform corner radius,
        // so approximate per-corner radii with their average
        // until vello grows support for non-uniform ones (linebender/vello#1245).
        let radius = box_shadow.logical_border_radius() * sf;
        let base_radius =
            (radius.top_left + radius.top_right + radius.bottom_right + radius.bottom_left) as f64
                / 4.;

        if box_shadow.inset() {
            self.draw_inset_shadow(
                color,
                kurbo::Vec2::new(offset.x as f64, offset.y as f64),
                spread,
                blur,
                base_radius,
                to_kurbo_size(phys_size),
            );
            return;
        }

        let radius = base_radius + spread;

        let rect = kurbo::Rect::new(
            offset.x as f64 - spread,
            offset.y as f64 - spread,
            offset.x as f64 + phys_size.width as f64 + spread,
            offset.y as f64 + phys_size.height as f64 + spread,
        );
        if rect.is_zero_area() {
            return;
        }

        if blur == 0. {
            // No blur: a plain rounded rectangle fill matches exactly.
            let shape = RectShape::uniform(rect, radius);
            self.scene.fill(
                peniko::Fill::default(),
                self.current_state.transform,
                peniko::BrushRef::Solid(to_peniko_color(color)),
                None,
                &shape,
            );
        } else {
            // The CSS drop-shadow convention Slint follows: the Gaussian's
            // standard deviation is half the blur radius.
            self.scene.draw_box_shadow(
                self.current_state.transform,
                rect,
                to_peniko_color(color),
                radius,
                blur / 2.,
            );
        }
    }

    fn combine_clip(&mut self, clip_rect: LogicalRect, radius: LogicalBorderRadius) -> bool {
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

        let clip_shape = phys_rect_shape(clip_rect * self.scale_factor, radius * self.scale_factor);

        self.scene.push_clip_layer(self.current_state.transform, &clip_shape);
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

    fn scale_factor(&self) -> ScaleFactor {
        self.scale_factor
    }

    fn draw_cached_pixmap(
        &mut self,
        item_rc: &ItemRc,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        let image_data = self.item_image_cache.get_or_update_cache_entry(item_rc, || {
            let mut image_data = None;
            update_fn(&mut |width, height, data| {
                image_data = Some(std::rc::Rc::new(peniko::ImageData {
                    data: peniko::Blob::new(Arc::new(data.to_vec())),
                    format: peniko::ImageFormat::Rgba8,
                    alpha_type: peniko::ImageAlphaType::AlphaPremultiplied,
                    width,
                    height,
                }));
            });
            image_data
        });
        let Some(image_data) = image_data else { return };
        let image_brush = peniko::ImageBrush::new((*image_data).clone());
        self.scene.fill(
            peniko::Fill::default(),
            self.current_state.transform,
            peniko::BrushRef::Image(image_brush.as_ref()),
            None,
            &kurbo::Rect::new(0., 0., image_data.width as f64, image_data.height as f64),
        );
    }

    fn draw_string(&mut self, string: &str, color: Color) {
        sharedparley::draw_text(
            self,
            std::pin::pin!((SharedString::from(string), Brush::from(color))),
            None,
            logical_size_from_api(self.window.size().to_logical(self.scale_factor().get())),
            None,
        );
    }

    fn draw_image_direct(&mut self, image: i_slint_core::graphics::Image) {
        let Some(image_data) = load_image(
            image.clone(),
            &|| LogicalSize::from_untyped(image.size().cast()),
            ImageFit::Fill,
            self.scale_factor,
            self.image_cache,
        ) else {
            return;
        };

        let shape = kurbo::Rect::new(0., 0., image_data.width as f64, image_data.height as f64);

        let image_brush = peniko::ImageBrush::new((*image_data).clone());
        self.scene.fill(
            peniko::Fill::default(),
            self.current_state.transform,
            peniko::BrushRef::Image(image_brush.as_ref()),
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
        self.current_state.clip_rect = self.current_state.clip_rect.translate(-distance);
        let distance = distance * self.scale_factor;
        self.current_state.transform = self
            .current_state
            .transform
            .then_translate(kurbo::Vec2::new(distance.x as f64, distance.y as f64));
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
        if opacity < 1.0 {
            // The layer is popped again by restore_state().
            self.push_unclipped_layer(peniko::BlendMode::default(), opacity);
            self.current_state.layer_count += 1;
        }
    }
}

#[derive(Clone)]
pub struct GlyphBrush {
    peniko_brush: peniko::Brush,
    brush_transform: Option<kurbo::Affine>,
    style: peniko::Style,
}

impl<'a, S: PaintScene> GlyphRenderer for AnyrenderItemRenderer<'a, S> {
    type PlatformBrush = GlyphBrush;

    fn platform_text_fill_brush(
        &mut self,
        brush: Brush,
        size: LogicalSize,
    ) -> Option<Self::PlatformBrush> {
        let (peniko_brush, brush_transform) = self.brush(brush, size * self.scale_factor)?;
        Some(GlyphBrush {
            peniko_brush,
            brush_transform,
            style: peniko::Style::Fill(peniko::Fill::default()),
        })
    }

    fn platform_brush_for_color(
        &mut self,
        color: &i_slint_core::Color,
    ) -> Option<Self::PlatformBrush> {
        self.platform_text_fill_brush(Brush::SolidColor(*color), LogicalSize::default())
    }

    fn platform_text_stroke_brush(
        &mut self,
        stroke_brush: Brush,
        physical_stroke_width: f32,
        size: LogicalSize,
    ) -> Option<Self::PlatformBrush> {
        let (peniko_brush, brush_transform) = self.brush(stroke_brush, size * self.scale_factor)?;

        Some(GlyphBrush {
            peniko_brush,
            brush_transform,
            style: peniko::Style::Stroke(kurbo::Stroke::new(physical_stroke_width as f64)),
        })
    }

    fn draw_glyph_run(
        &mut self,
        font: &parley::FontData,
        font_size: PhysicalLength,
        normalized_coords: &[i16],
        _synthesis: &fontique::Synthesis,
        brush: Self::PlatformBrush,
        y_offset: sharedparley::PhysicalLength,
        glyphs_it: &mut dyn Iterator<Item = parley::layout::Glyph>,
    ) {
        let transform = self
            .current_state
            .transform
            .then_translate(kurbo::Vec2::new(0., y_offset.get() as f64));
        let glyphs: Vec<_> =
            glyphs_it.map(|g| anyrender::Glyph { id: g.id, x: g.x, y: g.y }).collect();
        self.scene.draw_glyphs(
            font,
            font_size.get(),
            false,
            normalized_coords,
            kurbo::Vec2::ZERO,
            peniko::StyleRef::from(&brush.style),
            peniko::BrushRef::from(&brush.peniko_brush),
            1.0,
            transform,
            None,
            glyphs.into_iter(),
        );
    }

    fn fill_rectangle(
        &mut self,
        physical_rect: sharedparley::PhysicalRect,
        brush: Self::PlatformBrush,
        radius: sharedparley::PhysicalLength,
        border: Option<sharedparley::RectangleBorder<Self::PlatformBrush>>,
    ) {
        let shape = RectShape::uniform(to_kurbo_rect(physical_rect), radius.get() as f64);
        self.fill_and_stroke(&shape, brush, border);
    }
}

impl<'a, S: PaintScene> AnyrenderItemRenderer<'a, S> {
    /// Draw an inset shadow.
    ///
    /// There's no primitive for a blurred rounded rectangle that's transparent
    /// inside and opaque outside (linebender/vello#1374).
    /// So fill the border box with the shadow color,
    /// then punch the blurred interior back out.
    fn draw_inset_shadow(
        &mut self,
        color: Color,
        offset: kurbo::Vec2,
        spread: f64,
        blur: f64,
        base_radius: f64,
        size: kurbo::Size,
    ) {
        let border_rect = kurbo::Rect::new(0., 0., size.width, size.height);
        if border_rect.is_zero_area() {
            return;
        }
        let border_shape = RectShape::uniform(border_rect, base_radius);

        // The shadow must not paint outside the item.
        self.scene.push_clip_layer(self.current_state.transform, &border_shape);
        self.scene.fill(
            peniko::Fill::default(),
            self.current_state.transform,
            peniko::BrushRef::Solid(to_peniko_color(color)),
            None,
            &border_rect,
        );

        let interior = kurbo::Rect::new(
            offset.x + spread,
            offset.y + spread,
            offset.x + size.width - spread,
            offset.y + size.height - spread,
        );
        if interior.width() > 0. && interior.height() > 0. {
            let interior_radius = (base_radius - spread).max(0.);
            // Bounded to the border box, which the outer clip enforces anyway:
            // destructive compose modes mustn't use an unbounded layer,
            // see the `UNCLIPPED` constant at the top of this file.
            self.scene.push_layer(
                peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::DestOut),
                1.0,
                self.current_state.transform,
                &border_rect,
                None,
                None,
            );
            // Punch out at full strength so the interior is completely
            // clear of shadow, regardless of the shadow color's alpha.
            let opaque = peniko::color::palette::css::BLACK;
            if blur == 0. {
                self.scene.fill(
                    peniko::Fill::default(),
                    self.current_state.transform,
                    peniko::BrushRef::Solid(opaque),
                    None,
                    &RectShape::uniform(interior, interior_radius),
                );
            } else {
                self.scene.draw_box_shadow(
                    self.current_state.transform,
                    interior,
                    opaque,
                    interior_radius,
                    blur / 2.,
                );
            }
            self.scene.pop_layer();
        }

        self.scene.pop_layer();
    }

    /// Push a compositing layer that does not clip its content.
    fn push_unclipped_layer(&mut self, blend: peniko::BlendMode, alpha: f32) {
        self.scene.push_layer(blend, alpha, kurbo::Affine::IDENTITY, &UNCLIPPED, None, None);
    }

    /// Resolve the Slint `brush` (sized against `brush_size`) and fill
    /// `shape` with it. Transparent brushes draw nothing.
    fn fill_with_brush(
        &mut self,
        brush: Brush,
        brush_size: PhysicalSize,
        transform: kurbo::Affine,
        style: peniko::Fill,
        shape: &impl kurbo::Shape,
    ) {
        if let Some((brush, brush_transform)) = self.brush(brush, brush_size) {
            self.scene.fill(
                style,
                transform,
                peniko::BrushRef::from(&brush),
                brush_transform,
                shape,
            );
        }
    }

    /// Resolve the Slint `brush` (sized against `brush_size`) and stroke
    /// `shape` with it. Transparent brushes draw nothing.
    fn stroke_with_brush(
        &mut self,
        brush: Brush,
        brush_size: PhysicalSize,
        transform: kurbo::Affine,
        stroke: &kurbo::Stroke,
        shape: &impl kurbo::Shape,
    ) {
        if let Some((brush, brush_transform)) = self.brush(brush, brush_size) {
            self.scene.stroke(
                stroke,
                transform,
                peniko::BrushRef::from(&brush),
                brush_transform,
                shape,
            );
        }
    }

    /// Fill `shape` with `brush`, then stroke the same shape with `border`
    /// if one is given.
    fn fill_and_stroke(
        &mut self,
        shape: &impl kurbo::Shape,
        brush: GlyphBrush,
        border: Option<sharedparley::RectangleBorder<GlyphBrush>>,
    ) {
        self.scene.fill(
            peniko::Fill::default(),
            self.current_state.transform,
            peniko::BrushRef::from(&brush.peniko_brush),
            brush.brush_transform,
            shape,
        );

        if let Some(border) = border
            && border.width.get() > 0.
        {
            self.scene.stroke(
                &kurbo::Stroke::new(border.width.get() as f64),
                self.current_state.transform,
                peniko::BrushRef::from(&border.brush.peniko_brush),
                border.brush.brush_transform,
                shape,
            );
        }
    }

    fn rect(&self, rect: LogicalRect) -> kurbo::Rect {
        to_kurbo_rect(rect * self.scale_factor)
    }

    fn brush(
        &self,
        brush: Brush,
        shape_size: PhysicalSize,
    ) -> Option<(peniko::Brush, Option<kurbo::Affine>)> {
        let resolved =
            i_slint_core::graphics::resolve_brush(&brush, shape_size, self.scale_factor)?;

        Some(match resolved {
            ResolvedBrush::SolidColor(color) => (to_peniko_color(color).into(), None),
            ResolvedBrush::LinearGradient(gradient) => {
                let mut peniko_gradient = peniko::Gradient::new_linear(
                    to_kurbo_point(gradient.start),
                    to_kurbo_point(gradient.end),
                );
                peniko_gradient.stops = to_peniko_stops(&gradient.stops);

                (peniko_gradient.into(), None)
            }
            ResolvedBrush::RadialGradient(gradient) => {
                let mut peniko_gradient =
                    peniko::Gradient::new_radial(kurbo::Point::new(0., 0.), 1.0);
                peniko_gradient.stops = to_peniko_stops(&gradient.stops);

                // A unit circle at the origin, scaled to the radius and moved
                // to the center, so the color stops span [0, radius].
                (
                    peniko_gradient.into(),
                    Some(kurbo::Affine::scale(gradient.radius.get() as f64).then_translate(
                        kurbo::Vec2::new(gradient.center.x as f64, gradient.center.y as f64),
                    )),
                )
            }
            ResolvedBrush::ConicGradient(gradient) => {
                let center = kurbo::Point::new(gradient.center.x as f64, gradient.center.y as f64);

                let mut peniko_gradient =
                    peniko::Gradient::new_sweep(center, 0., 360f32.to_radians());
                peniko_gradient.stops = to_peniko_stops(&gradient.stops);

                // Sweep gradients start at 3 o'clock (east); Slint's 0° is at
                // 12 o'clock, so rotate the brush by -90° around the center.
                (
                    peniko_gradient.into(),
                    Some(kurbo::Affine::rotate_about(-std::f64::consts::FRAC_PI_2, center)),
                )
            }
        })
    }
}

fn to_peniko_stops(stops: &[i_slint_core::graphics::GradientStop]) -> peniko::ColorStops {
    peniko::ColorStops(
        stops
            .iter()
            .map(|stop| peniko::ColorStop {
                offset: stop.position,
                color: peniko::color::DynamicColor::from_alpha_color(to_peniko_color(stop.color)),
            })
            .collect(),
    )
}

fn to_kurbo_point(p: PhysicalPoint) -> kurbo::Point {
    (p.x, p.y).into()
}

fn to_kurbo_rect(rect: PhysicalRect) -> kurbo::Rect {
    kurbo::Rect::new(
        rect.min_x() as f64,
        rect.min_y() as f64,
        rect.max_x() as f64,
        rect.max_y() as f64,
    )
}

fn to_kurbo_size(size: PhysicalSize) -> kurbo::Size {
    kurbo::Size::new(size.width as f64, size.height as f64)
}

fn phys_rect_shape(rect: PhysicalRect, radius: PhysicalBorderRadius) -> RectShape {
    let rect = to_kurbo_rect(rect);
    if radius.is_zero() {
        return RectShape::Sharp(rect);
    }
    RectShape::Rounded(kurbo::RoundedRect::from_rect(
        rect,
        kurbo::RoundedRectRadii::new(
            radius.top_left as f64,
            radius.top_right as f64,
            radius.bottom_right as f64,
            radius.bottom_left as f64,
        ),
    ))
}

/// A rectangle that may have rounded corners, staying a plain
/// [`kurbo::Rect`] when none of them do.
///
/// [`kurbo::RoundedRect`] does not collapse zero radii: it emits one
/// degenerate cubic per corner whatever the radii are, which backends then
/// encode and flatten. Square corners are the common case in a user
/// interface, and keeping those a `Rect` also keeps
/// [`kurbo::Shape::as_rect`] answering, so a backend with a fast path for
/// axis-aligned rectangles can still recognize one.
#[derive(Clone, Copy)]
enum RectShape {
    Sharp(kurbo::Rect),
    Rounded(kurbo::RoundedRect),
}

impl RectShape {
    /// Like [`phys_rect_shape`] but for a single radius shared by all four
    /// corners, already in device pixels.
    fn uniform(rect: kurbo::Rect, radius: f64) -> Self {
        if radius > 0. {
            Self::Rounded(kurbo::RoundedRect::from_rect(rect, radius))
        } else {
            Self::Sharp(rect)
        }
    }
}

// The rounded variant is the bigger one by far, but it is what kurbo itself
// hands out for a rounded rectangle, and the iterator is a short-lived local.
// Boxing it would move an allocation into every fill.
#[allow(clippy::large_enum_variant)]
enum RectShapePathIter {
    Sharp(kurbo::RectPathIter),
    Rounded(kurbo::RoundedRectPathIter),
}

impl Iterator for RectShapePathIter {
    type Item = kurbo::PathEl;

    fn next(&mut self) -> Option<kurbo::PathEl> {
        match self {
            Self::Sharp(iter) => iter.next(),
            Self::Rounded(iter) => iter.next(),
        }
    }
}

/// Delegates to whichever variant is held, so that a `RectShape` behaves
/// exactly like the `kurbo` shape inside it.
impl kurbo::Shape for RectShape {
    type PathElementsIter<'iter> = RectShapePathIter;

    fn path_elements(&self, tolerance: f64) -> RectShapePathIter {
        match self {
            Self::Sharp(rect) => RectShapePathIter::Sharp(rect.path_elements(tolerance)),
            Self::Rounded(rect) => RectShapePathIter::Rounded(rect.path_elements(tolerance)),
        }
    }

    fn area(&self) -> f64 {
        match self {
            Self::Sharp(rect) => rect.area(),
            Self::Rounded(rect) => rect.area(),
        }
    }

    fn perimeter(&self, accuracy: f64) -> f64 {
        match self {
            Self::Sharp(rect) => rect.perimeter(accuracy),
            Self::Rounded(rect) => rect.perimeter(accuracy),
        }
    }

    fn winding(&self, pt: kurbo::Point) -> i32 {
        match self {
            Self::Sharp(rect) => rect.winding(pt),
            Self::Rounded(rect) => rect.winding(pt),
        }
    }

    fn bounding_box(&self) -> kurbo::Rect {
        match self {
            Self::Sharp(rect) => rect.bounding_box(),
            Self::Rounded(rect) => rect.bounding_box(),
        }
    }

    fn as_rect(&self) -> Option<kurbo::Rect> {
        match self {
            Self::Sharp(rect) => rect.as_rect(),
            Self::Rounded(rect) => rect.as_rect(),
        }
    }

    fn as_rounded_rect(&self) -> Option<kurbo::RoundedRect> {
        match self {
            Self::Sharp(rect) => rect.as_rounded_rect(),
            Self::Rounded(rect) => rect.as_rounded_rect(),
        }
    }
}

/// Snap the translation of `transform` to whole device pixels, if it is a
/// pure translation (no scale, rotation, or skew). Returns the transform
/// unchanged otherwise.
fn snap_translation_to_pixel_grid(transform: kurbo::Affine) -> kurbo::Affine {
    let [a, b, c, d, e, f] = transform.as_coeffs();
    if a == 1. && b == 0. && c == 0. && d == 1. {
        kurbo::Affine::new([a, b, c, d, e.round(), f.round()])
    } else {
        transform
    }
}

pub(crate) fn to_peniko_color(color: Color) -> peniko::Color {
    let color = color.to_argb_u8();
    peniko::Color::from_rgba8(color.red, color.green, color.blue, color.alpha)
}

fn load_image(
    image: Image,
    target_size_fn: &dyn Fn() -> LogicalSize,
    image_fit: ImageFit,
    scale_factor: ScaleFactor,
    image_cache: &std::cell::RefCell<crate::ImageConversionCache>,
) -> Option<crate::SharedImageData> {
    use crate::imagecache::ImageVariant;

    let image_inner: &ImageInner = (&image).into();
    match image_inner {
        ImageInner::None => None,
        ImageInner::EmbeddedImage { buffer, cache_key } => image_cache.borrow_mut().get_or_insert(
            Some(cache_key.clone()),
            ImageVariant::Full,
            || image_buffer_to_peniko_image(buffer),
        ),
        ImageInner::Svg(svg) => {
            // Query target_width/height here again to ensure that changes will invalidate the item rendering cache.
            let render_size = i_slint_core::graphics::scalable_render_size(
                svg.size(),
                image_fit,
                target_size_fn() * scale_factor,
                scale_factor,
                Default::default(),
            )?;
            image_cache.borrow_mut().get_or_insert(
                ImageCacheKey::new(image_inner),
                ImageVariant::Sized { width: render_size.width, height: render_size.height },
                || {
                    let pixels = match svg.render(Some(render_size)).ok()? {
                        SharedImageBuffer::RGB8(_) => unreachable!(),
                        SharedImageBuffer::RGBA8(_) => unreachable!(),
                        SharedImageBuffer::RGBA8Premultiplied(pixels) => pixels,
                    };

                    let width = pixels.width();
                    let height = pixels.height();

                    let data = peniko::Blob::new(Arc::new(PixelBufferWrap(pixels)));

                    Some(peniko::ImageData {
                        data,
                        format: peniko::ImageFormat::Rgba8,
                        alpha_type: peniko::ImageAlphaType::AlphaPremultiplied,
                        width,
                        height,
                    })
                },
            )
        }
        ImageInner::StaticTextures(_) => image_cache.borrow_mut().get_or_insert(
            ImageCacheKey::new(image_inner),
            ImageVariant::Full,
            || {
                let buffer = image_inner.render_to_buffer(None)?;
                image_buffer_to_peniko_image(&buffer)
            },
        ),
        // Backend storage is only produced by other renderers in the same
        // process; their data is not usable here.
        ImageInner::BackendStorage(..) => None,
        ImageInner::NineSlice(n) => {
            load_image(n.image(), target_size_fn, ImageFit::Preserve, scale_factor, image_cache)
        }
        // Remaining variants hold live GPU resources (borrowed GL textures,
        // wgpu textures behind the unstable-wgpu-* features) that this
        // backend-agnostic renderer cannot import.
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

/// Clamp the given image-data-space coordinates to the image bounds and
/// round them to whole pixels; returns `None` for a degenerate (empty) crop.
fn crop_rect(
    image: &peniko::ImageData,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Option<(u32, u32, u32, u32)> {
    let x = (x.round().max(0.) as u32).min(image.width);
    let y = (y.round().max(0.) as u32).min(image.height);
    let width = (width.round().max(0.) as u32).min(image.width - x);
    let height = (height.round().max(0.) as u32).min(image.height - y);
    if width == 0 || height == 0 { None } else { Some((x, y, width, height)) }
}

/// Extract a sub-rectangle of an RGBA8 image into its own [`peniko::ImageData`],
/// for use as a repeating tile: [`peniko::Extend::Repeat`] wraps the whole
/// brush image, so the tile must be exactly the image. The rectangle must
/// come from [`crop_rect`].
fn crop_image_data(
    image: &peniko::ImageData,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> peniko::ImageData {
    debug_assert!(matches!(image.format, peniko::ImageFormat::Rgba8));
    let src = image.data.data();
    let stride = image.width as usize * 4;
    let mut out = Vec::with_capacity(width as usize * height as usize * 4);
    for row in y..y + height {
        let start = row as usize * stride + x as usize * 4;
        out.extend_from_slice(&src[start..start + width as usize * 4]);
    }

    peniko::ImageData {
        data: peniko::Blob::new(Arc::new(out)),
        format: image.format,
        alpha_type: image.alpha_type,
        width,
        height,
    }
}

fn image_buffer_to_peniko_image(buffer: &SharedImageBuffer) -> Option<peniko::ImageData> {
    let (data, format, alpha_type) = match buffer {
        SharedImageBuffer::RGB8(shared_pixel_buffer) => {
            let rgba: Vec<u8> = shared_pixel_buffer
                .as_bytes()
                .as_chunks::<3>()
                .0
                .iter()
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
            Arc::new(PixelBufferWrap(shared_pixel_buffer.clone()))
                as Arc<dyn AsRef<[u8]> + Send + Sync>,
            peniko::ImageFormat::Rgba8,
            peniko::ImageAlphaType::Alpha,
        ),
        SharedImageBuffer::RGBA8Premultiplied(shared_pixel_buffer) => (
            Arc::new(PixelBufferWrap(shared_pixel_buffer.clone()))
                as Arc<dyn AsRef<[u8]> + Send + Sync>,
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

struct PixelBufferWrap<Pixel>(SharedPixelBuffer<Pixel>);
impl<Pixel: Clone + rgb::Pod> AsRef<[u8]> for PixelBufferWrap<Pixel>
where
    [Pixel]: rgb::ComponentBytes<u8>,
{
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}
