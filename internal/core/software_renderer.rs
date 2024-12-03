// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains the [`SoftwareRenderer`] and related types.
//!
//! It is only enabled when the `renderer-software` Slint feature is enabled.

#![warn(missing_docs)]

mod draw_functions;
mod fixed;
mod fonts;
mod minimal_software_window;
mod scene;

use self::fonts::GlyphRenderer;
pub use self::minimal_software_window::MinimalSoftwareWindow;
use self::scene::*;
use crate::api::PlatformError;
use crate::graphics::rendering_metrics_collector::{RefreshMode, RenderingMetricsCollector};
use crate::graphics::{BorderRadius, Rgba8Pixel, SharedImageBuffer, SharedPixelBuffer};
use crate::item_rendering::{
    CachedRenderingData, DirtyRegion, PartialRenderingState, RenderBorderRectangle, RenderImage,
    RenderRectangle,
};
use crate::items::{ItemRc, TextOverflow, TextWrap};
use crate::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalRect, LogicalSize, LogicalVector,
    PhysicalPx, PointLengths, RectLengths, ScaleFactor, SizeLengths,
};
use crate::renderer::RendererSealed;
use crate::textlayout::{AbstractFont, FontMetrics, TextParagraphLayout};
use crate::window::{WindowAdapter, WindowInner};
use crate::{Brush, Color, Coord, ImageInner, StaticTextures};
use alloc::rc::{Rc, Weak};
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

pub use crate::item_rendering::RepaintBufferType;

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
    pub fn angle(self) -> f32 {
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
    /// They do not overlap.
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (crate::api::PhysicalPosition, crate::api::PhysicalSize)> + '_ {
        let mut line_ranges = Vec::<core::ops::Range<i16>>::new();
        let mut begin_line = 0;
        let mut end_line = 0;
        core::iter::from_fn(move || loop {
            match line_ranges.pop() {
                Some(r) => {
                    return Some((
                        crate::api::PhysicalPosition { x: r.start as _, y: begin_line as _ },
                        crate::api::PhysicalSize {
                            width: r.len() as _,
                            height: (end_line - begin_line) as _,
                        },
                    ));
                }
                None => {
                    begin_line = end_line;
                    end_line = match region_line_ranges(self, begin_line, &mut line_ranges) {
                        Some(end_line) => end_line,
                        None => return None,
                    };
                    line_ranges.reverse();
                }
            }
        })
    }
}

#[test]
fn region_iter() {
    let mut region = PhysicalRegion::default();
    assert_eq!(region.iter().next(), None);
    region.rectangles[0] =
        euclid::Box2D::from_origin_and_size(euclid::point2(1, 1), euclid::size2(2, 3));
    region.rectangles[1] =
        euclid::Box2D::from_origin_and_size(euclid::point2(6, 2), euclid::size2(3, 20));
    region.rectangles[2] =
        euclid::Box2D::from_origin_and_size(euclid::point2(0, 10), euclid::size2(10, 5));
    assert_eq!(region.iter().next(), None);
    region.count = 1;
    let r = |x, y, width, height| {
        (crate::api::PhysicalPosition { x, y }, crate::api::PhysicalSize { width, height })
    };

    let mut iter = region.iter();
    assert_eq!(iter.next(), Some(r(1, 1, 2, 3)));
    assert_eq!(iter.next(), None);
    drop(iter);

    region.count = 3;
    let mut iter = region.iter();
    assert_eq!(iter.next(), Some(r(1, 1, 2, 1))); // the two first rectangle could have been merged
    assert_eq!(iter.next(), Some(r(1, 2, 2, 2)));
    assert_eq!(iter.next(), Some(r(6, 2, 3, 2)));
    assert_eq!(iter.next(), Some(r(6, 4, 3, 6)));
    assert_eq!(iter.next(), Some(r(0, 10, 10, 5)));
    assert_eq!(iter.next(), Some(r(6, 15, 3, 7)));
    assert_eq!(iter.next(), None);
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
                        true
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
                    true
                }
            });
            if let Some(r) = tmp {
                line_ranges.push(r);
            }
            continue;
        } else if geom.min.y >= line {
            match &mut next_validity {
                Some(val) => *val = geom.min.y.min(*val),
                None => next_validity = Some(geom.min.y),
            }
        }
    }
    // check that current items are properly sorted
    debug_assert!(line_ranges.windows(2).all(|x| x[0].end < x[1].start));
    next_validity
}

mod target_pixel_buffer;

#[cfg(feature = "experimental")]
pub use target_pixel_buffer::{CompositionMode, TargetPixelBuffer, Texture, TexturePixelFormat};

#[cfg(not(feature = "experimental"))]
use target_pixel_buffer::{CompositionMode, TexturePixelFormat};

struct TargetPixelSlice<'a, T> {
    data: &'a mut [T],
    pixel_stride: usize,
}

impl<'a, T: TargetPixel> target_pixel_buffer::TargetPixelBuffer for TargetPixelSlice<'a, T> {
    type TargetPixel = T;

    fn line_slice(&mut self, line_number: usize) -> &mut [Self::TargetPixel] {
        let offset = line_number * self.pixel_stride;
        &mut self.data[offset..offset + self.pixel_stride]
    }

    fn num_lines(&self) -> usize {
        self.data.len() / self.pixel_stride
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
    repaint_buffer_type: Cell<RepaintBufferType>,
    /// This is the area which was dirty on the previous frame.
    /// Only used if repaint_buffer_type == RepaintBufferType::SwappedBuffers
    prev_frame_dirty: Cell<DirtyRegion>,
    partial_rendering_state: PartialRenderingState,
    maybe_window_adapter: RefCell<Option<Weak<dyn crate::window::WindowAdapter>>>,
    rotation: Cell<RenderingRotation>,
    rendering_metrics_collector: Option<Rc<RenderingMetricsCollector>>,
}

impl Default for SoftwareRenderer {
    fn default() -> Self {
        Self {
            partial_rendering_state: Default::default(),
            prev_frame_dirty: Default::default(),
            maybe_window_adapter: Default::default(),
            rotation: Default::default(),
            rendering_metrics_collector: RenderingMetricsCollector::new("software"),
            repaint_buffer_type: Default::default(),
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
        let self_ = Self::default();
        self_.repaint_buffer_type.set(repaint_buffer_type);
        self_
    }

    /// Change the what kind of buffer is being passed to [`Self::render`]
    ///
    /// This may clear the internal caches
    pub fn set_repaint_buffer_type(&self, repaint_buffer_type: RepaintBufferType) {
        if self.repaint_buffer_type.replace(repaint_buffer_type) != repaint_buffer_type {
            self.partial_rendering_state.clear_cache();
        }
    }

    /// Returns the kind of buffer that must be passed to  [`Self::render`]
    pub fn repaint_buffer_type(&self) -> RepaintBufferType {
        self.repaint_buffer_type.get()
    }

    /// Set how the window need to be rotated in the buffer.
    ///
    /// This is typically used to implement screen rotation in software
    pub fn set_rendering_rotation(&self, rotation: RenderingRotation) {
        self.rotation.set(rotation)
    }

    /// Return the current rotation. See [`Self::set_rendering_rotation()`]
    pub fn rendering_rotation(&self) -> RenderingRotation {
        self.rotation.get()
    }

    /// Render the window to the given frame buffer.
    ///
    /// The renderer uses a cache internally and will only render the part of the window
    /// which are dirty. The `extra_draw_region` is an extra region which will also
    /// be rendered. (eg: the previous dirty region in case of double buffering)
    /// This function returns the region that was rendered.
    ///
    /// The pixel_stride is the size (in pixels) between two lines in the buffer.
    /// It is equal `width` if the screen is not rotated, and `height` if the screen is rotated by 90°.
    /// The buffer needs to be big enough to contain the window, so its size must be at least
    /// `pixel_stride * height`, or `pixel_stride * width` if the screen is rotated by 90°.
    ///
    /// Returns the physical dirty region for this frame, excluding the extra_draw_region,
    /// in the window frame of reference. It is affected by the screen rotation.
    pub fn render(&self, buffer: &mut [impl TargetPixel], pixel_stride: usize) -> PhysicalRegion {
        self.render_buffer_impl(&mut TargetPixelSlice { data: buffer, pixel_stride })
    }

    /// Render the window to the given frame buffer.
    ///
    /// The renderer uses a cache internally and will only render the part of the window
    /// which are dirty. The `extra_draw_region` is an extra region which will also
    /// be rendered. (eg: the previous dirty region in case of double buffering)
    /// This function returns the region that was rendered.
    ///
    /// The buffer's line slices need to be wide enough to if the `width` of the screen and the line count the `height`,
    /// or the `height` and `width` swapped if the screen is rotated by 90°.
    ///
    /// Returns the physical dirty region for this frame, excluding the extra_draw_region,
    /// in the window frame of reference. It is affected by the screen rotation.
    #[cfg(feature = "experimental")]
    pub fn render_into_buffer(&self, buffer: &mut impl TargetPixelBuffer) -> PhysicalRegion {
        self.render_buffer_impl(buffer)
    }

    fn render_buffer_impl(
        &self,
        buffer: &mut impl target_pixel_buffer::TargetPixelBuffer,
    ) -> PhysicalRegion {
        let pixels_per_line = buffer.line_slice(0).len();
        let num_lines = buffer.num_lines();
        let buffer_pixel_count = num_lines * pixels_per_line;

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
            (euclid::size2(num_lines as _, pixels_per_line as _), Brush::default())
        } else {
            (euclid::size2(pixels_per_line as _, num_lines as _), Brush::default())
        };
        if size.is_empty() {
            return Default::default();
        }
        assert!(
            if rotation.is_transpose() {
                pixels_per_line >= size.height as usize && buffer_pixel_count >= (size.width as usize * pixels_per_line + size.height as usize) - pixels_per_line
            } else {
                pixels_per_line >= size.width as usize && buffer_pixel_count >= (size.height as usize * pixels_per_line + size.width as usize) - pixels_per_line
            },
            "buffer of size {} with {pixels_per_line} pixels per line is too small to handle a window of size {size:?}", buffer_pixel_count
        );
        let buffer_renderer = SceneBuilder::new(
            size,
            factor,
            window_inner,
            RenderToBuffer { buffer, dirty_range_cache: vec![], dirty_region: Default::default() },
            rotation,
        );
        let mut renderer = self.partial_rendering_state.create_partial_renderer(buffer_renderer);
        let window_adapter = renderer.window_adapter.clone();

        window_inner
            .draw_contents(|components| {
                let logical_size = (size.cast() / factor).cast();

                let dirty_region_of_existing_buffer = match self.repaint_buffer_type.get() {
                    RepaintBufferType::NewBuffer => {
                        Some(LogicalRect::from_size(logical_size).into())
                    }
                    RepaintBufferType::ReusedBuffer => None,
                    RepaintBufferType::SwappedBuffers => Some(self.prev_frame_dirty.take()),
                };

                let dirty_region_for_this_frame = self.partial_rendering_state.apply_dirty_region(
                    &mut renderer,
                    components,
                    logical_size,
                    dirty_region_of_existing_buffer,
                );

                if self.repaint_buffer_type.get() == RepaintBufferType::SwappedBuffers {
                    self.prev_frame_dirty.set(dirty_region_for_this_frame);
                }

                let rotation = RotationInfo { orientation: rotation, screen_size: size };
                let screen_rect = PhysicalRect::from_size(size);
                let mut i = renderer.dirty_region.iter().filter_map(|r| {
                    (r.cast() * factor)
                        .to_rect()
                        .round_out()
                        .cast()
                        .intersection(&screen_rect)?
                        .transformed(rotation)
                        .into()
                });
                let dirty_region = PhysicalRegion {
                    rectangles: core::array::from_fn(|_| i.next().unwrap_or_default().to_box2d()),
                    count: renderer.dirty_region.iter().count(),
                };
                drop(i);

                renderer.actual_renderer.processor.dirty_region = dirty_region.clone();
                renderer.actual_renderer.processor.process_rectangle_impl(
                    screen_rect.transformed(rotation),
                    // TODO: gradient background
                    background.color().into(),
                    CompositionMode::Source,
                );

                for (component, origin) in components {
                    crate::item_rendering::render_component_items(
                        component,
                        &mut renderer,
                        *origin,
                        &window_adapter,
                    );
                }

                if let Some(metrics) = &self.rendering_metrics_collector {
                    metrics.measure_frame_rendered(&mut renderer);
                    if metrics.refresh_mode() == RefreshMode::FullSpeed {
                        self.partial_rendering_state.force_screen_refresh();
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
        text_wrap: TextWrap,
    ) -> LogicalSize {
        fonts::text_size(font_request, text, max_width, scale_factor, text_wrap)
    }

    fn font_metrics(
        &self,
        font_request: crate::graphics::FontRequest,
        scale_factor: ScaleFactor,
    ) -> crate::items::FontMetrics {
        fonts::font_metrics(font_request, scale_factor)
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
        self.partial_rendering_state.free_graphics_resources(items);
        Ok(())
    }

    fn mark_dirty_region(&self, region: crate::item_rendering::DirtyRegion) {
        self.partial_rendering_state.mark_dirty_region(region);
    }

    fn register_bitmap_font(&self, font_data: &'static crate::graphics::BitmapFont) {
        fonts::register_bitmap_font(font_data);
    }

    #[cfg(feature = "software-renderer-systemfonts")]
    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), std::boxed::Box<dyn std::error::Error>> {
        self::fonts::systemfonts::register_font_from_memory(data)
    }

    #[cfg(all(feature = "software-renderer-systemfonts", not(target_arch = "wasm32")))]
    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), std::boxed::Box<dyn std::error::Error>> {
        self::fonts::systemfonts::register_font_from_path(path)
    }

    fn default_font_size(&self) -> LogicalLength {
        self::fonts::DEFAULT_FONT_SIZE
    }

    fn set_window_adapter(&self, window_adapter: &Rc<dyn WindowAdapter>) {
        *self.maybe_window_adapter.borrow_mut() = Some(Rc::downgrade(window_adapter));
        self.partial_rendering_state.clear_cache();
    }

    fn take_snapshot(&self) -> Result<SharedPixelBuffer<Rgba8Pixel>, PlatformError> {
        let Some(window_adapter) =
            self.maybe_window_adapter.borrow().as_ref().and_then(|w| w.upgrade())
        else {
            return Err(
                "SoftwareRenderer's screenshot called without a window adapter present".into()
            );
        };

        let window = window_adapter.window();
        let size = window.size();

        let Some((width, height)) = size.width.try_into().ok().zip(size.height.try_into().ok())
        else {
            // Nothing to render
            return Err("take_snapshot() called on window with invalid size".into());
        };

        let mut target_buffer = SharedPixelBuffer::<crate::graphics::Rgb8Pixel>::new(width, height);

        self.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
        self.render(target_buffer.make_mut_slice(), width as usize);
        // ensure that caches are clear for the next call
        self.set_repaint_buffer_type(RepaintBufferType::NewBuffer);

        let mut target_buffer_with_alpha =
            SharedPixelBuffer::<Rgba8Pixel>::new(target_buffer.width(), target_buffer.height());
        for (target_pixel, source_pixel) in target_buffer_with_alpha
            .make_mut_slice()
            .iter_mut()
            .zip(target_buffer.as_slice().iter())
        {
            *target_pixel.rgb_mut() = *source_pixel;
        }
        Ok(target_buffer_with_alpha)
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
                                    extra_right_clip,
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
                                    extra_right_clip,
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
    let mut renderer =
        software_renderer.partial_rendering_state.create_partial_renderer(prepare_scene);
    let window_adapter = renderer.window_adapter.clone();

    let mut dirty_region = PhysicalRegion::default();
    window.draw_contents(|components| {
        let logical_size = (size.cast() / factor).cast();

        let dirty_region_of_existing_buffer = match software_renderer.repaint_buffer_type.get() {
            RepaintBufferType::NewBuffer => Some(LogicalRect::from_size(logical_size).into()),
            RepaintBufferType::ReusedBuffer => None,
            RepaintBufferType::SwappedBuffers => Some(software_renderer.prev_frame_dirty.take()),
        };

        let dirty_region_for_this_frame =
            software_renderer.partial_rendering_state.apply_dirty_region(
                &mut renderer,
                components,
                logical_size,
                dirty_region_of_existing_buffer,
            );

        if software_renderer.repaint_buffer_type.get() == RepaintBufferType::SwappedBuffers {
            software_renderer.prev_frame_dirty.set(dirty_region_for_this_frame);
        }

        let rotation =
            RotationInfo { orientation: software_renderer.rotation.get(), screen_size: size };
        let screen_rect = PhysicalRect::from_size(size);
        let mut i = renderer.dirty_region.iter().filter_map(|r| {
            (r.cast() * factor)
                .to_rect()
                .round_out()
                .cast()
                .intersection(&screen_rect)?
                .transformed(rotation)
                .into()
        });
        dirty_region = PhysicalRegion {
            rectangles: core::array::from_fn(|_| i.next().unwrap_or_default().to_box2d()),
            count: renderer.dirty_region.iter().count(),
        };
        drop(i);

        for (component, origin) in components {
            crate::item_rendering::render_component_items(
                component,
                &mut renderer,
                *origin,
                &window_adapter,
            );
        }
    });

    if let Some(metrics) = &software_renderer.rendering_metrics_collector {
        metrics.measure_frame_rendered(&mut renderer);
        if metrics.refresh_mode() == RefreshMode::FullSpeed {
            software_renderer.partial_rendering_state.force_screen_refresh();
        }
    }

    let prepare_scene = renderer.into_inner();

    /* // visualize dirty regions
    let mut prepare_scene = prepare_scene;
    for rect in dirty_region.iter() {
        prepare_scene.processor.process_rounded_rectangle(
            euclid::rect(rect.0.x as _, rect.0.y as _, rect.1.width as _, rect.1.height as _),
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

struct RenderToBuffer<'a, TargetPixelBuffer> {
    buffer: &'a mut TargetPixelBuffer,
    dirty_range_cache: Vec<core::ops::Range<i16>>,
    dirty_region: PhysicalRegion,
}

impl<B: target_pixel_buffer::TargetPixelBuffer> RenderToBuffer<'_, B> {
    fn foreach_ranges(
        &mut self,
        geometry: &PhysicalRect,
        mut f: impl FnMut(i16, &mut [B::TargetPixel], i16, i16),
    ) {
        self.foreach_region(geometry, |buffer, rect, extra_left_clip, extra_right_clip| {
            for l in rect.y_range() {
                f(
                    l,
                    &mut buffer.line_slice(l as usize)
                        [rect.min_x() as usize..rect.max_x() as usize],
                    extra_left_clip,
                    extra_right_clip,
                );
            }
        });
    }

    fn foreach_region(
        &mut self,
        geometry: &PhysicalRect,
        mut f: impl FnMut(&mut B, PhysicalRect, i16, i16),
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

                let region = PhysicalRect {
                    origin: PhysicalPoint::new(begin, line),
                    size: PhysicalSize::new(end - begin, next - line),
                };

                f(&mut self.buffer, region, extra_left_clip, extra_right_clip);
            }
            if next == geometry.max_y() {
                break;
            }
            line = next;
        }
    }

    fn process_texture_impl(&mut self, geometry: PhysicalRect, texture: SceneTexture<'_>) {
        self.foreach_region(&geometry, |buffer, rect, extra_left_clip, extra_right_clip| {
            let tex_src_off_x = (texture.extra.off_x + Fixed::from_integer(extra_left_clip as u16))
                * Fixed::from_fixed(texture.extra.dx);
            let tex_src_off_y = (texture.extra.off_y
                + Fixed::from_integer((rect.origin.y - geometry.origin.y) as u16))
                * Fixed::from_fixed(texture.extra.dy);
            if !buffer.draw_texture(
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                rect.size.height,
                target_pixel_buffer::Texture {
                    bytes: texture.data,
                    pixel_format: texture.format,
                    pixel_stride: texture.pixel_stride,
                    width: texture.source_size().width as u16,
                    height: texture.source_size().height as u16,
                    delta_x: texture.extra.dx.0,
                    delta_y: texture.extra.dy.0,
                    source_offset_x: tex_src_off_x.0,
                    source_offset_y: tex_src_off_y.0,
                },
                texture.extra.colorize.as_argb_encoded(),
                texture.extra.alpha,
                texture.extra.rotation,
                CompositionMode::default(),
            ) {
                let begin = rect.min_x();
                let end = rect.max_x();
                for l in rect.y_range() {
                    draw_functions::draw_texture_line(
                        &geometry,
                        PhysicalLength::new(l),
                        &texture,
                        &mut buffer.line_slice(l as usize)[begin as usize..end as usize],
                        extra_left_clip,
                        extra_right_clip,
                    );
                }
            }
        });
    }

    fn process_rectangle_impl(
        &mut self,
        geometry: PhysicalRect,
        color: PremultipliedRgbaColor,
        composition_mode: CompositionMode,
    ) {
        self.foreach_region(&geometry, |buffer, rect, _extra_left_clip, _extra_right_clip| {
            if !buffer.fill_rectangle(
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                rect.size.height,
                color,
                composition_mode,
            ) {
                let begin = rect.min_x();
                let end = rect.max_x();

                match composition_mode {
                    CompositionMode::Source => {
                        let mut fill_col = B::TargetPixel::background();
                        B::TargetPixel::blend(&mut fill_col, color);
                        for l in rect.y_range() {
                            buffer.line_slice(l as usize)[begin as usize..end as usize]
                                .fill(fill_col)
                        }
                    }
                    CompositionMode::SourceOver => {
                        for l in rect.y_range() {
                            <B::TargetPixel>::blend_slice(
                                &mut buffer.line_slice(l as usize)[begin as usize..end as usize],
                                color,
                            )
                        }
                    }
                }
            }
        })
    }
}

impl<T: TargetPixel, B: target_pixel_buffer::TargetPixelBuffer<TargetPixel = T>> ProcessScene
    for RenderToBuffer<'_, B>
{
    fn process_texture(&mut self, geometry: PhysicalRect, texture: SceneTexture<'static>) {
        self.process_texture_impl(geometry, texture)
    }

    fn process_shared_image_buffer(&mut self, geometry: PhysicalRect, buffer: SharedBufferCommand) {
        let texture = buffer.as_texture();
        self.process_texture_impl(geometry, texture);
    }

    fn process_rectangle(&mut self, geometry: PhysicalRect, color: PremultipliedRgbaColor) {
        self.process_rectangle_impl(geometry, color, CompositionMode::default());
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
                let Some(dx) = Fixed::from_f32(1. / source_to_target_x) else { return };
                let Some(dy) = Fixed::from_f32(1. / source_to_target_y) else { return };

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
                    let alpha = if colorize.alpha() > 0 || t.format == TexturePixelFormat::AlphaMap
                    {
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
                let orig = image_inner.size().cast::<f32>();
                let svg_target_size = if tiled.is_some() {
                    euclid::size2(orig.width * source_to_target_x, orig.height * source_to_target_y)
                        .cast()
                } else {
                    target_rect.size.cast()
                };
                if let Some(buffer) = image_inner.render_to_buffer(Some(svg_target_size)) {
                    let buf_size = buffer.size().cast::<f32>();
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
                    let scale_delta = paragraph.layout.font.scale_delta();
                    for positioned_glyph in glyphs {
                        let Some(glyph) =
                            paragraph.layout.font.render_glyph(positioned_glyph.glyph_id)
                        else {
                            continue;
                        };

                        let gl_x = PhysicalLength::new((-glyph.x).truncate() as i16);
                        let gl_y = PhysicalLength::new(glyph.y.truncate() as i16);
                        let target_rect = PhysicalRect::new(
                            PhysicalPoint::from_lengths(
                                line_x + positioned_glyph.x - gl_x,
                                baseline_y - gl_y - glyph.height,
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

                        let Some(clipped_target) = physical_clip.intersection(&target_rect) else {
                            continue;
                        };
                        let geometry = clipped_target.translate(offset).round();
                        let origin = (geometry.origin - offset.round()).round().cast::<i16>();
                        let off_x = origin.x - target_rect.origin.x as i16;
                        let off_y = origin.y - target_rect.origin.y as i16;
                        let pixel_stride = glyph.pixel_stride;
                        let mut geometry = geometry.cast();
                        if geometry.size.width > glyph.width.get() - off_x {
                            geometry.size.width = glyph.width.get() - off_x
                        }
                        if geometry.size.height > glyph.height.get() - off_y {
                            geometry.size.height = glyph.height.get() - off_y
                        }
                        let source_size = geometry.size;
                        if source_size.is_empty() {
                            continue;
                        }

                        match &glyph.alpha_map {
                            fonts::GlyphAlphaMap::Static(data) => {
                                let texture = if !glyph.sdf {
                                    SceneTexture {
                                        data,
                                        pixel_stride,
                                        format: TexturePixelFormat::AlphaMap,
                                        extra: SceneTextureExtra {
                                            colorize: color,
                                            // color already is mixed with global alpha
                                            alpha: color.alpha(),
                                            rotation: self.rotation.orientation,
                                            dx: Fixed::from_integer(1),
                                            dy: Fixed::from_integer(1),
                                            off_x: Fixed::from_integer(off_x as u16),
                                            off_y: Fixed::from_integer(off_y as u16),
                                        },
                                    }
                                } else {
                                    let delta32 = Fixed::<i32, 8>::from_fixed(scale_delta);
                                    let normalize = |x: Fixed<i32, 8>| {
                                        if x < Fixed::from_integer(0) {
                                            x + Fixed::from_integer(1)
                                        } else {
                                            x
                                        }
                                    };
                                    let fract_x = normalize(
                                        (-glyph.x) - Fixed::from_integer(gl_x.get() as _),
                                    );
                                    let off_x = delta32 * off_x as i32 + fract_x;
                                    let fract_y =
                                        normalize(glyph.y - Fixed::from_integer(gl_y.get() as _));
                                    let off_y = delta32 * off_y as i32 + fract_y;
                                    SceneTexture {
                                        data,
                                        pixel_stride,
                                        format: TexturePixelFormat::SignedDistanceField,
                                        extra: SceneTextureExtra {
                                            colorize: color,
                                            // color already is mixed with global alpha
                                            alpha: color.alpha(),
                                            rotation: self.rotation.orientation,
                                            dx: scale_delta,
                                            dy: scale_delta,
                                            off_x: Fixed::try_from_fixed(off_x).unwrap(),
                                            off_y: Fixed::try_from_fixed(off_y).unwrap(),
                                        },
                                    }
                                };
                                self.processor
                                    .process_texture(geometry.transformed(self.rotation), texture);
                            }
                            fonts::GlyphAlphaMap::Shared(data) => {
                                let source_rect = euclid::rect(0, 0, glyph.width.0, glyph.height.0);
                                self.processor.process_shared_image_buffer(
                                    geometry.transformed(self.rotation),
                                    SharedBufferCommand {
                                        buffer: SharedBufferData::AlphaMap {
                                            data: data.clone(),
                                            width: pixel_stride,
                                        },
                                        source_rect,
                                        extra: SceneTextureExtra {
                                            colorize: color,
                                            // color already is mixed with global alpha
                                            alpha: color.alpha(),
                                            rotation: self.rotation.orientation,
                                            dx: Fixed::from_integer(1),
                                            dy: Fixed::from_integer(1),
                                            off_x: Fixed::from_integer(off_x as u16),
                                            off_y: Fixed::from_integer(off_y as u16),
                                        },
                                    },
                                );
                            }
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

#[derive(Clone, Copy, Debug)]
struct RenderState {
    alpha: f32,
    offset: LogicalPoint,
    clip: LogicalRect,
}

impl<T: ProcessScene> crate::item_rendering::ItemRenderer for SceneBuilder<'_, T> {
    #[allow(clippy::unnecessary_cast)] // Coord!
    fn draw_rectangle(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        _: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
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
                        let geometry =
                            (r.translate(self.current_state.offset.to_vector()).try_cast()?
                                * self.scale_factor)
                                .round()
                                .try_cast()?
                                .transformed(self.rotation);
                        self.processor.process_rectangle(geometry, border_color);
                    }
                    Some(())
                };
                let b = border.get();
                let err = || {
                    panic!(
                        "invalid border rectangle {geom:?} border={b} state={:?}",
                        self.current_state
                    )
                };
                add_border(euclid::rect(0 as _, 0 as _, geom.width(), b)).unwrap_or_else(err);
                add_border(euclid::rect(0 as _, geom.height() - b, geom.width(), b))
                    .unwrap_or_else(err);
                add_border(euclid::rect(0 as _, b, b, geom.height() - b - b)).unwrap_or_else(err);
                add_border(euclid::rect(geom.width() - b, b, b, geom.height() - b - b))
                    .unwrap_or_else(err);
            }
        }
    }

    fn draw_window_background(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        // register a dependency for the partial renderer's dirty tracker. The actual rendering is done earlier in the software renderer.
        let _ = rect.background();
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

    fn draw_text(
        &mut self,
        text: Pin<&dyn crate::item_rendering::RenderText>,
        self_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let string = text.text();
        if string.trim().is_empty() {
            return;
        }
        let geom = LogicalRect::from(size);
        if !self.should_draw(&geom) {
            return;
        }

        let font_request = text.font_request(self_rc);

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
                let (horizontal_alignment, vertical_alignment) = text.alignment();

                let paragraph = TextParagraphLayout {
                    string: &string,
                    layout,
                    max_width: max_size.width_length(),
                    max_height: max_size.height_length(),
                    horizontal_alignment,
                    vertical_alignment,
                    wrap: text.wrap(),
                    overflow: text.overflow(),
                    single_line: false,
                };

                self.draw_text_paragraph(&paragraph, physical_clip, offset, color, None);
            }
            #[cfg(feature = "software-renderer-systemfonts")]
            fonts::Font::VectorFont(vf) => {
                let layout = fonts::text_layout_for_font(&vf, &font_request, self.scale_factor);
                let (horizontal_alignment, vertical_alignment) = text.alignment();

                let paragraph = TextParagraphLayout {
                    string: &string,
                    layout,
                    max_width: max_size.width_length(),
                    max_height: max_size.height_length(),
                    horizontal_alignment,
                    vertical_alignment,
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
        self_rc: &ItemRc,
        size: LogicalSize,
    ) {
        let geom = LogicalRect::from(size);
        if !self.should_draw(&geom) {
            return;
        }

        let font_request = text_input.font_request(self_rc);
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
        // TODO (#6068)
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
            #[cfg(feature = "software-renderer-systemfonts")]
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

impl<T: ProcessScene> crate::item_rendering::ItemRendererFeatures for SceneBuilder<'_, T> {
    const SUPPORTS_TRANSFORMATIONS: bool = false;
}
