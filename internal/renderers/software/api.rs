// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The public API of the software renderer, which `slint::platform::software_renderer`
//! re-exports. The other public items of this crate are for the crates of this repository.

pub use crate::draw_functions::{
    PremultipliedRgbaColor, Rgb565BigEndianPixel, Rgb565Pixel, TargetPixel,
};
pub use crate::minimal_software_window::MinimalSoftwareWindow;
#[cfg(feature = "experimental")]
pub use crate::target_pixel_buffer::{
    DrawRectangleArgs, DrawTextureArgs, TargetPixelBuffer, TexturePixelFormat,
};
pub use i_slint_core::partial_renderer::RepaintBufferType;

use crate::{
    DirtyRegionAlignment, PhysicalRect, TargetPixelSlice, region_line_ranges,
    render_window_frame_by_line,
};
use alloc::rc::{Rc, Weak};
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use i_slint_core::graphics::rendering_metrics_collector::RenderingMetricsCollector;
use i_slint_core::lengths::{LogicalSize, PhysicalPx, ScaleFactor};
use i_slint_core::partial_renderer::{DirtyRegion, PartialRenderingState};
#[cfg(feature = "systemfonts")]
use i_slint_core::textlayout::sharedparley;
use i_slint_core::window::WindowInner;

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
    pub(crate) fn is_transpose(self) -> bool {
        matches!(self, Self::Rotate90 | Self::Rotate270)
    }
    pub(crate) fn mirror_width(self) -> bool {
        matches!(self, Self::Rotate270 | Self::Rotate180)
    }
    pub(crate) fn mirror_height(self) -> bool {
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
    /// The `range` is the range within the line that is going to be rendered (eg, within the dirty region).
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
pub(crate) const PHYSICAL_REGION_MAX_SIZE: usize = DirtyRegion::MAX_COUNT;
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
    pub(crate) rectangles: [euclid::Box2D<i16, PhysicalPx>; PHYSICAL_REGION_MAX_SIZE],
    pub(crate) count: usize,
}

impl PhysicalRegion {
    pub(crate) fn iter_box(&self) -> impl Iterator<Item = euclid::Box2D<i16, PhysicalPx>> + '_ {
        (0..self.count).map(|x| self.rectangles[x])
    }

    pub(crate) fn bounding_rect(&self) -> PhysicalRect {
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
    pub fn bounding_box_size(&self) -> i_slint_core::api::PhysicalSize {
        let bb = self.bounding_rect();
        i_slint_core::api::PhysicalSize { width: bb.width() as _, height: bb.height() as _ }
    }
    /// Returns the origin of the bounding box of this region.
    pub fn bounding_box_origin(&self) -> i_slint_core::api::PhysicalPosition {
        let bb = self.bounding_rect();
        i_slint_core::api::PhysicalPosition { x: bb.origin.x as _, y: bb.origin.y as _ }
    }

    /// Returns an iterator over the rectangles in this region.
    /// Each rectangle is represented by its position and its size.
    /// They do not overlap.
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (i_slint_core::api::PhysicalPosition, i_slint_core::api::PhysicalSize)> + '_
    {
        let mut line_ranges = Vec::<core::ops::Range<i16>>::new();
        let mut begin_line = 0;
        let mut end_line = 0;
        core::iter::from_fn(move || {
            loop {
                match line_ranges.pop() {
                    Some(r) => {
                        return Some((
                            i_slint_core::api::PhysicalPosition {
                                x: r.start as _,
                                y: begin_line as _,
                            },
                            i_slint_core::api::PhysicalSize {
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
            }
        })
    }

    pub(crate) fn intersection(&self, clip: &PhysicalRect) -> PhysicalRegion {
        let mut res = Self::default();
        let clip = clip.to_box2d();
        let mut count = 0;
        for i in 0..self.count {
            if let Some(r) = self.rectangles[i].intersection(&clip) {
                res.rectangles[count] = r;
                count += 1;
            }
        }
        res.count = count;
        res
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
    pub(crate) repaint_buffer_type: Cell<RepaintBufferType>,
    pub(crate) dirty_region_alignment: Cell<DirtyRegionAlignment>,
    /// This is the area which was dirty on the previous frame.
    /// Only used if repaint_buffer_type == RepaintBufferType::SwappedBuffers
    pub(crate) prev_frame_dirty: Cell<DirtyRegion>,
    pub(crate) partial_rendering_state: PartialRenderingState,
    pub(crate) maybe_window_adapter: RefCell<Option<Weak<dyn i_slint_core::window::WindowAdapter>>>,
    pub(crate) rotation: Cell<RenderingRotation>,
    pub(crate) rendering_metrics_collector: Option<Rc<RenderingMetricsCollector>>,
    #[cfg(feature = "systemfonts")]
    pub(crate) text_layout_cache: sharedparley::TextLayoutCache,
}

impl Default for SoftwareRenderer {
    fn default() -> Self {
        Self {
            partial_rendering_state: Default::default(),
            prev_frame_dirty: Default::default(),
            dirty_region_alignment: Default::default(),
            maybe_window_adapter: Default::default(),
            rotation: Default::default(),
            rendering_metrics_collector: RenderingMetricsCollector::new("software"),
            repaint_buffer_type: Default::default(),
            #[cfg(feature = "systemfonts")]
            text_layout_cache: Default::default(),
        }
    }
}

#[cfg(feature = "testing")]
impl SoftwareRenderer {
    /// Returns a reference to the text layout cache for testing purposes.
    pub fn text_layout_cache(&self) -> &sharedparley::TextLayoutCache {
        &self.text_layout_cache
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

    /// Aligns dirty regions to a grid of `horizontal` by `vertical` physical pixels.
    ///
    /// Use this for display controllers that require the origin and size of address windows
    /// to be multiples of a fixed number of pixels.
    /// The pixels the alignment adds are repainted,
    /// so the region returned by [`Self::render`] and the range passed to
    /// [`LineBufferProvider::process_line`] can be sent to the display as they are.
    ///
    /// The axes are those of the screen after [`RenderingRotation`] is applied.
    /// Its width must be a multiple of `horizontal`, and its height a multiple of `vertical`.
    /// The default of one pixel on each axis leaves dirty regions unaligned.
    /// Values of zero are treated as one.
    pub fn set_dirty_region_alignment(&self, horizontal: u16, vertical: u16) {
        self.dirty_region_alignment
            .set(DirtyRegionAlignment { horizontal: horizontal.max(1), vertical: vertical.max(1) });
    }

    /// Set how the window need to be rotated in the buffer.
    ///
    /// This is typically used to implement screen rotation in software
    ///
    /// **Note:** This only affects rendering. Input events must still be given to
    /// Slint in logical (un-rotated) coordinates.
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
    /// # use i_slint_renderer_software::{LineBufferProvider, SoftwareRenderer, Rgb565Pixel};
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
        let component = i_slint_core::item_tree::ItemTreeRc::borrow_pin(&component_rc);
        if let Some(window_item) = i_slint_core::items::ItemRef::downcast_pin::<
            i_slint_core::items::WindowItem,
        >(component.as_ref().get_item_ref(0))
        {
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
