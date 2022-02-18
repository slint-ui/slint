// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::{vec, vec::Vec};
use core::pin::Pin;

use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use i_slint_core::graphics::{FontRequest, IntRect, PixelFormat, Rect as RectF};
use i_slint_core::{Color, ImageInner};

use crate::fonts::FontMetrics;
use crate::lengths::PhysicalPx;
use crate::profiler;
use crate::{
    Devices, LogicalItemGeometry, LogicalPoint, LogicalRect, PhysicalLength, PhysicalPoint,
    PhysicalRect, PhysicalSize, PointLengths, RectLengths, ScaleFactor, SizeLengths,
};

use euclid::num::Zero;

pub fn render_window_frame(
    runtime_window: Rc<i_slint_core::window::Window>,
    background: Rgb888,
    devices: &mut dyn Devices,
) {
    let size = devices.screen_size();
    let mut scene = prepare_scene(runtime_window, size, devices);

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

    let mut line_processing_profiler = profiler::Timer::new_stopped();
    let mut span_drawing_profiler = profiler::Timer::new_stopped();
    let mut screen_fill_profiler = profiler::Timer::new_stopped();

    let mut line_buffer = vec![background; size.width as usize];
    while scene.current_line < size.height_length() {
        line_buffer.fill(background);
        line_processing_profiler.start(devices);
        let line = scene.process_line();
        line_processing_profiler.stop(devices);
        span_drawing_profiler.start(devices);
        for span in line.spans.iter().rev() {
            match span.command {
                SceneCommand::Rectangle => {
                    let color = scene.rectangles[span.data_index];
                    let alpha = color.alpha();
                    if alpha == u8::MAX {
                        line_buffer[span.pos.x as usize
                            ..(span.pos.x_length() + span.size.width_length()).get() as usize]
                            .fill(to_rgb888_color_discard_alpha(color))
                    } else {
                        for pix in &mut line_buffer[span.pos.x as usize
                            ..(span.pos.x_length() + span.size.width_length()).get() as usize]
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
                SceneCommand::Texture => {
                    let SceneTexture { data, format, stride, source_size, color } =
                        scene.textures[span.data_index];

                    let sx: euclid::Scale<f32, PhysicalPx, PhysicalPx> =
                        span.size.width_length().cast::<f32>()
                            / source_size.width_length().cast::<f32>();
                    let sy: euclid::Scale<f32, PhysicalPx, PhysicalPx> =
                        span.size.height_length().cast::<f32>()
                            / source_size.height_length().cast::<f32>();
                    let bpp = bpp(format) as usize;
                    let y = (line.line - span.pos.y_length()).cast::<f32>();

                    for (x, pix) in line_buffer[span.pos.x as usize
                        ..(span.pos.x_length() + span.size.width_length()).get() as usize]
                        .iter_mut()
                        .enumerate()
                        .map(|(x, pix)| (PhysicalLength::new(x as i16).cast::<f32>(), pix))
                    {
                        let pos = ((y / sy).get() as usize * stride as usize)
                            + (x / sx).get() as usize * bpp;
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
        span_drawing_profiler.stop(devices);
        screen_fill_profiler.start(devices);
        devices.fill_region(euclid::rect(0, line.line.get() as i16, size.width, 1), &line_buffer);
        screen_fill_profiler.stop(devices);
    }

    line_processing_profiler.stop_profiling(devices, "line processing");
    span_drawing_profiler.stop_profiling(devices, "span drawing");
    screen_fill_profiler.stop_profiling(devices, "screen fill");
}

struct Scene {
    /// the next line to be processed
    current_line: PhysicalLength,

    /// Element that have `y > current_line`
    /// They must be sorted by `y` in reverse order (bottom to top)
    /// then by `z` top to bottom
    future_items: Vec<SceneItem>,

    /// The items that overlap with the current line, sorted by z top to bottom
    current_items: VecDeque<SceneItem>,

    /// Some staging buffer of scene item
    next_items: VecDeque<SceneItem>,

    rectangles: Vec<Color>,
    textures: Vec<SceneTexture>,
}

impl Scene {
    fn new(mut items: Vec<SceneItem>, rectangles: Vec<Color>, textures: Vec<SceneTexture>) -> Self {
        items.sort_unstable_by(|a, b| compare_scene_item(a, b).reverse());
        Self {
            future_items: items,
            current_line: PhysicalLength::zero(),
            current_items: Default::default(),
            next_items: Default::default(),
            rectangles,
            textures,
        }
    }

    /// Will generate a LineCommand for the current_line, remove all items that are done from the items
    fn process_line(&mut self) -> LineCommand {
        let mut command = vec![];
        // Take the next element from current_items or future_items
        loop {
            let a_next_z = self
                .future_items
                .last()
                .filter(|i| i.pos.y_length() == self.current_line)
                .map(|i| i.z);
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
            if item.pos.y_length() + item.size.height_length()
                > self.current_line + PhysicalLength::new(1)
            {
                self.next_items.push_back(item.clone());
            }
            command.push(item);
        }
        core::mem::swap(&mut self.next_items, &mut self.current_items);
        let line = self.current_line;
        self.current_line += PhysicalLength::new(1);
        LineCommand { spans: command, line }
    }
}

#[derive(Clone, Copy)]
struct SceneItem {
    pos: PhysicalPoint,
    size: PhysicalSize,
    // this is the order of the item from which it is in the item tree
    z: u16,
    command: SceneCommand,
    data_index: usize, // SceneCommand specific index in textures, etc. arrays for additional data
}

struct LineCommand {
    line: PhysicalLength,
    // Fixme: we need to process these so we do not draw items under opaque regions
    spans: Vec<SceneItem>,
}

fn compare_scene_item(a: &SceneItem, b: &SceneItem) -> core::cmp::Ordering {
    // First, order by line (top to bottom)
    match a.pos.y.partial_cmp(&b.pos.y) {
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
#[repr(u8)]
enum SceneCommand {
    Rectangle,
    Texture,
}

struct SceneTexture {
    data: &'static [u8],
    format: PixelFormat,
    /// bytes between two lines in the source
    stride: u16,
    source_size: PhysicalSize,
    color: Color,
}

fn prepare_scene(
    runtime_window: Rc<i_slint_core::window::Window>,
    size: PhysicalSize,
    devices: &dyn Devices,
) -> Scene {
    let prepare_scene_profiler = profiler::Timer::new(devices);
    let mut prepare_scene = PrepareScene::new(
        size,
        ScaleFactor::new(runtime_window.scale_factor()),
        runtime_window.default_font_properties(),
    );
    runtime_window.draw_contents(|components| {
        for (component, origin) in components {
            i_slint_core::item_rendering::render_component_items(
                component,
                &mut prepare_scene,
                origin.clone(),
            );
        }
    });
    prepare_scene_profiler.stop_profiling(devices, "prepare_scene");
    Scene::new(prepare_scene.items, prepare_scene.rectangles, prepare_scene.textures)
}

struct PrepareScene {
    items: Vec<SceneItem>,
    rectangles: Vec<Color>,
    textures: Vec<SceneTexture>,
    state_stack: Vec<RenderState>,
    current_state: RenderState,
    scale_factor: ScaleFactor,
    default_font: FontRequest,
}

impl PrepareScene {
    fn new(size: PhysicalSize, scale_factor: ScaleFactor, default_font: FontRequest) -> Self {
        Self {
            items: vec![],
            rectangles: vec![],
            textures: vec![],
            state_stack: vec![],
            current_state: RenderState {
                alpha: 1.,
                offset: LogicalPoint::default(),
                clip: LogicalRect::new(LogicalPoint::default(), size.cast() / scale_factor),
            },
            scale_factor,
            default_font,
        }
    }

    fn should_draw(&self, rect: &LogicalRect) -> bool {
        !rect.size.is_empty()
            && self.current_state.alpha > 0.01
            && self.current_state.clip.intersects(rect)
    }

    fn new_scene_rectangle(&mut self, geometry: LogicalRect, color: Color) {
        self.rectangles.push(color);
        self.new_scene_item(geometry, SceneCommand::Rectangle, self.rectangles.len() - 1);
    }

    fn new_scene_texture(&mut self, geometry: LogicalRect, texture: SceneTexture) {
        self.textures.push(texture);
        self.new_scene_item(geometry, SceneCommand::Texture, self.textures.len() - 1);
    }

    fn new_scene_item(&mut self, geometry: LogicalRect, command: SceneCommand, data_index: usize) {
        let z = self.items.len() as u16;

        self.items.push(SceneItem {
            pos: ((geometry.origin + self.current_state.offset.to_vector()) * self.scale_factor)
                .cast(),
            size: (geometry.size * self.scale_factor).cast(),
            z,
            command,
            data_index,
        });
    }

    fn draw_image_impl(
        &mut self,
        geom: LogicalRect,
        source: &i_slint_core::graphics::Image,
        source_clip: IntRect,
        colorize: Color,
    ) {
        let image_inner: &ImageInner = source.into();
        match image_inner {
            ImageInner::None => (),
            ImageInner::AbsoluteFilePath(_) | ImageInner::EmbeddedData { .. } => {
                unimplemented!()
            }
            ImageInner::EmbeddedImage(_) => todo!(),
            ImageInner::StaticTextures { size, data, textures } => {
                let sx = geom.width() / (size.width as f32);
                let sy = geom.height() / (size.height as f32);
                for t in textures.as_slice() {
                    if let Some(dest_rect) = t.rect.intersection(&source_clip).and_then(|r| {
                        r.intersection(
                            &self
                                .current_state
                                .clip
                                .to_untyped()
                                .scale(1. / sx, 1. / sy)
                                .round_in()
                                .cast(),
                        )
                    }) {
                        let actual_x = dest_rect.origin.x - t.rect.origin.x;
                        let actual_y = dest_rect.origin.y - t.rect.origin.y;
                        let stride = t.rect.width() as u16 * bpp(t.format);
                        self.new_scene_texture(
                            LogicalRect::from_untyped(&dest_rect.cast().scale(sx, sy)),
                            SceneTexture {
                                data: &data.as_slice()[(t.index
                                    + (stride as usize) * (actual_y as usize)
                                    + (bpp(t.format) as usize) * (actual_x as usize))..],
                                stride,
                                source_size: PhysicalSize::from_untyped(dest_rect.size.cast()),
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
    offset: LogicalPoint,
    clip: LogicalRect,
}

impl i_slint_core::item_rendering::ItemRenderer for PrepareScene {
    fn draw_rectangle(&mut self, rect: Pin<&i_slint_core::items::Rectangle>) {
        let geom = LogicalRect::new(LogicalPoint::default(), rect.logical_geometry().size_length());
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
            self.new_scene_rectangle(geom, color);
        }
    }

    fn draw_border_rectangle(&mut self, rect: Pin<&i_slint_core::items::BorderRectangle>) {
        let geom = LogicalRect::new(LogicalPoint::default(), rect.logical_geometry().size_length());
        if self.should_draw(&geom) {
            let border = rect.border_width();
            // FIXME: gradients
            let color = rect.background().color();
            if color.alpha() > 0 {
                if let Some(r) =
                    geom.inflate(-border, -border).intersection(&self.current_state.clip)
                {
                    self.new_scene_rectangle(r, color);
                }
            }
            if border > 0.01 {
                // FIXME: radius
                // FIXME: gradients
                let border_color = rect.border_color().color();
                if border_color.alpha() > 0 {
                    let mut add_border = |r: LogicalRect| {
                        if let Some(r) = r.intersection(&self.current_state.clip) {
                            self.new_scene_rectangle(r, border_color);
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

    fn draw_image(&mut self, image: Pin<&i_slint_core::items::ImageItem>) {
        let geom =
            LogicalRect::new(LogicalPoint::default(), image.logical_geometry().size_length());
        if self.should_draw(&geom) {
            self.draw_image_impl(
                geom,
                &image.source(),
                euclid::rect(0, 0, i32::MAX, i32::MAX),
                Default::default(),
            );
        }
    }

    fn draw_clipped_image(&mut self, image: Pin<&i_slint_core::items::ClippedImage>) {
        // when the source_clip size is empty, make it full
        let a = |v| if v == 0 { i32::MAX } else { v };

        let geom =
            LogicalRect::new(LogicalPoint::default(), image.logical_geometry().size_length());
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

    fn draw_text(&mut self, text: Pin<&i_slint_core::items::Text>) {
        let font_request = text.unresolved_font_request().merge(&self.default_font);
        let (font, glyphs) = crate::fonts::match_font(&font_request, self.scale_factor);

        let color = text.color().color();

        let baseline_y = glyphs.ascent(font);

        for (glyph_baseline_x, glyph) in crate::fonts::glyphs_for_text(font, glyphs, &text.text()) {
            if let Some(dest_rect) = (PhysicalRect::new(
                PhysicalPoint::from_lengths(
                    glyph_baseline_x + glyph.x(),
                    baseline_y - glyph.y() - glyph.height(),
                ),
                glyph.size(),
            )
            .cast()
                / self.scale_factor)
                .intersection(&self.current_state.clip)
            {
                let stride = glyph.width().get() as u16;

                self.new_scene_texture(
                    dest_rect,
                    SceneTexture {
                        data: glyph.data().as_slice(),
                        stride,
                        source_size: glyph.size(),
                        format: PixelFormat::AlphaMap,
                        color,
                    },
                );
            }
        }
    }

    fn draw_text_input(&mut self, _text_input: Pin<&i_slint_core::items::TextInput>) {
        // TODO
    }

    #[cfg(feature = "std")]
    fn draw_path(&mut self, _path: Pin<&i_slint_core::items::Path>) {
        // TODO
    }

    fn draw_box_shadow(&mut self, _box_shadow: Pin<&i_slint_core::items::BoxShadow>) {
        // TODO
    }

    fn combine_clip(&mut self, other: RectF, _radius: f32, _border_width: f32) {
        match self.current_state.clip.intersection(&LogicalRect::from_untyped(&other)) {
            Some(r) => {
                self.current_state.clip = r;
            }
            None => {
                self.current_state.clip = LogicalRect::default();
            }
        };
        // TODO: handle radius and border
    }

    fn get_current_clip(&self) -> i_slint_core::graphics::Rect {
        self.current_state.clip.to_untyped()
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
        self.scale_factor.0
    }

    fn draw_cached_pixmap(
        &mut self,
        _item_cache: &i_slint_core::item_rendering::CachedRenderingData,
        _update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        todo!()
    }

    fn draw_string(&mut self, _string: &str, _color: Color) {
        todo!()
    }

    fn window(&self) -> i_slint_core::window::WindowRc {
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
