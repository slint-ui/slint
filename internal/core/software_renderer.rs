// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! This module contains the [`SoftwareRenderer`] and related types.

#![warn(missing_docs)]

mod draw_functions;
mod fonts;

use crate::api::Window;
use crate::graphics::{IntRect, PixelFormat, SharedImageBuffer};
use crate::item_rendering::ItemRenderer;
use crate::items::{ImageFit, Item, ItemRc};
use crate::lengths::{
    LogicalLength, LogicalPoint, LogicalRect, LogicalSize, LogicalVector, PhysicalPx, PointLengths,
    RectLengths, ScaleFactor, SizeLengths,
};
use crate::renderer::Renderer;
use crate::textlayout::{FontMetrics as _, TextParagraphLayout};
use crate::window::{WindowAdapter, WindowInner};
use crate::{Brush, Color, Coord, ImageInner, StaticTextures};
use alloc::rc::{Rc, Weak};
use alloc::{vec, vec::Vec};
use core::cell::{Cell, RefCell};
use core::pin::Pin;
use euclid::num::Zero;

pub use draw_functions::{PremultipliedRgbaColor, Rgb565Pixel, TargetPixel};

type PhysicalLength = euclid::Length<i16, PhysicalPx>;
type PhysicalRect = euclid::Rect<i16, PhysicalPx>;
type PhysicalSize = euclid::Size2D<i16, PhysicalPx>;
type PhysicalPoint = euclid::Point2D<i16, PhysicalPx>;

type DirtyRegion = PhysicalRect;

/// This trait defines a bi-directional interface between Slint and your code to send lines to your screen, when using
/// the [`SoftwareRenderer::render_by_line`] function.
///
/// * Through the associated `TargetPixel` type Slint knows how to create and manipulate pixels without having to know
///   the exact device-specific binary representation and operations for blending.
/// * Through the `process_line` function Slint notifies you when a line can be rendered and provides a callback that
///   you can invoke to fill a slice of pixels for the given line.
///
/// See the [`render_by_line`](SoftwareRenderer::render_by_line) documentation for an example.
pub trait LineBufferProvider {
    /// The pixel type of the buffer
    type TargetPixel: TargetPixel;

    /// Called once per line, you will have to call the render_fn back with the buffer.
    ///
    /// The `line` is the y position of the line to be drawn.
    /// The `range` is the range within the line that is going to be rendered (eg, within the dirty region)
    /// The `render_fn` function should be called to render the line, passing the buffer
    /// corresponding to the specified line and range.
    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    );
}

/// A Renderer that do the rendering in software
///
/// The renderer can remember what items needs to be redrawn from the previous iteration.
///
/// There are two kind of possible rendering
///  1. Using [`render()`](Self::render()) to render the window in a buffer
///  2. Using [`render_by_line()`](Self::render()) to render the window line by line. This
///     is only useful if the device does not have enough memory to render the whole window
///     in one single buffer
///
/// ### `MAX_BUFFER_AGE`
///
/// The `MAX_BUFFER_AGE` parameter specifies how many buffers are being re-used.
/// This means that the buffer passed to the render functions still contains a rendering of
/// the window that was refreshed as least that amount of frame ago.
/// It will impact how much of the screen needs to be redrawn.
///
/// Typical value can be:
///  - **0:** No attempt at tracking dirty items will be made. The full screen is always redrawn.
///  - **1:** Only redraw the parts that have changed since the previous call to render.
///           This is assuming that the same buffer is passed on every call to render.
///  - **2:** Redraw the part that have changed during the two last frames.
///           This is assuming double buffering and swapping of the buffers.
pub struct SoftwareRenderer<const MAX_BUFFER_AGE: usize> {
    partial_cache: RefCell<crate::item_rendering::PartialRenderingCache>,
    /// This is the area which we are going to redraw in the next frame, no matter if the items are dirty or not
    force_dirty: Cell<crate::item_rendering::DirtyRegion>,
    /// This is the area which was dirty on the previous frames, in case we do double buffering
    ///
    /// We really only need MAX_BUFFER_AGE - 1 but that's not allowed because we cannot do operations with
    /// generic parameters
    prev_frame_dirty: [Cell<DirtyRegion>; MAX_BUFFER_AGE],
    window: Weak<dyn crate::window::WindowAdapter>,
}

impl<const MAX_BUFFER_AGE: usize> SoftwareRenderer<MAX_BUFFER_AGE> {
    /// Create a new Renderer for a given window.
    ///
    /// The `window` parameter can be coming from [`Rc::new_cyclic()`](alloc::rc::Rc::new_cyclic())
    /// since the `WindowAdapter` most likely own the Renderer
    pub fn new(window: Weak<dyn crate::window::WindowAdapter>) -> Self {
        Self {
            window: window.clone(),
            partial_cache: Default::default(),
            force_dirty: Default::default(),
            prev_frame_dirty: [DirtyRegion::default(); MAX_BUFFER_AGE].map(|x| x.into()),
        }
    }

    /// Internal function to apply a dirty region depending on the dirty_tracking_policy.
    /// Returns the region to actually draw.
    fn apply_dirty_region(
        &self,
        dirty_region: DirtyRegion,
        screen_size: PhysicalSize,
    ) -> DirtyRegion {
        if MAX_BUFFER_AGE == 0 {
            PhysicalRect { origin: euclid::point2(0, 0), size: screen_size }
        } else if MAX_BUFFER_AGE == 1 {
            dirty_region
        } else if MAX_BUFFER_AGE == 2 {
            dirty_region.union(&self.prev_frame_dirty[0].replace(dirty_region))
        } else {
            let mut prev = dirty_region;
            let mut union = dirty_region;
            for x in self.prev_frame_dirty.iter().skip(1) {
                prev = x.replace(prev);
                union = union.union(&prev);
            }
            union
        }
        .intersection(&PhysicalRect { origin: euclid::point2(0, 0), size: screen_size })
        .unwrap_or_default()
    }

    /// Render the window to the given frame buffer.
    ///
    /// The renderer uses a cache internally and will only render the part of the window
    /// which are dirty. The `extra_draw_region` is an extra regin which will also
    /// be rendered. (eg: the previous dirty region in case of double buffering)
    ///
    /// returns the dirty region for this frame (not including the extra_draw_region)
    pub fn render(&self, buffer: &mut [impl TargetPixel], buffer_stride: usize) {
        let window = self.window.upgrade().expect("render() called on a destroyed Window");
        let window_inner = WindowInner::from_pub(window.window());
        let factor = ScaleFactor::new(window_inner.scale_factor());
        let (size, background) = if let Some(window_item) =
            window_inner.window_item().as_ref().map(|item| item.as_pin_ref())
        {
            (
                (LogicalSize::from_lengths(window_item.width(), window_item.height()).cast()
                    * factor)
                    .cast(),
                window_item.background(),
            )
        } else {
            (
                euclid::size2(buffer_stride as _, (buffer.len() / buffer_stride) as _),
                Brush::default(),
            )
        };
        let buffer_renderer = SceneBuilder::new(
            size,
            factor,
            window_inner,
            RenderToBuffer { buffer, stride: buffer_stride },
        );
        let mut renderer = crate::item_rendering::PartialRenderer::new(
            &self.partial_cache,
            self.force_dirty.take(),
            buffer_renderer,
        );

        window_inner.draw_contents(|components| {
            for (component, origin) in components {
                renderer.compute_dirty_regions(component, *origin);
            }

            let dirty_region = (renderer.dirty_region.to_rect().cast() * factor).round_out().cast();

            let to_draw = self.apply_dirty_region(dirty_region, size);

            renderer.combine_clip(
                (to_draw.cast() / factor).cast(),
                LogicalLength::zero(),
                LogicalLength::zero(),
            );

            if !background.is_transparent() {
                // FIXME: gradient
                renderer.actual_renderer.processor.process_rectangle(to_draw, background.color());
            }
            for (component, origin) in components {
                crate::item_rendering::render_component_items(component, &mut renderer, *origin);
            }
        });
    }

    /// Render the window, line by line, into the line buffer provided by the `line_callback` function.
    ///
    /// The renderer uses a cache internally and will only render the part of the window
    /// which are dirty, depending on the dirty tracking policy set in [`SoftwareRenderer::new`]
    ///
    /// The line callback will be called for each line and should provide a buffer to draw into.
    ///
    /// As an example, let's imagine we want to render into a plain buffer.
    /// (You wouldn't normally use `render_by_line` for that because the [`Self::render`] would
    /// then be more efficient)
    ///
    /// ```rust
    /// # use i_slint_core::software_renderer::{LineBufferProvider, SoftwareRenderer, Rgb565Pixel};
    /// # fn xxx<'a>(the_frame_buffer: &'a mut [Rgb565Pixel], display_width: usize, renderer: &SoftwareRenderer<0>) {
    /// struct FrameBuffer<'a>{ frame_buffer: &'a mut [Rgb565Pixel], stride: usize }
    /// impl<'a> LineBufferProvider for FrameBuffer<'a> {
    ///     type TargetPixel = Rgb565Pixel;
    ///     fn process_line(
    ///         &mut self,
    ///         line: usize,
    ///         range: core::ops::Range<usize>,
    ///         render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ///     ) {
    ///         let line_begin = line * self.stride;
    ///         render_fn(&mut self.frame_buffer[line_begin..][range]);
    ///         // The line has been rendered and there could be code here to
    ///         // send the pixel to the display
    ///     }
    /// }
    /// renderer.render_by_line(FrameBuffer{ frame_buffer: the_frame_buffer, stride: display_width });
    /// # }
    /// ```
    pub fn render_by_line(&self, line_buffer: impl LineBufferProvider) {
        let window = self.window.upgrade().expect("render() called on a destroyed Window");
        let window_inner = WindowInner::from_pub(window.window());
        let component_rc = window_inner.component();
        let component = crate::component::ComponentRc::borrow_pin(&component_rc);
        if let Some(window_item) = crate::items::ItemRef::downcast_pin::<crate::items::WindowItem>(
            component.as_ref().get_item_ref(0),
        ) {
            let factor = ScaleFactor::new(window_inner.scale_factor());
            let size = LogicalSize::from_lengths(window_item.width(), window_item.height()).cast()
                * factor;
            render_window_frame_by_line(
                window_inner,
                window_item.background(),
                size.cast(),
                &self,
                line_buffer,
            );
        }
    }

    #[doc(hidden)]
    pub fn default_font_size() -> LogicalLength {
        self::fonts::DEFAULT_FONT_SIZE
    }
}

#[doc(hidden)]
impl<const MAX_BUFFER_AGE: usize> Renderer for SoftwareRenderer<MAX_BUFFER_AGE> {
    fn text_size(
        &self,
        font_request: crate::graphics::FontRequest,
        text: &str,
        max_width: Option<LogicalLength>,
        scale_factor: ScaleFactor,
    ) -> LogicalSize {
        fonts::text_size(font_request, text, max_width, scale_factor)
    }

    fn text_input_byte_offset_for_position(
        &self,
        _text_input: Pin<&crate::items::TextInput>,
        _pos: LogicalPoint,
    ) -> usize {
        0
    }
    fn text_input_cursor_rect_for_byte_offset(
        &self,
        _text_input: Pin<&crate::items::TextInput>,
        _byte_offset: usize,
    ) -> LogicalRect {
        Default::default()
    }

    fn free_graphics_resources(
        &self,
        items: &mut dyn Iterator<Item = Pin<crate::items::ItemRef<'_>>>,
    ) {
        for item in items {
            let cache_entry =
                item.cached_rendering_data_offset().release(&mut self.partial_cache.borrow_mut());
            drop(cache_entry);
        }
    }

    fn mark_dirty_region(&self, region: crate::item_rendering::DirtyRegion) {
        self.force_dirty.set(self.force_dirty.get().union(&region))
    }

    fn register_bitmap_font(&self, font_data: &'static crate::graphics::BitmapFont) {
        fonts::register_bitmap_font(font_data);
    }
}

fn render_window_frame_by_line<const MAX_BUFFER_AGE: usize>(
    window: &WindowInner,
    background: Brush,
    size: PhysicalSize,
    renderer: &SoftwareRenderer<MAX_BUFFER_AGE>,
    mut line_buffer: impl LineBufferProvider,
) {
    let mut scene = prepare_scene(window, size, renderer);

    let dirty_region = scene.dirty_region;

    debug_assert!(scene.current_line >= dirty_region.origin.y_length());

    // FIXME gradient
    let background_color = background.color().into();

    while scene.current_line < dirty_region.origin.y_length() + dirty_region.size.height_length() {
        line_buffer.process_line(
            scene.current_line.get() as usize,
            dirty_region.min_x() as usize..dirty_region.max_x() as usize,
            |line_buffer| {
                let offset = dirty_region.min_x() as usize;

                TargetPixel::blend_slice(line_buffer, background_color);
                for span in scene.items[0..scene.current_items_index].iter().rev() {
                    debug_assert!(scene.current_line >= span.pos.y_length());
                    debug_assert!(
                        scene.current_line < span.pos.y_length() + span.size.height_length(),
                    );
                    match span.command {
                        SceneCommand::Rectangle { color } => {
                            TargetPixel::blend_slice(
                                &mut line_buffer[span.pos.x as usize - offset
                                    ..(span.pos.x_length() + span.size.width_length()).get()
                                        as usize
                                        - offset],
                                color,
                            );
                        }
                        SceneCommand::Texture { texture_index } => {
                            let texture = &scene.textures[texture_index as usize];
                            draw_functions::draw_texture_line(
                                &PhysicalRect {
                                    origin: span.pos - euclid::vec2(offset as i16, 0),
                                    size: span.size,
                                },
                                scene.current_line,
                                texture,
                                line_buffer,
                            );
                        }
                        SceneCommand::SharedBuffer { shared_buffer_index } => {
                            let texture =
                                scene.shared_buffers[shared_buffer_index as usize].as_texture();
                            draw_functions::draw_texture_line(
                                &PhysicalRect {
                                    origin: span.pos - euclid::vec2(offset as i16, 0),
                                    size: span.size,
                                },
                                scene.current_line,
                                &texture,
                                line_buffer,
                            );
                        }
                        SceneCommand::RoundedRectangle { rectangle_index } => {
                            let rr = &scene.rounded_rectangles[rectangle_index as usize];
                            draw_functions::draw_rounded_rectangle_line(
                                &PhysicalRect {
                                    origin: span.pos - euclid::vec2(offset as i16, 0),
                                    size: span.size,
                                },
                                scene.current_line,
                                rr,
                                line_buffer,
                            );
                        }
                    }
                }
            },
        );

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

    textures: Vec<SceneTexture<'static>>,
    rounded_rectangles: Vec<RoundedRectangle>,
    shared_buffers: Vec<SharedBufferCommand>,
    dirty_region: DirtyRegion,
}

impl Scene {
    pub fn new(
        mut items: Vec<SceneItem>,
        textures: Vec<SceneTexture<'static>>,
        rounded_rectangles: Vec<RoundedRectangle>,
        shared_buffers: Vec<SharedBufferCommand>,
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
            shared_buffers,
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
        color: PremultipliedRgbaColor,
    },
    /// texture_index is an index in the Scene::textures array
    Texture {
        texture_index: u16,
    },
    /// shared_buffer_index is an index in Scene::shared_buffers
    SharedBuffer {
        shared_buffer_index: u16,
    },
    /// rectangle_index is an index in the Scene::rounded_rectangle array
    RoundedRectangle {
        rectangle_index: u16,
    },
}

struct SceneTexture<'a> {
    data: &'a [u8],
    format: PixelFormat,
    /// bytes between two lines in the source
    stride: u16,
    source_size: PhysicalSize,
    color: Color,
}

struct SharedBufferCommand {
    buffer: SharedImageBuffer,
    /// The source rectangle that is mapped into this command span
    source_rect: PhysicalRect,
    colorize: Color,
}

impl SharedBufferCommand {
    fn as_texture(&self) -> SceneTexture<'_> {
        let begin = self.buffer.width() as usize * self.source_rect.min_y() as usize
            + self.source_rect.min_x() as usize;

        match &self.buffer {
            SharedImageBuffer::RGB8(b) => SceneTexture {
                data: &b.as_bytes()[begin * 3..],
                stride: 3 * b.stride() as u16,
                format: PixelFormat::Rgb,
                source_size: self.source_rect.size,
                color: self.colorize,
            },
            SharedImageBuffer::RGBA8(b) => SceneTexture {
                data: &b.as_bytes()[begin * 4..],
                stride: 4 * b.stride() as u16,
                format: PixelFormat::Rgba,
                source_size: self.source_rect.size,
                color: self.colorize,
            },
            SharedImageBuffer::RGBA8Premultiplied(b) => SceneTexture {
                data: &b.as_bytes()[begin * 4..],
                stride: 4 * b.stride() as u16,
                format: PixelFormat::RgbaPremultiplied,
                source_size: self.source_rect.size,
                color: self.colorize,
            },
        }
    }
}

#[derive(Debug)]
struct RoundedRectangle {
    radius: PhysicalLength,
    /// the border's width
    width: PhysicalLength,
    border_color: PremultipliedRgbaColor,
    inner_color: PremultipliedRgbaColor,
    /// The clips is the amount of pixels of the rounded rectangle that is clipped away.
    /// For example, if left_clip > width, then the left border will not be visible, and
    /// if left_clip > radius, then no radius will be seen in the left side
    left_clip: PhysicalLength,
    right_clip: PhysicalLength,
    top_clip: PhysicalLength,
    bottom_clip: PhysicalLength,
}

fn prepare_scene<const MAX_BUFFER_AGE: usize>(
    window: &WindowInner,
    size: PhysicalSize,
    software_renderer: &SoftwareRenderer<MAX_BUFFER_AGE>,
) -> Scene {
    let factor = ScaleFactor::new(window.scale_factor());
    let prepare_scene = SceneBuilder::new(size, factor, window, PrepareScene::default());
    let mut renderer = crate::item_rendering::PartialRenderer::new(
        &software_renderer.partial_cache,
        software_renderer.force_dirty.take(),
        prepare_scene,
    );

    let mut dirty_region = PhysicalRect::default();
    window.draw_contents(|components| {
        for (component, origin) in components {
            renderer.compute_dirty_regions(component, *origin);
        }

        dirty_region = (renderer.dirty_region.to_rect().cast() * factor).round_out().cast();
        dirty_region = software_renderer.apply_dirty_region(dirty_region, size);

        renderer.combine_clip(
            (dirty_region.cast() / factor).cast(),
            LogicalLength::zero(),
            LogicalLength::zero(),
        );
        for (component, origin) in components {
            crate::item_rendering::render_component_items(component, &mut renderer, *origin);
        }
    });

    let prepare_scene = renderer.into_inner();
    Scene::new(
        prepare_scene.processor.items,
        prepare_scene.processor.textures,
        prepare_scene.processor.rounded_rectangles,
        prepare_scene.processor.shared_buffers,
        dirty_region,
    )
}

trait ProcessScene {
    fn process_texture(&mut self, geometry: PhysicalRect, texture: SceneTexture<'static>);
    fn process_rectangle(&mut self, geometry: PhysicalRect, color: Color);
    fn process_rounded_rectangle(&mut self, geometry: PhysicalRect, data: RoundedRectangle);
    fn process_shared_image_buffer(&mut self, geometry: PhysicalRect, buffer: SharedBufferCommand);
}

struct RenderToBuffer<'a, TargetPixel> {
    buffer: &'a mut [TargetPixel],
    stride: usize,
}

impl<'a, T: TargetPixel> ProcessScene for RenderToBuffer<'a, T> {
    fn process_texture(&mut self, geometry: PhysicalRect, texture: SceneTexture<'static>) {
        for line in geometry.min_y()..geometry.max_y() {
            draw_functions::draw_texture_line(
                &geometry,
                PhysicalLength::new(line),
                &texture,
                &mut self.buffer[line as usize * self.stride..],
            );
        }
    }

    fn process_shared_image_buffer(&mut self, geometry: PhysicalRect, buffer: SharedBufferCommand) {
        let texture = buffer.as_texture();
        for line in geometry.min_y()..geometry.max_y() {
            draw_functions::draw_texture_line(
                &geometry,
                PhysicalLength::new(line),
                &texture,
                &mut self.buffer[line as usize * self.stride..],
            );
        }
    }

    fn process_rectangle(&mut self, geometry: PhysicalRect, color: Color) {
        let color = PremultipliedRgbaColor::from(color);
        for line in geometry.min_y()..geometry.max_y() {
            let begin = line as usize * self.stride + geometry.origin.x as usize;
            TargetPixel::blend_slice(
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
                &mut self.buffer[line as usize * self.stride..],
            );
        }
    }
}

#[derive(Default)]
struct PrepareScene {
    items: Vec<SceneItem>,
    textures: Vec<SceneTexture<'static>>,
    rounded_rectangles: Vec<RoundedRectangle>,
    shared_buffers: Vec<SharedBufferCommand>,
}

impl ProcessScene for PrepareScene {
    fn process_texture(&mut self, geometry: PhysicalRect, texture: SceneTexture<'static>) {
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

    fn process_shared_image_buffer(&mut self, geometry: PhysicalRect, buffer: SharedBufferCommand) {
        let size = geometry.size;
        if !size.is_empty() {
            let shared_buffer_index = self.shared_buffers.len() as u16;
            self.shared_buffers.push(buffer);
            self.items.push(SceneItem {
                pos: geometry.origin,
                size,
                z: self.items.len() as u16,
                command: SceneCommand::SharedBuffer { shared_buffer_index },
            });
        }
    }

    fn process_rectangle(&mut self, geometry: PhysicalRect, color: Color) {
        let size = geometry.size;
        if !size.is_empty() {
            let z = self.items.len() as u16;
            let pos = geometry.origin;
            let color = PremultipliedRgbaColor::from(color);
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

struct SceneBuilder<'a, T> {
    processor: T,
    state_stack: Vec<RenderState>,
    current_state: RenderState,
    scale_factor: ScaleFactor,
    window: &'a WindowInner,
}

impl<'a, T: ProcessScene> SceneBuilder<'a, T> {
    fn new(
        size: PhysicalSize,
        scale_factor: ScaleFactor,
        window: &'a WindowInner,
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
            window,
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
        let size: euclid::default::Size2D<u32> = source_rect.size.cast();
        let phys_size = geom.size_length().cast() * self.scale_factor;
        let source_to_target_x = phys_size.width / (size.width as f32);
        let source_to_target_y = phys_size.height / (size.height as f32);
        let mut image_fit_offset = euclid::Vector2D::default();
        let (source_to_target_x, source_to_target_y) = match image_fit {
            ImageFit::Fill => (source_to_target_x, source_to_target_y),
            ImageFit::Cover => {
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
            ImageFit::Contain => {
                let ratio = f32::min(source_to_target_x, source_to_target_y);
                if (size.width as f32) < phys_size.width / ratio {
                    image_fit_offset.x = (phys_size.width - size.width as f32 * ratio) / 2.;
                }
                if (size.height as f32) < phys_size.height / ratio {
                    image_fit_offset.y = (phys_size.height - size.height as f32 * ratio) / 2.;
                }
                (ratio, ratio)
            }
        };

        let offset =
            self.current_state.offset.to_vector().cast() * self.scale_factor + image_fit_offset;

        let renderer_clip_in_source_rect_space = (self.current_state.clip.cast()
            * self.scale_factor)
            .scale(1. / source_to_target_x, 1. / source_to_target_y);
        match image_inner {
            ImageInner::None => (),
            ImageInner::StaticTextures(StaticTextures { data, textures, .. }) => {
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
                        let stride = t.rect.width() as u16 * t.format.bpp() as u16;
                        self.processor.process_texture(
                            target_rect.cast(),
                            SceneTexture {
                                data: &data.as_slice()[(t.index
                                    + (stride as usize) * actual_y
                                    + (t.format.bpp()) * actual_x)..],
                                stride,
                                source_size: clipped_relative_source_rect.size.ceil().cast(),
                                format: t.format,
                                color: if colorize.alpha() > 0 { colorize } else { t.color },
                            },
                        );
                    }
                }
            }
            _ => {
                let img_src_size = source.size();
                if let Some(buffer) = image_inner.render_to_buffer(Some(
                    crate::graphics::fit_size(image_fit, phys_size, img_src_size).cast(),
                )) {
                    if let Some(clipped_relative_source_rect) = renderer_clip_in_source_rect_space
                        .intersection(&euclid::rect(
                            0.,
                            0.,
                            source_rect.width() as f32,
                            source_rect.height() as f32,
                        ))
                    {
                        let target_rect = clipped_relative_source_rect
                            .scale(source_to_target_x, source_to_target_y)
                            .translate(offset)
                            .round();
                        let buf_size = buffer.size().cast::<f32>();

                        self.processor.process_shared_image_buffer(
                            target_rect.cast(),
                            SharedBufferCommand {
                                buffer,
                                source_rect: clipped_relative_source_rect
                                    .translate(
                                        euclid::Point2D::from_untyped(source_rect.origin.cast())
                                            .to_vector(),
                                    )
                                    .scale(
                                        buf_size.width / img_src_size.width as f32,
                                        buf_size.height / img_src_size.height as f32,
                                    )
                                    .cast(),
                                colorize,
                            },
                        );
                    }
                } else {
                    unimplemented!("The image cannot be rendered")
                }
            }
        };
    }

    /// Returns the color of the brush, mixed with the current_state's alpha
    fn alpha_color(&self, brush: &Brush) -> Color {
        let mut color = brush.color();

        if self.current_state.alpha < 1.0 {
            color = Color::from_argb_u8(
                (color.alpha() as f32 * self.current_state.alpha) as u8,
                color.red(),
                color.green(),
                color.blue(),
            );
        }

        color
    }
}

#[derive(Clone, Copy)]
struct RenderState {
    alpha: f32,
    offset: LogicalPoint,
    clip: LogicalRect,
}

impl<'a, T: ProcessScene> crate::item_rendering::ItemRenderer for SceneBuilder<'a, T> {
    fn draw_rectangle(&mut self, rect: Pin<&crate::items::Rectangle>, _: &ItemRc) {
        let geom = LogicalRect::new(LogicalPoint::default(), rect.geometry().size_length());
        if self.should_draw(&geom) {
            let geom = match geom.intersection(&self.current_state.clip) {
                Some(geom) => geom,
                None => return,
            };

            // FIXME: gradients
            let color = self.alpha_color(&rect.background());

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
        let geom = LogicalRect::new(LogicalPoint::default(), rect.geometry().size_length());
        if self.should_draw(&geom) {
            let border = rect.border_width();
            let radius = rect.border_radius();
            // FIXME: gradients
            let color = self.alpha_color(&rect.background());
            let border_color = if border.get() as f32 > 0.01 {
                self.alpha_color(&rect.border_color())
            } else {
                Color::default()
            };

            if radius.get() > 0 as _ {
                let radius = radius
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
                            width: (border.cast() * self.scale_factor).cast(),
                            border_color: border_color.into(),
                            inner_color: color.into(),
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
                if let Some(r) = geom
                    .inflate(-border.get(), -border.get())
                    .intersection(&self.current_state.clip)
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

            // FIXME: gradients
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
                let b = border.get();
                add_border(euclid::rect(0 as _, 0 as _, geom.width(), b));
                add_border(euclid::rect(0 as _, geom.height() - b, geom.width(), b));
                add_border(euclid::rect(0 as _, b, b, geom.height() - b - b));
                add_border(euclid::rect(geom.width() - b, b, b, geom.height() - b - b));
            }
        }
    }

    fn draw_image(&mut self, image: Pin<&crate::items::ImageItem>, _: &ItemRc) {
        let geom =
            LogicalRect::new(LogicalPoint::default(), image.as_ref().geometry().size_length());
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
        let geom = LogicalRect::new(LogicalPoint::default(), image.geometry().size_length());
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
        let geom = LogicalRect::new(LogicalPoint::default(), text.geometry().size_length());
        if !self.should_draw(&geom) {
            return;
        }

        let font_request = text.font_request(self.window);
        let font = fonts::match_font(&font_request, self.scale_factor);
        let layout = fonts::text_layout_for_font(&font, &font_request, self.scale_factor);

        let color = self.alpha_color(&text.color());
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
        text_input.geometry();
        // TODO
    }

    #[cfg(feature = "std")]
    fn draw_path(&mut self, path: Pin<&crate::items::Path>, _: &ItemRc) {
        path.geometry();
        // TODO
    }

    fn draw_box_shadow(&mut self, box_shadow: Pin<&crate::items::BoxShadow>, _: &ItemRc) {
        box_shadow.geometry();
        // TODO
    }

    fn combine_clip(
        &mut self,
        other: LogicalRect,
        _radius: LogicalLength,
        _border_width: LogicalLength,
    ) -> bool {
        match self.current_state.clip.intersection(&other) {
            Some(r) => {
                self.current_state.clip = r;
                true
            }
            None => {
                self.current_state.clip = LogicalRect::default();
                false
            }
        }
        // TODO: handle radius and border
    }

    fn get_current_clip(&self) -> LogicalRect {
        self.current_state.clip
    }

    fn translate(&mut self, distance: LogicalVector) {
        self.current_state.offset += distance;
        self.current_state.clip = self.current_state.clip.translate(-distance)
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

    fn window(&self) -> &crate::api::Window {
        unreachable!("this backend don't query the window")
    }

    fn as_any(&mut self) -> Option<&mut dyn core::any::Any> {
        None
    }
}

/// This is a minimal adaptor for a Window that doesn't have any other feature than rendering
/// using the software renderer.
///
/// The [`MAX_BUFFER_AGE`](SoftwareRenderer#max_buffer_age) generic parameter is forwarded to
/// the [`SoftwareRenderer`]
pub struct MinimalSoftwareWindow<const MAX_BUFFER_AGE: usize> {
    window: Window,
    renderer: SoftwareRenderer<MAX_BUFFER_AGE>,
    needs_redraw: Cell<bool>,
}

impl<const MAX_BUFFER_AGE: usize> MinimalSoftwareWindow<MAX_BUFFER_AGE> {
    /// Instantiate a new MinimalWindowAdaptor
    pub fn new() -> Rc<Self> {
        Rc::new_cyclic(|w: &Weak<Self>| Self {
            window: Window::new(w.clone()),
            renderer: SoftwareRenderer::new(w.clone()),
            needs_redraw: Default::default(),
        })
    }
    /// If the window needs to be redrawn, the callback will be called with the
    /// [renderer](SoftwareRenderer) that should be used to do the drawing.
    ///
    /// [`SoftwareRenderer::render()`] or [`SoftwareRenderer::render_by_line()`] should be called
    /// in that callback.
    ///
    /// Return true if something was redrawn.
    pub fn draw_if_needed(
        &self,
        render_callback: impl FnOnce(&SoftwareRenderer<MAX_BUFFER_AGE>),
    ) -> bool {
        if self.needs_redraw.replace(false) {
            render_callback(&self.renderer);
            true
        } else {
            false
        }
    }
}

impl<const MAX_BUFFER_AGE: usize> crate::window::WindowAdapterSealed
    for MinimalSoftwareWindow<MAX_BUFFER_AGE>
{
    fn request_redraw(&self) {
        self.needs_redraw.set(true);
    }
    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }
    fn register_root_component(&self, window_item: Pin<&crate::items::WindowItem>) {
        let default_font_size_prop =
            crate::items::WindowItem::FIELD_OFFSETS.default_font_size.apply_pin(window_item);
        if default_font_size_prop.get().get() <= 0 as Coord {
            default_font_size_prop.set(SoftwareRenderer::<MAX_BUFFER_AGE>::default_font_size());
        }
    }
}

impl<const MAX_BUFFER_AGE: usize> WindowAdapter for MinimalSoftwareWindow<MAX_BUFFER_AGE> {
    fn window(&self) -> &Window {
        &self.window
    }
}

impl<const MAX_BUFFER_AGE: usize> core::ops::Deref for MinimalSoftwareWindow<MAX_BUFFER_AGE> {
    type Target = Window;
    fn deref(&self) -> &Self::Target {
        &self.window
    }
}
