// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! This module contains the [`SoftwareRenderer`] and related types.

#![warn(missing_docs)]

mod draw_functions;
mod fonts;

use crate::api::Window;
use crate::graphics::{IntRect, PixelFormat, SharedImageBuffer, SharedPixelBuffer};
use crate::item_rendering::ItemRenderer;
use crate::items::{ImageFit, ItemRc, TextOverflow};
use crate::lengths::{
    LogicalLength, LogicalPoint, LogicalRect, LogicalSize, LogicalVector, PhysicalPx, PointLengths,
    RectLengths, ScaleFactor, SizeLengths,
};
use crate::renderer::Renderer;
use crate::textlayout::{AbstractFont, FontMetrics, TextParagraphLayout};
use crate::window::{WindowAdapter, WindowInner};
use crate::{Brush, Color, Coord, ImageInner, StaticTextures};
use alloc::rc::{Rc, Weak};
use alloc::{vec, vec::Vec};
use core::cell::{Cell, RefCell};
use core::pin::Pin;
use euclid::num::Zero;
use euclid::Length;
#[allow(unused)]
use num_traits::Float;

pub use draw_functions::{PremultipliedRgbaColor, Rgb565Pixel, TargetPixel};

use self::fonts::GlyphRenderer;

type PhysicalLength = euclid::Length<i16, PhysicalPx>;
type PhysicalRect = euclid::Rect<i16, PhysicalPx>;
type PhysicalSize = euclid::Size2D<i16, PhysicalPx>;
type PhysicalPoint = euclid::Point2D<i16, PhysicalPx>;

type DirtyRegion = PhysicalRect;

/// This enum describes which parts of the buffer passed to the [`SoftwareRenderer`] may be re-used to speed up painting.
#[derive(PartialEq, Eq, Debug, Clone, Default)]
pub enum RepaintBufferType {
    #[default]
    /// The full window is always redrawn. No attempt at partial rendering will be made.
    NewBuffer,
    /// Only redraw the parts that have changed since the previous call to render().
    ///
    /// This variant assumes that the same buffer is passed on every call to render() and
    /// that it still contains the previously rendered frame.
    ReusedBuffer,

    /// Redraw the part that have changed since the last two frames were drawn.
    ///
    /// This is used when using double buffering and swapping of the buffers.
    SwappedBuffers,
}

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

/// Represents a rectangular region on the screen, used for partial rendering.
///
/// The region may be composed of multiple sub-regions.
#[derive(Clone, Debug, Default)]
pub struct PhysicalRegion(PhysicalRect);

impl PhysicalRegion {
    /// Returns the size of the bounding box of this region.
    pub fn bounding_box_size(&self) -> crate::api::PhysicalSize {
        crate::api::PhysicalSize { width: self.0.width() as _, height: self.0.height() as _ }
    }
    /// Returns the origin of the bounding box of this region.
    pub fn bounding_box_origin(&self) -> crate::api::PhysicalPosition {
        crate::api::PhysicalPosition { x: self.0.origin.x as _, y: self.0.origin.y as _ }
    }
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
pub struct SoftwareRenderer {
    partial_cache: RefCell<crate::item_rendering::PartialRenderingCache>,
    repaint_buffer_type: RepaintBufferType,
    /// This is the area which we are going to redraw in the next frame, no matter if the items are dirty or not
    force_dirty: Cell<crate::item_rendering::DirtyRegion>,
    /// Force a redraw in the next frame, no matter what's dirty. Use only as a last resort.
    force_screen_refresh: Cell<bool>,
    /// This is the area which was dirty on the previous frame.
    /// Only used if repaint_buffer_type == RepaintBufferType::SwappedBuffers
    prev_frame_dirty: Cell<DirtyRegion>,
    window: RefCell<Option<Weak<dyn crate::window::WindowAdapter>>>,
}

impl SoftwareRenderer {
    /// Create a new Renderer for a given window.
    ///
    /// The `repaint_buffer_type` parameter specify what kind of buffer are passed to [`Self::render`]
    ///
    /// The `window` parameter can be coming from [`Rc::new_cyclic()`](alloc::rc::Rc::new_cyclic())
    /// since the `WindowAdapter` most likely own the Renderer
    #[doc(hidden)]
    #[deprecated(
        since = "1.0.3",
        note = "Use MinimalSoftwareWindow instead of constructing a SoftwareRenderer Directly"
    )]
    pub fn new(
        repaint_buffer_type: RepaintBufferType,
        window: Weak<dyn crate::window::WindowAdapter>,
    ) -> Self {
        Self {
            window: RefCell::new(Some(window.clone())),
            repaint_buffer_type,
            partial_cache: Default::default(),
            force_dirty: Default::default(),
            force_screen_refresh: Default::default(),
            prev_frame_dirty: Default::default(),
        }
    }

    /// Create a new Renderer for a given window.
    ///
    /// The `repaint_buffer_type` parameter specify what kind of buffer are passed to [`Self::render`]
    ///
    /// The `window` parameter can be coming from [`Rc::new_cyclic()`](alloc::rc::Rc::new_cyclic())
    /// since the `WindowAdapter` most likely own the Renderer
    #[doc(hidden)]
    pub fn new_without_window(repaint_buffer_type: RepaintBufferType) -> Self {
        Self {
            window: RefCell::new(None),
            repaint_buffer_type,
            partial_cache: Default::default(),
            force_dirty: Default::default(),
            force_screen_refresh: Default::default(),
            prev_frame_dirty: Default::default(),
        }
    }

    /// Sets the window to be use for future rendering operations. Call this before calling
    /// rendering.
    #[doc(hidden)]
    pub fn set_window(&self, window: &crate::api::Window) {
        *self.window.borrow_mut() =
            Some(Rc::downgrade(&WindowInner::from_pub(window).window_adapter().clone()));
    }

    /// Internal function to apply a dirty region depending on the dirty_tracking_policy.
    /// Returns the region to actually draw.
    fn apply_dirty_region(
        &self,
        mut dirty_region: DirtyRegion,
        screen_size: PhysicalSize,
    ) -> DirtyRegion {
        let screen_region = PhysicalRect { origin: euclid::point2(0, 0), size: screen_size };

        if self.force_screen_refresh.take() {
            dirty_region = screen_region;
        }

        match self.repaint_buffer_type {
            RepaintBufferType::NewBuffer => {
                PhysicalRect { origin: euclid::point2(0, 0), size: screen_size }
            }
            RepaintBufferType::ReusedBuffer => dirty_region,
            RepaintBufferType::SwappedBuffers => {
                dirty_region.union(&self.prev_frame_dirty.replace(dirty_region))
            }
        }
        .intersection(&screen_region)
        .unwrap_or_default()
    }

    /// Render the window to the given frame buffer.
    ///
    /// The renderer uses a cache internally and will only render the part of the window
    /// which are dirty. The `extra_draw_region` is an extra regin which will also
    /// be rendered. (eg: the previous dirty region in case of double buffering)
    /// This function returns the region that was rendered.
    ///
    /// returns the dirty region for this frame (not including the extra_draw_region)
    pub fn render(&self, buffer: &mut [impl TargetPixel], pixel_stride: usize) -> PhysicalRegion {
        let window = self
            .window
            .borrow()
            .as_ref()
            .and_then(|w| w.upgrade())
            .expect("render() called on a destroyed Window");
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
            (euclid::size2(pixel_stride as _, (buffer.len() / pixel_stride) as _), Brush::default())
        };
        let buffer_renderer = SceneBuilder::new(
            size,
            factor,
            window_inner,
            RenderToBuffer { buffer, stride: pixel_stride },
        );
        let mut renderer = crate::item_rendering::PartialRenderer::new(
            &self.partial_cache,
            self.force_dirty.take(),
            buffer_renderer,
        );

        window_inner
            .draw_contents(|components| {
                for (component, origin) in components {
                    renderer.compute_dirty_regions(component, *origin);
                }

                let dirty_region =
                    (renderer.dirty_region.to_rect().cast() * factor).round_out().cast();

                let to_draw = self.apply_dirty_region(dirty_region, size);

                renderer.combine_clip(
                    (to_draw.cast() / factor).cast(),
                    LogicalLength::zero(),
                    LogicalLength::zero(),
                );

                if !background.is_transparent() {
                    // FIXME: gradient
                    renderer
                        .actual_renderer
                        .processor
                        .process_rectangle(to_draw, background.color().into());
                }
                for (component, origin) in components {
                    crate::item_rendering::render_component_items(
                        component,
                        &mut renderer,
                        *origin,
                    );
                }

                PhysicalRegion(to_draw)
            })
            .unwrap_or_default()
    }

    /// Render the window, line by line, into the line buffer provided by the [`LineBufferProvider`].
    ///
    /// The renderer uses a cache internally and will only render the part of the window
    /// which are dirty, depending on the dirty tracking policy set in [`SoftwareRenderer::new`]
    /// This function returns the region that was rendered.
    ///
    /// The [`LineBufferProvider::process_line()`] function will be called for each line and should
    ///  provide a buffer to draw into.
    ///
    /// As an example, let's imagine we want to render into a plain buffer.
    /// (You wouldn't normally use `render_by_line` for that because the [`Self::render`] would
    /// then be more efficient)
    ///
    /// ```rust
    /// # use i_slint_core::software_renderer::{LineBufferProvider, SoftwareRenderer, Rgb565Pixel};
    /// # fn xxx<'a>(the_frame_buffer: &'a mut [Rgb565Pixel], display_width: usize, renderer: &SoftwareRenderer) {
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
    pub fn render_by_line(&self, line_buffer: impl LineBufferProvider) -> PhysicalRegion {
        let window = self
            .window
            .borrow()
            .as_ref()
            .and_then(|w| w.upgrade())
            .expect("render() called on a destroyed Window");
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
                self,
                line_buffer,
            )
        } else {
            PhysicalRegion(Default::default())
        }
    }
}

#[doc(hidden)]
impl Renderer for SoftwareRenderer {
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
        text_input: Pin<&crate::items::TextInput>,
        pos: LogicalPoint,
        font_request: crate::graphics::FontRequest,
        scale_factor: ScaleFactor,
    ) -> usize {
        let visual_representation = text_input.visual_representation(None);

        let font = fonts::match_font(&font_request, scale_factor);

        let width = (text_input.width().cast() * scale_factor).cast();
        let height = (text_input.height().cast() * scale_factor).cast();

        let pos = (pos.cast() * scale_factor).cast();

        match font {
            fonts::Font::PixelFont(pf) => {
                let layout = fonts::text_layout_for_font(&pf, &font_request, scale_factor);

                let paragraph = TextParagraphLayout {
                    string: &visual_representation.text,
                    layout,
                    max_width: width,
                    max_height: height,
                    horizontal_alignment: text_input.horizontal_alignment(),
                    vertical_alignment: text_input.vertical_alignment(),
                    wrap: text_input.wrap(),
                    overflow: TextOverflow::Clip,
                    single_line: false,
                };

                return visual_representation.map_byte_offset_from_byte_offset_in_visual_text(
                    paragraph.byte_offset_for_position((pos.x_length(), pos.y_length())),
                );
            }
            #[cfg(feature = "software-renderer-systemfonts")]
            fonts::Font::VectorFont(vf) => {
                let layout = fonts::text_layout_for_font(&vf, &font_request, scale_factor);

                let paragraph = TextParagraphLayout {
                    string: &visual_representation.text,
                    layout,
                    max_width: width,
                    max_height: height,
                    horizontal_alignment: text_input.horizontal_alignment(),
                    vertical_alignment: text_input.vertical_alignment(),
                    wrap: text_input.wrap(),
                    overflow: TextOverflow::Clip,
                    single_line: false,
                };

                return visual_representation.map_byte_offset_from_byte_offset_in_visual_text(
                    paragraph.byte_offset_for_position((pos.x_length(), pos.y_length())),
                );
            }
        };
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: Pin<&crate::items::TextInput>,
        byte_offset: usize,
        font_request: crate::graphics::FontRequest,
        scale_factor: ScaleFactor,
    ) -> LogicalRect {
        let visual_representation = text_input.visual_representation(None);

        let font = fonts::match_font(&font_request, scale_factor);

        let width = (text_input.width().cast() * scale_factor).cast();
        let height = (text_input.height().cast() * scale_factor).cast();

        let (cursor_position, cursor_height) = match font {
            fonts::Font::PixelFont(pf) => {
                let layout = fonts::text_layout_for_font(&pf, &font_request, scale_factor);

                let paragraph = TextParagraphLayout {
                    string: &visual_representation.text,
                    layout,
                    max_width: width,
                    max_height: height,
                    horizontal_alignment: text_input.horizontal_alignment(),
                    vertical_alignment: text_input.vertical_alignment(),
                    wrap: text_input.wrap(),
                    overflow: TextOverflow::Clip,
                    single_line: false,
                };

                (paragraph.cursor_pos_for_byte_offset(byte_offset), pf.height())
            }
            #[cfg(feature = "software-renderer-systemfonts")]
            fonts::Font::VectorFont(vf) => {
                let layout = fonts::text_layout_for_font(&vf, &font_request, scale_factor);

                let paragraph = TextParagraphLayout {
                    string: &visual_representation.text,
                    layout,
                    max_width: width,
                    max_height: height,
                    horizontal_alignment: text_input.horizontal_alignment(),
                    vertical_alignment: text_input.vertical_alignment(),
                    wrap: text_input.wrap(),
                    overflow: TextOverflow::Clip,
                    single_line: false,
                };

                (paragraph.cursor_pos_for_byte_offset(byte_offset), vf.height())
            }
        };

        return (PhysicalRect::new(
            PhysicalPoint::from_lengths(cursor_position.0, cursor_position.1),
            PhysicalSize::from_lengths(
                (text_input.text_cursor_width().cast() * scale_factor).cast(),
                cursor_height,
            ),
        )
        .cast()
            / scale_factor)
            .cast();
    }

    fn free_graphics_resources(
        &self,
        _component: crate::component::ComponentRef,
        items: &mut dyn Iterator<Item = Pin<crate::items::ItemRef<'_>>>,
    ) -> Result<(), crate::platform::PlatformError> {
        for item in items {
            item.cached_rendering_data_offset().release(&mut self.partial_cache.borrow_mut());
        }
        // We don't have a way to determine the screen region of the delete items, what's in the cache is relative. So
        // as a last resort, refresh everything.
        self.force_screen_refresh.set(true);
        Ok(())
    }

    fn mark_dirty_region(&self, region: crate::item_rendering::DirtyRegion) {
        self.force_dirty.set(self.force_dirty.get().union(&region))
    }

    fn register_bitmap_font(&self, font_data: &'static crate::graphics::BitmapFont) {
        fonts::register_bitmap_font(font_data);
    }

    #[cfg(feature = "software-renderer-systemfonts")]
    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        self::fonts::systemfonts::register_font_from_memory(data)
    }

    #[cfg(feature = "software-renderer-systemfonts")]
    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self::fonts::systemfonts::register_font_from_path(path)
    }

    fn default_font_size(&self) -> LogicalLength {
        self::fonts::DEFAULT_FONT_SIZE
    }
}

fn render_window_frame_by_line(
    window: &WindowInner,
    background: Brush,
    size: PhysicalSize,
    renderer: &SoftwareRenderer,
    mut line_buffer: impl LineBufferProvider,
) -> PhysicalRegion {
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
                            let texture = &scene.vectors.textures[texture_index as usize];
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
                            let texture = scene.vectors.shared_buffers
                                [shared_buffer_index as usize]
                                .as_texture();
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
                            let rr = &scene.vectors.rounded_rectangles[rectangle_index as usize];
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
                        SceneCommand::Gradient { gradient_index } => {
                            let g = &scene.vectors.gradients[gradient_index as usize];

                            draw_functions::draw_gradient_line(
                                &PhysicalRect {
                                    origin: span.pos - euclid::vec2(offset as i16, 0),
                                    size: span.size,
                                },
                                scene.current_line,
                                g,
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
    PhysicalRegion(dirty_region)
}

#[derive(Default)]
struct SceneVectors {
    textures: Vec<SceneTexture<'static>>,
    rounded_rectangles: Vec<RoundedRectangle>,
    shared_buffers: Vec<SharedBufferCommand>,
    gradients: Vec<GradientCommand>,
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

    vectors: SceneVectors,

    future_items_index: usize,
    current_items_index: usize,

    dirty_region: DirtyRegion,
}

impl Scene {
    pub fn new(
        mut items: Vec<SceneItem>,
        vectors: SceneVectors,
        dirty_region: DirtyRegion,
    ) -> Self {
        let current_line = dirty_region.origin.y_length();
        items.retain(|i| i.pos.y_length() + i.size.height_length() > current_line);
        items.sort_unstable_by(compare_scene_item);
        let current_items_index = items.partition_point(|i| i.pos.y_length() <= current_line);
        items[..current_items_index].sort_unstable_by(|a, b| b.z.cmp(&a.z));
        Self {
            items,
            current_line,
            current_items_index,
            future_items_index: current_items_index,
            vectors,
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
    /// texture_index is an index in the [`SceneVectors::textures`] array
    Texture {
        texture_index: u16,
    },
    /// shared_buffer_index is an index in [`SceneVectors::shared_buffers`]
    SharedBuffer {
        shared_buffer_index: u16,
    },
    /// rectangle_index is an index in the [`SceneVectors::rounded_rectangle`] array
    RoundedRectangle {
        rectangle_index: u16,
    },
    /// rectangle_index is an index in the [`SceneVectors::rounded_gradients`] array
    Gradient {
        gradient_index: u16,
    },
}

struct SceneTexture<'a> {
    data: &'a [u8],
    format: PixelFormat,
    /// bytes between two lines in the source
    stride: u16,
    source_size: PhysicalSize,
    /// Color to colorize. When not transparent, consider that the image is an alpha map and always use that color.
    /// The alpha of this color is ignored. (it is supposed to be mixed in `Self::alpha`)
    color: Color,
    alpha: u8,
}

enum SharedBufferData {
    SharedImage(SharedImageBuffer),
    AlphaMap { data: Rc<[u8]>, width: u16 },
}

impl SharedBufferData {
    fn width(&self) -> usize {
        match self {
            SharedBufferData::SharedImage(image) => image.width() as usize,
            SharedBufferData::AlphaMap { width, .. } => *width as usize,
        }
    }
}

struct SharedBufferCommand {
    buffer: SharedBufferData,
    /// The source rectangle that is mapped into this command span
    source_rect: PhysicalRect,
    colorize: Color,
    alpha: u8,
}

impl SharedBufferCommand {
    fn as_texture(&self) -> SceneTexture<'_> {
        let begin = self.buffer.width() * self.source_rect.min_y() as usize
            + self.source_rect.min_x() as usize;

        match &self.buffer {
            SharedBufferData::SharedImage(SharedImageBuffer::RGB8(b)) => SceneTexture {
                data: &b.as_bytes()[begin * 3..],
                stride: 3 * b.width() as u16,
                format: PixelFormat::Rgb,
                source_size: self.source_rect.size,
                color: self.colorize,
                alpha: self.alpha,
            },
            SharedBufferData::SharedImage(SharedImageBuffer::RGBA8(b)) => SceneTexture {
                data: &b.as_bytes()[begin * 4..],
                stride: 4 * b.width() as u16,
                format: PixelFormat::Rgba,
                source_size: self.source_rect.size,
                color: self.colorize,
                alpha: self.alpha,
            },
            SharedBufferData::SharedImage(SharedImageBuffer::RGBA8Premultiplied(b)) => {
                SceneTexture {
                    data: &b.as_bytes()[begin * 4..],
                    stride: 4 * b.width() as u16,
                    format: PixelFormat::RgbaPremultiplied,
                    source_size: self.source_rect.size,
                    color: self.colorize,
                    alpha: self.alpha,
                }
            }
            SharedBufferData::AlphaMap { data, width } => SceneTexture {
                data: &data[begin..],
                stride: *width,
                format: PixelFormat::AlphaMap,
                source_size: self.source_rect.size,
                color: self.colorize,
                alpha: self.alpha,
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

/// Goes from color 1 to color2
///
/// depending of `flags & 0b1`
///  - if false: on the left side, goes from `start` to 1, on the right side, goes from 0 to `1-start`
///  - if true: on the left side, goes from 0 to `1-start`, on the right side, goes from `start` to `1`
#[derive(Debug)]
struct GradientCommand {
    color1: PremultipliedRgbaColor,
    color2: PremultipliedRgbaColor,
    start: u8,
    /// bit 0: if the slope is positive or negative
    /// bit 1: if we should fill with color1 on the left side when left_clip is negative (or transparent)
    /// bit 2: if we should fill with color2 on the left side when right_clip is negative (or transparent)
    flags: u8,
    /// If positive, the clip has the same meaning as in RoundedRectangle.
    /// If negative, that means the "stop" is only starting or stopping at that point
    left_clip: PhysicalLength,
    right_clip: PhysicalLength,
    top_clip: PhysicalLength,
    bottom_clip: PhysicalLength,
}

fn prepare_scene(
    window: &WindowInner,
    size: PhysicalSize,
    software_renderer: &SoftwareRenderer,
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
    Scene::new(prepare_scene.processor.items, prepare_scene.processor.vectors, dirty_region)
}

trait ProcessScene {
    fn process_texture(&mut self, geometry: PhysicalRect, texture: SceneTexture<'static>);
    fn process_rectangle(&mut self, geometry: PhysicalRect, color: PremultipliedRgbaColor);
    fn process_rounded_rectangle(&mut self, geometry: PhysicalRect, data: RoundedRectangle);
    fn process_shared_image_buffer(&mut self, geometry: PhysicalRect, buffer: SharedBufferCommand);
    fn process_gradient(&mut self, geometry: PhysicalRect, gradient: GradientCommand);
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

    fn process_rectangle(&mut self, geometry: PhysicalRect, color: PremultipliedRgbaColor) {
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

    fn process_gradient(&mut self, geometry: PhysicalRect, g: GradientCommand) {
        for line in geometry.min_y()..geometry.max_y() {
            draw_functions::draw_gradient_line(
                &geometry,
                PhysicalLength::new(line),
                &g,
                &mut self.buffer[line as usize * self.stride..],
            );
        }
    }
}

#[derive(Default)]
struct PrepareScene {
    items: Vec<SceneItem>,
    vectors: SceneVectors,
}

impl ProcessScene for PrepareScene {
    fn process_texture(&mut self, geometry: PhysicalRect, texture: SceneTexture<'static>) {
        let size = geometry.size;
        if !size.is_empty() {
            let texture_index = self.vectors.textures.len() as u16;
            self.vectors.textures.push(texture);
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
            let shared_buffer_index = self.vectors.shared_buffers.len() as u16;
            self.vectors.shared_buffers.push(buffer);
            self.items.push(SceneItem {
                pos: geometry.origin,
                size,
                z: self.items.len() as u16,
                command: SceneCommand::SharedBuffer { shared_buffer_index },
            });
        }
    }

    fn process_rectangle(&mut self, geometry: PhysicalRect, color: PremultipliedRgbaColor) {
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
            let rectangle_index = self.vectors.rounded_rectangles.len() as u16;
            self.vectors.rounded_rectangles.push(data);
            self.items.push(SceneItem {
                pos: geometry.origin,
                size,
                z: self.items.len() as u16,
                command: SceneCommand::RoundedRectangle { rectangle_index },
            });
        }
    }

    fn process_gradient(&mut self, geometry: PhysicalRect, gradient: GradientCommand) {
        let size = geometry.size;
        if !size.is_empty() {
            let gradient_index = self.vectors.gradients.len() as u16;
            self.vectors.gradients.push(gradient);
            self.items.push(SceneItem {
                pos: geometry.origin,
                size,
                z: self.items.len() as u16,
                command: SceneCommand::Gradient { gradient_index },
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
        let global_alpha_u16 = (self.current_state.alpha * 255.) as u16;
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
            .translate(-image_fit_offset)
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
                        let color = if colorize.alpha() > 0 { colorize } else { t.color };
                        let alpha = if colorize.alpha() > 0 || t.format == PixelFormat::AlphaMap {
                            color.alpha() as u16 * global_alpha_u16 / 255
                        } else {
                            global_alpha_u16
                        } as u8;

                        self.processor.process_texture(
                            target_rect.cast(),
                            SceneTexture {
                                data: &data.as_slice()[(t.index
                                    + (stride as usize) * actual_y
                                    + (t.format.bpp()) * actual_x)..],
                                stride,
                                source_size: clipped_relative_source_rect.size.ceil().cast(),
                                format: t.format,
                                color,
                                alpha,
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

                        let alpha = if colorize.alpha() > 0 {
                            colorize.alpha() as u16 * global_alpha_u16 / 255
                        } else {
                            global_alpha_u16
                        } as u8;

                        self.processor.process_shared_image_buffer(
                            target_rect.cast(),
                            SharedBufferCommand {
                                buffer: SharedBufferData::SharedImage(buffer),
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
                                alpha,
                            },
                        );
                    }
                } else {
                    unimplemented!("The image cannot be rendered")
                }
            }
        };
    }

    fn draw_text_paragraph<'b, Font: AbstractFont>(
        &mut self,
        paragraph: &TextParagraphLayout<'b, Font>,
        physical_clip: euclid::Rect<f32, PhysicalPx>,
        offset: euclid::Vector2D<f32, PhysicalPx>,
        color: Color,
    ) where
        Font: crate::textlayout::TextShaper<Length = PhysicalLength>,
        Font: GlyphRenderer,
    {
        paragraph
            .layout_lines::<()>(|glyphs, line_x, line_y, _| {
                let baseline_y = line_y + paragraph.layout.font.ascent();
                while let Some(positioned_glyph) = glyphs.next() {
                    let glyph = paragraph.layout.font.render_glyph(positioned_glyph.glyph_id);

                    let src_rect = PhysicalRect::new(
                        PhysicalPoint::from_lengths(
                            line_x + positioned_glyph.x + glyph.x,
                            baseline_y - glyph.y - glyph.height,
                        ),
                        glyph.size(),
                    )
                    .cast();

                    if let Some(clipped_src) = src_rect.intersection(&physical_clip) {
                        let geometry = clipped_src.translate(offset).round();
                        let origin = (geometry.origin - offset.round()).cast::<usize>();
                        let actual_x = origin.x - src_rect.origin.x as usize;
                        let actual_y = origin.y - src_rect.origin.y as usize;
                        let stride = glyph.width.get() as u16;
                        let geometry = geometry.cast();

                        match &glyph.alpha_map {
                            fonts::GlyphAlphaMap::Static(data) => {
                                self.processor.process_texture(
                                    geometry,
                                    SceneTexture {
                                        data: &data[actual_x + actual_y * stride as usize..],
                                        stride,
                                        source_size: geometry.size,
                                        format: PixelFormat::AlphaMap,
                                        color,
                                        // color already is mixed with global alpha
                                        alpha: color.alpha(),
                                    },
                                );
                            }
                            fonts::GlyphAlphaMap::Shared(data) => {
                                self.processor.process_shared_image_buffer(
                                    geometry,
                                    SharedBufferCommand {
                                        buffer: SharedBufferData::AlphaMap {
                                            data: data.clone(),
                                            width: stride,
                                        },
                                        source_rect: PhysicalRect::new(
                                            PhysicalPoint::new(actual_x as _, actual_y as _),
                                            geometry.size,
                                        ),
                                        colorize: color,
                                        // color already is mixed with global alpha
                                        alpha: color.alpha(),
                                    },
                                );
                            }
                        };
                    }
                }
                core::ops::ControlFlow::Continue(())
            })
            .ok();
    }

    /// Returns the color, mixed with the current_state's alpha
    fn alpha_color(&self, color: Color) -> Color {
        if self.current_state.alpha < 1.0 {
            Color::from_argb_u8(
                (color.alpha() as f32 * self.current_state.alpha) as u8,
                color.red(),
                color.green(),
                color.blue(),
            )
        } else {
            color
        }
    }
}

#[derive(Clone, Copy)]
struct RenderState {
    alpha: f32,
    offset: LogicalPoint,
    clip: LogicalRect,
}

impl<'a, T: ProcessScene> crate::item_rendering::ItemRenderer for SceneBuilder<'a, T> {
    fn draw_rectangle(
        &mut self,
        rect: Pin<&crate::items::Rectangle>,
        _: &ItemRc,
        size: LogicalSize,
    ) {
        let geom = LogicalRect::from(size);
        if self.should_draw(&geom) {
            let clipped = match geom.intersection(&self.current_state.clip) {
                Some(geom) => geom,
                None => return,
            };

            let background = rect.background();
            if let Brush::LinearGradient(g) = background {
                let geom2 = geom.cast() * self.scale_factor;
                let clipped2 = clipped.cast() * self.scale_factor;
                let act_rect = (clipped.translate(self.current_state.offset.to_vector()).cast()
                    * self.scale_factor)
                    .round()
                    .cast();

                let angle = g.angle();

                let tan = angle.to_radians().tan().abs();
                let start = if !tan.is_finite() {
                    255.
                } else {
                    let h = tan * geom.width() as f32;
                    255. * h / (h + geom.height() as f32)
                } as u8;
                let mut angle = angle as i32 % 360;
                if angle < 0 {
                    angle += 360;
                }
                let mut stops = g.stops().copied().peekable();
                let mut idx = 0;
                let stop_count = g.stops().count();
                while let (Some(mut s1), Some(mut s2)) = (stops.next(), stops.peek().copied()) {
                    let mut flags = 0;
                    if (angle % 180) > 90 {
                        flags |= 0b1;
                    }
                    if angle <= 90 || angle > 270 {
                        core::mem::swap(&mut s1, &mut s2);
                        s1.position = 1. - s1.position;
                        s2.position = 1. - s2.position;
                        if idx == 0 {
                            flags |= 0b100;
                        }
                        if idx == stop_count - 2 {
                            flags |= 0b010;
                        }
                    } else {
                        if idx == 0 {
                            flags |= 0b010;
                        }
                        if idx == stop_count - 2 {
                            flags |= 0b100;
                        }
                    }

                    idx += 1;

                    let (adjust_left, adjust_right) = if (angle % 180) > 90 {
                        (
                            (geom2.width() * s1.position).floor() as i16,
                            (geom2.width() * (1. - s2.position)).ceil() as i16,
                        )
                    } else {
                        (
                            (geom2.width() * (1. - s2.position)).ceil() as i16,
                            (geom2.width() * s1.position).floor() as i16,
                        )
                    };

                    let gr = GradientCommand {
                        color1: self.alpha_color(s1.color).into(),
                        color2: self.alpha_color(s2.color).into(),
                        start,
                        flags,
                        top_clip: Length::new(
                            (clipped2.min_y() - geom2.min_y()) as i16
                                - (geom2.height() * s1.position).floor() as i16,
                        ),
                        bottom_clip: Length::new(
                            (geom2.max_y() - clipped2.max_y()) as i16
                                - (geom2.height() * (1. - s2.position)).ceil() as i16,
                        ),
                        left_clip: Length::new(
                            (clipped2.min_x() - geom2.min_x()) as i16 - adjust_left,
                        ),
                        right_clip: Length::new(
                            (geom2.max_x() - clipped2.max_x()) as i16 - adjust_right,
                        ),
                    };

                    let size_y = act_rect.height_length() + gr.top_clip + gr.bottom_clip;
                    let size_x = act_rect.width_length() + gr.left_clip + gr.right_clip;
                    if size_x.get() == 0 || size_y.get() == 0 {
                        // the position are too close to each other
                        // FIXME: For the first or the last, we should draw a plain color to the end
                        continue;
                    }

                    self.processor.process_gradient(act_rect, gr);
                }
                return;
            }

            // FIXME: gradients
            let color = self.alpha_color(background.color());

            if color.alpha() == 0 {
                return;
            }
            self.processor.process_rectangle(
                (clipped.translate(self.current_state.offset.to_vector()).cast()
                    * self.scale_factor)
                    .round()
                    .cast(),
                color.into(),
            );
        }
    }

    fn draw_border_rectangle(
        &mut self,
        rect: Pin<&crate::items::BorderRectangle>,
        _: &ItemRc,
        size: LogicalSize,
    ) {
        let geom = LogicalRect::from(size);
        if self.should_draw(&geom) {
            let mut border = rect.border_width();
            let radius = rect.border_radius();
            // FIXME: gradients
            let color = self.alpha_color(rect.background().color());
            let border_color = if border.get() as f32 > 0.01 {
                self.alpha_color(rect.border_color().color())
            } else {
                Color::default()
            };

            let mut border_color = PremultipliedRgbaColor::from(border_color);
            let color = PremultipliedRgbaColor::from(color);
            if border_color.alpha == 0 {
                border = LogicalLength::new(0 as _);
            } else if border_color.alpha < 255 {
                // Find a color for the border which is an equivalent to blend the background and then the border.
                // In the end, the resulting of blending the background and the color is
                // (A + B) + C, where A is the buffer color, B is the background, and C is the border.
                // which expands to (A*(1-Bα) + B*Bα)*(1-Cα) + C*Cα = A*(1-(Bα+Cα-Bα*Cα)) + B*Bα*(1-Cα) + C*Cα
                // so let the new alpha be: Nα = Bα+Cα-Bα*Cα, then this is A*(1-Nα) + N*Nα
                // with N = (B*Bα*(1-Cα) + C*Cα)/Nα
                // N being the equivalent color of the border that mixes the background and the border
                // In pre-multiplied space, the formula simplifies further N' = B'*(1-Cα) + C'
                let b = border_color;
                let b_alpha_16 = b.alpha as u16;
                border_color = PremultipliedRgbaColor {
                    red: ((color.red as u16 * (255 - b_alpha_16)) / 255) as u8 + b.red as u8,
                    green: ((color.green as u16 * (255 - b_alpha_16)) / 255) as u8 + b.green as u8,
                    blue: ((color.blue as u16 * (255 - b_alpha_16)) / 255) as u8 + b.blue as u8,
                    alpha: (color.alpha as u16 + b_alpha_16
                        - (color.alpha as u16 * b_alpha_16) / 255) as u8,
                }
            }

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
                            border_color,
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

            if color.alpha > 0 {
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
            if border_color.alpha > 0 {
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

    fn draw_image(&mut self, image: Pin<&crate::items::ImageItem>, _: &ItemRc, size: LogicalSize) {
        let geom = LogicalRect::from(size);
        if self.should_draw(&geom) {
            let source = image.source();
            self.draw_image_impl(
                geom,
                &source,
                euclid::Rect::new(Default::default(), source.size().cast()),
                image.image_fit(),
                image.colorize().color(),
            );
        }
    }

    fn draw_clipped_image(
        &mut self,
        image: Pin<&crate::items::ClippedImage>,
        _: &ItemRc,
        size: LogicalSize,
    ) {
        let geom = LogicalRect::from(size);
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

    fn draw_text(&mut self, text: Pin<&crate::items::Text>, _: &ItemRc, size: LogicalSize) {
        let string = text.text();
        if string.trim().is_empty() {
            return;
        }
        let geom = LogicalRect::from(size);
        if !self.should_draw(&geom) {
            return;
        }

        let font_request = text.font_request(self.window);

        let color = self.alpha_color(text.color().color());
        let max_size = (geom.size.cast() * self.scale_factor).cast();

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

        let font = fonts::match_font(&font_request, self.scale_factor);

        match font {
            fonts::Font::PixelFont(pf) => {
                let layout = fonts::text_layout_for_font(&pf, &font_request, self.scale_factor);

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

                self.draw_text_paragraph(&paragraph, physical_clip, offset, color);
            }
            #[cfg(feature = "software-renderer-systemfonts")]
            fonts::Font::VectorFont(vf) => {
                let layout = fonts::text_layout_for_font(&vf, &font_request, self.scale_factor);

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

                self.draw_text_paragraph(&paragraph, physical_clip, offset, color);
            }
        }
    }

    fn draw_text_input(
        &mut self,
        text_input: Pin<&crate::items::TextInput>,
        _: &ItemRc,
        size: LogicalSize,
    ) {
        let geom = LogicalRect::from(size);
        if !self.should_draw(&geom) {
            return;
        }

        let font_request = text_input.font_request(&self.window.window_adapter());

        let color = self.alpha_color(text_input.color().color());
        let max_size = (geom.size.cast() * self.scale_factor).cast();

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

        let font = fonts::match_font(&font_request, self.scale_factor);

        let text_visual_representation = text_input.visual_representation(None);

        let cursor_pos_and_height = match font {
            fonts::Font::PixelFont(pf) => {
                let paragraph = TextParagraphLayout {
                    string: &text_visual_representation.text,
                    layout: fonts::text_layout_for_font(&pf, &font_request, self.scale_factor),
                    max_width: max_size.width_length(),
                    max_height: max_size.height_length(),
                    horizontal_alignment: text_input.horizontal_alignment(),
                    vertical_alignment: text_input.vertical_alignment(),
                    wrap: text_input.wrap(),
                    overflow: TextOverflow::Clip,
                    single_line: text_input.single_line(),
                };

                self.draw_text_paragraph(&paragraph, physical_clip, offset, color);

                text_visual_representation.cursor_position.map(|cursor_offset| {
                    (paragraph.cursor_pos_for_byte_offset(cursor_offset), pf.height())
                })
            }
            #[cfg(feature = "software-renderer-systemfonts")]
            fonts::Font::VectorFont(vf) => {
                let paragraph = TextParagraphLayout {
                    string: &text_visual_representation.text,
                    layout: fonts::text_layout_for_font(&vf, &font_request, self.scale_factor),
                    max_width: max_size.width_length(),
                    max_height: max_size.height_length(),
                    horizontal_alignment: text_input.horizontal_alignment(),
                    vertical_alignment: text_input.vertical_alignment(),
                    wrap: text_input.wrap(),
                    overflow: TextOverflow::Clip,
                    single_line: text_input.single_line(),
                };

                self.draw_text_paragraph(&paragraph, physical_clip, offset, color);

                text_visual_representation.cursor_position.map(|cursor_offset| {
                    (paragraph.cursor_pos_for_byte_offset(cursor_offset), vf.height())
                })
            }
        };

        if let Some(((cursor_x, cursor_y), cursor_height)) = cursor_pos_and_height {
            let cursor_rect = PhysicalRect::new(
                PhysicalPoint::from_lengths(cursor_x, cursor_y),
                PhysicalSize::from_lengths(
                    (text_input.text_cursor_width().cast() * self.scale_factor).cast(),
                    cursor_height,
                ),
            );

            if let Some(clipped_src) = cursor_rect.intersection(&physical_clip.cast()) {
                self.processor
                    .process_rectangle(clipped_src.translate(offset.cast()), color.into());
            }
        }
    }

    #[cfg(feature = "std")]
    fn draw_path(&mut self, _path: Pin<&crate::items::Path>, _: &ItemRc, _size: LogicalSize) {
        // TODO
    }

    fn draw_box_shadow(
        &mut self,
        _box_shadow: Pin<&crate::items::BoxShadow>,
        _: &ItemRc,
        _size: LogicalSize,
    ) {
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
        _item: &ItemRc,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        // FIXME: actually cache the pixmap
        update_fn(&mut |width, height, data| {
            let img = SharedImageBuffer::RGBA8Premultiplied(SharedPixelBuffer::clone_from_slice(
                data, width, height,
            ));

            let physical_clip = self.current_state.clip.cast() * self.scale_factor;
            let src_rect = euclid::rect(0., 0., width as f32, height as f32);

            if let Some(clipped_src) = src_rect.intersection(&physical_clip) {
                let offset = self.current_state.offset.to_vector().cast() * self.scale_factor;
                let geometry = clipped_src.translate(offset).round();
                let origin = (geometry.origin - offset.round()).cast::<usize>();
                let actual_x = origin.x - src_rect.origin.x as usize;
                let actual_y = origin.y - src_rect.origin.y as usize;
                let geometry = geometry.cast();

                self.processor.process_shared_image_buffer(
                    geometry,
                    SharedBufferCommand {
                        buffer: SharedBufferData::SharedImage(img),
                        source_rect: PhysicalRect::new(
                            PhysicalPoint::new(actual_x as _, actual_y as _),
                            geometry.size,
                        ),
                        colorize: Default::default(),
                        alpha: (self.current_state.alpha * 255.) as u8,
                    },
                );
            }
        });
    }

    fn draw_string(&mut self, _string: &str, _color: Color) {
        todo!()
    }

    fn window(&self) -> &crate::window::WindowInner {
        self.window
    }

    fn as_any(&mut self) -> Option<&mut dyn core::any::Any> {
        None
    }
}

/// This is a minimal adapter for a Window that doesn't have any other feature than rendering
/// using the software renderer.
pub struct MinimalSoftwareWindow {
    window: Window,
    renderer: SoftwareRenderer,
    needs_redraw: Cell<bool>,
    size: Cell<crate::api::PhysicalSize>,
}

impl MinimalSoftwareWindow {
    /// Instantiate a new MinimalWindowAdaptor
    ///
    /// The `repaint_buffer_type` parameter specify what kind of buffer are passed to the [`SoftwareRenderer`]
    pub fn new(repaint_buffer_type: RepaintBufferType) -> Rc<Self> {
        Rc::new_cyclic(|w: &Weak<Self>| Self {
            window: Window::new(w.clone()),
            renderer: SoftwareRenderer::new_without_window(repaint_buffer_type),
            needs_redraw: Default::default(),
            size: Default::default(),
        })
    }
    /// If the window needs to be redrawn, the callback will be called with the
    /// [renderer](SoftwareRenderer) that should be used to do the drawing.
    ///
    /// [`SoftwareRenderer::render()`] or [`SoftwareRenderer::render_by_line()`] should be called
    /// in that callback.
    ///
    /// Return true if something was redrawn.
    pub fn draw_if_needed(&self, render_callback: impl FnOnce(&SoftwareRenderer)) -> bool {
        if self.needs_redraw.replace(false) {
            self.renderer.set_window(&self.window);
            render_callback(&self.renderer);
            true
        } else {
            false
        }
    }
}

impl crate::window::WindowAdapterSealed for MinimalSoftwareWindow {
    fn request_redraw(&self) {
        self.needs_redraw.set(true);
    }
    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn unregister_component<'a>(
        &self,
        _component: crate::component::ComponentRef,
        _items: &mut dyn Iterator<Item = Pin<crate::items::ItemRef<'a>>>,
    ) {
    }

    fn size(&self) -> crate::api::PhysicalSize {
        self.size.get()
    }
    fn set_size(&self, size: crate::api::WindowSize) {
        self.size.set(size.to_physical(1.));
        self.window
            .dispatch_event(crate::platform::WindowEvent::Resized { size: size.to_logical(1.) })
    }
}

impl WindowAdapter for MinimalSoftwareWindow {
    fn window(&self) -> &Window {
        &self.window
    }
}

impl core::ops::Deref for MinimalSoftwareWindow {
    type Target = Window;
    fn deref(&self) -> &Self::Target {
        &self.window
    }
}
