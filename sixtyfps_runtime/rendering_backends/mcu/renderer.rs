/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::{vec, vec::Vec};
use core::pin::Pin;

use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use sixtyfps_corelib::graphics::{
    IntRect, PixelFormat, Point as PointF, Rect as RectF, Size as SizeF,
};
use sixtyfps_corelib::items::Item;
use sixtyfps_corelib::{Color, ImageInner};

pub fn render_window_frame<T: DrawTarget<Color = Rgb888>>(
    runtime_window: Rc<sixtyfps_corelib::window::Window>,
    background: Rgb888,
    display: &mut T,
) where
    T::Error: core::fmt::Debug,
{
    let size = display.bounding_box().size;
    let mut scene = prepare_scene(runtime_window, SizeF::new(size.width as _, size.height as _));

    /*for item in scene.future_items {
        match item.command {
            SceneCommand::Rectangle { color } => {
                embedded_graphics::primitives::Rectangle {
                    top_left: Point { x: item.x as _, y: item.y as _ },
                    size: Size { width: item.width as _, height: item.height as _ },
                }
                .into_styled(
                    embedded_graphics::primitives::PrimitiveStyleBuilder::new()
                        .fill_color(Rgb888::new(color.red(), color.green(), color.blue()))
                        .build(),
                )
                .draw(display)
                .unwrap();
            }
            SceneCommand::Texture { data, format, stride, source_width, source_height, color } => {
                let sx = item.width as f32 / source_width as f32;
                let sy = item.height as f32 / source_height as f32;
                let bpp = bpp(format) as usize;
                for y in 0..item.height {
                    let pixel_iter = (0..item.width).into_iter().map(|x| {
                        let pos = ((y as f32 / sy) as usize * stride as usize)
                            + (x as f32 / sx) as usize * bpp;
                        to_color(&data[pos..], format, color)
                    });

                    display
                        .fill_contiguous(
                            &embedded_graphics::primitives::Rectangle::new(
                                Point::new(item.x as i32, (item.y + y) as i32),
                                Size::new(item.width as u32, 1),
                            ),
                            pixel_iter,
                        )
                        .unwrap()
                }
            }
        }
    }*/

    let mut line_buffer = vec![background; size.width as usize];
    while scene.current_line < size.height as u16 {
        line_buffer.fill(background);
        let line = scene.process_line();
        for span in line.spans.iter().rev() {
            match span.command {
                SceneCommand::Rectangle { color } => {
                    let alpha = color.alpha();
                    if alpha == u8::MAX {
                        line_buffer[(span.x) as usize..(span.x + span.width) as usize]
                            .fill(to_rgb888_color_discard_alpha(color))
                    } else {
                        for pix in
                            &mut line_buffer[(span.x) as usize..(span.x + span.width) as usize]
                        {
                            let a = (u8::MAX - alpha) as u16;
                            let b = alpha as u16;
                            *pix = Rgb888::new(
                                ((pix.r() as u16 * a + color.red() as u16 * b) >> 8) as u8,
                                ((pix.g() as u16 * a + color.green() as u16 * b) >> 8) as u8,
                                ((pix.b() as u16 * a + color.blue() as u16 * b) >> 8) as u8,
                            );
                        }
                    }
                }
                SceneCommand::Texture {
                    data,
                    format,
                    stride,
                    source_width,
                    source_height,
                    color,
                } => {
                    let sx = span.width as f32 / source_width as f32;
                    let sy = span.height as f32 / source_height as f32;
                    let bpp = bpp(format) as usize;
                    let y = line.line - span.y;

                    for (x, pix) in line_buffer[(span.x) as usize..(span.x + span.width) as usize]
                        .iter_mut()
                        .enumerate()
                    {
                        let pos = ((y as f32 / sy) as usize * stride as usize)
                            + (x as f32 / sx) as usize * bpp;
                        *pix = match format {
                            PixelFormat::Rgb => {
                                Rgb888::new(data[pos + 0], data[pos + 1], data[pos + 2])
                            }
                            PixelFormat::Rgba => {
                                if color.alpha() == 0 {
                                    let a = (u8::MAX - data[pos + 3]) as u16;
                                    let b = data[pos + 3] as u16;
                                    Rgb888::new(
                                        ((pix.r() as u16 * a + data[pos + 0] as u16 * b) >> 8)
                                            as u8,
                                        ((pix.g() as u16 * a + data[pos + 1] as u16 * b) >> 8)
                                            as u8,
                                        ((pix.b() as u16 * a + data[pos + 2] as u16 * b) >> 8)
                                            as u8,
                                    )
                                } else {
                                    let a = (u8::MAX - data[pos + 3]) as u16;
                                    let b = data[pos + 3] as u16;
                                    Rgb888::new(
                                        ((pix.r() as u16 * a + color.red() as u16 * b) >> 8) as u8,
                                        ((pix.g() as u16 * a + color.green() as u16 * b) >> 8)
                                            as u8,
                                        ((pix.b() as u16 * a + color.blue() as u16 * b) >> 8) as u8,
                                    )
                                }
                            }
                            PixelFormat::AlphaMap => {
                                let a = (u8::MAX - data[pos]) as u16;
                                let b = data[pos] as u16;
                                Rgb888::new(
                                    ((pix.r() as u16 * a + color.red() as u16 * b) >> 8) as u8,
                                    ((pix.g() as u16 * a + color.green() as u16 * b) >> 8) as u8,
                                    ((pix.b() as u16 * a + color.blue() as u16 * b) >> 8) as u8,
                                )
                            }
                        }
                    }
                }
            }
        }
        display
            .fill_contiguous(
                &embedded_graphics::primitives::Rectangle::new(
                    Point::new(0, line.line as i32),
                    Size::new(size.width, 1),
                ),
                line_buffer.iter().copied(),
            )
            .unwrap()
    }
}

struct Scene {
    /// the next line to be processed
    current_line: u16,

    /// Element that have `y > current_line`
    /// They must be sorted by `y` in reverse order (bottom to top)
    /// then by `z` top to bottom
    future_items: Vec<SceneItem>,

    /// The items that overlap with the current line, sorted by z top to bottom
    current_items: VecDeque<SceneItem>,

    /// Some staging buffer of scene item
    next_items: VecDeque<SceneItem>,
}

impl Scene {
    fn new(mut items: Vec<SceneItem>) -> Self {
        items.sort_by(|a, b| compare_scene_item(a, b).reverse());
        Self {
            future_items: items,
            current_line: 0,
            current_items: Default::default(),
            next_items: Default::default(),
        }
    }

    /// Will generate a LineCommand for the current_line, remove all items that are done from the items
    fn process_line(&mut self) -> LineCommand {
        let mut command = vec![];
        // Take the next element from current_items or future_items
        loop {
            let a_next_z =
                self.future_items.last().filter(|i| i.y == self.current_line).map(|i| i.z);
            let b_next_z = self.current_items.front().map(|i| i.z);
            let item = match (a_next_z, b_next_z) {
                (Some(a), Some(b)) => {
                    if a > b {
                        self.future_items.pop()
                    } else {
                        self.current_items.pop_front()
                    }
                }
                (Some(_), None) => self.future_items.pop(),
                (None, Some(_)) => self.current_items.pop_front(),
                _ => break,
            };
            let item = item.unwrap();
            if item.y + item.height > self.current_line + 1 {
                self.next_items.push_back(item.clone());
            }
            command.push(item);
        }
        core::mem::swap(&mut self.next_items, &mut self.current_items);
        let line = self.current_line;
        self.current_line += 1;
        LineCommand { spans: command, line }
    }
}

#[derive(Clone, Copy)]
struct SceneItem {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    // this is the order of the item from which it is in the item tree
    z: u16,
    command: SceneCommand,
}

struct LineCommand {
    line: u16,
    // Fixme: we need to process these so we do not draw items under opaque regions
    spans: Vec<SceneItem>,
}

fn compare_scene_item(a: &SceneItem, b: &SceneItem) -> core::cmp::Ordering {
    // First, order by line (top to bottom)
    match a.y.partial_cmp(&b.y) {
        None | Some(core::cmp::Ordering::Equal) => {}
        Some(ord) => return ord,
    }
    // Then by the reverse z (front to back)
    match a.z.partial_cmp(&b.z) {
        None | Some(core::cmp::Ordering::Equal) => {}
        Some(ord) => return ord.reverse(),
    }

    // anything else, we don't care
    core::cmp::Ordering::Equal
}

#[derive(Clone, Copy)]
enum SceneCommand {
    Rectangle {
        color: Color,
    },
    Texture {
        data: &'static [u8],
        format: PixelFormat,
        stride: u16,
        source_width: u16,
        source_height: u16,
        color: Color,
    },
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
    Scene::new(prepare_scene.items)
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

    fn draw_image_impl(
        &mut self,
        geom: RectF,
        source: &sixtyfps_corelib::graphics::Image,
        source_clip: IntRect,
        colorize: Color,
    ) {
        let image_inner: &ImageInner = source.into();
        match image_inner {
            ImageInner::None => return,
            ImageInner::AbsoluteFilePath(_) | ImageInner::EmbeddedData { .. } => {
                unimplemented!()
            }
            ImageInner::EmbeddedImage(_) => todo!(),
            ImageInner::StaticTextures { size, data, textures } => {
                let sx = geom.width() / (size.width as f32);
                let sy = geom.height() / (size.height as f32);
                for t in textures.as_slice() {
                    if let Some(dest_rect) = t
                        .rect
                        .intersection(&source_clip)
                        .and_then(|r| r.cast().scale(sx, sy).intersection(&self.current_state.clip))
                    {
                        let actual_x = (dest_rect.origin.x / sx) as i32 - t.rect.origin.x;
                        let actual_y = (dest_rect.origin.y / sy) as i32 - t.rect.origin.y;
                        let stride = t.rect.width() as u16 * bpp(t.format);

                        self.new_scene_item(
                            dest_rect,
                            SceneCommand::Texture {
                                data: &data.as_slice()[(t.index
                                    + (stride as usize) * (actual_y as usize)
                                    + (bpp(t.format) as usize) * (actual_x as usize))..],
                                stride,
                                source_height: (dest_rect.height() / sy) as u16,
                                source_width: (dest_rect.width() / sx) as u16,
                                format: t.format,
                                color: if colorize.alpha() > 0 { colorize } else { t.color },
                            },
                        );
                    }
                }
            }
        };
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
            let geom = match geom.intersection(&self.current_state.clip) {
                Some(geom) => geom,
                None => return,
            };

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
                if let Some(r) =
                    geom.inflate(-border, -border).intersection(&self.current_state.clip)
                {
                    self.new_scene_item(r, SceneCommand::Rectangle { color });
                }
            }
            if border > 0.01 {
                // FIXME: radius
                // FIXME: gradients
                let border_color = rect.border_color().color();
                if border_color.alpha() > 0 {
                    let mut add_border = |r: RectF| {
                        if let Some(r) = r.intersection(&self.current_state.clip) {
                            self.new_scene_item(r, SceneCommand::Rectangle { color: border_color });
                        }
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

    fn draw_image(&mut self, image: Pin<&sixtyfps_corelib::items::ImageItem>) {
        let geom = RectF::new(PointF::default(), image.geometry().size);
        if self.should_draw(&geom) {
            self.draw_image_impl(
                geom,
                &image.source(),
                euclid::rect(0, 0, i32::MAX, i32::MAX),
                Default::default(),
            );
        }
    }

    fn draw_clipped_image(&mut self, image: Pin<&sixtyfps_corelib::items::ClippedImage>) {
        // when the source_clip size is empty, make it full
        let a = |v| if v == 0 { i32::MAX } else { v };

        let geom = RectF::new(PointF::default(), image.geometry().size);
        if self.should_draw(&geom) {
            self.draw_image_impl(
                geom,
                &image.source(),
                euclid::rect(
                    image.source_clip_x(),
                    image.source_clip_y(),
                    a(image.source_clip_width()),
                    a(image.source_clip_height()),
                ),
                image.colorize().color(),
            );
        }
    }

    fn draw_text(&mut self, _text: Pin<&sixtyfps_corelib::items::Text>) {
        // TODO
    }

    fn draw_text_input(&mut self, _text_input: Pin<&sixtyfps_corelib::items::TextInput>) {
        // TODO
    }

    #[cfg(feature = "simulator")]
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

/// bytes per pixels
fn bpp(format: PixelFormat) -> u16 {
    match format {
        PixelFormat::Rgb => 3,
        PixelFormat::Rgba => 4,
        PixelFormat::AlphaMap => 1,
    }
}
/*
fn to_color(data: &[u8], format: PixelFormat, color: Color) -> Rgb888 {
    match format {
        PixelFormat::Rgba if color.alpha() > 0 => Rgb888::new(
            ((color.red() as u16 * data[3] as u16) >> 8) as u8,
            ((color.green() as u16 * data[3] as u16) >> 8) as u8,
            ((color.blue() as u16 * data[3] as u16) >> 8) as u8,
        ),
        PixelFormat::Rgb => Rgb888::new(data[0], data[1], data[2]),
        PixelFormat::Rgba => Rgb888::new(data[0], data[1], data[2]),
        PixelFormat::AlphaMap => Rgb888::new(
            ((color.red() as u16 * data[0] as u16) >> 8) as u8,
            ((color.green() as u16 * data[0] as u16) >> 8) as u8,
            ((color.blue() as u16 * data[0] as u16) >> 8) as u8,
        ),
    }
}*/

pub fn to_rgb888_color_discard_alpha(col: Color) -> Rgb888 {
    Rgb888::new(col.red(), col.green(), col.blue())
}
