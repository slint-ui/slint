// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

mod draw_functions;

use crate::lengths::PhysicalPx;
use crate::{
    LogicalItemGeometry, LogicalLength, LogicalPoint, LogicalRect, PhysicalLength, PhysicalPoint,
    PhysicalRect, PhysicalSize, PointLengths, RectLengths, ScaleFactor, SizeLengths,
};
use alloc::rc::Rc;
use alloc::{vec, vec::Vec};
use core::pin::Pin;
pub use draw_functions::TargetPixel;
use embedded_graphics::pixelcolor::Rgb888;
use i_slint_core::graphics::{FontRequest, IntRect, PixelFormat, Rect as RectF};
use i_slint_core::item_rendering::{ItemRenderer, PartialRenderingCache};
use i_slint_core::items::ImageFit;
use i_slint_core::textlayout::{FontMetrics as _, TextParagraphLayout};
use i_slint_core::{Color, Coord, ImageInner, StaticTextures};

type DirtyRegion = PhysicalRect;

pub trait LineBufferProvider {
    /// Called before the frame is being drawn, with the dirty region. Return the actual dirty region
    ///
    /// The default implementation simply returns the dirty_region
    fn set_dirty_region(&mut self, dirty_region: PhysicalRect) -> PhysicalRect {
        dirty_region
    }

    /// Called once per line, you will have to call the render_fn back with the buffer.
    fn process_line(
        &mut self,
        line: PhysicalLength,
        render_fn: impl FnOnce(&mut [super::TargetPixel]),
    );
}

#[derive(Default)]
pub struct LineRenderer {
    partial_cache: i_slint_core::item_rendering::PartialRenderingCache,
}

impl LineRenderer {
    /// Render the window, line by line, into the buffer provided by the `line_buffer` function.
    ///
    /// The renderer uses a cache internally and will only render the part of the window
    /// which are dirty. The `initial_dirty_region` is an extra dirty regin which will also
    /// be rendered.
    ///
    /// TODO: the window should be the public slint::Window type
    /// TODO: what about async and threading.
    ///       (can we call the line_buffer function from different thread?)
    /// TODO: should `initial_dirty_region` be set from a different call?
    pub fn render(
        &mut self,
        window: Rc<i_slint_core::window::Window>,
        initial_dirty_region: i_slint_core::item_rendering::DirtyRegion,
        line_buffer: impl LineBufferProvider,
    ) {
        let component_rc = window.component();
        let component = i_slint_core::component::ComponentRc::borrow_pin(&component_rc);
        if let Some(window_item) = i_slint_core::items::ItemRef::downcast_pin::<
            i_slint_core::items::WindowItem,
        >(component.as_ref().get_item_ref(0))
        {
            let size = euclid::size2(window_item.width() as f32, window_item.height() as f32)
                * ScaleFactor::new(window.scale_factor());
            let background = to_rgb888_color_discard_alpha(window_item.background());
            render_window_frame(
                window,
                background.into(),
                size.cast(),
                initial_dirty_region,
                &mut self.partial_cache,
                line_buffer,
            );
        }
    }

    pub fn free_graphics_resources(
        &mut self,
        items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'_>>>,
    ) {
        for item in items {
            let cache_entry = item.cached_rendering_data_offset().release(&mut self.partial_cache);
            drop(cache_entry);
        }
    }
}

fn render_window_frame(
    runtime_window: Rc<i_slint_core::window::Window>,
    background: super::TargetPixel,
    size: PhysicalSize,
    initial_dirty_region: i_slint_core::item_rendering::DirtyRegion,
    cache: &mut PartialRenderingCache,
    mut line_buffer: impl LineBufferProvider,
) {
    let mut scene =
        prepare_scene(runtime_window, size, initial_dirty_region, &mut line_buffer, cache);

    let dirty_region = scene.dirty_region;

    debug_assert!(scene.current_line >= dirty_region.origin.y_length());
    while scene.current_line < dirty_region.origin.y_length() + dirty_region.size.height_length() {
        line_buffer.process_line(scene.current_line, |line_buffer| {
            line_buffer[dirty_region.min_x() as usize..dirty_region.max_x() as usize]
                .fill(background);
            for span in scene.items[0..scene.current_items_index].iter().rev() {
                debug_assert!(scene.current_line >= span.pos.y_length());
                debug_assert!(scene.current_line < span.pos.y_length() + span.size.height_length(),);
                match span.command {
                    SceneCommand::Rectangle { color } => {
                        TargetPixel::blend_buffer(
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
                             line_buffer,
                        );
                    }
                    SceneCommand::RoundedRectangle { rectangle_index } => {
                        let rr = &scene.rounded_rectangles[rectangle_index as usize];
                        draw_functions::draw_rounded_rectangle_line(
                            span,
                            scene.current_line,
                            rr,
                             line_buffer,
                        );
                    }
                }
            }
        });

        if scene.current_line < dirty_region.origin.y_length() + dirty_region.size.height_length() {
            scene.next_line();
        }
    }
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

#[derive(Debug)]
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
    initial_dirty_region: i_slint_core::item_rendering::DirtyRegion,
    line_buffer: &mut impl LineBufferProvider,
    cache: &mut PartialRenderingCache,
) -> Scene {
    let factor = ScaleFactor::new(runtime_window.scale_factor());
    let prepare_scene = PrepareScene::new(size, factor, runtime_window.default_font_properties());
    let mut renderer = i_slint_core::item_rendering::PartialRenderer::new(
        cache,
        initial_dirty_region,
        prepare_scene,
    );

    let mut dirty_region = PhysicalRect::default();
    runtime_window.draw_contents(|components| {
        for (component, origin) in components {
            renderer.compute_dirty_regions(component, *origin);
        }

        dirty_region = (LogicalRect::from_untyped(&renderer.dirty_region.to_rect()).cast()
            * factor)
            .round_out()
            .cast()
            .intersection(&PhysicalRect { origin: euclid::point2(0, 0), size })
            .unwrap_or_default();
        dirty_region = line_buffer.set_dirty_region(dirty_region);

        renderer.combine_clip((dirty_region.cast() / factor).to_untyped().cast(), 0 as _, 0 as _);
        for (component, origin) in components {
            i_slint_core::item_rendering::render_component_items(component, &mut renderer, *origin);
        }
    });

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
                clip: LogicalRect::new(
                    LogicalPoint::default(),
                    (size.cast() / scale_factor).cast(),
                ),
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

    fn new_scene_texture(&mut self, geometry: PhysicalRect, texture: SceneTexture) {
        let size = geometry.size;
        if !size.is_empty() {
            let texture_index = self.textures.len() as u16;
            self.textures.push(texture);
            self.items.push(SceneItem {
                pos: geometry.origin,
                size,
                z: self.items.len() as u16,
                command: SceneCommand::Texture { texture_index },
            });
        }
    }

    fn new_scene_item(&mut self, geometry: LogicalRect, command: SceneCommand) {
        let geometry = (geometry.translate(self.current_state.offset.to_vector()).cast()
            * self.scale_factor)
            .round()
            .cast();
        let size = geometry.size;
        if !size.is_empty() {
            let z = self.items.len() as u16;
            let pos = geometry.origin;
            self.items.push(SceneItem { pos, size, z, command });
        }
    }

    fn draw_image_impl(
        &mut self,
        geom: LogicalRect,
        source: &i_slint_core::graphics::Image,
        mut source_clip: IntRect,
        image_fit: ImageFit,
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
                let phys_size = geom.size_length().cast() * self.scale_factor;
                let sx = phys_size.width / (size.width as f32);
                let sy = phys_size.height / (size.height as f32);
                let mut image_fit_offset = euclid::Vector2D::default();
                let (sx, sy) = match image_fit {
                    ImageFit::fill => (sx, sy),
                    ImageFit::cover => {
                        let ratio = f32::max(sx, sy);
                        if size.width as f32 > phys_size.width / ratio {
                            let diff = (size.width as f32 - phys_size.width / ratio) as i32;
                            source_clip.origin.x += diff / 2;
                            source_clip.size.width -= diff;
                        }
                        if size.height as f32 > phys_size.height / ratio {
                            let diff = (size.height as f32 - phys_size.height / ratio) as i32;
                            source_clip.origin.y += diff / 2;
                            source_clip.size.height -= diff;
                        }
                        (ratio, ratio)
                    }
                    ImageFit::contain => {
                        let ratio = f32::min(sx, sy);
                        if (size.width as f32) < phys_size.width / ratio {
                            image_fit_offset.x = (phys_size.width - size.width as f32 * ratio) / 2.;
                        }
                        if (size.height as f32) < phys_size.height / ratio {
                            image_fit_offset.y =
                                (phys_size.height - size.height as f32 * ratio) / 2.;
                        }
                        (ratio, ratio)
                    }
                };

                let offset = self.current_state.offset.to_vector().cast() * self.scale_factor
                    + image_fit_offset;
                let scaled_clip =
                    (self.current_state.clip.cast() * self.scale_factor).scale(1. / sx, 1. / sy);
                for t in textures.as_slice() {
                    if let Some(clipped_src) = t.rect.intersection(&source_clip).and_then(|r| {
                        euclid::Rect::<_, PhysicalPx>::from_untyped(&r.cast())
                            .intersection(&scaled_clip)
                    }) {
                        let geometry = clipped_src.scale(sx, sy).translate(offset).round();
                        let actual_x = clipped_src.origin.x as usize - t.rect.origin.x as usize;
                        let actual_y = clipped_src.origin.y as usize - t.rect.origin.y as usize;
                        let stride = t.rect.width() as u16 * bpp(t.format);
                        self.new_scene_texture(
                            geometry.cast(),
                            SceneTexture {
                                data: &data.as_slice()[(t.index
                                    + (stride as usize) * actual_y
                                    + (bpp(t.format) as usize) * actual_x)..],
                                stride,
                                source_size: clipped_src.size.ceil().cast(),
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
            self.new_scene_item(geom, SceneCommand::Rectangle { color: color });
        }
    }

    fn draw_border_rectangle(&mut self, rect: Pin<&i_slint_core::items::BorderRectangle>) {
        let geom = LogicalRect::new(LogicalPoint::default(), rect.logical_geometry().size_length());
        if self.should_draw(&geom) {
            let border = rect.border_width();
            let radius = rect.border_radius();
            // FIXME: gradients
            let color = rect.background().color();
            if radius > 0 as _ {
                let radius = LogicalLength::new(radius)
                    .min(geom.width_length() / 2 as Coord)
                    .min(geom.height_length() / 2 as Coord);
                if let Some(clipped) = geom.intersection(&self.current_state.clip) {
                    let geom2 = geom.cast() * self.scale_factor;
                    let clipped2 = clipped.cast() * self.scale_factor;
                    let rectangle_index = self.rounded_rectangles.len() as u16;
                    // Add a small value to make sure that the clip is always positive despite floating point shenanigans
                    const E: f32 = 0.00001;
                    self.rounded_rectangles.push(RoundedRectangle {
                        radius: (radius.cast() * self.scale_factor).cast(),
                        width: (LogicalLength::new(border).cast() * self.scale_factor).cast(),
                        border_color: rect.border_color().color(),
                        inner_color: color,
                        top_clip: PhysicalLength::new((clipped2.min_y() - geom2.min_y() + E) as _),
                        bottom_clip: PhysicalLength::new(
                            (geom2.max_y() - clipped2.max_y() + E) as _,
                        ),
                        left_clip: PhysicalLength::new((clipped2.min_x() - geom2.min_x() + E) as _),
                        right_clip: PhysicalLength::new(
                            (geom2.max_x() - clipped2.max_x() + E) as _,
                        ),
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
                    self.new_scene_item(r, SceneCommand::Rectangle { color });
                }
            }
            if border > 0.01 as Coord {
                // FIXME: radius
                // FIXME: gradients
                let border_color = rect.border_color().color();
                if border_color.alpha() > 0 {
                    let mut add_border = |r: LogicalRect| {
                        if let Some(r) = r.intersection(&self.current_state.clip) {
                            self.new_scene_item(r, SceneCommand::Rectangle { color: border_color });
                        }
                    };
                    let b = border;
                    add_border(euclid::rect(0 as _, 0 as _, geom.width(), b));
                    add_border(euclid::rect(0 as _, geom.height() - b, geom.width(), b));
                    add_border(euclid::rect(0 as _, b, b, geom.height() - b - b));
                    add_border(euclid::rect(geom.width() - b, b, b, geom.height() - b - b));
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
                image.image_fit(),
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
                image.image_fit(),
                image.colorize().color(),
            );
        }
    }

    fn draw_text(&mut self, text: Pin<&i_slint_core::items::Text>) {
        let string = text.text();
        if string.trim().is_empty() {
            return;
        }
        let geom = LogicalRect::new(LogicalPoint::default(), text.logical_geometry().size_length());
        if !self.should_draw(&geom) {
            return;
        }

        let font_request = text.unresolved_font_request().merge(&self.default_font);
        let font = crate::fonts::match_font(&font_request, self.scale_factor);
        let layout = crate::fonts::text_layout_for_font(&font, &font_request, self.scale_factor);

        let color = text.color().color();
        let max_size = (geom.size.cast() * self.scale_factor).cast();

        let paragraph = TextParagraphLayout {
            string: &string,
            layout,
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
            while let Some(positioned_glyph) = glyphs.next() {
                let src_rect = PhysicalRect::new(
                    PhysicalPoint::from_lengths(
                        line_x + positioned_glyph.x + positioned_glyph.platform_glyph.x(),
                        baseline_y
                            - positioned_glyph.platform_glyph.y()
                            - positioned_glyph.platform_glyph.height(),
                    ),
                    positioned_glyph.platform_glyph.size(),
                )
                .cast();

                if let Some(clipped_src) =
                    src_rect.intersection(&(self.current_state.clip.cast() * self.scale_factor))
                {
                    let offset = self.current_state.offset.to_vector().cast() * self.scale_factor;
                    let geometry = clipped_src.translate(offset).round();
                    let origin = (geometry.origin - offset.round()).cast::<usize>();
                    let actual_x = origin.x - src_rect.origin.x as usize;
                    let actual_y = origin.y - src_rect.origin.y as usize;
                    let stride = positioned_glyph.platform_glyph.width().get() as u16;
                    let geometry = geometry.cast();
                    self.new_scene_texture(
                        geometry,
                        SceneTexture {
                            data: &positioned_glyph.platform_glyph.data().as_slice()
                                [actual_x + actual_y * stride as usize..],
                            stride,
                            source_size: geometry.size,
                            format: PixelFormat::AlphaMap,
                            color,
                        },
                    );
                }
            }
        });
    }

    fn draw_text_input(&mut self, text_input: Pin<&i_slint_core::items::TextInput>) {
        text_input.logical_geometry();
        // TODO
    }

    #[cfg(feature = "std")]
    fn draw_path(&mut self, path: Pin<&i_slint_core::items::Path>) {
        path.logical_geometry();
        // TODO
    }

    fn draw_box_shadow(&mut self, box_shadow: Pin<&i_slint_core::items::BoxShadow>) {
        box_shadow.logical_geometry();
        // TODO
    }

    fn combine_clip(&mut self, other: RectF, _radius: Coord, _border_width: Coord) {
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

    fn translate(&mut self, x: Coord, y: Coord) {
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

pub fn to_rgb888_color_discard_alpha(col: Color) -> Rgb888 {
    Rgb888::new(col.red(), col.green(), col.blue())
}
