// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::pin::Pin;

use anyrender::PaintScene;
use i_slint_core::graphics::euclid;
use i_slint_core::item_rendering::{
    CachedRenderingData, ItemRenderer, RenderBorderRectangle, RenderImage, RenderRectangle,
    RenderText,
};
use i_slint_core::items::{self, ItemRc, Opacity, RenderingResult};
use i_slint_core::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalRect, LogicalSize, LogicalVector,
    PhysicalBorderRadius, RectLengths, ScaleFactor,
};
use i_slint_core::{Brush, Color};

use super::{PhysicalLength, PhysicalRect, PhysicalSize};

/// anyrender's `push_layer` always clips; there is no "no clip", so layers
/// that should not clip use a rectangle larger than any real scene.
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

    #[allow(unused_variables)]
    fn draw_image(
        &mut self,
        image: Pin<&dyn RenderImage>,
        _item_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        todo!()
    }

    #[allow(unused_variables)]
    fn draw_text(
        &mut self,
        text: Pin<&dyn RenderText>,
        self_rc: &i_slint_core::items::ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        todo!()
    }

    #[allow(unused_variables)]
    fn draw_text_input(
        &mut self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        self_rc: &i_slint_core::items::ItemRc,
        size: LogicalSize,
    ) {
        todo!()
    }

    #[allow(unused_variables)]
    fn draw_path(&mut self, path: Pin<&items::Path>, item_rc: &ItemRc, size: LogicalSize) {
        todo!()
    }

    #[allow(unused_variables)]
    fn draw_box_shadow(
        &mut self,
        box_shadow: Pin<&items::BoxShadow>,
        _item_rc: &ItemRc,
        size: LogicalSize,
    ) {
        todo!()
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

    #[allow(unused_variables)]
    fn draw_cached_pixmap(
        &mut self,
        _item_rc: &ItemRc,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        todo!()
    }

    #[allow(unused_variables)]
    fn draw_string(&mut self, string: &str, color: Color) {
        todo!()
    }

    #[allow(unused_variables)]
    fn draw_image_direct(&mut self, image: i_slint_core::graphics::Image) {
        todo!()
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
