// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

mod draw_functions;

use crate::lengths::LogicalSize;
use crate::{
    profiler, Devices, LogicalLength, LogicalPoint, LogicalRect, PhysicalLength, PhysicalPoint,
    PhysicalRect, PhysicalSize, PointLengths, ScaleFactor, SizeLengths,
};
use alloc::rc::Rc;
use alloc::{vec, vec::Vec};
use core::pin::Pin;
use embedded_graphics::pixelcolor::Rgb888;
use i_slint_core::graphics::{FontRequest, IntRect, PixelFormat, Rect as RectF};
use i_slint_core::item_rendering::PartialRenderingCache;
use i_slint_core::textlayout::TextParagraphLayout;
use i_slint_core::{Color, ImageInner, StaticTextures};

type DirtyRegion = PhysicalRect;

pub fn render_window_frame(
    runtime_window: Rc<i_slint_core::window::Window>,
    background: Rgb888,
    devices: &mut dyn Devices,
    cache: &mut PartialRenderingCache,
) {
    let size = devices.screen_size();
    let mut scene = prepare_scene(runtime_window, size, devices, cache);

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
    let dirty_region = scene.dirty_region;

    debug_assert!(scene.current_line >= dirty_region.origin.y_length());
    while scene.current_line < dirty_region.origin.y_length() + dirty_region.size.height_length() {
        line_buffer.fill(background);
        span_drawing_profiler.start(devices);
        for span in scene.items[0..scene.current_items_index].iter().rev() {
            debug_assert!(scene.current_line >= span.pos.y_length());
            debug_assert!(scene.current_line < span.pos.y_length() + span.size.height_length(),);
            match span.command {
                SceneCommand::Rectangle { color } => {
                    draw_functions::blend_buffer(
                        &mut line_buffer[span.pos.x as usize
                            ..(span.pos.x_length() + span.size.width_length()).get() as usize],
                        color,
                    );
                }
                SceneCommand::Texture { texture_index } => {
                    let texture = &scene.textures[texture_index as usize];
                    draw_functions::draw_texture_line(
                        span,
                        scene.current_line,
                        texture,
                        &mut line_buffer,
                    );
                }
                SceneCommand::RoundedRectangle { rectangle_index } => {
                    let rr = &scene.rounded_rectangles[rectangle_index as usize];
                    draw_functions::draw_rounded_rectangle_line(
                        span,
                        scene.current_line,
                        rr,
                        &mut line_buffer,
                    );
                }
            }
        }
        span_drawing_profiler.stop(devices);
        screen_fill_profiler.start(devices);
        devices.fill_region(
            euclid::rect(
                dirty_region.origin.x,
                scene.current_line.get() as i16,
                dirty_region.size.width,
                1,
            ),
            &line_buffer[dirty_region.origin.x as usize
                ..(dirty_region.origin.x + dirty_region.size.width) as usize],
        );
        screen_fill_profiler.stop(devices);
        line_processing_profiler.start(devices);
        if scene.current_line < dirty_region.origin.y_length() + dirty_region.size.height_length() {
            scene.next_line();
        }
        line_processing_profiler.stop(devices);
    }

    line_processing_profiler.stop_profiling(devices, "line processing");
    span_drawing_profiler.stop_profiling(devices, "span drawing");
    screen_fill_profiler.stop_profiling(devices, "screen fill");
}

struct Scene {
    /// the next line to be processed
    current_line: PhysicalLength,

    /// The items are sorted like so:
    /// - `items[future_items_index..]` are the items that have `y > current_line`.
    ///   They must be sorted by `y` (top to bottom), then by `z` (front to back)
    /// - `items[..current_items_index]` are the items that overlap with the current_line,
    ///   sorted by z (front to back)
    items: Vec<SceneItem>,

    future_items_index: usize,
    current_items_index: usize,

    textures: Vec<SceneTexture>,
    rounded_rectangles: Vec<RoundedRectangle>,
    dirty_region: DirtyRegion,
}

impl Scene {
    pub fn new(
        mut items: Vec<SceneItem>,
        textures: Vec<SceneTexture>,
        rounded_rectangles: Vec<RoundedRectangle>,
        dirty_region: DirtyRegion,
    ) -> Self {
        let current_line = dirty_region.origin.y_length();
        items.retain(|i| i.pos.y_length() + i.size.height_length() > current_line);
        items.sort_unstable_by(|a, b| compare_scene_item(a, b));
        let current_items_index = items.partition_point(|i| i.pos.y_length() <= current_line);
        items[..current_items_index].sort_unstable_by(|a, b| b.z.cmp(&a.z));
        Self {
            items,
            current_line,
            current_items_index,
            future_items_index: current_items_index,
            textures,
            rounded_rectangles,
            dirty_region,
        }
    }

    /// Updates `current_items_index` and `future_items_index` to match the invariant
    pub fn next_line(&mut self) {
        self.current_line += PhysicalLength::new(1);

        // The items array is split in part:
        // 1. [0..i] are the items that have already been processed, that are on this line
        // 2. [j..current_items_index] are the items from the previous line that might still be
        //   valid on this line
        // 3. [tmp1, tmp2] is a buffer where we swap items so we can make room for the items in [0..i]
        // 4. [future_items_index..] are the items which might get processed now
        // 5. [current_items_index..tmp1], [tmp2..future_items_index] and [i..j] is garbage
        //
        // At each step, we selecting the item with the higher z from the list 2 or 3 or 4 and take it from
        // that list. Then we add it to the list [0..i] if it needs more processing. If needed,
        // we move the first  item from list  2. to list 3. to make some room

        let (mut i, mut j, mut tmp1, mut tmp2) =
            (0, 0, self.current_items_index, self.current_items_index);

        'outer: loop {
            let future_next_z = self
                .items
                .get(self.future_items_index)
                .filter(|i| i.pos.y_length() <= self.current_line)
                .map(|i| i.z);
            let item = loop {
                if tmp1 != tmp2 {
                    if future_next_z.map_or(true, |z| self.items[tmp1].z > z) {
                        let idx = tmp1;
                        tmp1 += 1;
                        if tmp1 == tmp2 {
                            tmp1 = self.current_items_index;
                            tmp2 = self.current_items_index;
                        }
                        break self.items[idx];
                    }
                } else if j < self.current_items_index {
                    let item = &self.items[j];
                    if item.pos.y_length() + item.size.height_length() <= self.current_line {
                        j += 1;
                        continue;
                    }
                    if future_next_z.map_or(true, |z| item.z > z) {
                        j += 1;
                        break *item;
                    }
                }
                if future_next_z.is_some() {
                    self.future_items_index += 1;
                    break self.items[self.future_items_index - 1];
                }
                break 'outer;
            };
            if i != j {
                // there is room
            } else if j >= self.current_items_index && tmp1 == tmp2 {
                // the current_items list is empty
                j += 1
            } else if self.items[j].pos.y_length() + self.items[j].size.height_length()
                <= self.current_line
            {
                // next item in the current_items array is no longer in this line
                j += 1;
            } else if tmp2 < self.future_items_index && j < self.current_items_index {
                // move the next item in current_items
                let to_move = self.items[j];
                self.items[tmp2] = to_move;
                j += 1;
                tmp2 += 1;
            } else {
                debug_assert!(tmp1 >= self.current_items_index);
                let sort_begin = i;
                // merge sort doesn't work because we don't have enough tmp space, just bring all items and use a normal sort.
                while j < self.current_items_index {
                    let item = self.items[j];
                    if item.pos.y_length() + item.size.height_length() > self.current_line {
                        self.items[i] = item;
                        i += 1;
                    }
                    j += 1;
                }
                self.items.copy_within(tmp1..tmp2, i);
                i += tmp2 - tmp1;
                debug_assert!(i < self.future_items_index);
                self.items[i] = item;
                i += 1;
                while self.future_items_index < self.items.len() {
                    let item = self.items[self.future_items_index];
                    if item.pos.y_length() > self.current_line {
                        break;
                    }
                    self.items[i] = item;
                    i += 1;
                    self.future_items_index += 1;
                }
                self.items[sort_begin..i].sort_unstable_by(|a, b| b.z.cmp(&a.z));
                break;
            }
            self.items[i] = item;
            i += 1;
        }
        self.current_items_index = i;
        // check that current items are properly sorted
        debug_assert!(self.items[0..self.current_items_index].windows(2).all(|x| x[0].z >= x[1].z));
    }
}

#[derive(Clone, Copy, Debug)]
struct SceneItem {
    pos: PhysicalPoint,
    size: PhysicalSize,
    // this is the order of the item from which it is in the item tree
    z: u16,
    command: SceneCommand,
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

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum SceneCommand {
    Rectangle {
        color: Color,
    },
    /// texture_index is an index in the Scene::textures array
    Texture {
        texture_index: u16,
    },
    /// rectangle_index is an index in the Scene::rounded_rectangle array
    RoundedRectangle {
        rectangle_index: u16,
    },
}

struct SceneTexture {
    data: &'static [u8],
    format: PixelFormat,
    /// bytes between two lines in the source
    stride: u16,
    source_size: PhysicalSize,
    color: Color,
}

struct RoundedRectangle {
    radius: PhysicalLength,
    /// the border's width
    width: PhysicalLength,
    border_color: Color,
    inner_color: Color,
    /// The clips is the amount of pixels of the rounded rectangle that is clipped away.
    /// For example, if left_clip > width, then the left border will not be visible, and
    /// if left_clip > radius, then no radius will be seen in the left side
    left_clip: PhysicalLength,
    right_clip: PhysicalLength,
    top_clip: PhysicalLength,
    bottom_clip: PhysicalLength,
}

fn prepare_scene(
    runtime_window: Rc<i_slint_core::window::Window>,
    size: PhysicalSize,
    devices: &dyn Devices,
    cache: &mut PartialRenderingCache,
) -> Scene {
    let prepare_scene_profiler = profiler::Timer::new(devices);
    let mut compute_dirty_region_profiler = profiler::Timer::new_stopped();
    let factor = ScaleFactor::new(runtime_window.scale_factor());
    let prepare_scene = PrepareScene::new(size, factor, runtime_window.default_font_properties());
    let mut renderer = i_slint_core::item_rendering::PartialRenderer::new(cache, prepare_scene);

    runtime_window.draw_contents(|components| {
        compute_dirty_region_profiler.start(devices);
        for (component, origin) in components {
            renderer.compute_dirty_regions(component, *origin);
        }
        compute_dirty_region_profiler.stop(devices);
        for (component, origin) in components {
            i_slint_core::item_rendering::render_component_items(component, &mut renderer, *origin);
        }
    });
    let dirty_region = (euclid::Rect::from_untyped(&renderer.dirty_region.to_rect()) * factor)
        .round_out()
        .cast()
        .intersection(&PhysicalRect { origin: euclid::point2(0, 0), size })
        .unwrap_or_default();
    prepare_scene_profiler.stop_profiling(devices, "prepare_scene");
    compute_dirty_region_profiler.stop_profiling(devices, "+    dirty_region");
    let prepare_scene = renderer.into_inner();
    Scene::new(
        prepare_scene.items,
        prepare_scene.textures,
        prepare_scene.rounded_rectangles,
        dirty_region,
    )
}

struct PrepareScene {
    items: Vec<SceneItem>,
    textures: Vec<SceneTexture>,
    rounded_rectangles: Vec<RoundedRectangle>,
    state_stack: Vec<RenderState>,
    current_state: RenderState,
    scale_factor: ScaleFactor,
    default_font: FontRequest,
}

impl PrepareScene {
    fn new(size: PhysicalSize, scale_factor: ScaleFactor, default_font: FontRequest) -> Self {
        Self {
            items: vec![],
            rounded_rectangles: vec![],
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
        self.new_scene_item(geometry, SceneCommand::Rectangle { color });
    }

    fn new_scene_texture(&mut self, geometry: LogicalRect, texture: SceneTexture) {
        let texture_index = self.textures.len() as u16;
        self.textures.push(texture);
        self.new_scene_item(geometry, SceneCommand::Texture { texture_index });
    }

    fn new_scene_item(&mut self, geometry: LogicalRect, command: SceneCommand) {
        let size = (geometry.size * self.scale_factor).cast();
        if !size.is_empty() {
            let z = self.items.len() as u16;
            let pos = ((geometry.origin + self.current_state.offset.to_vector())
                * self.scale_factor)
                .cast();
            self.items.push(SceneItem { pos, size, z, command });
        }
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
            ImageInner::StaticTextures(StaticTextures { size, data, textures, .. }) => {
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
        let geom = LogicalRect::new(
            LogicalPoint::default(),
            LogicalSize::new(rect.width(), rect.height()),
        );
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
        let geom = LogicalRect::new(
            LogicalPoint::default(),
            LogicalSize::new(rect.width(), rect.height()),
        );
        if self.should_draw(&geom) {
            let border = rect.border_width();
            let radius = rect.border_radius();
            // FIXME: gradients
            let color = rect.background().color();
            if radius > 0. {
                if let Some(clipped) = geom.intersection(&self.current_state.clip) {
                    let geom2 = (geom * self.scale_factor).cast::<i16>();
                    let clipped2 = (clipped * self.scale_factor).cast::<i16>();
                    let rectangle_index = self.rounded_rectangles.len() as u16;
                    self.rounded_rectangles.push(RoundedRectangle {
                        radius: (LogicalLength::new(radius) * self.scale_factor).cast(),
                        width: (LogicalLength::new(border) * self.scale_factor).cast(),
                        border_color: rect.border_color().color(),
                        inner_color: color,
                        top_clip: PhysicalLength::new(clipped2.min_y() - geom2.min_y()),
                        bottom_clip: PhysicalLength::new(geom2.max_y() - clipped2.max_y()),
                        left_clip: PhysicalLength::new(clipped2.min_x() - geom2.min_x()),
                        right_clip: PhysicalLength::new(geom2.max_x() - clipped2.max_x()),
                    });
                    self.new_scene_item(
                        clipped,
                        SceneCommand::RoundedRectangle { rectangle_index },
                    );
                }
                return;
            }

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
        let geom = LogicalRect::new(
            LogicalPoint::default(),
            LogicalSize::new(image.width(), image.height()),
        );
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

        let geom = LogicalRect::new(
            LogicalPoint::default(),
            LogicalSize::new(image.width(), image.height()),
        );
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
        let geom = LogicalRect::new(
            LogicalPoint::default(),
            LogicalSize::new(text.width(), text.height()),
        );
        if !self.should_draw(&geom) {
            return;
        }

        let font_request = text.unresolved_font_request().merge(&self.default_font);
        let font = crate::fonts::match_font(&font_request, self.scale_factor);

        let color = text.color().color();
        let max_size = (geom.size * self.scale_factor).cast();

        let paragraph = TextParagraphLayout {
            string: &text.text(),
            font: &font,
            font_height: font.height(),
            max_width: max_size.width_length(),
            max_height: max_size.height_length(),
            horizontal_alignment: text.horizontal_alignment(),
            vertical_alignment: text.vertical_alignment(),
            wrap: text.wrap(),
            overflow: text.overflow(),
            single_line: false,
        };

        paragraph.layout_lines(|glyphs, line_x, line_y| {
            let baseline_y = line_y + font.ascent();
            while let Some((glyph_baseline_x, glyph)) = glyphs.next() {
                let bitmap_glyph = match glyph {
                    Some(g) => g,
                    None => continue,
                };
                if let Some(dest_rect) = (PhysicalRect::new(
                    PhysicalPoint::from_lengths(
                        line_x + glyph_baseline_x + bitmap_glyph.x(),
                        baseline_y - bitmap_glyph.y() - bitmap_glyph.height(),
                    ),
                    bitmap_glyph.size(),
                )
                .cast()
                    / self.scale_factor)
                    .intersection(&self.current_state.clip)
                {
                    let stride = bitmap_glyph.width().get() as u16;

                    self.new_scene_texture(
                        dest_rect,
                        SceneTexture {
                            data: bitmap_glyph.data().as_slice(),
                            stride,
                            source_size: bitmap_glyph.size(),
                            format: PixelFormat::AlphaMap,
                            color,
                        },
                    );
                }
            }
        });
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
