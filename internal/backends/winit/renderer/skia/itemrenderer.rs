// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::pin::Pin;
use std::rc::Rc;

use i_slint_core::graphics::euclid;
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::{items, Brush, Color};

pub struct SkiaRenderer<'a> {
    pub canvas: &'a mut skia_safe::Canvas,
    pub window: Rc<i_slint_core::window::WindowInner>,
    pub scale_factor: f32,
}

impl<'a> SkiaRenderer<'a> {
    pub fn new(
        canvas: &'a mut skia_safe::Canvas,
        window: &Rc<i_slint_core::window::WindowInner>,
    ) -> Self {
        Self { canvas, window: window.clone(), scale_factor: window.scale_factor() }
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

        let paint = match self.brush_to_paint(rect.background()) {
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
        //todo!()
    }

    fn draw_image(
        &mut self,
        image: std::pin::Pin<&i_slint_core::items::ImageItem>,
        _self_rc: &i_slint_core::items::ItemRc,
    ) {
        //todo!()
    }

    fn draw_clipped_image(
        &mut self,
        image: std::pin::Pin<&i_slint_core::items::ClippedImage>,
        _self_rc: &i_slint_core::items::ItemRc,
    ) {
        //todo!()
    }

    fn draw_text(
        &mut self,
        text: std::pin::Pin<&i_slint_core::items::Text>,
        _self_rc: &i_slint_core::items::ItemRc,
    ) {
        //todo!()
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
        //todo!()
        true // clip region is valid and not empty
    }

    fn get_current_clip(&self) -> i_slint_core::graphics::Rect {
        from_skia_rect(&self.canvas.local_clip_bounds().unwrap())
    }

    fn translate(&mut self, x: i_slint_core::Coord, y: i_slint_core::Coord) {
        //todo!()
    }

    fn rotate(&mut self, angle_in_degrees: f32) {
        //todo!()
    }

    fn apply_opacity(&mut self, opacity: f32) {
        //todo!()
    }

    fn save_state(&mut self) {
        self.canvas.save();
    }

    fn restore_state(&mut self) {
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
        //todo!()
    }

    fn window(&self) -> i_slint_core::window::WindowRc {
        self.window.clone()
    }

    fn as_any(&mut self) -> Option<&mut dyn core::any::Any> {
        None
    }
}

impl<'a> SkiaRenderer<'a> {
    fn brush_to_paint(&self, brush: Brush) -> Option<skia_safe::Paint> {
        if brush.is_transparent() {
            return None;
        }
        let mut paint = skia_safe::Paint::default();
        match brush {
            Brush::SolidColor(color) => paint.set_color(to_skia_color(&color)),
            Brush::LinearGradient(_) => todo!(),
            Brush::RadialGradient(_) => todo!(),
            _ => return None,
        };
        Some(paint)
    }
}

pub fn from_skia_rect(rect: &skia_safe::Rect) -> i_slint_core::graphics::Rect {
    let top_left = euclid::Point2D::new(rect.left, rect.top);
    let bottom_right = euclid::Point2D::new(rect.right, rect.bottom);
    euclid::Box2D::new(top_left, bottom_right).to_rect()
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
