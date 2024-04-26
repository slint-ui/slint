// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

//! This module contains the [`SoftwareRenderer`] and related types.
//!
//! It is only enabled when the `renderer-software` Slint feature is enabled.

#![warn(missing_docs)]

mod draw_functions;
mod fixed;
mod fonts;

use self::fonts::GlyphRenderer;
use crate::api::Window;
use crate::graphics::rendering_metrics_collector::{RefreshMode, RenderingMetricsCollector};
use crate::graphics::{BorderRadius, PixelFormat, SharedImageBuffer, SharedPixelBuffer};
use crate::item_rendering::{CachedRenderingData, DirtyRegion, RenderBorderRectangle, RenderImage};
use crate::items::{ItemRc, TextOverflow};
use crate::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalRect, LogicalSize, LogicalVector,
    PhysicalPx, PointLengths, RectLengths, ScaleFactor, SizeLengths,
};
use crate::renderer::{Renderer, RendererSealed};
use crate::textlayout::{AbstractFont, FontMetrics, TextParagraphLayout};
use crate::window::{WindowAdapter, WindowInner};
use crate::{Brush, Color, Coord, ImageInner, StaticTextures};
use alloc::rc::{Rc, Weak};
#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec};
use core::cell::{Cell, RefCell};
use core::pin::Pin;
use euclid::Length;
use fixed::Fixed;
#[allow(unused)]
use num_traits::Float;
use num_traits::NumCast;

pub use draw_functions::{PremultipliedRgbaColor, Rgb565Pixel, TargetPixel};

type PhysicalLength = euclid::Length<i16, PhysicalPx>;
type PhysicalRect = euclid::Rect<i16, PhysicalPx>;
type PhysicalSize = euclid::Size2D<i16, PhysicalPx>;
type PhysicalPoint = euclid::Point2D<i16, PhysicalPx>;
type PhysicalBorderRadius = BorderRadius<i16, PhysicalPx>;

/// This enum describes which parts of the buffer passed to the [`SoftwareRenderer`] may be re-used to speed up painting.
// FIXME: #[non_exhaustive] #3023
#[derive(PartialEq, Eq, Debug, Clone, Default, Copy)]
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

/// This module is just a trick to make the Window public only when `feature = "software-renderer-rotation"`
#[allow(unused)]
mod internal {
    use super::*;
    /// This enum describes the rotation that should be applied to the contents rendered by the software renderer.
    ///
    /// Argument to be passed in [`SoftwareRenderer::set_rendering_rotation`].
    #[non_exhaustive]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Debug)]
    pub enum RenderingRotation {
        /// No rotation
        #[default]
        NoRotation,
        /// Rotate 90° to the right
        Rotate90,
        /// 180° rotation (upside-down)
        Rotate180,
        /// Rotate 90° to the left
        Rotate270,
    }
}

#[cfg(feature = "software-renderer-rotation")]
pub use internal::RenderingRotation;
#[cfg(not(feature = "software-renderer-rotation"))]
use internal::RenderingRotation;

impl RenderingRotation {
    fn is_transpose(self) -> bool {
        matches!(self, Self::Rotate90 | Self::Rotate270)
    }
    fn mirror_width(self) -> bool {
        matches!(self, Self::Rotate270 | Self::Rotate180)
    }
    fn mirror_height(self) -> bool {
        matches!(self, Self::Rotate90 | Self::Rotate180)
    }
    /// Angle of the rotation in degrees
    fn angle(self) -> f32 {
        match self {
            RenderingRotation::NoRotation => 0.,
            RenderingRotation::Rotate90 => 90.,
            RenderingRotation::Rotate180 => 180.,
            RenderingRotation::Rotate270 => 270.,
        }
    }
}

#[derive(Copy, Clone)]
struct RotationInfo {
    orientation: RenderingRotation,
    screen_size: PhysicalSize,
}

/// Extension trait for euclid type to transpose coordinates (swap x and y, as well as width and height)
trait Transform {
    /// Return a copy of Self whose coordinate are swapped (x swapped with y)
    #[must_use]
    fn transformed(self, info: RotationInfo) -> Self;
}

impl<T: Copy + NumCast + core::ops::Sub<Output = T>> Transform for euclid::Point2D<T, PhysicalPx> {
    fn transformed(mut self, info: RotationInfo) -> Self {
        if info.orientation.mirror_width() {
            self.x = T::from(info.screen_size.width).unwrap() - self.x - T::from(1).unwrap()
        }
        if info.orientation.mirror_height() {
            self.y = T::from(info.screen_size.height).unwrap() - self.y - T::from(1).unwrap()
        }
        if info.orientation.is_transpose() {
            core::mem::swap(&mut self.x, &mut self.y);
        }
        self
    }
}

impl<T: Copy> Transform for euclid::Size2D<T, PhysicalPx> {
    fn transformed(mut self, info: RotationInfo) -> Self {
        if info.orientation.is_transpose() {
            core::mem::swap(&mut self.width, &mut self.height);
        }
        self
    }
}

impl<T: Copy + NumCast + core::ops::Sub<Output = T>> Transform for euclid::Rect<T, PhysicalPx> {
    fn transformed(self, info: RotationInfo) -> Self {
        let one = T::from(1).unwrap();
        let mut origin = self.origin.transformed(info);
        let size = self.size.transformed(info);
        if info.orientation.mirror_width() {
            origin.y = origin.y - (size.height - one);
        }
        if info.orientation.mirror_height() {
            origin.x = origin.x - (size.width - one);
        }
        Self::new(origin, size)
    }
}

impl<T: Copy> Transform for BorderRadius<T, PhysicalPx> {
    fn transformed(self, info: RotationInfo) -> Self {
        match info.orientation {
            RenderingRotation::NoRotation => self,
            RenderingRotation::Rotate90 => {
                Self::new(self.bottom_left, self.top_left, self.top_right, self.bottom_right)
            }
            RenderingRotation::Rotate180 => {
                Self::new(self.bottom_right, self.bottom_left, self.top_left, self.top_right)
            }
            RenderingRotation::Rotate270 => {
                Self::new(self.top_right, self.bottom_right, self.bottom_left, self.top_left)
            }
        }
    }
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

#[cfg(not(cbindgen))]
const PHYSICAL_REGION_MAX_SIZE: usize = DirtyRegion::MAX_COUNT;
// cbindgen can't understand associated const correctly, so hardcode the value
#[cfg(cbindgen)]
pub const PHYSICAL_REGION_MAX_SIZE: usize = 3;
const _: () = {
    assert!(PHYSICAL_REGION_MAX_SIZE == 3);
    assert!(DirtyRegion::MAX_COUNT == 3);
};

/// Represents a rectangular region on the screen, used for partial rendering.
///
/// The region may be composed of multiple sub-regions.
#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct PhysicalRegion {
    rectangles: [euclid::Box2D<i16, PhysicalPx>; PHYSICAL_REGION_MAX_SIZE],
    count: usize,
}

impl PhysicalRegion {
    fn iter_box(&self) -> impl Iterator<Item = euclid::Box2D<i16, PhysicalPx>> + '_ {
        (0..self.count).map(|x| self.rectangles[x])
    }

    fn bounding_rect(&self) -> PhysicalRect {
        if self.count == 0 {
            return Default::default();
        }
        let mut r = self.rectangles[0];
        for i in 1..self.count {
            r = r.union(&self.rectangles[i]);
        }
        r.to_rect()
    }

    /// Returns the size of the bounding box of this region.
    pub fn bounding_box_size(&self) -> crate::api::PhysicalSize {
        let bb = self.bounding_rect();
        crate::api::PhysicalSize { width: bb.width() as _, height: bb.height() as _ }
    }
    /// Returns the origin of the bounding box of this region.
    pub fn bounding_box_origin(&self) -> crate::api::PhysicalPosition {
        let bb = self.bounding_rect();
        crate::api::PhysicalPosition { x: bb.origin.x as _, y: bb.origin.y as _ }
    }

    /// Returns an iterator over the rectangles in this region.
    /// Each rectangle is represented by its position and its size.
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (crate::api::PhysicalPosition, crate::api::PhysicalSize)> + '_ {
        self.iter_box().map(|r| {
            let r = r.to_rect();
            (
                crate::api::PhysicalPosition { x: r.origin.x as _, y: r.origin.y as _ },
                crate::api::PhysicalSize { width: r.width() as _, height: r.height() as _ },
            )
        })
    }
}

/// Computes what are the x ranges that intersects the region for specified y line.
///
/// This uses a mutable reference to a Vec so that the memory is re-used between calls.
///
/// Returns the y position until which this range is valid
fn region_line_ranges(
    region: &PhysicalRegion,
    line: i16,
    line_ranges: &mut Vec<core::ops::Range<i16>>,
) -> Option<i16> {
    line_ranges.clear();
    let mut next_validity = None::<i16>;
    for geom in region.iter_box() {
        if geom.is_empty() {
            continue;
        }
        if geom.y_range().contains(&line) {
            match &mut next_validity {
                Some(val) => *val = geom.max.y.min(*val),
                None => next_validity = Some(geom.max.y),
            }
            let mut tmp = Some(geom.x_range());
            line_ranges.retain_mut(|it| {
                if let Some(r) = &mut tmp {
                    if it.end < r.start {
                        return true;
                    } else if it.start <= r.start {
                        if it.end >= r.end {
                            tmp = None;
                            return true;
                        }
                        r.start = it.start;
                        return false;
                    } else if it.start <= r.end {
                        if it.end <= r.end {
                            return false;
                        } else {
                            it.start = r.start;
                            tmp = None;
                            return true;
                        }
                    } else {
                        core::mem::swap(it, r);
                        return true;
                    }
                } else {
                    return true;
                }
            });
            if let Some(r) = tmp {
                line_ranges.push(r);
            }
            continue;
        } else {
            if geom.min.y >= line {
                match &mut next_validity {
                    Some(val) => *val = geom.min.y.min(*val),
                    None => next_validity = Some(geom.min.y),
                }
            }
        }
    }
    // check that current items are properly sorted
    debug_assert!(line_ranges.windows(2).all(|x| x[0].end < x[1].start));
    next_validity
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
    repaint_buffer_type: Cell<RepaintBufferType>,
    /// This is the area which we are going to redraw in the next frame, no matter if the items are dirty or not
    force_dirty: RefCell<DirtyRegion>,
    /// Force a redraw in the next frame, no matter what's dirty. Use only as a last resort.
    force_screen_refresh: Cell<bool>,
    /// This is the area which was dirty on the previous frame.
    /// Only used if repaint_buffer_type == RepaintBufferType::SwappedBuffers
    prev_frame_dirty: Cell<DirtyRegion>,
    maybe_window_adapter: RefCell<Option<Weak<dyn crate::window::WindowAdapter>>>,
    rotation: Cell<RenderingRotation>,
    rendering_metrics_collector: Option<Rc<RenderingMetricsCollector>>,
}

impl Default for SoftwareRenderer {
    fn default() -> Self {
        Self {
            partial_cache: Default::default(),
            repaint_buffer_type: Default::default(),
            force_dirty: Default::default(),
            force_screen_refresh: Default::default(),
            prev_frame_dirty: Default::default(),
            maybe_window_adapter: Default::default(),
            rotation: Default::default(),
            rendering_metrics_collector: RenderingMetricsCollector::new("software"),
        }
    }
}

impl SoftwareRenderer {
    /// Create a new Renderer
    pub fn new() -> Self {
        Default::default()
    }

    /// Create a new SoftwareRenderer.
    ///
    /// The `repaint_buffer_type` parameter specify what kind of buffer are passed to [`Self::render`]
    pub fn new_with_repaint_buffer_type(repaint_buffer_type: RepaintBufferType) -> Self {
        Self { repaint_buffer_type: repaint_buffer_type.into(), ..Default::default() }
    }

    /// Change the what kind of buffer is being passed to [`Self::render`]
    ///
    /// This may clear the internal caches
    pub fn set_repaint_buffer_type(&self, repaint_buffer_type: RepaintBufferType) {
        if self.repaint_buffer_type.replace(repaint_buffer_type) != repaint_buffer_type {
            self.partial_cache.borrow_mut().clear();
        }
    }

    /// Returns the kind of buffer that must be passed to  [`Self::render`]
    pub fn repaint_buffer_type(&self) -> RepaintBufferType {
        self.repaint_buffer_type.get()
    }

    /// Set how the window need to be rotated in the buffer.
    ///
    /// This is typically used to implement screen rotation in software
    #[cfg(feature = "software-renderer-rotation")]
    // This API is under a feature flag because it is experimental for now.
    // It should be a property of the Window instead (set via dispatch_event?)
    pub fn set_rendering_rotation(&self, rotation: RenderingRotation) {
        self.rotation.set(rotation)
    }

    /// Return the current rotation. See [`Self::set_rendering_rotation()`]
    #[cfg(feature = "software-renderer-rotation")]
    pub fn rendering_rotation(&self) -> RenderingRotation {
        self.rotation.get()
    }

    /// Internal function to apply a dirty region depending on the dirty_tracking_policy.
    /// Returns the region to actually draw.
    fn apply_dirty_region(&self, dirty_region: &mut DirtyRegion, screen_size: LogicalSize) {
        let screen_region = LogicalRect::from_size(screen_size);

        if self.force_screen_refresh.take() {
            *dirty_region = screen_region.into();
        }

        *dirty_region = match self.repaint_buffer_type() {
            RepaintBufferType::NewBuffer => screen_region.into(),
            RepaintBufferType::ReusedBuffer => dirty_region.clone(),
            RepaintBufferType::SwappedBuffers => {
                dirty_region.union(&self.prev_frame_dirty.replace(dirty_region.clone()))
            }
        }
        .intersection(screen_region)
    }

    /// Render the window to the given frame buffer.
    ///
    /// The renderer uses a cache internally and will only render the part of the window
    /// which are dirty. The `extra_draw_region` is an extra regin which will also
    /// be rendered. (eg: the previous dirty region in case of double buffering)
    /// This function returns the region that was rendered.
    ///
    /// The pixel_stride is the size, in pixel, between two line in the buffer
    /// The buffer needs to be big enough to contain the window, so its size must be at least
    /// `pixel_stride * height`, or `pixel_stride * width` if the screen is rotated by 90°.
    ///
    /// Returns the physical dirty region for this frame, excluding the extra_draw_region,
    /// in the window frame of reference. It affected by the screen rotation.
    pub fn render(&self, buffer: &mut [impl TargetPixel], pixel_stride: usize) -> PhysicalRegion {
        let Some(window) = self.maybe_window_adapter.borrow().as_ref().and_then(|w| w.upgrade())
        else {
            return Default::default();
        };
        let window_inner = WindowInner::from_pub(window.window());
        let factor = ScaleFactor::new(window_inner.scale_factor());
        let rotation = self.rotation.get();
        let (size, background) = if let Some(window_item) =
            window_inner.window_item().as_ref().map(|item| item.as_pin_ref())
        {
            (
                (LogicalSize::from_lengths(window_item.width(), window_item.height()).cast()
                    * factor)
                    .cast(),
                window_item.background(),
            )
        } else if rotation.is_transpose() {
            (euclid::size2((buffer.len() / pixel_stride) as _, pixel_stride as _), Brush::default())
        } else {
            (euclid::size2(pixel_stride as _, (buffer.len() / pixel_stride) as _), Brush::default())
        };
        if size.is_empty() {
            return Default::default();
        }
        assert!(
            if rotation.is_transpose() {
                pixel_stride >= size.height as usize && buffer.len() >= (size.width as usize * pixel_stride + size.height as usize) - pixel_stride
            } else {
                pixel_stride >= size.width as usize && buffer.len() >= (size.height as usize * pixel_stride + size.width as usize) - pixel_stride
            },
            "buffer of size {} with stride {pixel_stride} is too small to handle a window of size {size:?}", buffer.len()
        );
        let buffer_renderer = SceneBuilder::new(
            size,
            factor,
            window_inner,
            RenderToBuffer {
                buffer,
                stride: pixel_stride,
                dirty_range_cache: vec![],
                dirty_region: Default::default(),
            },
            rotation,
        );
        let mut renderer = crate::item_rendering::PartialRenderer::new(
            &self.partial_cache,
            self.force_dirty.take(),
            buffer_renderer,
        );

        window_inner
            .draw_contents(|components| {
                let logical_size = (size.cast() / factor).cast();
                for (component, origin) in components {
                    renderer.compute_dirty_regions(component, *origin, logical_size);
                }
                self.apply_dirty_region(&mut renderer.dirty_region, logical_size);
                let rotation = RotationInfo { orientation: rotation, screen_size: size };
                let mut i = renderer.dirty_region.iter().map(|r| {
                    (r.cast() * factor).to_rect().round_out().cast().transformed(rotation)
                });
                let dirty_region = PhysicalRegion {
                    rectangles: core::array::from_fn(|_| i.next().unwrap_or_default().to_box2d()),
                    count: renderer.dirty_region.iter().count(),
                };
                drop(i);

                let mut bg = TargetPixel::background();
                // TODO: gradient background
                TargetPixel::blend(&mut bg, background.color().into());
                let mut line = 0;
                while let Some(next) = region_line_ranges(
                    &dirty_region,
                    line,
                    &mut renderer.actual_renderer.processor.dirty_range_cache,
                ) {
                    for l in line..next {
                        for r in &renderer.actual_renderer.processor.dirty_range_cache {
                            renderer.actual_renderer.processor.buffer[l as usize * pixel_stride..]
                                [r.start as usize..r.end as usize]
                                .fill(bg);
                        }
                    }
                    line = next;
                }

                renderer.actual_renderer.processor.dirty_region = dirty_region.clone();

                for (component, origin) in components {
                    crate::item_rendering::render_component_items(
                        component,
                        &mut renderer,
                        *origin,
                    );
                }

                if let Some(metrics) = &self.rendering_metrics_collector {
                    metrics.measure_frame_rendered(&mut renderer);
                    if metrics.refresh_mode() == RefreshMode::FullSpeed {
                        self.force_screen_refresh.set(true);
                    }
                }

                dirty_region
            })
            .unwrap_or_default()
    }

    /// Render the window, line by line, into the line buffer provided by the [`LineBufferProvider`].
    ///
    /// The renderer uses a cache internally and will only render the part of the window
    /// which are dirty, depending on the dirty tracking policy set in [`SoftwareRenderer::new`]
    /// This function returns the physical region that was rendered considering the rotation.
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
        let Some(window) = self.maybe_window_adapter.borrow().as_ref().and_then(|w| w.upgrade())
        else {
            return Default::default();
        };
        let window_inner = WindowInner::from_pub(window.window());
        let component_rc = window_inner.component();
        let component = crate::item_tree::ItemTreeRc::borrow_pin(&component_rc);
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
            PhysicalRegion { ..Default::default() }
        }
    }
}

#[doc(hidden)]
impl RendererSealed for SoftwareRenderer {
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

        let pos = (pos.cast() * scale_factor)
            .clamp(euclid::point2(0., 0.), euclid::point2(i16::MAX, i16::MAX).cast())
            .cast();

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

                visual_representation.map_byte_offset_from_byte_offset_in_visual_text(
                    paragraph.byte_offset_for_position((pos.x_length(), pos.y_length())),
                )
            }
            #[cfg(all(feature = "software-renderer-systemfonts", not(target_arch = "wasm32")))]
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

                visual_representation.map_byte_offset_from_byte_offset_in_visual_text(
                    paragraph.byte_offset_for_position((pos.x_length(), pos.y_length())),
                )
            }
        }
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
            #[cfg(all(feature = "software-renderer-systemfonts", not(target_arch = "wasm32")))]
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

        (PhysicalRect::new(
            PhysicalPoint::from_lengths(cursor_position.0, cursor_position.1),
            PhysicalSize::from_lengths(
                (text_input.text_cursor_width().cast() * scale_factor).cast(),
                cursor_height,
            ),
        )
        .cast()
            / scale_factor)
            .cast()
    }

    fn free_graphics_resources(
        &self,
        _component: crate::item_tree::ItemTreeRef,
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
        self.force_dirty.replace_with(|r| r.union(&region));
    }

    fn register_bitmap_font(&self, font_data: &'static crate::graphics::BitmapFont) {
        fonts::register_bitmap_font(font_data);
    }

    #[cfg(all(feature = "software-renderer-systemfonts", not(target_arch = "wasm32")))]
    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        self::fonts::systemfonts::register_font_from_memory(data)
    }

    #[cfg(all(feature = "software-renderer-systemfonts", not(target_arch = "wasm32")))]
    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self::fonts::systemfonts::register_font_from_path(path)
    }

    fn default_font_size(&self) -> LogicalLength {
        self::fonts::DEFAULT_FONT_SIZE
    }

    fn set_window_adapter(&self, window_adapter: &Rc<dyn WindowAdapter>) {
        *self.maybe_window_adapter.borrow_mut() = Some(Rc::downgrade(window_adapter));
        self.partial_cache.borrow_mut().clear();
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

    let to_draw_tr = scene.dirty_region.bounding_rect();

    let mut background_color = TargetPixel::background();
    // FIXME gradient
    TargetPixel::blend(&mut background_color, background.color().into());

    while scene.current_line < to_draw_tr.origin.y_length() + to_draw_tr.size.height_length() {
        for r in &scene.current_line_ranges {
            line_buffer.process_line(
                scene.current_line.get() as usize,
                r.start as usize..r.end as usize,
                |line_buffer| {
                    let offset = r.start;

                    line_buffer.fill(background_color);
                    for span in scene.items[0..scene.current_items_index].iter().rev() {
                        debug_assert!(scene.current_line >= span.pos.y_length());
                        debug_assert!(
                            scene.current_line < span.pos.y_length() + span.size.height_length(),
                        );
                        if span.pos.x >= r.end {
                            continue;
                        }
                        let begin = r.start.max(span.pos.x);
                        let end = r.end.min(span.pos.x + span.size.width);
                        if begin >= end {
                            continue;
                        }

                        let extra_left_clip = begin - span.pos.x;
                        let extra_right_clip = span.pos.x + span.size.width - end;
                        let range_buffer =
                            &mut line_buffer[(begin - offset) as usize..(end - offset) as usize];

                        match span.command {
                            SceneCommand::Rectangle { color } => {
                                TargetPixel::blend_slice(range_buffer, color);
                            }
                            SceneCommand::Texture { texture_index } => {
                                let texture = &scene.vectors.textures[texture_index as usize];
                                draw_functions::draw_texture_line(
                                    &PhysicalRect { origin: span.pos, size: span.size },
                                    scene.current_line,
                                    texture,
                                    range_buffer,
                                    extra_left_clip,
                                );
                            }
                            SceneCommand::SharedBuffer { shared_buffer_index } => {
                                let texture = scene.vectors.shared_buffers
                                    [shared_buffer_index as usize]
                                    .as_texture();
                                draw_functions::draw_texture_line(
                                    &PhysicalRect { origin: span.pos, size: span.size },
                                    scene.current_line,
                                    &texture,
                                    range_buffer,
                                    extra_left_clip,
                                );
                            }
                            SceneCommand::RoundedRectangle { rectangle_index } => {
                                let rr =
                                    &scene.vectors.rounded_rectangles[rectangle_index as usize];
                                draw_functions::draw_rounded_rectangle_line(
                                    &PhysicalRect { origin: span.pos, size: span.size },
                                    scene.current_line,
                                    rr,
                                    range_buffer,
                                    extra_left_clip,
                                    extra_right_clip,
                                );
                            }
                            SceneCommand::Gradient { gradient_index } => {
                                let g = &scene.vectors.gradients[gradient_index as usize];

                                draw_functions::draw_gradient_line(
                                    &PhysicalRect { origin: span.pos, size: span.size },
                                    scene.current_line,
                                    g,
                                    range_buffer,
                                    extra_left_clip,
                                );
                            }
                        }
                    }
                },
            );
        }

        if scene.current_line < to_draw_tr.origin.y_length() + to_draw_tr.size.height_length() {
            scene.next_line();
        }
    }
    scene.dirty_region
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

    dirty_region: PhysicalRegion,

    current_line_ranges: Vec<core::ops::Range<i16>>,
    range_valid_until_line: PhysicalLength,
}

impl Scene {
    pub fn new(
        mut items: Vec<SceneItem>,
        vectors: SceneVectors,
        dirty_region: PhysicalRegion,
    ) -> Self {
        let current_line =
            dirty_region.iter_box().map(|x| x.min.y_length()).min().unwrap_or_default();
        items.retain(|i| i.pos.y_length() + i.size.height_length() > current_line);
        items.sort_unstable_by(compare_scene_item);
        let current_items_index = items.partition_point(|i| i.pos.y_length() <= current_line);
        items[..current_items_index].sort_unstable_by(|a, b| b.z.cmp(&a.z));
        let mut r = Self {
            items,
            current_line,
            current_items_index,
            future_items_index: current_items_index,
            vectors,
            dirty_region,
            current_line_ranges: Default::default(),
            range_valid_until_line: Default::default(),
        };
        r.recompute_ranges();
        debug_assert_eq!(r.current_line, r.dirty_region.bounding_rect().origin.y_length());
        r
    }

    /// Updates `current_items_index` and `future_items_index` to match the invariant
    pub fn next_line(&mut self) {
        self.current_line += PhysicalLength::new(1);

        let skipped = self.current_line >= self.range_valid_until_line && self.recompute_ranges();

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

        if skipped {
            // Merge sort doesn't work in that case.
            while j < self.current_items_index {
                let item = self.items[j];
                if item.pos.y_length() + item.size.height_length() > self.current_line {
                    self.items[i] = item;
                    i += 1;
                }
                j += 1;
            }
            while self.future_items_index < self.items.len() {
                let item = self.items[self.future_items_index];
                if item.pos.y_length() > self.current_line {
                    break;
                }
                self.future_items_index += 1;
                if item.pos.y_length() + item.size.height_length() < self.current_line {
                    continue;
                }
                self.items[i] = item;
                i += 1;
            }
            self.items[0..i].sort_unstable_by(|a, b| b.z.cmp(&a.z));
            self.current_items_index = i;
            return;
        }

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
                    self.future_items_index += 1;
                    self.items[i] = item;
                    i += 1;
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

    // return true if lines were skipped
    fn recompute_ranges(&mut self) -> bool {
        let validity = region_line_ranges(
            &self.dirty_region,
            self.current_line.get(),
            &mut self.current_line_ranges,
        );
        if self.current_line_ranges.is_empty() {
            if let Some(next) = validity {
                self.current_line = Length::new(next);
                self.range_valid_until_line = Length::new(
                    region_line_ranges(
                        &self.dirty_region,
                        self.current_line.get(),
                        &mut self.current_line_ranges,
                    )
                    .unwrap_or_default(),
                );
                return true;
            }
        }
        self.range_valid_until_line = Length::new(validity.unwrap_or_default());
        false
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
    /// This should have a size so that the entire slice is ((height - 1) * pixel_stride + width) * bpp
    data: &'a [u8],
    format: PixelFormat,
    /// number of pixels between two lines in the source
    pixel_stride: u16,

    extra: SceneTextureExtra,
}

impl<'a> SceneTexture<'a> {
    fn source_size(&self) -> PhysicalSize {
        let len = self.data.len() / self.format.bpp();
        let stride = self.pixel_stride as usize;
        let h = len / stride;
        let w = len % stride;
        if w == 0 {
            PhysicalSize::new(stride as _, h as _)
        } else {
            PhysicalSize::new(w as _, (h + 1) as _)
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SceneTextureExtra {
    /// Delta x: the amount of "image pixel" that we need to skip for each physical pixel in the target buffer
    dx: Fixed<u16, 8>,
    dy: Fixed<u16, 8>,
    /// Offset which is the coordinate of the "image pixel" which going to be drawn at location SceneItem::pos
    off_x: Fixed<u16, 4>,
    off_y: Fixed<u16, 4>,
    /// Color to colorize. When not transparent, consider that the image is an alpha map and always use that color.
    /// The alpha of this color is ignored. (it is supposed to be mixed in `Self::alpha`)
    colorize: Color,
    alpha: u8,
    rotation: RenderingRotation,
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
    extra: SceneTextureExtra,
}

impl SharedBufferCommand {
    fn as_texture(&self) -> SceneTexture<'_> {
        let stride = self.buffer.width();
        let core::ops::Range { start, end } = compute_range_in_buffer(&self.source_rect, stride);

        match &self.buffer {
            SharedBufferData::SharedImage(SharedImageBuffer::RGB8(b)) => SceneTexture {
                data: &b.as_bytes()[start * 3..end * 3],
                pixel_stride: stride as u16,
                format: PixelFormat::Rgb,
                extra: self.extra,
            },
            SharedBufferData::SharedImage(SharedImageBuffer::RGBA8(b)) => SceneTexture {
                data: &b.as_bytes()[start * 4..end * 4],
                pixel_stride: stride as u16,
                format: PixelFormat::Rgba,
                extra: self.extra,
            },
            SharedBufferData::SharedImage(SharedImageBuffer::RGBA8Premultiplied(b)) => {
                SceneTexture {
                    data: &b.as_bytes()[start * 4..end * 4],
                    pixel_stride: stride as u16,
                    format: PixelFormat::RgbaPremultiplied,
                    extra: self.extra,
                }
            }
            SharedBufferData::AlphaMap { data, width } => SceneTexture {
                data: &data[start..end],
                pixel_stride: *width,
                format: PixelFormat::AlphaMap,
                extra: self.extra,
            },
        }
    }
}

// Given a rectangle of coordinate in a buffer and a stride, compute the range, in pixel
fn compute_range_in_buffer(
    source_rect: &PhysicalRect,
    pixel_stride: usize,
) -> core::ops::Range<usize> {
    let start = pixel_stride * source_rect.min_y() as usize + source_rect.min_x() as usize;
    let end = pixel_stride * (source_rect.max_y() - 1) as usize + source_rect.max_x() as usize;
    start..end
}

#[derive(Debug)]
struct RoundedRectangle {
    radius: PhysicalBorderRadius,
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
    let prepare_scene = SceneBuilder::new(
        size,
        factor,
        window,
        PrepareScene::default(),
        software_renderer.rotation.get(),
    );
    let mut renderer = crate::item_rendering::PartialRenderer::new(
        &software_renderer.partial_cache,
        software_renderer.force_dirty.take(),
        prepare_scene,
    );

    let mut dirty_region = PhysicalRegion::default();
    window.draw_contents(|components| {
        let logical_size = (size.cast() / factor).cast();
        for (component, origin) in components {
            renderer.compute_dirty_regions(component, *origin, logical_size);
        }

        software_renderer.apply_dirty_region(&mut renderer.dirty_region, logical_size);
        let rotation =
            RotationInfo { orientation: software_renderer.rotation.get(), screen_size: size };
        let mut i = renderer
            .dirty_region
            .iter()
            .map(|r| (r.cast() * factor).to_rect().round_out().cast().transformed(rotation));
        dirty_region = PhysicalRegion {
            rectangles: core::array::from_fn(|_| i.next().unwrap_or_default().to_box2d()),
            count: renderer.dirty_region.iter().count(),
        };
        drop(i);

        for (component, origin) in components {
            crate::item_rendering::render_component_items(component, &mut renderer, *origin);
        }
    });

    if let Some(metrics) = &software_renderer.rendering_metrics_collector {
        metrics.measure_frame_rendered(&mut renderer);
        if metrics.refresh_mode() == RefreshMode::FullSpeed {
            software_renderer.force_screen_refresh.set(true);
        }
    }

    let prepare_scene = renderer.into_inner();

    /* // visualize dirty regions
    let mut prepare_scene = prepare_scene;
    for rect in dirty_region.iter() {
        prepare_scene.processor.process_rounded_rectangle(
            rect.to_rect(),
            RoundedRectangle {
                radius: BorderRadius::default(),
                width: Length::new(1),
                border_color: Color::from_argb_u8(128, 255, 0, 0).into(),
                inner_color: PremultipliedRgbaColor::default(),
                left_clip: Length::default(),
                right_clip: Length::default(),
                top_clip: Length::default(),
                bottom_clip: Length::default(),
            },
        )
    } // */

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
    dirty_range_cache: Vec<core::ops::Range<i16>>,
    dirty_region: PhysicalRegion,
}

impl<'a, T: TargetPixel> RenderToBuffer<'a, T> {
    fn foreach_ranges(
        &mut self,
        geometry: &PhysicalRect,
        mut f: impl FnMut(i16, &mut [T], i16, i16),
    ) {
        let mut line = geometry.min_y();
        while let Some(mut next) =
            region_line_ranges(&self.dirty_region, line, &mut self.dirty_range_cache)
        {
            next = next.min(geometry.max_y());
            for r in &self.dirty_range_cache {
                if geometry.origin.x >= r.end {
                    continue;
                }
                let begin = r.start.max(geometry.origin.x);
                let end = r.end.min(geometry.origin.x + geometry.size.width);
                if begin >= end {
                    continue;
                }
                let extra_left_clip = begin - geometry.origin.x;
                let extra_right_clip = geometry.origin.x + geometry.size.width - end;

                for l in line..next {
                    f(
                        l,
                        &mut self.buffer[l as usize * self.stride..][begin as usize..end as usize],
                        extra_left_clip,
                        extra_right_clip,
                    );
                }
            }
            if next == geometry.max_y() {
                break;
            }
            line = next;
        }
    }

    fn process_texture_impl(&mut self, geometry: PhysicalRect, texture: SceneTexture<'_>) {
        self.foreach_ranges(&geometry, |line, buffer, extra_left_clip, _extra_right_clip| {
            draw_functions::draw_texture_line(
                &geometry,
                PhysicalLength::new(line),
                &texture,
                buffer,
                extra_left_clip,
            );
        });
    }
}

impl<'a, T: TargetPixel> ProcessScene for RenderToBuffer<'a, T> {
    fn process_texture(&mut self, geometry: PhysicalRect, texture: SceneTexture<'static>) {
        self.process_texture_impl(geometry, texture)
    }

    fn process_shared_image_buffer(&mut self, geometry: PhysicalRect, buffer: SharedBufferCommand) {
        let texture = buffer.as_texture();
        self.process_texture_impl(geometry, texture);
    }

    fn process_rectangle(&mut self, geometry: PhysicalRect, color: PremultipliedRgbaColor) {
        self.foreach_ranges(&geometry, |_line, buffer, _extra_left_clip, _extra_right_clip| {
            TargetPixel::blend_slice(buffer, color);
        });
    }

    fn process_rounded_rectangle(&mut self, geometry: PhysicalRect, rr: RoundedRectangle) {
        self.foreach_ranges(&geometry, |line, buffer, extra_left_clip, extra_right_clip| {
            draw_functions::draw_rounded_rectangle_line(
                &geometry,
                PhysicalLength::new(line),
                &rr,
                buffer,
                extra_left_clip,
                extra_right_clip,
            );
        });
    }

    fn process_gradient(&mut self, geometry: PhysicalRect, g: GradientCommand) {
        self.foreach_ranges(&geometry, |line, buffer, extra_left_clip, _extra_right_clip| {
            draw_functions::draw_gradient_line(
                &geometry,
                PhysicalLength::new(line),
                &g,
                buffer,
                extra_left_clip,
            );
        });
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
    rotation: RotationInfo,
}

impl<'a, T: ProcessScene> SceneBuilder<'a, T> {
    fn new(
        screen_size: PhysicalSize,
        scale_factor: ScaleFactor,
        window: &'a WindowInner,
        processor: T,
        orientation: RenderingRotation,
    ) -> Self {
        Self {
            processor,
            state_stack: vec![],
            current_state: RenderState {
                alpha: 1.,
                offset: LogicalPoint::default(),
                clip: LogicalRect::new(
                    LogicalPoint::default(),
                    (screen_size.cast() / scale_factor).cast(),
                ),
            },
            scale_factor,
            window,
            rotation: RotationInfo { orientation, screen_size },
        }
    }

    fn should_draw(&self, rect: &LogicalRect) -> bool {
        !rect.size.is_empty()
            && self.current_state.alpha > 0.01
            && self.current_state.clip.intersects(rect)
    }

    fn draw_image_impl(
        &mut self,
        image_inner: &ImageInner,
        crate::graphics::FitResult {
            clip_rect: source_rect,
            source_to_target_x,
            source_to_target_y,
            size: fit_size,
            offset: image_fit_offset,
            tiled,
        }: crate::graphics::FitResult,
        colorize: Color,
    ) {
        let global_alpha_u16 = (self.current_state.alpha * 255.) as u16;
        let offset =
            self.current_state.offset.cast() * self.scale_factor + image_fit_offset.to_vector();

        let physical_clip =
            (self.current_state.clip.translate(self.current_state.offset.to_vector()).cast()
                * self.scale_factor)
                .round()
                .cast();

        let tiled_off = tiled.unwrap_or_default();

        match image_inner {
            ImageInner::None => (),
            ImageInner::StaticTextures(StaticTextures {
                data,
                textures,
                size,
                original_size,
                ..
            }) => {
                let adjust_x = size.width as f32 / original_size.width as f32;
                let adjust_y = size.height as f32 / original_size.height as f32;
                let source_to_target_x = source_to_target_x / adjust_x;
                let source_to_target_y = source_to_target_y / adjust_y;
                let source_rect =
                    source_rect.cast::<f32>().scale(adjust_x, adjust_y).round().cast();
                let dx = Fixed::from_f32(1. / source_to_target_x).unwrap();
                let dy = Fixed::from_f32(1. / source_to_target_y).unwrap();

                for t in textures.as_slice() {
                    // That's the source rect in the whole image coordinate
                    let Some(src_rect) = t.rect.intersection(&source_rect) else { continue };

                    let target_rect = if tiled.is_some() {
                        // FIXME! there could be gaps between the tiles
                        euclid::Rect::new(offset, fit_size).round().cast::<i32>()
                    } else {
                        // map t.rect to to the target
                        euclid::Rect::<f32, PhysicalPx>::from_untyped(
                            &src_rect.translate(-source_rect.origin.to_vector()).cast(),
                        )
                        .scale(source_to_target_x, source_to_target_y)
                        .translate(offset.to_vector())
                        .round()
                        .cast::<i32>()
                    };

                    let Some(clipped_target) = physical_clip.intersection(&target_rect) else {
                        continue;
                    };

                    let off_x = Fixed::from_integer(tiled_off.x as i32)
                        + (Fixed::<i32, 8>::from_fixed(dx))
                            * (clipped_target.origin.x - target_rect.origin.x) as i32;
                    let off_y = Fixed::from_integer(tiled_off.y as i32)
                        + (Fixed::<i32, 8>::from_fixed(dy))
                            * (clipped_target.origin.y - target_rect.origin.y) as i32;

                    let pixel_stride = t.rect.width() as u16;
                    let core::ops::Range { start, end } = compute_range_in_buffer(
                        &PhysicalRect::from_untyped(
                            &src_rect.translate(-t.rect.origin.to_vector()).cast(),
                        ),
                        pixel_stride as usize,
                    );
                    let bpp = t.format.bpp();

                    let color = if colorize.alpha() > 0 { colorize } else { t.color };
                    let alpha = if colorize.alpha() > 0 || t.format == PixelFormat::AlphaMap {
                        color.alpha() as u16 * global_alpha_u16 / 255
                    } else {
                        global_alpha_u16
                    } as u8;

                    self.processor.process_texture(
                        clipped_target.cast().transformed(self.rotation),
                        SceneTexture {
                            data: &data.as_slice()[t.index..][start * bpp..end * bpp],
                            pixel_stride,
                            format: t.format,
                            extra: SceneTextureExtra {
                                colorize: color,
                                alpha,
                                rotation: self.rotation.orientation,
                                dx,
                                dy,
                                off_x: Fixed::try_from_fixed(off_x).unwrap(),
                                off_y: Fixed::try_from_fixed(off_y).unwrap(),
                            },
                        },
                    );
                }
            }

            ImageInner::NineSlice(..) => unreachable!(),
            _ => {
                let target_rect = euclid::Rect::new(offset, fit_size).round().cast();
                let Some(clipped_target) = physical_clip.intersection(&target_rect) else {
                    return;
                };
                if let Some(buffer) = image_inner.render_to_buffer(Some(target_rect.size.cast())) {
                    let buf_size = buffer.size().cast::<f32>();
                    let orig = image_inner.size().cast::<f32>();
                    let dx =
                        Fixed::from_f32(buf_size.width / orig.width / source_to_target_x).unwrap();
                    let dy = Fixed::from_f32(buf_size.height / orig.height / source_to_target_y)
                        .unwrap();

                    let off_x = (Fixed::<i32, 8>::from_fixed(dx))
                        * (clipped_target.origin.x - target_rect.origin.x) as i32
                        + Fixed::from_f32(tiled_off.x as f32 * buf_size.width / orig.width)
                            .unwrap();
                    let off_y = (Fixed::<i32, 8>::from_fixed(dy))
                        * (clipped_target.origin.y - target_rect.origin.y) as i32
                        + Fixed::from_f32(tiled_off.y as f32 * buf_size.height / orig.height)
                            .unwrap();

                    let alpha = if colorize.alpha() > 0 {
                        colorize.alpha() as u16 * global_alpha_u16 / 255
                    } else {
                        global_alpha_u16
                    } as u8;

                    self.processor.process_shared_image_buffer(
                        clipped_target.cast().transformed(self.rotation),
                        SharedBufferCommand {
                            buffer: SharedBufferData::SharedImage(buffer),
                            source_rect: PhysicalRect::from_untyped(
                                &source_rect
                                    .cast::<f32>()
                                    .scale(
                                        buf_size.width / orig.width,
                                        buf_size.height / orig.height,
                                    )
                                    .round()
                                    .cast(),
                            ),

                            extra: SceneTextureExtra {
                                colorize,
                                alpha,
                                rotation: self.rotation.orientation,
                                dx,
                                dy,
                                off_x: Fixed::try_from_fixed(off_x).unwrap(),
                                off_y: Fixed::try_from_fixed(off_y).unwrap(),
                            },
                        },
                    );
                } else {
                    unimplemented!("The image cannot be rendered")
                }
            }
        };
    }

    fn draw_text_paragraph<Font>(
        &mut self,
        paragraph: &TextParagraphLayout<'_, Font>,
        physical_clip: euclid::Rect<f32, PhysicalPx>,
        offset: euclid::Vector2D<f32, PhysicalPx>,
        color: Color,
        selection: Option<SelectionInfo>,
    ) where
        Font: AbstractFont + crate::textlayout::TextShaper<Length = PhysicalLength> + GlyphRenderer,
    {
        paragraph
            .layout_lines::<()>(
                |glyphs, line_x, line_y, _, sel| {
                    let baseline_y = line_y + paragraph.layout.font.ascent();
                    if let (Some(sel), Some(selection)) = (sel, &selection) {
                        let geometry = euclid::rect(
                            line_x.get() + sel.start.get(),
                            line_y.get(),
                            (sel.end - sel.start).get(),
                            paragraph.layout.font.height().get(),
                        );
                        if let Some(clipped_src) = geometry.intersection(&physical_clip.cast()) {
                            let geometry =
                                clipped_src.translate(offset.cast()).transformed(self.rotation);
                            self.processor
                                .process_rectangle(geometry, selection.selection_background.into());
                        }
                    }
                    for positioned_glyph in glyphs {
                        let glyph = paragraph.layout.font.render_glyph(positioned_glyph.glyph_id);

                        let src_rect = PhysicalRect::new(
                            PhysicalPoint::from_lengths(
                                line_x + positioned_glyph.x + glyph.x,
                                baseline_y - glyph.y - glyph.height,
                            ),
                            glyph.size(),
                        )
                        .cast();

                        let color = match &selection {
                            Some(s) if s.selection.contains(&positioned_glyph.text_byte_offset) => {
                                s.selection_color
                            }
                            _ => color,
                        };

                        if let Some(clipped_src) = src_rect.intersection(&physical_clip) {
                            let geometry = clipped_src.translate(offset).round();
                            let origin = (geometry.origin - offset.round()).round().cast::<i16>();
                            let actual_x = (origin.x - src_rect.origin.x as i16) as usize;
                            let actual_y = (origin.y - src_rect.origin.y as i16) as usize;
                            let pixel_stride = glyph.width.get() as u16;
                            let mut geometry = geometry.cast();
                            if geometry.size.width > glyph.width.get() - (actual_x as i16) {
                                geometry.size.width = glyph.width.get() - (actual_x as i16)
                            }
                            if geometry.size.height > glyph.height.get() - (actual_y as i16) {
                                geometry.size.height = glyph.height.get() - (actual_y as i16)
                            }
                            let source_size = geometry.size;
                            if source_size.is_empty() {
                                continue;
                            }
                            match &glyph.alpha_map {
                                fonts::GlyphAlphaMap::Static(data) => {
                                    self.processor.process_texture(
                                        geometry.transformed(self.rotation),
                                        SceneTexture {
                                            data: &data
                                                [actual_x + actual_y * pixel_stride as usize..],
                                            pixel_stride,
                                            format: PixelFormat::AlphaMap,
                                            extra: SceneTextureExtra {
                                                colorize: color,
                                                // color already is mixed with global alpha
                                                alpha: color.alpha(),
                                                rotation: self.rotation.orientation,
                                                dx: Fixed::from_integer(1),
                                                dy: Fixed::from_integer(1),
                                                off_x: Fixed::from_integer(0),
                                                off_y: Fixed::from_integer(0),
                                            },
                                        },
                                    );
                                }
                                fonts::GlyphAlphaMap::Shared(data) => {
                                    self.processor.process_shared_image_buffer(
                                        geometry.transformed(self.rotation),
                                        SharedBufferCommand {
                                            buffer: SharedBufferData::AlphaMap {
                                                data: data.clone(),
                                                width: pixel_stride,
                                            },
                                            source_rect: PhysicalRect::new(
                                                PhysicalPoint::new(actual_x as _, actual_y as _),
                                                source_size,
                                            ),
                                            extra: SceneTextureExtra {
                                                colorize: color,
                                                // color already is mixed with global alpha
                                                alpha: color.alpha(),
                                                rotation: self.rotation.orientation,
                                                dx: Fixed::from_integer(1),
                                                dy: Fixed::from_integer(1),
                                                off_x: Fixed::from_integer(0),
                                                off_y: Fixed::from_integer(0),
                                            },
                                        },
                                    );
                                }
                            };
                        }
                    }
                    core::ops::ControlFlow::Continue(())
                },
                selection.as_ref().map(|s| s.selection.clone()),
            )
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

struct SelectionInfo {
    selection_color: Color,
    selection_background: Color,
    selection: core::ops::Range<usize>,
}

#[derive(Clone, Copy)]
struct RenderState {
    alpha: f32,
    offset: LogicalPoint,
    clip: LogicalRect,
}

impl<'a, T: ProcessScene> crate::item_rendering::ItemRenderer for SceneBuilder<'a, T> {
    #[allow(clippy::unnecessary_cast)] // Coord!
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
                let geom2 = (geom.cast() * self.scale_factor).transformed(self.rotation);
                let clipped2 = (clipped.cast() * self.scale_factor).transformed(self.rotation);
                let act_rect = (clipped.translate(self.current_state.offset.to_vector()).cast()
                    * self.scale_factor)
                    .round()
                    .cast()
                    .transformed(self.rotation);
                let axis_angle = (360. - self.rotation.orientation.angle()) % 360.;
                let angle = g.angle() - axis_angle;
                let tan = angle.to_radians().tan().abs();
                let start = if !tan.is_finite() {
                    255.
                } else {
                    let h = tan * geom2.width() as f32;
                    255. * h / (h + geom2.height() as f32)
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

            let color = self.alpha_color(background.color());

            if color.alpha() == 0 {
                return;
            }
            let geometry = (clipped.translate(self.current_state.offset.to_vector()).cast()
                * self.scale_factor)
                .round()
                .cast()
                .transformed(self.rotation);

            self.processor.process_rectangle(geometry, color.into());
        }
    }

    #[allow(clippy::unnecessary_cast)] // Coord
    fn draw_border_rectangle(
        &mut self,
        rect: Pin<&dyn RenderBorderRectangle>,
        _: &ItemRc,
        size: LogicalSize,
        _: &CachedRenderingData,
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
                    red: ((color.red as u16 * (255 - b_alpha_16)) / 255) as u8 + b.red,
                    green: ((color.green as u16 * (255 - b_alpha_16)) / 255) as u8 + b.green,
                    blue: ((color.blue as u16 * (255 - b_alpha_16)) / 255) as u8 + b.blue,
                    alpha: (color.alpha as u16 + b_alpha_16
                        - (color.alpha as u16 * b_alpha_16) / 255) as u8,
                }
            }

            if !radius.is_zero() {
                let radius = radius
                    .min(LogicalBorderRadius::from_length(geom.width_length() / 2 as Coord))
                    .min(LogicalBorderRadius::from_length(geom.height_length() / 2 as Coord));
                if let Some(clipped) = geom.intersection(&self.current_state.clip) {
                    let geom2 = (geom.cast() * self.scale_factor).transformed(self.rotation);
                    let clipped2 = (clipped.cast() * self.scale_factor).transformed(self.rotation);
                    let geometry =
                        (clipped.translate(self.current_state.offset.to_vector()).cast()
                            * self.scale_factor)
                            .round()
                            .cast()
                            .transformed(self.rotation);
                    let radius =
                        (radius.cast() * self.scale_factor).cast().transformed(self.rotation);
                    // Add a small value to make sure that the clip is always positive despite floating point shenanigans
                    const E: f32 = 0.00001;

                    self.processor.process_rounded_rectangle(
                        geometry,
                        RoundedRectangle {
                            radius,
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
                    let geometry = (r.translate(self.current_state.offset.to_vector()).cast()
                        * self.scale_factor)
                        .round()
                        .cast()
                        .transformed(self.rotation);
                    self.processor.process_rectangle(geometry, color);
                }
            }

            // FIXME: gradients
            if border_color.alpha > 0 {
                let mut add_border = |r: LogicalRect| {
                    if let Some(r) = r.intersection(&self.current_state.clip) {
                        let geometry = (r.translate(self.current_state.offset.to_vector()).cast()
                            * self.scale_factor)
                            .round()
                            .cast()
                            .transformed(self.rotation);
                        self.processor.process_rectangle(geometry, border_color);
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

    fn draw_image(
        &mut self,
        image: Pin<&dyn RenderImage>,
        _: &ItemRc,
        size: LogicalSize,
        _: &CachedRenderingData,
    ) {
        let geom = LogicalRect::from(size);
        if self.should_draw(&geom) {
            let source = image.source();

            let image_inner: &ImageInner = (&source).into();
            if let ImageInner::NineSlice(nine) = image_inner {
                let colorize = image.colorize().color();
                let source_size = source.size();
                for fit in crate::graphics::fit9slice(
                    source_size,
                    nine.1,
                    size.cast() * self.scale_factor,
                    self.scale_factor,
                    image.alignment(),
                    image.tiling(),
                ) {
                    self.draw_image_impl(&nine.0, fit, colorize);
                }
                return;
            }

            let source_clip = image.source_clip().map_or_else(
                || euclid::Rect::new(Default::default(), source.size().cast()),
                |clip| {
                    clip.intersection(&euclid::Rect::from_size(source.size().cast()))
                        .unwrap_or_default()
                },
            );

            let phys_size = geom.size_length().cast() * self.scale_factor;
            let fit = crate::graphics::fit(
                image.image_fit(),
                phys_size,
                source_clip,
                self.scale_factor,
                image.alignment(),
                image.tiling(),
            );
            self.draw_image_impl(image_inner, fit, image.colorize().color());
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

                self.draw_text_paragraph(&paragraph, physical_clip, offset, color, None);
            }
            #[cfg(all(feature = "software-renderer-systemfonts", not(target_arch = "wasm32")))]
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

                self.draw_text_paragraph(&paragraph, physical_clip, offset, color, None);
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
        let color = self.alpha_color(text_visual_representation.text_color.color());

        let selection =
            (!text_visual_representation.selection_range.is_empty()).then_some(SelectionInfo {
                selection_background: self.alpha_color(text_input.selection_background_color()),
                selection_color: self.alpha_color(text_input.selection_foreground_color()),
                selection: text_visual_representation.selection_range.clone(),
            });

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

                self.draw_text_paragraph(&paragraph, physical_clip, offset, color, selection);

                text_visual_representation.cursor_position.map(|cursor_offset| {
                    (paragraph.cursor_pos_for_byte_offset(cursor_offset), pf.height())
                })
            }
            #[cfg(all(feature = "software-renderer-systemfonts", not(target_arch = "wasm32")))]
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

                self.draw_text_paragraph(&paragraph, physical_clip, offset, color, selection);

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
                let geometry = clipped_src.translate(offset.cast()).transformed(self.rotation);
                #[allow(unused_mut)]
                let mut cursor_color = text_visual_representation.cursor_color;
                #[cfg(all(feature = "std", target_os = "macos"))]
                {
                    // On macOs, the cursor color is different than other platform. Use a hack to pass the screenshot test.
                    static IS_SCREENSHOT_TEST: std::sync::OnceLock<bool> =
                        std::sync::OnceLock::new();
                    if *IS_SCREENSHOT_TEST.get_or_init(|| {
                        std::env::var_os("CARGO_PKG_NAME").unwrap_or_default()
                            == "test-driver-screenshots"
                    }) {
                        cursor_color = color;
                    }
                }
                self.processor.process_rectangle(geometry, self.alpha_color(cursor_color).into());
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
        _radius: LogicalBorderRadius,
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

    fn translation(&self) -> LogicalVector {
        self.current_state.offset.to_vector()
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

            let physical_clip = (self.current_state.clip.cast() * self.scale_factor).cast();
            let source_rect = euclid::rect(0, 0, width as _, height as _);

            if let Some(clipped_src) = source_rect.intersection(&physical_clip) {
                let geometry = clipped_src
                    .translate(
                        (self.current_state.offset.cast() * self.scale_factor).to_vector().cast(),
                    )
                    .round_in();

                self.processor.process_shared_image_buffer(
                    geometry.cast().transformed(self.rotation),
                    SharedBufferCommand {
                        buffer: SharedBufferData::SharedImage(img),
                        source_rect,
                        extra: SceneTextureExtra {
                            colorize: Default::default(),
                            alpha: (self.current_state.alpha * 255.) as u8,
                            rotation: self.rotation.orientation,
                            dx: Fixed::from_integer(1),
                            dy: Fixed::from_integer(1),
                            off_x: Fixed::from_integer(clipped_src.min_x() as _),
                            off_y: Fixed::from_integer(clipped_src.min_y() as _),
                        },
                    },
                );
            }
        });
    }

    fn draw_string(&mut self, string: &str, color: Color) {
        let font_request = Default::default();
        let font = fonts::match_font(&font_request, self.scale_factor);
        let clip = self.current_state.clip.cast() * self.scale_factor;

        match font {
            fonts::Font::PixelFont(pf) => {
                let layout = fonts::text_layout_for_font(&pf, &font_request, self.scale_factor);

                let paragraph = TextParagraphLayout {
                    string,
                    layout,
                    max_width: clip.width_length().cast(),
                    max_height: clip.height_length().cast(),
                    horizontal_alignment: Default::default(),
                    vertical_alignment: Default::default(),
                    wrap: Default::default(),
                    overflow: Default::default(),
                    single_line: false,
                };

                self.draw_text_paragraph(&paragraph, clip, Default::default(), color, None);
            }
            #[cfg(all(feature = "software-renderer-systemfonts", not(target_arch = "wasm32")))]
            fonts::Font::VectorFont(vf) => {
                let layout = fonts::text_layout_for_font(&vf, &font_request, self.scale_factor);

                let paragraph = TextParagraphLayout {
                    string,
                    layout,
                    max_width: clip.width_length().cast(),
                    max_height: clip.height_length().cast(),
                    horizontal_alignment: Default::default(),
                    vertical_alignment: Default::default(),
                    wrap: Default::default(),
                    overflow: Default::default(),
                    single_line: false,
                };

                self.draw_text_paragraph(&paragraph, clip, Default::default(), color, None);
            }
        }
    }

    fn draw_image_direct(&mut self, _image: crate::graphics::Image) {
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
            renderer: SoftwareRenderer::new_with_repaint_buffer_type(repaint_buffer_type),
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
        if self.needs_redraw.replace(false) || self.renderer.rendering_metrics_collector.is_some() {
            render_callback(&self.renderer);
            true
        } else {
            false
        }
    }

    #[doc(hidden)]
    /// Forward to the window through Deref
    /// (Before 1.1, WindowAdapter didn't have set_size, so the one from Deref was used.
    /// But in Slint 1.1, if one had imported the WindowAdapter trait, the other one would be found)
    pub fn set_size(&self, size: impl Into<crate::api::WindowSize>) {
        self.window.set_size(size);
    }
}

impl WindowAdapter for MinimalSoftwareWindow {
    fn window(&self) -> &Window {
        &self.window
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn size(&self) -> crate::api::PhysicalSize {
        self.size.get()
    }
    fn set_size(&self, size: crate::api::WindowSize) {
        self.size.set(size.to_physical(1.));
        self.window
            .dispatch_event(crate::platform::WindowEvent::Resized { size: size.to_logical(1.) })
    }

    fn request_redraw(&self) {
        self.needs_redraw.set(true);
    }
}

impl core::ops::Deref for MinimalSoftwareWindow {
    type Target = Window;
    fn deref(&self) -> &Self::Target {
        &self.window
    }
}
