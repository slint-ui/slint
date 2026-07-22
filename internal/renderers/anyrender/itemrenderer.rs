// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::pin::Pin;
use std::sync::Arc;

use anyrender::PaintScene;
use i_slint_core::graphics::euclid;
use i_slint_core::graphics::{Image, ImageCacheKey, IntRect, SharedImageBuffer, SharedPixelBuffer};
use i_slint_core::item_rendering::{
    CachedRenderingData, ItemCache, ItemRenderer, RenderBorderRectangle, RenderImage,
    RenderRectangle, RenderText,
};
use i_slint_core::items::{
    self, FillRule, ImageFit, ImageRendering, ItemRc, Opacity, RenderingResult,
};
use i_slint_core::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalRect, LogicalSize, LogicalVector,
    PhysicalBorderRadius, RectLengths, ScaleFactor, logical_size_from_api,
};
use i_slint_core::textlayout::sharedparley::{self, GlyphRenderer, fontique, parley};
use i_slint_core::{Brush, Color, ImageInner, SharedString};

use super::{PhysicalLength, PhysicalRect, PhysicalSize};

/// anyrender's `push_layer` always clips; there is no "no clip", so layers
/// that should not clip use a rectangle larger than any real scene.
///
/// Only safe for non-destructive compose modes: vello_cpu <= 0.0.9 mishandles
/// layers with destructive compose modes (`SrcIn`, `DestOut`) whose bounds
/// greatly exceed the viewport — everything beyond the first 256px wide-tile
/// column is lost. Bound such layers to the area they affect instead. Fixed
/// upstream by the frontend rewrite (linebender/vello#1701), but bounding
/// destructive layers stays worthwhile on fixed versions too: it spares
/// vello_cpu from compositing the entire surface.
const UNCLIPPED: kurbo::Rect = kurbo::Rect::new(0., 0., 1e9, 1e9);

#[derive(Clone, Copy)]
struct RenderState {
    clip_rect: LogicalRect,
    transform: kurbo::Affine,
    layer_count: usize,
}

pub struct AnyrenderItemRenderer<'a, S: PaintScene> {
    window: &'a i_slint_core::api::Window,
    scale_factor: ScaleFactor,
    scene: &'a mut S,
    image_cache: &'a std::cell::RefCell<crate::ImageConversionCache>,
    item_image_cache: &'a ItemCache<Option<crate::SharedImageData>>,
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
    ) -> Self {
        Self::new_with_initial_transform(
            scene,
            width,
            height,
            window,
            image_cache,
            item_image_cache,
            kurbo::Affine::IDENTITY,
        )
    }

    /// Like [`new`](Self::new) but starts with a non-identity transform —
    /// used by linuxkms to apply a screen rotation that all subsequent
    /// items inherit.
    pub fn new_with_initial_transform(
        scene: &'a mut S,
        width: u32,
        height: u32,
        window: &'a i_slint_core::api::Window,
        image_cache: &'a std::cell::RefCell<crate::ImageConversionCache>,
        item_image_cache: &'a ItemCache<Option<crate::SharedImageData>>,
        initial_transform: kurbo::Affine,
    ) -> Self {
        let scale_factor = ScaleFactor::new(window.scale_factor());
        Self {
            window,
            scale_factor,
            scene,
            image_cache,
            item_image_cache,
            state_stack: vec![],
            current_state: RenderState {
                clip_rect: LogicalRect::from_size(
                    PhysicalSize::new(width as f32, height as f32) / scale_factor,
                ),
                transform: initial_transform,
                layer_count: 0,
            },
        }
    }
}

impl<'a, S: PaintScene> ItemRenderer for AnyrenderItemRenderer<'a, S> {
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
            self.size(size),
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
        let mut geometry = PhysicalRect::from(size * self.scale_factor);
        if geometry.is_empty() {
            return;
        }

        // Save the original element bounds for gradient positioning. The CSS
        // model positions gradients relative to the border box (full element),
        // but adjust_rect_and_border_for_inner_drawing shrinks the geometry,
        // which would shift the gradient center inward.
        let brush_size = to_kurbo_size(geometry.size);

        let border_color = rect.border_color();
        let opaque_border = border_color.is_opaque();
        let mut border_width = if border_color.is_transparent() {
            PhysicalLength::new(0.)
        } else {
            rect.border_width() * self.scale_factor
        };

        let mut fill_radius = rect.border_radius() * self.scale_factor;
        // The stroke is centered on the path (50% inside, 50% outside). We want
        // the CSS model where the border is entirely inside. Adjust the outer
        // radius so that corners with a positive radius are at least
        // border_width/2. This is incorrect if the radius is smaller than
        // border_width/2, but that can't be helped - better a radius a bit
        // too big than no radius at all.
        let radius_epsilon = PhysicalLength::new(0.01);
        fill_radius = fill_radius.outer(border_width / 2. + radius_epsilon);
        let stroke_border_radius = fill_radius.inner(border_width / 2.);

        let (background_shape, border_shape) = if opaque_border {
            // When the border is opaque, the fill doesn't need to extend under it,
            // so both fill and stroke use the same adjusted (inset) geometry.
            adjust_rect_and_border_for_inner_drawing(&mut geometry, &mut border_width);
            let shape = phys_rounded_rect(geometry, stroke_border_radius);
            (shape, shape)
        } else {
            // When the border is transparent/semi-transparent, the fill must cover
            // the full outer rectangle so the background shows through.
            let background_shape = phys_rounded_rect(geometry, fill_radius);
            adjust_rect_and_border_for_inner_drawing(&mut geometry, &mut border_width);
            let border_shape = phys_rounded_rect(geometry, stroke_border_radius);
            (background_shape, border_shape)
        };

        let transform = self.current_state.transform;
        self.fill_with_brush(
            rect.background(),
            brush_size,
            transform,
            peniko::Fill::default(),
            &background_shape,
        );

        if border_width.get() > 0.0 {
            self.stroke_with_brush(
                border_color,
                brush_size,
                transform,
                &kurbo::Stroke::new(border_width.get() as f64),
                &border_shape,
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
            self.size(size),
            self.current_state.transform,
            peniko::Fill::default(),
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

        // The per-item cache tracks the properties read in the closure
        // (source, image-fit, target size) and invalidates on change; the
        // shared conversion cache deduplicates across items.
        let image_data = self.item_image_cache.get_or_update_cache_entry(_item_rc, || {
            let image_fit =
                if tiling != Default::default() { ImageFit::Preserve } else { image.image_fit() };
            load_image(
                image.source(),
                &|| image.target_size(),
                image_fit,
                self.scale_factor,
                self.image_cache,
            )
        });
        let Some(image_data) = image_data else {
            return;
        };
        let image_fit =
            if tiling != Default::default() { ImageFit::Preserve } else { image.image_fit() };
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
            // the UNCLIPPED comment).
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
                    image_data.clone()
                } else {
                    let Some(tile) = self.image_cache.borrow_mut().get_or_insert(
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
                    ) else {
                        continue;
                    };
                    tile
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
            if let Some((brush, brush_transform)) = self.brush(colorize_brush, dest_rect.size()) {
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
        sharedparley::draw_text(self, text, Some(self_rc), size, None);
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

        let brush_size = to_kurbo_size(size * sf);

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

        // anyrender's box-shadow primitive supports a single uniform corner
        // radius; approximate per-corner radii with their average.
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
            let shape = kurbo::RoundedRect::from_rect(rect, radius);
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

    fn visit_opacity(
        &mut self,
        opacity_item: Pin<&Opacity>,
        _item_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        let opacity = opacity_item.opacity();
        if opacity < 1.0 {
            self.push_unclipped_layer(peniko::BlendMode::default(), opacity);
            self.current_state.layer_count += 1;
        }
        RenderingResult::ContinueRenderingChildren
    }

    fn combine_clip(
        &mut self,
        clip_rect: LogicalRect,
        radius: LogicalBorderRadius,
        border_width: LogicalLength,
    ) -> bool {
        let mut phys_rect = clip_rect * self.scale_factor;
        let mut phys_border_width = border_width * self.scale_factor;
        // In CSS the border is entirely towards the inside of the boundary
        // geometry, so the clip applies to the region inside the border -
        // same adjustment as the skia and femtovg renderers.
        adjust_rect_and_border_for_inner_drawing(&mut phys_rect, &mut phys_border_width);

        let adjusted_clip_rect = phys_rect / self.scale_factor;
        let clip = &mut self.current_state.clip_rect;
        let clip_region_valid = match clip.intersection(&adjusted_clip_rect) {
            Some(r) => {
                *clip = r;
                true
            }
            None => {
                *clip = LogicalRect::default();
                false
            }
        };

        let clip_shape = phys_rounded_rect(phys_rect, radius * self.scale_factor);

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

    fn scale_factor(&self) -> f32 {
        self.scale_factor.get()
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
            logical_size_from_api(self.window.size().to_logical(self.scale_factor())),
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

    fn apply_opacity(&mut self, _opacity: f32) {
        // Opacity is applied through the alpha layer pushed in
        // visit_opacity(); since that is overridden, the default trait
        // implementation that calls this never runs.
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
        let (peniko_brush, brush_transform) = self.brush(brush, self.size(size))?;
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
        let (peniko_brush, brush_transform) = self.brush(stroke_brush, self.size(size))?;

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
        let shape =
            kurbo::RoundedRect::from_rect(to_kurbo_rect(physical_rect), radius.get() as f64);

        self.scene.fill(
            peniko::Fill::default(),
            self.current_state.transform,
            peniko::BrushRef::from(&brush.peniko_brush),
            brush.brush_transform,
            &shape,
        );

        if let Some(border) = border
            && border.width.get() > 0.
        {
            self.scene.stroke(
                &kurbo::Stroke::new(border.width.get() as f64),
                self.current_state.transform,
                peniko::BrushRef::from(&border.brush.peniko_brush),
                border.brush.brush_transform,
                &shape,
            );
        }
    }
}

impl<'a, S: PaintScene> AnyrenderItemRenderer<'a, S> {
    /// Draw an inset shadow: the border box filled with the shadow color,
    /// minus the blurred interior rectangle (inset by `spread`, translated
    /// by `offset`). Relies on the linearity of the Gaussian —
    /// blur(complement of rect) = 1 − blur(rect) — by filling the clipped
    /// border box with the shadow color and punching out the blurred
    /// interior through a DestOut layer. The clip layer isolates the
    /// punch-out so content drawn before the shadow is unaffected.
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
        let border_shape = kurbo::RoundedRect::from_rect(border_rect, base_radius);

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
            // Bounded to the border box (the outer clip limits the shadow to it
            // anyway): destructive compose modes must not use an unbounded layer
            // (see the UNCLIPPED comment).
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
                    &kurbo::RoundedRect::from_rect(interior, interior_radius),
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
        brush_size: kurbo::Size,
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
        brush_size: kurbo::Size,
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

    fn rect(&self, rect: LogicalRect) -> kurbo::Rect {
        to_kurbo_rect(rect * self.scale_factor)
    }

    fn size(&self, size: LogicalSize) -> kurbo::Size {
        to_kurbo_size(size * self.scale_factor)
    }

    fn brush(
        &self,
        brush: Brush,
        shape_size: kurbo::Size,
    ) -> Option<(peniko::Brush, Option<kurbo::Affine>)> {
        if brush.is_transparent() {
            return None;
        }

        Some(match brush {
            Brush::SolidColor(color) => (to_peniko_color(color).into(), None),
            Brush::LinearGradient(gradient) => {
                let (stops, extent) = sanitize_color_stops(gradient.stops(), true);
                let (start, end) = i_slint_core::graphics::line_for_angle(
                    gradient.angle(),
                    [shape_size.width as f32, shape_size.height as f32].into(),
                );
                let start = to_kurbo_point(start);
                let mut end = to_kurbo_point(end);
                if extent != 1.0 {
                    // Stops reached beyond 100%: lengthen the gradient line so
                    // the normalized offsets cover the same colors.
                    end = start + (end - start) * extent as f64;
                }

                let mut peniko_gradient = peniko::Gradient::new_linear(start, end);
                peniko_gradient.stops = stops;

                (peniko_gradient.into(), None)
            }
            Brush::RadialGradient(gradient) => {
                let (stops, extent) = sanitize_color_stops(gradient.stops(), true);
                let (center_x, center_y) = gradient.center_or_default_scaled(
                    shape_size.width as f32,
                    shape_size.height as f32,
                    self.scale_factor.get(),
                );
                let radius = gradient.radius_or_default_scaled(
                    shape_size.width as f32,
                    shape_size.height as f32,
                    self.scale_factor.get(),
                );
                // Stops reaching beyond 100% grow the circle so the
                // normalized offsets cover the same colors.
                let radius = radius * extent;

                let mut peniko_gradient =
                    peniko::Gradient::new_radial(kurbo::Point::new(0., 0.), 1.0);
                peniko_gradient.stops = stops;

                // A unit circle at the origin, scaled to the radius and moved
                // to the center, so the color stops span [0, radius].
                (
                    peniko_gradient.into(),
                    Some(
                        kurbo::Affine::scale(radius as f64)
                            .then_translate(kurbo::Vec2::new(center_x as f64, center_y as f64)),
                    ),
                )
            }
            Brush::ConicGradient(gradient) => {
                // A sweep gradient's angular range cannot be extended beyond a
                // full turn, so out-of-range stops are clamped instead.
                let (stops, _) = sanitize_color_stops(gradient.stops(), false);
                let (center_x, center_y) = gradient.center_or_default_scaled(
                    shape_size.width as f32,
                    shape_size.height as f32,
                    self.scale_factor.get(),
                );
                let center = kurbo::Point::new(center_x as f64, center_y as f64);

                let mut peniko_gradient =
                    peniko::Gradient::new_sweep(center, 0., 360f32.to_radians());
                peniko_gradient.stops = stops;

                // Sweep gradients start at 3 o'clock (east); Slint's 0° is at
                // 12 o'clock, so rotate the brush by -90° around the center.
                (
                    peniko_gradient.into(),
                    Some(kurbo::Affine::rotate_about(-std::f64::consts::FRAC_PI_2, center)),
                )
            }
            _ => return None,
        })
    }
}

/// Massage gradient color stops into the form vello accepts: sorted,
/// strictly increasing, and with offsets in [0, 1]. vello renders gradients
/// with out-of-range or non-increasing stop offsets as a solid fill of the
/// first color instead.
///
/// Stops below 0 are replaced by the interpolated color at 0 (the CSS
/// behavior). For stops beyond 1: with `can_extend`, all offsets are divided
/// by the maximum and that maximum is returned as the second tuple element,
/// so the caller can grow the gradient geometry (radius, line) by the same
/// factor; without it, they are clamped to the interpolated color at 1.
/// Duplicate offsets — hard color steps — are separated by the smallest
/// representable amount.
fn sanitize_color_stops<'a>(
    stops: impl Iterator<Item = &'a i_slint_core::graphics::GradientStop>,
    can_extend: bool,
) -> (peniko::ColorStops, f32) {
    /// Plain per-channel interpolation between the colors of `a` and `b` at
    /// `position`, matching how the gradient ramp itself blends.
    fn color_at(
        position: f32,
        a: &i_slint_core::graphics::GradientStop,
        b: &i_slint_core::graphics::GradientStop,
    ) -> Color {
        let t = if b.position > a.position {
            ((position - a.position) / (b.position - a.position)).clamp(0., 1.)
        } else {
            0.
        };
        let (ca, cb) = (a.color.to_argb_u8(), b.color.to_argb_u8());
        let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
        Color::from_argb_u8(
            lerp(ca.alpha, cb.alpha),
            lerp(ca.red, cb.red),
            lerp(ca.green, cb.green),
            lerp(ca.blue, cb.blue),
        )
    }

    let mut stops: Vec<i_slint_core::graphics::GradientStop> = stops.cloned().collect();
    stops.sort_by(|a, b| a.position.total_cmp(&b.position));

    // Replace everything below 0 with the interpolated color at 0.
    while stops.len() >= 2 && stops[1].position <= 0. {
        stops.remove(0);
    }
    if let [first, second, ..] = stops.as_slice()
        && first.position < 0.
    {
        stops[0] = i_slint_core::graphics::GradientStop {
            color: color_at(0., first, second),
            position: 0.,
        };
    } else if let [only] = stops.as_slice()
        && only.position < 0.
    {
        stops[0].position = 0.;
    }

    // Handle stops beyond 1: normalize (the caller extends the geometry) or
    // clamp to the interpolated color at 1.
    let mut extent = 1.0f32;
    if stops.last().is_some_and(|last| last.position > 1.) {
        if can_extend {
            extent = stops.last().unwrap().position;
            for stop in &mut stops {
                stop.position /= extent;
            }
        } else {
            while stops.len() >= 2 && stops[stops.len() - 2].position >= 1. {
                stops.pop();
            }
            let clamped_last = match stops.as_slice() {
                [.., second_to_last, last] if last.position > 1. => {
                    Some(i_slint_core::graphics::GradientStop {
                        color: color_at(1., second_to_last, last),
                        position: 1.,
                    })
                }
                [only] if only.position > 1. => {
                    Some(i_slint_core::graphics::GradientStop { color: only.color, position: 1. })
                }
                _ => None,
            };
            if let Some(stop) = clamped_last {
                *stops.last_mut().unwrap() = stop;
            }
        }
    }

    // Make offsets strictly increasing: separate duplicates (hard color
    // steps) by the smallest representable amount, then push back anything
    // that got nudged past 1.
    let mut previous = f32::NEG_INFINITY;
    for stop in &mut stops {
        if stop.position <= previous {
            stop.position = previous.next_up();
        }
        previous = stop.position;
    }
    let mut next = 1.0f32.next_up();
    for stop in stops.iter_mut().rev() {
        if stop.position >= next {
            stop.position = next.next_down();
        }
        next = stop.position;
    }

    let stops = peniko::ColorStops(
        stops
            .iter()
            .map(|stop| peniko::ColorStop {
                offset: stop.position,
                color: peniko::color::DynamicColor::from_alpha_color(to_peniko_color(stop.color)),
            })
            .collect(),
    );
    (stops, extent)
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

fn to_kurbo_point(p: euclid::default::Point2D<f32>) -> kurbo::Point {
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

fn phys_rounded_rect(rect: PhysicalRect, radius: PhysicalBorderRadius) -> kurbo::RoundedRect {
    kurbo::RoundedRect::from_rect(
        to_kurbo_rect(rect),
        kurbo::RoundedRectRadii::new(
            radius.top_left as f64,
            radius.top_right as f64,
            radius.bottom_right as f64,
            radius.bottom_left as f64,
        ),
    )
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
            let render_size: euclid::Size2D<u32, i_slint_core::lengths::PhysicalPx> =
                target_size.cast();
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
/// for use as a repeating tile — [`peniko::Extend::Repeat`] wraps the whole
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
