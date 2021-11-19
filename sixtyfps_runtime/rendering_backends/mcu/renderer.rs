/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use core::pin::Pin;
use std::rc::Rc;

use embedded_graphics::prelude::*;
use sixtyfps_corelib::graphics::{Point as PointF, Rect as RectF, Size as SizeF};
use sixtyfps_corelib::items::Item;
use sixtyfps_corelib::Color;

pub fn render_window_frame<T: DrawTarget<Color = embedded_graphics::pixelcolor::Rgb888>>(
    runtime_window: Rc<sixtyfps_corelib::window::Window>,
    display: &mut T,
) where
    T::Error: std::fmt::Debug,
{
    let size = display.bounding_box().size;
    let scene = prepare_scene(runtime_window, SizeF::new(size.width as _, size.height as _));
    // TODO: process the scene to render line by line.
    // for now, just draw them
    for item in scene.items {
        match item.command {
            SceneCommand::Rectangle { color } => {
                embedded_graphics::primitives::Rectangle {
                    top_left: Point { x: item.x as _, y: item.y as _ },
                    size: Size { width: item.width as _, height: item.height as _ },
                }
                .into_styled(
                    embedded_graphics::primitives::PrimitiveStyleBuilder::new()
                        .fill_color(embedded_graphics::pixelcolor::Rgb888::new(
                            color.red(),
                            color.green(),
                            color.blue(),
                        ))
                        .build(),
                )
                .draw(display)
                .unwrap();
            }
        }
    }
}

struct Scene {
    items: Vec<SceneItem>,
}

struct SceneItem {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    // this is the order of the item from which it is in the item tree
    z: u16,
    command: SceneCommand,
}

enum SceneCommand {
    Rectangle { color: Color },
}

fn prepare_scene(runtime_window: Rc<sixtyfps_corelib::window::Window>, size: SizeF) -> Scene {
    let mut prepare_scene = PrepareScene::new(size);
    runtime_window.clone().draw_contents(|components| {
        for (component, origin) in components {
            sixtyfps_corelib::item_rendering::render_component_items(
                &component,
                &mut prepare_scene,
                origin.clone(),
            );
        }
    });
    Scene { items: prepare_scene.items }
}

struct PrepareScene {
    items: Vec<SceneItem>,
    state_stack: Vec<RenderState>,
    current_state: RenderState,
}

impl PrepareScene {
    fn new(size: SizeF) -> Self {
        Self {
            items: vec![],
            state_stack: vec![],
            current_state: RenderState {
                alpha: 1.,
                offset: PointF::default(),
                clip: RectF::new(PointF::default(), size),
            },
        }
    }

    fn should_draw(&self, rect: &RectF) -> bool {
        !rect.size.is_empty()
            && self.current_state.alpha > 0.01
            && self.current_state.clip.intersects(&rect)
    }

    fn new_scene_item(&mut self, geometry: RectF, command: SceneCommand) {
        match geometry.intersection(&self.current_state.clip) {
            Some(geometry) => {
                let z = self.items.len() as u16;
                self.items.push(SceneItem {
                    x: (self.current_state.offset.x + geometry.origin.x) as _,
                    y: (self.current_state.offset.y + geometry.origin.y) as _,
                    width: geometry.size.width as _,
                    height: geometry.size.height as _,
                    z,
                    command,
                });
            }
            None => (),
        }
    }
}

#[derive(Clone, Copy)]
struct RenderState {
    alpha: f32,
    offset: PointF,
    clip: RectF,
}

impl sixtyfps_corelib::item_rendering::ItemRenderer for PrepareScene {
    fn draw_rectangle(&mut self, rect: Pin<&sixtyfps_corelib::items::Rectangle>) {
        let geom = RectF::new(PointF::default(), rect.geometry().size);
        if self.should_draw(&geom) {
            // FIXME: gradients
            let color = rect.background().color();
            if color.alpha() == 0 {
                return;
            }
            self.new_scene_item(geom, SceneCommand::Rectangle { color });
        }
    }

    fn draw_border_rectangle(&mut self, rect: Pin<&sixtyfps_corelib::items::BorderRectangle>) {
        let geom = RectF::new(PointF::default(), rect.geometry().size);
        if self.should_draw(&geom) {
            let border = rect.border_width();
            // FIXME: gradients
            let color = rect.background().color();
            if color.alpha() > 0 {
                self.new_scene_item(
                    geom.inflate(-border, -border),
                    SceneCommand::Rectangle { color },
                );
            }
            if border > 0.01 {
                // FIXME: radius
                // FIXME: gradients
                let border_color = rect.border_color().color();
                if border_color.alpha() > 0 {
                    let mut add_border = |r| {
                        self.new_scene_item(r, SceneCommand::Rectangle { color: border_color });
                    };
                    add_border(euclid::rect(0., 0., geom.width(), border));
                    add_border(euclid::rect(0., geom.height() - border, geom.width(), border));
                    add_border(euclid::rect(0., border, border, geom.height() - border - border));
                    add_border(euclid::rect(
                        geom.width() - border,
                        border,
                        border,
                        geom.height() - border - border,
                    ));
                }
            }
        }
    }

    fn draw_image(&mut self, _image: Pin<&sixtyfps_corelib::items::ImageItem>) {
        // TODO
    }

    fn draw_clipped_image(&mut self, _image: Pin<&sixtyfps_corelib::items::ClippedImage>) {
        // TODO
    }

    fn draw_text(&mut self, _text: Pin<&sixtyfps_corelib::items::Text>) {
        // TODO
    }

    fn draw_text_input(&mut self, _text_input: Pin<&sixtyfps_corelib::items::TextInput>) {
        // TODO
    }

    fn draw_path(&mut self, _path: Pin<&sixtyfps_corelib::items::Path>) {
        // TODO
    }

    fn draw_box_shadow(&mut self, _box_shadow: Pin<&sixtyfps_corelib::items::BoxShadow>) {
        // TODO
    }

    fn combine_clip(&mut self, other: RectF, _radius: f32, _border_width: f32) {
        match self.current_state.clip.intersection(&other) {
            Some(r) => {
                self.current_state.clip = r;
            }
            None => {
                self.current_state.clip = RectF::default();
            }
        };
        // TODO: handle radius and border
    }

    fn get_current_clip(&self) -> sixtyfps_corelib::graphics::Rect {
        self.current_state.clip
    }

    fn translate(&mut self, x: f32, y: f32) {
        self.current_state.offset.x += x;
        self.current_state.offset.y += y;
        self.current_state.clip = self.current_state.clip.translate((-x, -y).into())
    }

    fn rotate(&mut self, _angle_in_degrees: f32) {
        todo!()
    }

    fn apply_opacity(&mut self, opacity: f32) {
        self.current_state.alpha *= opacity;
    }

    fn save_state(&mut self) {
        self.state_stack.push(self.current_state);
    }

    fn restore_state(&mut self) {
        self.current_state = self.state_stack.pop().unwrap();
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
        unreachable!("this backend don't query the window")
    }

    fn as_any(&mut self) -> &mut dyn core::any::Any {
        self
    }
}
