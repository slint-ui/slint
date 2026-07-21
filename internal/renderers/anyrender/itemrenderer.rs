// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::pin::Pin;
use std::sync::Arc;

use anyrender::PaintScene;
use i_slint_core::graphics::euclid;
use i_slint_core::graphics::{
    Image, ImageCacheKey, IntRect, IntSize, OpaqueImage, OpaqueImageVTable, SharedImageBuffer,
    SharedPixelBuffer,
};
use i_slint_core::item_rendering::{
    CachedRenderingData, ItemRenderer, RenderBorderRectangle, RenderImage, RenderRectangle,
    RenderText,
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
    state_stack: Vec<RenderState>,
    current_state: RenderState,
}

impl<'a, S: PaintScene> AnyrenderItemRenderer<'a, S> {
    pub fn new(
        scene: &'a mut S,
        width: u32,
        height: u32,
        window: &'a i_slint_core::api::Window,
    ) -> Self {
        Self::new_with_initial_transform(scene, width, height, window, kurbo::Affine::IDENTITY)
    }

    /// Like [`new`](Self::new) but starts with a non-identity transform —
    /// used by linuxkms to apply a screen rotation that all subsequent
    /// items inherit.
    pub fn new_with_initial_transform(
        scene: &'a mut S,
        width: u32,
        height: u32,
        window: &'a i_slint_core::api::Window,
        initial_transform: kurbo::Affine,
    ) -> Self {
        let scale_factor = ScaleFactor::new(window.scale_factor());
        Self {
            window,
            scale_factor,
            scene,
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

        let shape_size = to_kurbo_size(geometry.size);

        let transform = self.current_state.transform;
        self.fill_with_brush(
            rect.background(),
            shape_size,
            transform,
            peniko::Fill::default(),
            &background_shape,
        );

        if border_width.get() > 0.0 {
            self.stroke_with_brush(
                border_color,
                shape_size,
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

                kurbo::Affine::translate((
                    -(tiled_offset.x as f64 * ratio_x + clip_x),
                    -(tiled_offset.y as f64 * ratio_y + clip_y),
                ))
                .then_scale_non_uniform(scale_x, scale_y)
            } else {
                kurbo::Affine::translate((-clip_x, -clip_y)).then_scale_non_uniform(
                    fit.size.width as f64 / clip_w,
                    fit.size.height as f64 / clip_h,
                )
            };

            let shape = kurbo::Rect::new(0., 0., fit.size.width as f64, fit.size.height as f64);

            let transform = self
                .current_state
                .transform
                .then_translate(kurbo::Vec2::new(fit.offset.x as f64, fit.offset.y as f64));

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
        if box_shadow.inset() {
            // anyrender's box-shadow primitive only draws drop shadows.
            return;
        }

        let sf = self.scale_factor;
        let offset = LogicalPoint::from_lengths(box_shadow.offset_x(), box_shadow.offset_y()) * sf;
        let spread = (box_shadow.spread() * sf).get() as f64;
        let blur = (box_shadow.blur() * sf).get().max(0.) as f64;
        let phys_size = size * sf;

        let rect = kurbo::Rect::new(
            offset.x as f64 - spread,
            offset.y as f64 - spread,
            offset.x as f64 + phys_size.width as f64 + spread,
            offset.y as f64 + phys_size.height as f64 + spread,
        );
        if rect.is_zero_area() {
            return;
        }

        // anyrender's box-shadow primitive supports a single uniform corner
        // radius; approximate per-corner radii with their average.
        let radius = box_shadow.logical_border_radius() * sf;
        let radius = (radius.top_left + radius.top_right + radius.bottom_right + radius.bottom_left)
            as f64
            / 4.
            + spread;

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
        _item_rc: &ItemRc,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        // This renderer keeps no persistent per-item graphics cache, so the
        // pixmap is re-uploaded on every call. anyrender backends can still
        // deduplicate by blob identity within a frame.
        let transform = self.current_state.transform;
        let scene = &mut *self.scene;
        update_fn(&mut |width, height, data| {
            let image_data = peniko::ImageData {
                data: peniko::Blob::new(Arc::new(data.to_vec())),
                format: peniko::ImageFormat::Rgba8,
                alpha_type: peniko::ImageAlphaType::AlphaPremultiplied,
                width,
                height,
            };
            let image_brush = peniko::ImageBrush::new(image_data);
            scene.fill(
                peniko::Fill::default(),
                transform,
                peniko::BrushRef::Image(image_brush.as_ref()),
                None,
                &kurbo::Rect::new(0., 0., width as f64, height as f64),
            );
        });
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
        ) else {
            return;
        };

        let shape = kurbo::Rect::new(0., 0., image_data.width as f64, image_data.height as f64);

        let image_brush = peniko::ImageBrush::new(image_data);
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

        fn convert_color_stops<'a>(
            stops: impl Iterator<Item = &'a i_slint_core::graphics::GradientStop>,
        ) -> peniko::ColorStops {
            peniko::ColorStops(
                stops
                    .map(|stop| peniko::ColorStop {
                        offset: stop.position,
                        color: peniko::color::DynamicColor::from_alpha_color(to_peniko_color(
                            stop.color,
                        )),
                    })
                    .collect(),
            )
        }

        Some(match brush {
            Brush::SolidColor(color) => (to_peniko_color(color).into(), None),
            Brush::LinearGradient(gradient) => {
                let (start, end) = i_slint_core::graphics::line_for_angle(
                    gradient.angle(),
                    [shape_size.width as f32, shape_size.height as f32].into(),
                );
                let start = to_kurbo_point(start);
                let end = to_kurbo_point(end);

                let mut peniko_gradient = peniko::Gradient::new_linear(start, end);
                peniko_gradient.stops = convert_color_stops(gradient.stops());

                (peniko_gradient.into(), None)
            }
            Brush::RadialGradient(gradient) => {
                let circle_scale = 0.5
                    * (shape_size.width * shape_size.width + shape_size.height * shape_size.height)
                        .sqrt();

                let mut peniko_gradient =
                    peniko::Gradient::new_radial(kurbo::Point::new(0., 0.), 1.0);
                peniko_gradient.stops = convert_color_stops(gradient.stops());

                (
                    peniko_gradient.into(),
                    Some(kurbo::Affine::scale(circle_scale).then_translate(kurbo::Vec2::new(
                        shape_size.width / 2.,
                        shape_size.height / 2.,
                    ))),
                )
            }
            Brush::ConicGradient(gradient) => {
                let center = kurbo::Point::new(shape_size.width / 2., shape_size.height / 2.);

                let mut peniko_gradient =
                    peniko::Gradient::new_sweep(center, 0., 360f32.to_radians());
                peniko_gradient.stops = convert_color_stops(gradient.stops());

                (peniko_gradient.into(), None)
            }
            _ => return None,
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

pub(crate) fn to_peniko_color(color: Color) -> peniko::Color {
    let color = color.to_argb_u8();
    peniko::Color::from_rgba8(color.red, color.green, color.blue, color.alpha)
}

struct AnyrenderCachedImage {
    image_data: peniko::ImageData,
    size: IntSize,
    cache_key: ImageCacheKey,
}

i_slint_core::OpaqueImageVTable_static! {
    static ANYRENDER_CACHED_IMAGE_VT for AnyrenderCachedImage
}

impl OpaqueImage for AnyrenderCachedImage {
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
                    AnyrenderCachedImage {
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

            Some(peniko::ImageData {
                data,
                format: peniko::ImageFormat::Rgba8,
                alpha_type: peniko::ImageAlphaType::AlphaPremultiplied,
                width,
                height,
            })
        }
        ImageInner::StaticTextures(_) => {
            let buffer = image_inner.render_to_buffer(None)?;
            image_buffer_to_peniko_image(&buffer)
        }
        ImageInner::BackendStorage(x) => {
            vtable::VRc::borrow(x).downcast::<AnyrenderCachedImage>().map(|x| x.image_data.clone())
        }
        ImageInner::NineSlice(n) => {
            load_image(n.image(), target_size_fn, ImageFit::Preserve, scale_factor)
        }
        // Remaining variants hold live GPU resources (borrowed GL textures,
        // wgpu textures behind the unstable-wgpu-* features) that this
        // backend-agnostic renderer cannot import.
        #[allow(unreachable_patterns)]
        _ => None,
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
