/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use std::rc::Rc;

pub struct SoftwareRenderer<'a, Target: embedded_graphics::draw_target::DrawTarget + 'static> {
    pub draw_target: &'a mut Target,
    pub window: Rc<sixtyfps_corelib::window::Window>,
}

impl<Target: embedded_graphics::draw_target::DrawTarget>
    sixtyfps_corelib::item_rendering::ItemRenderer for SoftwareRenderer<'_, Target>
{
    fn draw_rectangle(&mut self, _rect: std::pin::Pin<&sixtyfps_corelib::items::Rectangle>) {
        // TODO
    }

    fn draw_border_rectangle(
        &mut self,
        _rect: std::pin::Pin<&sixtyfps_corelib::items::BorderRectangle>,
    ) {
        // TODO
    }

    fn draw_image(&mut self, _image: std::pin::Pin<&sixtyfps_corelib::items::ImageItem>) {
        // TODO
    }

    fn draw_clipped_image(
        &mut self,
        _image: std::pin::Pin<&sixtyfps_corelib::items::ClippedImage>,
    ) {
        // TODO
    }

    fn draw_text(&mut self, _text: std::pin::Pin<&sixtyfps_corelib::items::Text>) {
        // TODO
    }

    fn draw_text_input(&mut self, _text_input: std::pin::Pin<&sixtyfps_corelib::items::TextInput>) {
        // TODO
    }

    fn draw_path(&mut self, _path: std::pin::Pin<&sixtyfps_corelib::items::Path>) {
        // TODO
    }

    fn draw_box_shadow(&mut self, _box_shadow: std::pin::Pin<&sixtyfps_corelib::items::BoxShadow>) {
        // TODO
    }

    fn combine_clip(
        &mut self,
        _rect: sixtyfps_corelib::graphics::Rect,
        _radius: f32,
        _border_width: f32,
    ) {
        // TODO
    }

    fn get_current_clip(&self) -> sixtyfps_corelib::graphics::Rect {
        Default::default()
    }

    fn translate(&mut self, _x: f32, _y: f32) {
        // TODO
    }

    fn rotate(&mut self, _angle_in_degrees: f32) {
        // TODO
    }

    fn apply_opacity(&mut self, _opacity: f32) {
        // TODO
    }

    fn save_state(&mut self) {
        // TODO
    }

    fn restore_state(&mut self) {
        // TODO
    }

    fn scale_factor(&self) -> f32 {
        // TODO
        1.0
    }

    fn draw_cached_pixmap(
        &mut self,
        _item_cache: &sixtyfps_corelib::item_rendering::CachedRenderingData,
        _update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        todo!()
    }

    fn window(&self) -> sixtyfps_corelib::window::WindowRc {
        self.window.clone()
    }

    fn as_any(&mut self) -> &mut dyn core::any::Any {
        self.draw_target
    }
}
