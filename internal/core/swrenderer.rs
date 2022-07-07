// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

mod draw_functions;
pub mod fonts;

use crate::graphics::{FontRequest, IntRect, PixelFormat, Rect as RectF};
use crate::item_rendering::{ItemRenderer, PartialRenderingCache};
use crate::items::{ImageFit, ItemRc};
use crate::lengths::{
    LogicalItemGeometry, LogicalLength, LogicalPoint, LogicalRect, PhysicalLength, PhysicalPoint,
    PhysicalPx, PhysicalRect, PhysicalSize, PointLengths, RectLengths, ScaleFactor, SizeLengths,
};
use crate::textlayout::{FontMetrics as _, TextParagraphLayout};
use crate::{Color, Coord, ImageInner, StaticTextures};
use alloc::rc::Rc;
use alloc::{vec, vec::Vec};
use core::cell::{Cell, RefCell};
use core::pin::Pin;
pub use draw_functions::TargetPixel;

pub type DirtyRegion = PhysicalRect;

pub trait LineBufferProvider {
    /// The pixel type of the buffer
    type TargetPixel: TargetPixel;

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
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    );
}

#[derive(Default)]
pub struct SoftwareRenderer {
    partial_cache: RefCell<crate::item_rendering::PartialRenderingCache>,
    last_scene_capacities: Cell<SceneCapacities>,
}

impl SoftwareRenderer {
    /// Render the window to the given frame buffer.
    ///
    /// returns the dirty region for this frame (not including the initial_dirty_region)
    pub fn render(
        &self,
        window: Rc<crate::window::Window>,
        initial_dirty_region: DirtyRegion,
        buffer: &mut [impl TargetPixel],
        buffer_stride: PhysicalLength,
    ) -> DirtyRegion {
        let component_rc = window.component();
        let component = crate::component::ComponentRc::borrow_pin(&component_rc);
        let factor = ScaleFactor::new(window.scale_factor());
        let (size, background) = if let Some(window_item) =
            crate::items::ItemRef::downcast_pin::<crate::items::WindowItem>(
                component.as_ref().get_item_ref(0),
            ) {
            (
                (euclid::size2(window_item.width() as f32, window_item.height() as f32) * factor)
                    .cast(),
                window_item.background(),
            )
        } else {
            (
                euclid::size2(
                    buffer_stride.get(),
                    (buffer.len() / (buffer_stride.get() as usize)) as _,
                ),
                Color::default(),
            )
        };
        let buffer_renderer = SceneBuilder::new(
            size,
            factor,
            window.default_font_properties(),
            RenderToBuffer { buffer, stride: buffer_stride },
        );
        let mut renderer = crate::item_rendering::PartialRenderer::new(
            &self.partial_cache,
            Default::default(),
            buffer_renderer,
        );

        let mut dirty_region = PhysicalRect::default();
        window.draw_contents(|components| {
            for (component, origin) in components {
                renderer.compute_dirty_regions(component, *origin);
            }

            dirty_region = (LogicalRect::from_untyped(&renderer.dirty_region.to_rect()).cast()
                * factor)
                .round_out()
                .cast();

            let to_draw = dirty_region
                .union(&initial_dirty_region)
                .intersection(&PhysicalRect { origin: euclid::point2(0, 0), size })
                .unwrap_or_default();
            renderer.combine_clip((to_draw.cast() / factor).to_untyped().cast(), 0 as _, 0 as _);

            if background.alpha() != 0 {
                renderer.actual_renderer.processor.process_rectangle(to_draw, background);
            }
            for (component, origin) in components {
                crate::item_rendering::render_component_items(component, &mut renderer, *origin);
            }
        });
        dirty_region
    }

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
    pub fn render_by_line(
        &self,
        window: Rc<crate::window::Window>,
        initial_dirty_region: crate::item_rendering::DirtyRegion,
        mut line_buffer: impl LineBufferProvider,
    ) {
        let component_rc = window.component();
        let component = crate::component::ComponentRc::borrow_pin(&component_rc);
        if let Some(window_item) = crate::items::ItemRef::downcast_pin::<crate::items::WindowItem>(
            component.as_ref().get_item_ref(0),
        ) {
            let size = euclid::size2(window_item.width() as f32, window_item.height() as f32)
                * ScaleFactor::new(window.scale_factor());
            let mut scene = prepare_scene(
                window,
                size.cast(),
                initial_dirty_region,
                &mut line_buffer,
                &self.partial_cache,
                self.last_scene_capacities.get(),
            );

            render_window_frame_by_line(window_item.background(), line_buffer, &mut scene);

            self.last_scene_capacities.set(scene.capacities());
        } else {
            Default::default()
        }
    }

    pub fn free_graphics_resources(
        &self,
        items: &mut dyn Iterator<Item = Pin<crate::items::ItemRef<'_>>>,
    ) {
        for item in items {
            let cache_entry =
                item.cached_rendering_data_offset().release(&mut self.partial_cache.borrow_mut());
            drop(cache_entry);
        }
    }
}

fn render_window_frame_by_line(
    background: Color,
    mut line_buffer: impl LineBufferProvider,
    scene: &mut Scene,
) {
    let dirty_region = scene.dirty_region;

    debug_assert!(scene.current_line >= dirty_region.origin.y_length());
    while scene.current_line < dirty_region.origin.y_length() + dirty_region.size.height_length() {
        line_buffer.process_line(scene.current_line, |line_buffer| {
            TargetPixel::blend_buffer(&mut line_buffer[dirty_region.min_x() as usize..dirty_region.max_x() as usize] , background);
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
                            &PhysicalRect{ origin: span.pos, size: span.size }  ,
                            scene.current_line,
                            texture,
                            line_buffer,
                        );
                    }
                    SceneCommand::RoundedRectangle { rectangle_index } => {
                        let rr = &scene.rounded_rectangles[rectangle_index as usize];
                        draw_functions::draw_rounded_rectangle_line(
                            &PhysicalRect{ origin: span.pos, size: span.size } ,
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

    fn capacities(&self) -> SceneCapacities {
        SceneCapacities {
            num_items: self.items.len(),
            num_textures: self.textures.len(),
            num_rounded_rectangles: self.rounded_rectangles.len(),
        }
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
    runtime_window: Rc<crate::window::Window>,
    size: PhysicalSize,
    initial_dirty_region: crate::item_rendering::DirtyRegion,
    line_buffer: &mut impl LineBufferProvider,
    cache: &RefCell<PartialRenderingCache>,
    scene_capacities: SceneCapacities,
) -> Scene {
    let factor = ScaleFactor::new(runtime_window.scale_factor());
    let prepare_scene = SceneBuilder::new(
        size,
        factor,
        runtime_window.default_font_properties(),
        PrepareScene::with_capacities(scene_capacities),
    );
    let mut renderer =
        crate::item_rendering::PartialRenderer::new(cache, initial_dirty_region, prepare_scene);

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
            crate::item_rendering::render_component_items(component, &mut renderer, *origin);
        }
    });

    let prepare_scene = renderer.into_inner();
    Scene::new(
        prepare_scene.processor.items,
        prepare_scene.processor.textures,
        prepare_scene.processor.rounded_rectangles,
        dirty_region,
    )
}

trait ProcessScene {
    fn process_texture(&mut self, geometry: PhysicalRect, texture: SceneTexture);
    fn process_rectangle(&mut self, geometry: PhysicalRect, color: Color);
    fn process_rounded_rectangle(&mut self, geometry: PhysicalRect, data: RoundedRectangle);
}

struct RenderToBuffer<'a, TargetPixel> {
    buffer: &'a mut [TargetPixel],
    stride: PhysicalLength,
}

impl<'a, T: TargetPixel> ProcessScene for RenderToBuffer<'a, T> {
    fn process_texture(&mut self, geometry: PhysicalRect, texture: SceneTexture) {
        for line in geometry.min_y()..geometry.max_y() {
            draw_functions::draw_texture_line(
                &geometry,
                PhysicalLength::new(line),
                &texture,
                &mut self.buffer[line as usize * self.stride.get() as usize..],
            );
        }
    }

    fn process_rectangle(&mut self, geometry: PhysicalRect, color: Color) {
        for line in geometry.min_y()..geometry.max_y() {
            let begin = line as usize * self.stride.get() as usize + geometry.origin.x as usize;
            TargetPixel::blend_buffer(
                &mut self.buffer[begin..begin + geometry.width() as usize],
                color,
            );
        }
    }

    fn process_rounded_rectangle(&mut self, geometry: PhysicalRect, rr: RoundedRectangle) {
        for line in geometry.min_y()..geometry.max_y() {
            draw_functions::draw_rounded_rectangle_line(
                &geometry,
                PhysicalLength::new(line),
                &rr,
                &mut self.buffer[line as usize * self.stride.get() as usize..],
            );
        }
    }
}

#[derive(Copy, Clone, Default)]
struct SceneCapacities {
    num_items: usize,
    num_textures: usize,
    num_rounded_rectangles: usize,
}

struct PrepareScene {
    items: Vec<SceneItem>,
    textures: Vec<SceneTexture>,
    rounded_rectangles: Vec<RoundedRectangle>,
}

impl PrepareScene {
    fn with_capacities(capacities: SceneCapacities) -> Self {
        Self {
            items: Vec::with_capacity(capacities.num_items),
            textures: Vec::with_capacity(capacities.num_textures),
            rounded_rectangles: Vec::with_capacity(capacities.num_rounded_rectangles),
        }
    }
}

impl ProcessScene for PrepareScene {
    fn process_texture(&mut self, geometry: PhysicalRect, texture: SceneTexture) {
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

    fn process_rectangle(&mut self, geometry: PhysicalRect, color: Color) {
        let size = geometry.size;
        if !size.is_empty() {
            let z = self.items.len() as u16;
            let pos = geometry.origin;
            self.items.push(SceneItem { pos, size, z, command: SceneCommand::Rectangle { color } });
        }
    }

    fn process_rounded_rectangle(&mut self, geometry: PhysicalRect, data: RoundedRectangle) {
        let size = geometry.size;
        if !size.is_empty() {
            let rectangle_index = self.rounded_rectangles.len() as u16;
            self.rounded_rectangles.push(data);
            self.items.push(SceneItem {
                pos: geometry.origin,
                size,
                z: self.items.len() as u16,
                command: SceneCommand::RoundedRectangle { rectangle_index },
            });
        }
    }
}

struct SceneBuilder<T> {
    processor: T,
    state_stack: Vec<RenderState>,
    current_state: RenderState,
    scale_factor: ScaleFactor,
    default_font: FontRequest,
}

impl<T: ProcessScene> SceneBuilder<T> {
    fn new(
        size: PhysicalSize,
        scale_factor: ScaleFactor,
        default_font: FontRequest,
        processor: T,
    ) -> Self {
        Self {
            processor,
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

    fn draw_image_impl(
        &mut self,
        geom: LogicalRect,
        source: &crate::graphics::Image,
        mut source_rect: IntRect,
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
            ImageInner::StaticTextures(StaticTextures { data, textures, .. }) => {
                let size: euclid::default::Size2D<u32> = source_rect.size.cast();
                let phys_size = geom.size_length().cast() * self.scale_factor;
                let source_to_target_x = phys_size.width / (size.width as f32);
                let source_to_target_y = phys_size.height / (size.height as f32);
                let mut image_fit_offset = euclid::Vector2D::default();
                let (source_to_target_x, source_to_target_y) = match image_fit {
                    ImageFit::fill => (source_to_target_x, source_to_target_y),
                    ImageFit::cover => {
                        let ratio = f32::max(source_to_target_x, source_to_target_y);
                        if size.width as f32 > phys_size.width / ratio {
                            let diff = (size.width as f32 - phys_size.width / ratio) as i32;
                            source_rect.origin.x += diff / 2;
                            source_rect.size.width -= diff;
                        }
                        if size.height as f32 > phys_size.height / ratio {
                            let diff = (size.height as f32 - phys_size.height / ratio) as i32;
                            source_rect.origin.y += diff / 2;
                            source_rect.size.height -= diff;
                        }
                        (ratio, ratio)
                    }
                    ImageFit::contain => {
                        let ratio = f32::min(source_to_target_x, source_to_target_y);
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

                let renderer_clip_in_source_rect_space = (self.current_state.clip.cast()
                    * self.scale_factor)
                    .scale(1. / source_to_target_x, 1. / source_to_target_y);

                for t in textures.as_slice() {
                    if let Some(clipped_relative_source_rect) =
                        t.rect.intersection(&source_rect).and_then(|clipped_source_rect| {
                            let relative_clipped_source_rect = clipped_source_rect
                                .translate(-source_rect.origin.to_vector())
                                .cast();
                            euclid::Rect::<_, PhysicalPx>::from_untyped(
                                &relative_clipped_source_rect,
                            )
                            .intersection(&renderer_clip_in_source_rect_space)
                        })
                    {
                        let target_rect = clipped_relative_source_rect
                            .scale(source_to_target_x, source_to_target_y)
                            .translate(offset)
                            .round();

                        let actual_x = clipped_relative_source_rect.origin.x as usize
                            + source_rect.origin.x as usize
                            - t.rect.origin.x as usize;
                        let actual_y = clipped_relative_source_rect.origin.y as usize
                            + source_rect.origin.y as usize
                            - t.rect.origin.y as usize;
                        let stride = t.rect.width() as u16 * bpp(t.format);
                        self.processor.process_texture(
                            target_rect.cast(),
                            SceneTexture {
                                data: &data.as_slice()[(t.index
                                    + (stride as usize) * actual_y
                                    + (bpp(t.format) as usize) * actual_x)..],
                                stride,
                                source_size: clipped_relative_source_rect.size.ceil().cast(),
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

impl<T: ProcessScene> crate::item_rendering::ItemRenderer for SceneBuilder<T> {
    fn draw_rectangle(&mut self, rect: Pin<&crate::items::Rectangle>, _: &ItemRc) {
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
            self.processor.process_rectangle(
                (geom.translate(self.current_state.offset.to_vector()).cast() * self.scale_factor)
                    .round()
                    .cast(),
                color,
            );
        }
    }

    fn draw_border_rectangle(&mut self, rect: Pin<&crate::items::BorderRectangle>, _: &ItemRc) {
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
                    // Add a small value to make sure that the clip is always positive despite floating point shenanigans
                    const E: f32 = 0.00001;
                    self.processor.process_rounded_rectangle(
                        (clipped.translate(self.current_state.offset.to_vector()).cast()
                            * self.scale_factor)
                            .round()
                            .cast(),
                        RoundedRectangle {
                            radius: (radius.cast() * self.scale_factor).cast(),
                            width: (LogicalLength::new(border).cast() * self.scale_factor).cast(),
                            border_color: rect.border_color().color(),
                            inner_color: color,
                            top_clip: PhysicalLength::new(
                                (clipped2.min_y() - geom2.min_y() + E) as _,
                            ),
                            bottom_clip: PhysicalLength::new(
                                (geom2.max_y() - clipped2.max_y() + E) as _,
                            ),
                            left_clip: PhysicalLength::new(
                                (clipped2.min_x() - geom2.min_x() + E) as _,
                            ),
                            right_clip: PhysicalLength::new(
                                (geom2.max_x() - clipped2.max_x() + E) as _,
                            ),
                        },
                    );
                }
                return;
            }

            if color.alpha() > 0 {
                if let Some(r) =
                    geom.inflate(-border, -border).intersection(&self.current_state.clip)
                {
                    self.processor.process_rectangle(
                        (r.translate(self.current_state.offset.to_vector()).cast()
                            * self.scale_factor)
                            .round()
                            .cast(),
                        color,
                    );
                }
            }
            if border > 0.01 as Coord {
                // FIXME: radius
                // FIXME: gradients
                let border_color = rect.border_color().color();
                if border_color.alpha() > 0 {
                    let mut add_border = |r: LogicalRect| {
                        if let Some(r) = r.intersection(&self.current_state.clip) {
                            self.processor.process_rectangle(
                                (r.translate(self.current_state.offset.to_vector()).cast()
                                    * self.scale_factor)
                                    .round()
                                    .cast(),
                                border_color,
                            );
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

    fn draw_image(&mut self, image: Pin<&crate::items::ImageItem>, _: &ItemRc) {
        let geom =
            LogicalRect::new(LogicalPoint::default(), image.logical_geometry().size_length());
        if self.should_draw(&geom) {
            let source = image.source();
            self.draw_image_impl(
                geom,
                &source,
                euclid::Rect::new(Default::default(), source.size().cast()),
                image.image_fit(),
                Default::default(),
            );
        }
    }

    fn draw_clipped_image(&mut self, image: Pin<&crate::items::ClippedImage>, _: &ItemRc) {
        let geom =
            LogicalRect::new(LogicalPoint::default(), image.logical_geometry().size_length());
        if self.should_draw(&geom) {
            let source = image.source();

            let source_clip_x = image.source_clip_x();
            let source_clip_y = image.source_clip_y();
            let source_size = source.size();
            let mut source_clip_width = image.source_clip_width();
            // when the source_clip size is empty, make it full
            if source_clip_width == 0 {
                source_clip_width = source_size.width as i32 - source_clip_x;
            }
            let mut source_clip_height = image.source_clip_height();
            if source_clip_height == 0 {
                source_clip_height = source_size.height as i32 - source_clip_y;
            }

            self.draw_image_impl(
                geom,
                &source,
                euclid::rect(source_clip_x, source_clip_y, source_clip_width, source_clip_height),
                image.image_fit(),
                image.colorize().color(),
            );
        }
    }

    fn draw_text(&mut self, text: Pin<&crate::items::Text>, _: &ItemRc) {
        let string = text.text();
        if string.trim().is_empty() {
            return;
        }
        let geom = LogicalRect::new(LogicalPoint::default(), text.logical_geometry().size_length());
        if !self.should_draw(&geom) {
            return;
        }

        let font_request = text.unresolved_font_request().merge(&self.default_font);
        let font = fonts::match_font(&font_request, self.scale_factor);
        let layout = fonts::text_layout_for_font(&font, &font_request, self.scale_factor);

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

        // Clip glyphs not only against the global clip but also against the Text's geometry to avoid drawing outside
        // of its boundaries (that breaks partial rendering and the cast to usize for the item relative coordinate below).
        // FIXME: we should allow drawing outside of the Text element's boundaries.
        let physical_clip = if let Some(logical_clip) = self.current_state.clip.intersection(&geom)
        {
            logical_clip.cast() * self.scale_factor
        } else {
            return; // This should have been caught earlier already
        };
        let offset = self.current_state.offset.to_vector().cast() * self.scale_factor;

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

                if let Some(clipped_src) = src_rect.intersection(&physical_clip) {
                    let geometry = clipped_src.translate(offset).round();
                    let origin = (geometry.origin - offset.round()).cast::<usize>();
                    let actual_x = origin.x - src_rect.origin.x as usize;
                    let actual_y = origin.y - src_rect.origin.y as usize;
                    let stride = positioned_glyph.platform_glyph.width().get() as u16;
                    let geometry = geometry.cast();
                    self.processor.process_texture(
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

    fn draw_text_input(&mut self, text_input: Pin<&crate::items::TextInput>, _: &ItemRc) {
        text_input.logical_geometry();
        // TODO
    }

    #[cfg(feature = "std")]
    fn draw_path(&mut self, path: Pin<&crate::items::Path>, _: &ItemRc) {
        path.logical_geometry();
        // TODO
    }

    fn draw_box_shadow(&mut self, box_shadow: Pin<&crate::items::BoxShadow>, _: &ItemRc) {
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

    fn get_current_clip(&self) -> crate::graphics::Rect {
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
        _: &ItemRc,
        _update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        todo!()
    }

    fn draw_string(&mut self, _string: &str, _color: Color) {
        todo!()
    }

    fn window(&self) -> crate::window::WindowRc {
        unreachable!("this backend don't query the window")
    }

    fn as_any(&mut self) -> &mut dyn core::any::Any {
        unimplemented!()
    }
}

/// bytes per pixels
fn bpp(format: PixelFormat) -> u16 {
    match format {
        PixelFormat::Rgb => 3,
        PixelFormat::Rgba => 4,
        PixelFormat::RgbaPremultiplied => 4,
        PixelFormat::AlphaMap => 1,
    }
}
