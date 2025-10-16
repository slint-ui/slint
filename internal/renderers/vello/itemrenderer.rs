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
    SharedImageBuffer, SharedPixelBuffer, Size, rgb,
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
        let shape = self.rect(LogicalRect::new(LogicalPoint::default(), size));

        let (brush, brush_transform) = self.brush(rect.background(), shape.bounding_box().size());

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
    }

    fn draw_window_background(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
    }

    fn draw_image(
        &mut self,
        image: Pin<&dyn RenderImage>,
        item_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
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

    fn draw_path(&mut self, path: Pin<&items::Path>, item_rc: &ItemRc, _size: LogicalSize) {}

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
        item_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        /*
        let opacity = opacity_item.opacity();
        if Opacity::need_layer(item_rc, opacity) {
            self.render_and_blend_layer(opacity, item_rc)
        } else {
            self.apply_opacity(opacity);
            self.graphics_cache.release(item_rc);
            RenderingResult::ContinueRenderingChildren
        }
        */
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
        item_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        RenderingResult::ContinueRenderingChildren

        /*
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
        */
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

        self.scene.push_clip_layer(self.current_state.transform, &clip_shape);
        self.current_state.layer_count += 1;

        clip_region_valid
    }

    fn get_current_clip(&self) -> LogicalRect {
        self.current_state.clip_rect
    }

    fn save_state(&mut self) {
        self.state_stack.push(self.current_state);
        self.state_stack.last_mut().unwrap().layer_count = 0;
    }

    fn restore_state(&mut self) {
        self.current_state = self.state_stack.pop().unwrap();
        for _ in 0..std::mem::take(&mut self.current_state.layer_count) {
            self.scene.pop_layer();
        }
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

    fn draw_image_direct(&mut self, image: i_slint_core::graphics::Image) {}

    fn window(&self) -> &i_slint_core::window::WindowInner {
        i_slint_core::window::WindowInner::from_pub(self.window)
    }

    fn as_any(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }

    fn translate(&mut self, distance: LogicalVector) {
        self.current_state.translation += distance;
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
        todo!()
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
        let (peniko_brush, brush_transform) = self.brush(brush, self.size(size));
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
        let (peniko_brush, brush_transform) = self.brush(stroke_brush, self.size(size));

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
        vello::kurbo::RoundedRect::new(
            phys_rect.min_y() as f64,
            phys_rect.min_y() as f64,
            phys_rect.max_x() as f64,
            phys_rect.max_y() as f64,
            vello::kurbo::RoundedRectRadii::new(
                phys_radius.top_left as f64,
                phys_radius.top_right as f64,
                phys_radius.bottom_right as f64,
                phys_radius.bottom_left as f64,
            ),
        )
    }

    fn rect(&self, rect: LogicalRect) -> vello::kurbo::Rect {
        let phys_rect = rect * self.scale_factor;
        vello::kurbo::Rect::new(
            phys_rect.min_y() as f64,
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
    ) -> (vello::peniko::Brush, Option<vello::kurbo::Affine>) {
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

        match brush {
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
        }
    }
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
        ImageInner::StaticTextures(_) => todo!(),
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
        SharedImageBuffer::RGB8(_) => return None,
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
