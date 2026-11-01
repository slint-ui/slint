// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore frontmost
#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(unsafe_code)]
#![cfg_attr(slint_nightly_test, feature(non_exhaustive_omitted_patterns_lint))]
#![cfg_attr(slint_nightly_test, warn(non_exhaustive_omitted_patterns))]
#![no_std]
#![warn(missing_docs)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod api;
mod box_shadow;
mod draw_functions;
mod fixed;
mod fonts;
mod minimal_software_window;
#[cfg(feature = "path")]
mod path;
mod scene;

pub use self::api::*;
use self::box_shadow::{BoxShadowCommand, process_drop_shadow};
use self::fonts::GlyphRenderer;
use self::scene::*;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::pin::Pin;
use euclid::Length;
use fixed::Fixed;
#[cfg(feature = "std")]
use i_slint_core::api::PlatformError;
#[cfg(feature = "owned-pre-rendered-resources")]
use i_slint_core::graphics::OwnedStorage;
#[cfg(feature = "std")]
use i_slint_core::graphics::Rgba8Pixel;
use i_slint_core::graphics::boxshadow::{BoxShadowOptions, drop_shadow_bounding_rect};
use i_slint_core::graphics::rendering_metrics_collector::RefreshMode;
use i_slint_core::graphics::{BorderRadius, SharedImageBuffer, SharedPixelBuffer};
use i_slint_core::graphics::{IntSize, StaticTexture};
use i_slint_core::item_rendering::HasFont;
use i_slint_core::item_rendering::{
    CachedRenderingData, ItemRenderer, ItemRendererFeatures, PlainOrStyledText,
    RenderBorderRectangle, RenderImage, RenderRectangle, clip_content_box,
};
use i_slint_core::item_tree::ItemTreeWeak;
use i_slint_core::items::{ItemRc, RenderingResult, TextOverflow, TextWrap};
use i_slint_core::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalRect, LogicalSize, LogicalVector,
    PhysicalPx, PointLengths, RectLengths, ScaleFactor, SizeLengths,
};
use i_slint_core::partial_renderer::{DirtyRegion, PartialRenderer};
use i_slint_core::renderer::RendererSealed;
use i_slint_core::textlayout::{AbstractFont, FontMetrics, TextParagraphLayout};
use i_slint_core::window::{WindowAdapter, WindowInner};
use i_slint_core::{Brush, Color, Coord, ImageInner, StaticTextures};
#[allow(unused)]
use num_traits::Float;
use num_traits::NumCast;

type PhysicalLength = euclid::Length<i16, PhysicalPx>;
type PhysicalRect = euclid::Rect<i16, PhysicalPx>;
type PhysicalSize = euclid::Size2D<i16, PhysicalPx>;
type PhysicalPoint = euclid::Point2D<i16, PhysicalPx>;
type PhysicalBorderRadius = BorderRadius<i16, PhysicalPx>;

use fonts::with_font;

#[derive(Copy, Clone, Debug)]
struct RotationInfo {
    orientation: RenderingRotation,
    screen_size: PhysicalSize,
}

impl RotationInfo {
    fn inverse(self) -> Self {
        let orientation = match self.orientation {
            RenderingRotation::Rotate90 => RenderingRotation::Rotate270,
            RenderingRotation::Rotate270 => RenderingRotation::Rotate90,
            orientation => orientation,
        };
        Self { orientation, screen_size: self.screen_size.transformed(self) }
    }
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

/// Aligns software-renderer dirty regions to a physical pixel grid.
///
/// Some display controllers require the origin and size of address windows to be multiples of a
/// fixed number of pixels.
/// The default alignment of one pixel on each axis preserves the renderer's existing behavior.
/// Set it with [`SoftwareRenderer::set_dirty_region_alignment`].
///
/// The physical screen width must be a multiple of the horizontal alignment, and the physical
/// screen height must be a multiple of the vertical alignment.
/// This applies after [`RenderingRotation`] transforms the screen.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
struct DirtyRegionAlignment {
    horizontal: u16,
    vertical: u16,
}

impl Default for DirtyRegionAlignment {
    fn default() -> Self {
        Self { horizontal: 1, vertical: 1 }
    }
}

fn expand_dirty_region_for_alignment(
    dirty_region: &DirtyRegion,
    factor: ScaleFactor,
    rotation: RenderingRotation,
    alignment: DirtyRegionAlignment,
) -> DirtyRegion {
    let (horizontal, vertical) = if rotation.is_transpose() {
        (alignment.vertical, alignment.horizontal)
    } else {
        (alignment.horizontal, alignment.vertical)
    };
    let factor = factor.get();
    // Inflating by the full alignment is deliberately more than the `alignment - 1` pixels that
    // snapping can add on each side: it stays a superset after the logical-to-physical rounding
    // and is cheaper than inverse-mapping the snapped physical region back to logical space.
    let horizontal = (horizontal as f32 / factor).ceil() as Coord;
    let vertical = (vertical as f32 / factor).ceil() as Coord;
    let mut expanded = DirtyRegion::default();
    for dirty_box in dirty_region.iter() {
        expanded.add_box(dirty_box.inflate(horizontal, vertical));
    }
    expanded
}

fn snap_interval_to_grid(min: i16, max: i16, granularity: u16, limit: i16) -> (i16, i16) {
    let granularity = granularity as i32;
    let min = min as i32;
    let max = max as i32;
    let limit = limit as i32;
    let snapped_min = min.div_euclid(granularity) * granularity;
    // The clamp can leave the extent unaligned when the screen size is not a multiple of the
    // granularity; that configuration is rejected by the debug_assert in to_physical_region,
    // and this is the release-mode fallback for it.
    let snapped_max = ((max + granularity - 1).div_euclid(granularity) * granularity).min(limit);
    (snapped_min as i16, snapped_max as i16)
}

fn to_physical_region(
    dirty_region: &DirtyRegion,
    factor: ScaleFactor,
    rotation: RotationInfo,
    size: PhysicalSize,
    alignment: DirtyRegionAlignment,
) -> PhysicalRegion {
    let screen_rect = PhysicalRect::from_size(size);
    let panel_size = size.transformed(rotation);
    debug_assert!(
        panel_size.width <= 0 || panel_size.width as i32 % alignment.horizontal as i32 == 0,
        "the screen width must be a multiple of the horizontal dirty-region alignment"
    );
    debug_assert!(
        panel_size.height <= 0 || panel_size.height as i32 % alignment.vertical as i32 == 0,
        "the screen height must be a multiple of the vertical dirty-region alignment"
    );

    let mut physical_region = PhysicalRegion::default();
    for dirty_box in dirty_region.iter() {
        let Some(rect) =
            (dirty_box.cast() * factor).to_rect().round_out().cast().intersection(&screen_rect)
        else {
            continue;
        };
        let mut aligned_box = rect.transformed(rotation).to_box2d();
        if alignment.horizontal > 1 {
            (aligned_box.min.x, aligned_box.max.x) = snap_interval_to_grid(
                aligned_box.min.x,
                aligned_box.max.x,
                alignment.horizontal,
                panel_size.width,
            );
        }
        if alignment.vertical > 1 {
            (aligned_box.min.y, aligned_box.max.y) = snap_interval_to_grid(
                aligned_box.min.y,
                aligned_box.max.y,
                alignment.vertical,
                panel_size.height,
            );
        }
        if aligned_box.is_empty() {
            continue;
        }
        debug_assert!(physical_region.count < PHYSICAL_REGION_MAX_SIZE);
        physical_region.rectangles[physical_region.count] = aligned_box;
        physical_region.count += 1;
    }
    physical_region
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
        (
            i_slint_core::api::PhysicalPosition { x, y },
            i_slint_core::api::PhysicalSize { width, height },
        )
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

#[test]
fn dirty_region_alignment_snaps_minimal_region() {
    use i_slint_core::lengths::LogicalRect;

    let factor = ScaleFactor::new(1.0);
    let size = euclid::size2(64, 64);
    let rotation = RotationInfo { orientation: RenderingRotation::NoRotation, screen_size: size };
    let mut dirty_region = DirtyRegion::default();
    dirty_region.add_rect(LogicalRect::new(euclid::point2(3.0, 5.0), euclid::size2(7.0, 9.0)));

    let aligned = to_physical_region(
        &dirty_region,
        factor,
        rotation,
        size,
        DirtyRegionAlignment { horizontal: 2, vertical: 2 },
    );
    assert_eq!(aligned.count, 1);
    assert_eq!(aligned.rectangles[0].min, euclid::point2(2, 4));
    assert_eq!(aligned.rectangles[0].max, euclid::point2(10, 14));

    let unaligned = to_physical_region(&dirty_region, factor, rotation, size, Default::default());
    assert_eq!(unaligned.count, 1);
    assert_eq!(unaligned.rectangles[0].min, euclid::point2(3, 5));
    assert_eq!(unaligned.rectangles[0].max, euclid::point2(10, 14));
}

#[test]
fn dirty_region_alignment_expands_with_scale_factor() {
    use i_slint_core::lengths::LogicalRect;

    let factor = ScaleFactor::new(1.5);
    let size = euclid::size2(48, 48);
    let rotation = RotationInfo { orientation: RenderingRotation::NoRotation, screen_size: size };
    let alignment = DirtyRegionAlignment { horizontal: 2, vertical: 2 };
    let mut dirty_region = DirtyRegion::default();
    dirty_region.add_rect(LogicalRect::new(euclid::point2(3.0, 5.0), euclid::size2(7.0, 9.0)));

    let aligned = to_physical_region(&dirty_region, factor, rotation, size, alignment);
    assert_eq!(aligned.count, 1);
    assert_eq!(aligned.rectangles[0].min, euclid::point2(4, 6));
    assert_eq!(aligned.rectangles[0].max, euclid::point2(16, 22));

    // The logical expansion is in logical pixels, so it has to cover the alignment divided by
    // the scale factor, rounded up.
    let expanded = expand_dirty_region_for_alignment(
        &dirty_region,
        factor,
        RenderingRotation::NoRotation,
        alignment,
    );
    let expanded_box = expanded.iter().next().unwrap();
    assert_eq!(expanded_box.min, euclid::point2(1.0, 3.0));
    assert_eq!(expanded_box.max, euclid::point2(12.0, 16.0));

    // What is drawn must cover what is reported as rendered.
    let redrawn = to_physical_region(&expanded, factor, rotation, size, Default::default());
    assert!(redrawn.rectangles[0].contains_box(&aligned.rectangles[0]));
}

#[test]
fn dirty_region_alignment_accepts_non_power_of_two_grid() {
    use i_slint_core::lengths::LogicalRect;

    let factor = ScaleFactor::new(1.0);
    let size = euclid::size2(48, 48);
    let rotation = RotationInfo { orientation: RenderingRotation::NoRotation, screen_size: size };
    let mut dirty_region = DirtyRegion::default();
    dirty_region.add_rect(LogicalRect::new(euclid::point2(4.0, 7.0), euclid::size2(5.0, 5.0)));

    let aligned = to_physical_region(
        &dirty_region,
        factor,
        rotation,
        size,
        DirtyRegionAlignment { horizontal: 3, vertical: 3 },
    );
    assert_eq!(aligned.count, 1);
    assert_eq!(aligned.rectangles[0].min, euclid::point2(3, 6));
    assert_eq!(aligned.rectangles[0].max, euclid::point2(9, 12));
}

#[test]
fn dirty_region_alignment_uses_rotated_panel_axes() {
    use i_slint_core::lengths::LogicalRect;

    let factor = ScaleFactor::new(1.0);
    let size = euclid::size2(80, 40);
    let rotation = RotationInfo { orientation: RenderingRotation::Rotate90, screen_size: size };
    let alignment = DirtyRegionAlignment { horizontal: 4, vertical: 8 };
    let mut dirty_region = DirtyRegion::default();
    dirty_region.add_rect(LogicalRect::new(euclid::point2(70.0, 30.0), euclid::size2(7.0, 7.0)));

    let aligned = to_physical_region(&dirty_region, factor, rotation, size, alignment);
    assert_eq!(aligned.count, 1);
    assert_eq!(aligned.rectangles[0].min, euclid::point2(0, 64));
    assert_eq!(aligned.rectangles[0].max, euclid::point2(12, 80));

    let expanded = expand_dirty_region_for_alignment(
        &dirty_region,
        factor,
        RenderingRotation::Rotate90,
        alignment,
    );
    let expanded_box = expanded.iter().next().unwrap();
    assert_eq!(expanded_box.min, euclid::point2(62.0, 26.0));
    assert_eq!(expanded_box.max, euclid::point2(85.0, 41.0));
}

#[test]
fn rotation_inverse_round_trips() {
    let screen_size = euclid::size2(80, 40);
    let rect = PhysicalRect::new(euclid::point2(3, 5), euclid::size2(7, 11));
    for orientation in [
        RenderingRotation::NoRotation,
        RenderingRotation::Rotate90,
        RenderingRotation::Rotate180,
        RenderingRotation::Rotate270,
    ] {
        let info = RotationInfo { orientation, screen_size };
        assert_eq!(rect.transformed(info).transformed(info.inverse()), rect, "{orientation:?}");
    }
}

#[test]
fn physical_region_count_excludes_clipped_rectangles() {
    use i_slint_core::lengths::LogicalRect;

    let factor = ScaleFactor::new(1.0);
    let size = euclid::size2(64, 64);
    let rotation = RotationInfo { orientation: RenderingRotation::NoRotation, screen_size: size };
    let mut dirty_region = DirtyRegion::default();
    dirty_region
        .add_rect(LogicalRect::new(euclid::point2(100.0, 100.0), euclid::size2(10.0, 10.0)));
    dirty_region.add_rect(LogicalRect::new(euclid::point2(3.0, 5.0), euclid::size2(7.0, 9.0)));

    let physical = to_physical_region(&dirty_region, factor, rotation, size, Default::default());
    assert_eq!(physical.count, 1);
    assert_eq!(physical.rectangles[0].min, euclid::point2(3, 5));
    assert_eq!(physical.rectangles[0].max, euclid::point2(10, 14));
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
                        false
                    } else if it.start <= r.end {
                        if it.end <= r.end {
                            false
                        } else {
                            it.start = r.start;
                            tmp = None;
                            true
                        }
                    } else {
                        core::mem::swap(it, r);
                        true
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
    debug_assert!(line_ranges.array_windows().all(|[a, b]| a.end < b.start));
    next_validity
}

mod target_pixel_buffer;

#[cfg(not(feature = "experimental"))]
use target_pixel_buffer::TexturePixelFormat;

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

impl SoftwareRenderer {
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
                pixels_per_line >= size.height as usize
                    && buffer_pixel_count
                        >= (size.width as usize * pixels_per_line + size.height as usize)
                            - pixels_per_line
            } else {
                pixels_per_line >= size.width as usize
                    && buffer_pixel_count
                        >= (size.height as usize * pixels_per_line + size.width as usize)
                            - pixels_per_line
            },
            "buffer of size {} with {pixels_per_line} pixels per line is too small to handle a window of size {size:?}",
            buffer_pixel_count
        );
        let buffer_renderer = SceneBuilder::new(
            size,
            factor,
            window_inner,
            RenderToBuffer {
                buffer,
                dirty_range_cache: Vec::new(),
                dirty_region: Default::default(),
                scale_factor: factor,
                clips: Vec::new(),
            },
            rotation,
            #[cfg(feature = "systemfonts")]
            &self.text_layout_cache,
        );
        let mut renderer = self.partial_rendering_state.create_partial_renderer(buffer_renderer);
        let window_adapter = renderer.window_adapter.clone();

        window_inner
            .draw_contents(|components, post_render| {
                let logical_size = (size.cast() / factor).cast();

                let dirty_region = self.compute_frame_dirty_region(
                    &mut renderer,
                    components,
                    logical_size,
                    factor,
                    size,
                );

                renderer.actual_renderer.processor.dirty_region = dirty_region.clone();
                if !renderer
                    .actual_renderer
                    .processor
                    .buffer
                    .fill_background(&background, &dirty_region)
                {
                    let gradient_background = gradient_background(&background);
                    if !gradient_background.is_some_and(Brush::is_opaque) {
                        let mut bg = TargetPixel::background();
                        if gradient_background.is_none() {
                            TargetPixel::blend(&mut bg, background.color().into());
                        }
                        renderer.actual_renderer.processor.foreach_ranges(
                            &dirty_region.bounding_rect(),
                            |_, buffer, _, _| {
                                buffer.fill(bg);
                            },
                        );
                    }
                    if let Some(gradient) = gradient_background {
                        renderer.actual_renderer.draw_window_background_gradient(gradient);
                    }
                }

                let partial = self.repaint_buffer_type.get() != RepaintBufferType::NewBuffer;
                for (component, origin) in components {
                    if let Some(component) = ItemTreeWeak::upgrade(component) {
                        i_slint_core::item_rendering::render_component_items(
                            &component,
                            if partial { &mut renderer } else { &mut renderer.actual_renderer },
                            *origin,
                            &window_adapter,
                        );
                    }
                }

                if partial {
                    post_render(&mut renderer);
                } else {
                    post_render(&mut renderer.actual_renderer);
                }

                self.measure_frame_rendered(&mut renderer);

                dirty_region
            })
            .unwrap_or_default()
    }
    /// Computes the dirty region for this frame according to the repaint buffer type, converts
    /// it to the physical region to return to the caller, applying the configured
    /// dirty region alignment, and expands the logical dirty region in place so that the
    /// partial renderer repaints every pixel the alignment added.
    ///
    /// This runs before the items are drawn, so `renderer`'s dirty region is what the partial
    /// renderer culls against.
    ///
    /// For `SwappedBuffers`, `prev_frame_dirty` intentionally keeps the unexpanded region:
    /// the next frame starts from a superset of it and re-applies the expansion.
    fn compute_frame_dirty_region<T: ItemRenderer + ItemRendererFeatures>(
        &self,
        renderer: &mut PartialRenderer<'_, T>,
        components: &[(ItemTreeWeak, LogicalPoint)],
        logical_size: LogicalSize,
        factor: ScaleFactor,
        size: PhysicalSize,
    ) -> PhysicalRegion {
        match self.repaint_buffer_type.get() {
            RepaintBufferType::NewBuffer => {
                // NewBuffer always redraws the full screen, so skip dirty region
                // tracking to avoid unbounded growth of the partial rendering cache.
                renderer.dirty_region = LogicalRect::from_size(logical_size).into();
                self.partial_rendering_state.clear_cache();
            }
            RepaintBufferType::ReusedBuffer => {
                self.partial_rendering_state.apply_dirty_region(
                    renderer,
                    components,
                    logical_size,
                    None,
                );
            }
            RepaintBufferType::SwappedBuffers => {
                let dirty_region_for_this_frame = self.partial_rendering_state.apply_dirty_region(
                    renderer,
                    components,
                    logical_size,
                    Some(self.prev_frame_dirty.take()),
                );
                self.prev_frame_dirty.set(dirty_region_for_this_frame);
            }
        }

        let alignment = self.dirty_region_alignment.get();
        let rotation = self.rotation.get();
        let physical_region = to_physical_region(
            &renderer.dirty_region,
            factor,
            RotationInfo { orientation: rotation, screen_size: size },
            size,
            alignment,
        );
        if alignment != DirtyRegionAlignment::default()
            && self.repaint_buffer_type.get() != RepaintBufferType::NewBuffer
        {
            renderer.dirty_region = expand_dirty_region_for_alignment(
                &renderer.dirty_region,
                factor,
                rotation,
                alignment,
            );
        }
        physical_region
    }
    fn measure_frame_rendered(&self, renderer: &mut dyn ItemRenderer) {
        if let Some(metrics) = &self.rendering_metrics_collector {
            let prev_frame_dirty = self.prev_frame_dirty.take();
            let m = i_slint_core::graphics::rendering_metrics_collector::RenderingMetrics {
                dirty_region: Some(prev_frame_dirty.clone()),
                ..Default::default()
            };
            self.prev_frame_dirty.set(prev_frame_dirty);
            metrics.measure_frame_rendered(renderer, m);
            if metrics.refresh_mode() == RefreshMode::FullSpeed {
                self.partial_rendering_state.force_screen_refresh();
            }
        }
    }
}

#[doc(hidden)]
impl RendererSealed for SoftwareRenderer {
    #[cfg(feature = "systemfonts")]
    fn text_layout_cache(&self) -> Option<&sharedparley::TextLayoutCache> {
        Some(&self.text_layout_cache)
    }

    fn text_size(
        &self,
        text_item: Pin<&dyn i_slint_core::item_rendering::RenderString>,
        item_rc: &i_slint_core::item_tree::ItemRc,
        max_width: Option<LogicalLength>,
        text_wrap: TextWrap,
    ) -> LogicalSize {
        let Some(scale_factor) = self.scale_factor() else {
            return LogicalSize::default();
        };
        let font_request = text_item.font_request(item_rc);
        // Evaluate text() before borrowing font_context: the binding can
        // re-enter text_size for other elements and would panic on a second
        // borrow_mut().
        let content = text_item.text();
        #[cfg(feature = "systemfonts")]
        let Some(slint_ctx) = self.slint_context() else {
            return Default::default();
        };
        let font = {
            #[cfg(feature = "systemfonts")]
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            fonts::match_font(
                &font_request,
                scale_factor,
                #[cfg(feature = "systemfonts")]
                &mut font_ctx,
            )
        };

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            return sharedparley::text_size(
                self,
                text_item,
                item_rc,
                max_width,
                text_wrap,
                Some(&self.text_layout_cache),
            )
            .unwrap_or_default();
        }

        let max_lines = text_item.line_limit();
        let string = match &content {
            PlainOrStyledText::Plain(string) => alloc::borrow::Cow::Borrowed(string.as_str()),
            PlainOrStyledText::Styled(styled_text) => {
                i_slint_core::styled_text::get_raw_text(styled_text)
            }
        };
        let (longest_line_width, height) = with_font!(&font, |font| {
            let layout = fonts::text_layout_for_font(
                font,
                &font_request,
                text_item.line_height_factor(),
                scale_factor,
            );
            layout.text_size(
                &string,
                max_width.map(|max_width| (max_width.cast() * scale_factor).cast()),
                text_wrap,
                max_lines,
            )
        });
        (PhysicalSize::from_lengths(longest_line_width, height).cast() / scale_factor).cast()
    }

    // Mirrors the font selection in text_size(), so both widths always come from the
    // same font as what is rendered. The bitmap path measures word breaks only, which is
    // all the caller asks for: char-wrap has no minimum and never reaches here.
    fn text_content_widths(
        &self,
        text_item: Pin<&dyn i_slint_core::item_rendering::RenderString>,
        item_rc: &i_slint_core::item_tree::ItemRc,
    ) -> Option<i_slint_core::renderer::ContentWidths> {
        let scale_factor = self.scale_factor()?;
        let font_request = text_item.font_request(item_rc);
        // Evaluate text() before borrowing font_context, as in text_size().
        let content = text_item.text();
        #[cfg(feature = "systemfonts")]
        let slint_ctx = self.slint_context()?;
        let font = {
            #[cfg(feature = "systemfonts")]
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            fonts::match_font(
                &font_request,
                scale_factor,
                #[cfg(feature = "systemfonts")]
                &mut font_ctx,
            )
        };

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            return sharedparley::text_content_widths(
                self,
                text_item,
                item_rc,
                Some(&self.text_layout_cache),
            );
        }

        let max_lines = text_item.line_limit();
        let string = match &content {
            PlainOrStyledText::Plain(string) => alloc::borrow::Cow::Borrowed(string.as_str()),
            PlainOrStyledText::Styled(styled_text) => {
                i_slint_core::styled_text::get_raw_text(styled_text)
            }
        };
        let (min, max) = with_font!(&font, |font| {
            fonts::text_layout_for_font(font, &font_request, None, scale_factor)
                .content_widths(&string, max_lines)
        });
        Some(i_slint_core::renderer::ContentWidths {
            min: (min.cast() / scale_factor).cast(),
            max: (max.cast() / scale_factor).cast(),
        })
    }

    // Bitmap fonts fall back to text_size() with the same font.
    fn text_line_height(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        line_height_factor: Option<f32>,
    ) -> Option<LogicalLength> {
        #[cfg(feature = "systemfonts")]
        {
            let scale_factor = self.scale_factor()?;
            let slint_ctx = self.slint_context()?;
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            let font = fonts::match_font(&font_request, scale_factor, &mut font_ctx);
            if uses_parley(&font) {
                return sharedparley::text_line_height(
                    &mut font_ctx,
                    &font_request,
                    line_height_factor,
                );
            }
        }
        #[cfg(not(feature = "systemfonts"))]
        let _ = (font_request, line_height_factor);
        None
    }

    fn char_size(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        line_height_factor: Option<f32>,
        ch: char,
    ) -> LogicalSize {
        let Some(scale_factor) = self.scale_factor() else {
            return LogicalSize::default();
        };
        #[cfg(feature = "systemfonts")]
        let Some(slint_ctx) = self.slint_context() else {
            return Default::default();
        };
        let font = {
            #[cfg(feature = "systemfonts")]
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            fonts::match_font(
                &font_request,
                scale_factor,
                #[cfg(feature = "systemfonts")]
                &mut font_ctx,
            )
        };

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            return sharedparley::char_size(&mut font_ctx, &font_request, line_height_factor, ch)
                .unwrap_or_default();
        }

        let (longest_line_width, height) = with_font!(&font, |font| {
            let mut buf = [0u8, 0u8, 0u8, 0u8];
            let layout =
                fonts::text_layout_for_font(font, &font_request, line_height_factor, scale_factor);
            layout.text_size(ch.encode_utf8(&mut buf), None, TextWrap::NoWrap, None)
        });
        (PhysicalSize::from_lengths(longest_line_width, height).cast() / scale_factor).cast()
    }

    fn font_metrics(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
    ) -> i_slint_core::items::FontMetrics {
        let Some(scale_factor) = self.scale_factor() else {
            return i_slint_core::items::FontMetrics::default();
        };
        #[cfg(feature = "systemfonts")]
        let Some(slint_ctx) = self.slint_context() else {
            return Default::default();
        };
        #[cfg(feature = "systemfonts")]
        let mut font_ctx = slint_ctx.font_context().borrow_mut();
        let font = fonts::match_font(
            &font_request,
            scale_factor,
            #[cfg(feature = "systemfonts")]
            &mut font_ctx,
        );

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            return sharedparley::font_metrics(&mut font_ctx, font_request);
        }

        // `Font` forwards the metrics to the concrete font itself, so this needs no dispatch.
        let ascent: LogicalLength = (font.ascent().cast() / scale_factor).cast();
        let descent: LogicalLength = (font.descent().cast() / scale_factor).cast();
        let x_height: LogicalLength = (font.x_height().cast() / scale_factor).cast();
        let cap_height: LogicalLength = (font.cap_height().cast() / scale_factor).cast();

        i_slint_core::items::FontMetrics {
            ascent: ascent.get() as _,
            descent: descent.get() as _,
            x_height: x_height.get() as _,
            cap_height: cap_height.get() as _,
        }
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        item_rc: &ItemRc,
        pos: LogicalPoint,
    ) -> (usize, i_slint_core::items::TextCursorAffinity) {
        let Some(scale_factor) = self.scale_factor() else {
            return Default::default();
        };
        let font_request = text_input.font_request(item_rc);
        #[cfg(feature = "systemfonts")]
        let Some(slint_ctx) = self.slint_context() else {
            return Default::default();
        };
        let font = {
            #[cfg(feature = "systemfonts")]
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            fonts::match_font(
                &font_request,
                scale_factor,
                #[cfg(feature = "systemfonts")]
                &mut font_ctx,
            )
        };

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            return sharedparley::text_input_byte_offset_for_position(
                self,
                text_input,
                item_rc,
                pos,
                Some(&self.text_layout_cache),
            );
        }

        let visual_representation = text_input.visual_representation();

        let pos = (pos.cast() * scale_factor)
            .clamp(euclid::point2(0., 0.), euclid::point2(i16::MAX, i16::MAX).cast())
            .cast();

        let byte_offset = with_font!(&font, |font| {
            let layout = fonts::text_layout_for_font(
                font,
                &font_request,
                i_slint_core::item_rendering::RenderString::line_height_factor(text_input),
                scale_factor,
            );
            let paragraph = text_input_query_paragraph(
                text_input,
                &visual_representation.text,
                layout,
                scale_factor,
            );
            paragraph.byte_offset_for_position((pos.x_length(), pos.y_length()))
        });

        (
            visual_representation.map_byte_offset_from_visual_text_to_actual_text(byte_offset),
            i_slint_core::items::TextCursorAffinity::NextCharacter,
        )
    }

    // Answers for the accessibility tree what `draw_text_input` decides for drawing; both
    // ask `uses_parley`, so they cannot describe a layout that isn't the one drawn.
    #[cfg(feature = "systemfonts")]
    fn text_input_has_parley_layout(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        item_rc: &ItemRc,
    ) -> bool {
        self.has_parley_layout(text_input.font_request(item_rc))
    }

    // Answers for the mouse selection what `draw_text` decides for drawing.
    #[cfg(feature = "systemfonts")]
    fn text_has_parley_layout(
        &self,
        text: Pin<&dyn i_slint_core::item_rendering::RenderText>,
        item_rc: &ItemRc,
    ) -> bool {
        self.has_parley_layout(text.font_request(item_rc))
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        item_rc: &ItemRc,
        byte_offset: usize,
        affinity: i_slint_core::items::TextCursorAffinity,
    ) -> LogicalRect {
        #[cfg(not(feature = "systemfonts"))]
        let _ = affinity;
        let Some(scale_factor) = self.scale_factor() else {
            return LogicalRect::default();
        };
        let font_request = text_input.font_request(item_rc);
        #[cfg(feature = "systemfonts")]
        let Some(slint_ctx) = self.slint_context() else {
            return Default::default();
        };
        let font = {
            #[cfg(feature = "systemfonts")]
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            fonts::match_font(
                &font_request,
                scale_factor,
                #[cfg(feature = "systemfonts")]
                &mut font_ctx,
            )
        };

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            return sharedparley::text_input_cursor_rect_for_byte_offset(
                self,
                text_input,
                item_rc,
                byte_offset,
                affinity,
                Some(&self.text_layout_cache),
            );
        }

        let visual_representation = text_input.visual_representation();

        let (cursor_position, band_offset, cursor_height) = with_font!(&font, |font| {
            let layout = fonts::text_layout_for_font(
                font,
                &font_request,
                i_slint_core::item_rendering::RenderString::line_height_factor(text_input),
                scale_factor,
            );
            let paragraph = text_input_query_paragraph(
                text_input,
                &visual_representation.text,
                layout,
                scale_factor,
            );

            let cursor_position = paragraph.cursor_pos_for_byte_offset(byte_offset);
            let (band_offset, cursor_height) = paragraph.layout.cursor_band();
            (cursor_position, band_offset, cursor_height)
        });

        (PhysicalRect::new(
            PhysicalPoint::from_lengths(cursor_position.0, cursor_position.1 + band_offset),
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
        component: i_slint_core::item_tree::ItemTreeRef,
        items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'_>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        #[cfg(feature = "systemfonts")]
        self.text_layout_cache.component_destroyed(component);
        self.partial_rendering_state.free_graphics_resources(component, items);
        Ok(())
    }

    fn mark_dirty_region(&self, region: DirtyRegion) {
        self.partial_rendering_state.mark_dirty_region(region);
    }

    fn register_bitmap_font(&self, font_data: &'static i_slint_core::graphics::BitmapFont) {
        fonts::register_bitmap_font(font_data);
    }

    #[cfg(feature = "owned-pre-rendered-resources")]
    fn register_shared_bitmap_font(
        &self,
        font: &Rc<i_slint_core::graphics::BitmapFont<i_slint_core::graphics::OwnedStorage>>,
    ) {
        fonts::register_shared_bitmap_font(font);
    }

    #[cfg(feature = "systemfonts")]
    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), std::boxed::Box<dyn std::error::Error>> {
        let ctx = self.slint_context().ok_or("slint platform not initialized")?;
        ctx.font_context().borrow_mut().register_static_font(data);
        Ok(())
    }

    #[cfg(all(feature = "systemfonts", not(target_arch = "wasm32")))]
    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), std::boxed::Box<dyn std::error::Error>> {
        let ctx = self.slint_context().ok_or("slint platform not initialized")?;
        self::fonts::systemfonts::register_font_from_path(
            &mut ctx.font_context().borrow_mut(),
            path,
        )
    }

    fn set_window_adapter(&self, window_adapter: &Rc<dyn WindowAdapter>) {
        *self.maybe_window_adapter.borrow_mut() = Some(Rc::downgrade(window_adapter));
        #[cfg(feature = "systemfonts")]
        self.text_layout_cache.clear_all();
        self.partial_rendering_state.clear_cache();
    }

    fn window_adapter(&self) -> Option<Rc<dyn WindowAdapter>> {
        self.maybe_window_adapter
            .borrow()
            .as_ref()
            .and_then(|window_adapter| window_adapter.upgrade())
    }

    // Rendering to a second pixel format monomorphizes the pipeline twice; keep it out of MCU builds.
    #[cfg(feature = "std")]
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

        if size.width == 0 || size.height == 0 {
            // Nothing to render
            return Err("take_snapshot() called on window with invalid size".into());
        };

        // Render into a premultiplied buffer so that windows with a transparent
        // or semi-transparent background end up with the right alpha in the
        // snapshot. PremultipliedRgbaColor::background() is (0,0,0,0), so
        // anything the window doesn't paint stays fully transparent.
        let mut premul = SharedPixelBuffer::<PremultipliedRgbaColor>::new(size.width, size.height);

        let old_repaint_buffer_type = self.repaint_buffer_type();
        // ensure that caches are clear
        self.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
        self.render(premul.make_mut_slice(), size.width as usize);
        self.set_repaint_buffer_type(old_repaint_buffer_type);

        let mut target_buffer_with_alpha =
            SharedPixelBuffer::<Rgba8Pixel>::new(premul.width(), premul.height());
        for (target_pixel, source_pixel) in
            target_buffer_with_alpha.make_mut_slice().iter_mut().zip(premul.as_slice().iter())
        {
            // Un-premultiply: straight RGBA is what the public API exposes (and
            // what PNG encoders expect). Round half up to keep `255 * a / a == 255`.
            let a = source_pixel.alpha;
            if a == 0 {
                *target_pixel = Rgba8Pixel::new(0, 0, 0, 0);
            } else {
                let unp = |c: u8| ((c as u32 * 255 + (a as u32 / 2)) / a as u32).min(255) as u8;
                *target_pixel = Rgba8Pixel::new(
                    unp(source_pixel.red),
                    unp(source_pixel.green),
                    unp(source_pixel.blue),
                    a,
                );
            }
        }
        Ok(target_buffer_with_alpha)
    }

    fn supports_transformations(&self) -> bool {
        false
    }
}

/// Only the vector font path can hand off to parley, so this is only ever asked under
/// `systemfonts`; without it there is nothing to disable.
///
/// Read once: this sits on the per-item, per-frame text path, and every other `SLINT_`
/// switch is resolved once as well. Setting the variable after the first text is laid
/// out therefore has no effect.
#[cfg(feature = "systemfonts")]
fn parley_disabled() -> bool {
    static DISABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DISABLED.get_or_init(|| std::env::var_os("SLINT_SOFTWARE_RENDERER_PARLEY_DISABLED").is_some())
}

/// Whether this font is laid out by parley rather than by the local text layout.
///
/// Every text method has to agree on this: the accessibility tree describes the layout
/// `text_input_has_parley_layout` reports, and it has to be the one that was drawn.
#[cfg(feature = "systemfonts")]
fn uses_parley(font: &fonts::Font) -> bool {
    matches!(font, fonts::Font::Vector(_)) && !parley_disabled()
}

#[cfg(feature = "systemfonts")]
impl SoftwareRenderer {
    /// Whether text in the font `font_request` asks for is laid out through parley.
    fn has_parley_layout(&self, font_request: i_slint_core::graphics::FontRequest) -> bool {
        let (Some(scale_factor), Some(slint_ctx)) = (self.scale_factor(), self.slint_context())
        else {
            return false;
        };
        let font = {
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            fonts::match_font(&font_request, scale_factor, &mut font_ctx)
        };
        uses_parley(&font)
    }
}

/// The paragraph layout the cursor and hit-testing queries measure against.
///
/// `string` is the text input's visual representation, which the caller owns because the
/// returned layout borrows from it.
///
/// Note that this does not pass on the text input's `single_line`, matching what these
/// queries did before they shared this helper. `draw_text_input` does pass it on.
fn text_input_query_paragraph<'a, Font>(
    text_input: Pin<&i_slint_core::items::TextInput>,
    string: &'a str,
    layout: i_slint_core::textlayout::TextLayout<'a, Font>,
    scale_factor: ScaleFactor,
) -> TextParagraphLayout<'a, Font>
where
    Font: AbstractFont + i_slint_core::textlayout::TextShaper<Length = PhysicalLength>,
{
    TextParagraphLayout {
        string,
        layout,
        max_width: (text_input.width().cast() * scale_factor).cast(),
        max_height: (text_input.height().cast() * scale_factor).cast(),
        horizontal_alignment: text_input.horizontal_alignment(),
        vertical_alignment: text_input.vertical_alignment(),
        wrap: text_input.wrap(),
        overflow: TextOverflow::Clip,
        single_line: false,
        max_lines: None,
    }
}

fn render_window_frame_by_line(
    window: &WindowInner,
    background: Brush,
    size: PhysicalSize,
    renderer: &SoftwareRenderer,
    mut line_buffer: impl LineBufferProvider,
) -> PhysicalRegion {
    let gradient_background = gradient_background(&background);
    let mut scene = prepare_scene(window, size, gradient_background, renderer);

    let to_draw_tr = scene.dirty_region.bounding_rect();

    let mut background_color = TargetPixel::background();
    if gradient_background.is_none() {
        TargetPixel::blend(&mut background_color, background.color().into());
    }

    while scene.current_line < to_draw_tr.origin.y_length() + to_draw_tr.size.height_length() {
        for r in &scene.current_line_ranges {
            line_buffer.process_line(
                scene.current_line.get() as usize,
                r.start as usize..r.end as usize,
                |line_buffer| {
                    let offset = r.start;

                    let items = &scene.items[0..scene.current_items_index];
                    // A span that opaquely covers the whole range hides
                    // everything behind it, background included. Draw from the
                    // frontmost such span and drop the rest.
                    let first_cover = items.iter().position(|span| {
                        span.pos.x <= r.start
                            && span.pos.x + span.size.width >= r.end
                            && scene.covers_line(span)
                    });
                    let items = match first_cover {
                        Some(i) => &items[..=i],
                        None => {
                            line_buffer.fill(background_color);
                            items
                        }
                    };
                    for span in items.iter().rev() {
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

                        if let SceneCommand::Rectangle { color } = span.command
                            && span.clip_len == 0
                        {
                            TargetPixel::blend_slice(range_buffer, color);
                            continue;
                        }
                        let rect = PhysicalRect { origin: span.pos, size: span.size };
                        let clips = span.clips(&scene.vectors);
                        // Closures can't be generic, and `&dyn` would dispatch every unclipped line.
                        macro_rules! draw {
                            ($cmd:expr) => {
                                draw_functions::draw_clipped_line(
                                    &rect,
                                    scene.current_line,
                                    clips,
                                    $cmd,
                                    range_buffer,
                                    extra_left_clip,
                                    extra_right_clip,
                                )
                            };
                        }
                        match span.command {
                            SceneCommand::Rectangle { color } => {
                                draw!(&draw_functions::SolidColor(color))
                            }
                            SceneCommand::Texture { texture_index } => {
                                draw!(&scene.vectors.textures[texture_index as usize])
                            }
                            SceneCommand::SharedBuffer { shared_buffer_index } => draw!(
                                &scene.vectors.shared_buffers[shared_buffer_index as usize]
                                    .as_texture()
                            ),
                            SceneCommand::RoundedRectangle { rectangle_index } => {
                                draw!(&scene.vectors.rounded_rectangles[rectangle_index as usize])
                            }
                            SceneCommand::LinearGradient { linear_gradient_index } => {
                                draw!(
                                    &scene.vectors.linear_gradients[linear_gradient_index as usize]
                                )
                            }
                            SceneCommand::RadialGradient { radial_gradient_index } => {
                                draw!(
                                    &scene.vectors.radial_gradients[radial_gradient_index as usize]
                                )
                            }
                            SceneCommand::ConicGradient { conic_gradient_index } => {
                                draw!(&scene.vectors.conic_gradients[conic_gradient_index as usize])
                            }
                            SceneCommand::BoxShadow { box_shadow_index } => {
                                draw!(&scene.vectors.box_shadows[box_shadow_index as usize])
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

fn gradient_background(background: &Brush) -> Option<&Brush> {
    (!matches!(background, Brush::SolidColor(_))).then_some(background)
}

fn prepare_scene(
    window: &WindowInner,
    size: PhysicalSize,
    gradient_background: Option<&Brush>,
    software_renderer: &SoftwareRenderer,
) -> Scene {
    let factor = ScaleFactor::new(window.scale_factor());
    let prepare_scene = SceneBuilder::new(
        size,
        factor,
        window,
        PrepareScene { scale_factor: factor, ..Default::default() },
        software_renderer.rotation.get(),
        #[cfg(feature = "systemfonts")]
        &software_renderer.text_layout_cache,
    );
    let mut renderer =
        software_renderer.partial_rendering_state.create_partial_renderer(prepare_scene);
    let window_adapter = renderer.window_adapter.clone();

    let mut dirty_region = PhysicalRegion::default();
    window.draw_contents(|components, post_render| {
        let logical_size = (size.cast() / factor).cast();

        dirty_region = software_renderer.compute_frame_dirty_region(
            &mut renderer,
            components,
            logical_size,
            factor,
            size,
        );

        if let Some(gradient) = gradient_background {
            renderer.actual_renderer.draw_window_background_gradient(gradient);
        }

        let partial = software_renderer.repaint_buffer_type.get() != RepaintBufferType::NewBuffer;
        for (component, origin) in components {
            if let Some(component) = ItemTreeWeak::upgrade(component) {
                i_slint_core::item_rendering::render_component_items(
                    &component,
                    if partial { &mut renderer } else { &mut renderer.actual_renderer },
                    *origin,
                    &window_adapter,
                );
            }
        }

        if partial {
            post_render(&mut renderer);
        } else {
            post_render(&mut renderer.actual_renderer);
        }
    });

    software_renderer.measure_frame_rendered(&mut renderer);

    let prepare_scene = renderer.into_inner();

    /* // visualize dirty regions
    let mut prepare_scene = prepare_scene;
    for rect in dirty_region.iter() {
        prepare_scene.processor.process_rounded_rectangle(
            euclid::rect(rect.0.x as _, rect.0.y as _, rect.1.width as _, rect.1.height as _),
            RoundedRectangle {
                shape: RoundedShape::default(),
                width: Length::new(1),
                border_color: Color::from_argb_u8(128, 255, 0, 0).into(),
                inner_color: PremultipliedRgbaColor::default(),
            },
            &[],
        )
    } // */

    Scene::new(prepare_scene.processor.items, prepare_scene.processor.vectors, dirty_region)
}

/// Receives the commands of a frame.
/// Each command is clipped to the rounded clips it's given, and its rect lies inside theirs.
trait ProcessScene {
    fn process_scene_texture(
        &mut self,
        geometry: PhysicalRect,
        texture: SceneTexture<'static>,
        rounded_clips: &[RoundedClip],
    );
    fn process_shared_buffer(
        &mut self,
        geometry: PhysicalRect,
        command: SharedBufferCommand,
        rounded_clips: &[RoundedClip],
    );
    fn process_target_texture(
        &mut self,
        texture: &target_pixel_buffer::DrawTextureArgs,
        clip: PhysicalRect,
        rounded_clips: &[RoundedClip],
    );
    fn process_rectangle(
        &mut self,
        _: &target_pixel_buffer::DrawRectangleArgs,
        clip: PhysicalRect,
        rounded_clips: &[RoundedClip],
    );

    fn process_simple_rectangle(
        &mut self,
        geometry: PhysicalRect,
        color: PremultipliedRgbaColor,
        rounded_clips: &[RoundedClip],
    );
    fn process_rounded_rectangle(
        &mut self,
        geometry: PhysicalRect,
        data: RoundedRectangle,
        rounded_clips: &[RoundedClip],
    );
    /// `clip` is the gradient's own shape, which applies after `rounded_clips`.
    fn process_linear_gradient(
        &mut self,
        geometry: PhysicalRect,
        gradient: LinearGradientCommand,
        rounded_clips: &[RoundedClip],
        clip: Option<ShapeClip>,
    );
    fn process_radial_gradient(
        &mut self,
        geometry: PhysicalRect,
        gradient: RadialGradientCommand,
        rounded_clips: &[RoundedClip],
        clip: Option<ShapeClip>,
    );
    fn process_conic_gradient(
        &mut self,
        geometry: PhysicalRect,
        gradient: ConicGradientCommand,
        rounded_clips: &[RoundedClip],
        clip: Option<ShapeClip>,
    );
    fn process_box_shadow(
        &mut self,
        geometry: PhysicalRect,
        shadow: BoxShadowCommand,
        rounded_clips: &[RoundedClip],
    );
    #[cfg(feature = "path")]
    fn process_filled_path(
        &mut self,
        path_geometry: PhysicalRect,
        clip_geometry: PhysicalRect,
        commands: alloc::vec::Vec<path::Command>,
        paint: path::Paint,
        rounded_clips: &[RoundedClip],
    );
    #[cfg(feature = "path")]
    fn process_stroked_path(
        &mut self,
        path_geometry: PhysicalRect,
        clip_geometry: PhysicalRect,
        commands: alloc::vec::Vec<path::Command>,
        paint: path::Paint,
        stroke_style: path::StrokeStyle<'_>,
        rounded_clips: &[RoundedClip],
    );
}

/// A rounded clip in physical screen coordinates, after [`RenderingRotation`].
#[derive(Clone, Copy, Debug)]
struct RoundedClip {
    rect: PhysicalRect,
    radius: PhysicalBorderRadius,
}

fn clip_to_rounded_clips(
    rounded_clips: &[RoundedClip],
    rect: PhysicalRect,
) -> Option<PhysicalRect> {
    rounded_clips.iter().try_fold(rect, |rect, clip| rect.intersection(&clip.rect))
}

fn shape_clips<'a>(
    rounded_clips: &'a [RoundedClip],
    geometry: &'a PhysicalRect,
) -> impl Iterator<Item = ShapeClip> + 'a {
    rounded_clips
        .iter()
        .map(move |clip| {
            let rect = clip.rect;
            debug_assert!(rect.contains_rect(geometry), "{geometry:?} is outside {rect:?}");
            ShapeClip {
                shape: RoundedShape {
                    radius: clip.radius,
                    left_clip: Length::new(geometry.min_x() - rect.min_x()),
                    right_clip: Length::new(rect.max_x() - geometry.max_x()),
                    top_clip: Length::new(geometry.min_y() - rect.min_y()),
                    bottom_clip: Length::new(rect.max_y() - geometry.max_y()),
                },
                opaque_border: PhysicalLength::new(0),
            }
        })
        .filter(|clip| clip.touches_corner(geometry.size))
}

/// Whether the `TargetPixelBuffer` hooks may draw a command in `geometry`.
/// They can't clip to a rounded shape.
fn hook_can_draw(rounded_clips: &[RoundedClip], geometry: impl FnOnce() -> PhysicalRect) -> bool {
    rounded_clips.is_empty() || shape_clips(rounded_clips, &geometry()).next().is_none()
}

fn process_rectangle_impl(
    processor: &mut dyn ProcessScene,
    args: &target_pixel_buffer::DrawRectangleArgs,
    clip: &PhysicalRect,
    scale_factor: ScaleFactor,
    rounded_clips: &[RoundedClip],
) {
    let geom = args.geometry();
    let Some(clipped) = geom.intersection(&clip.cast()) else { return };
    let radius = PhysicalBorderRadius {
        top_left: args.top_left_radius as _,
        top_right: args.top_right_radius as _,
        bottom_right: args.bottom_right_radius as _,
        bottom_left: args.bottom_left_radius as _,
        _unit: Default::default(),
    };
    // Add a small value to make sure that the clip is always positive despite floating point
    // issues. A clip past `i16::MAX` saturates, which is still larger than any radius or border.
    const E: f32 = 0.00001;
    let rounded_shape = RoundedShape {
        radius,
        top_clip: PhysicalLength::new((clipped.min_y() - geom.min_y() + E) as _),
        bottom_clip: PhysicalLength::new((geom.max_y() - clipped.max_y() + E) as _),
        left_clip: PhysicalLength::new((clipped.min_x() - geom.min_x() + E) as _),
        right_clip: PhysicalLength::new((geom.max_x() - clipped.max_x() + E) as _),
    };

    let mut border_color =
        PremultipliedRgbaColor::from(alpha_color(args.border.color(), args.alpha));
    let border =
        PhysicalLength::new(if border_color.alpha == 0 { 0 } else { args.border_width as _ });
    let gradient_clip = |rect: &PhysicalRect| {
        Some(ShapeClip {
            shape: rounded_shape,
            opaque_border: if border_color.alpha == u8::MAX {
                border
            } else {
                PhysicalLength::new(0)
            },
        })
        .filter(|clip| clip.touches_corner(rect.size))
    };
    let fill = gradient_commands(
        &args.background,
        args.alpha,
        args.rotation,
        geom,
        clipped,
        scale_factor,
        &mut |rect, gradient| match gradient {
            AnyGradientCommand::Linear(g) => {
                processor.process_linear_gradient(rect, g, rounded_clips, gradient_clip(&rect))
            }
            AnyGradientCommand::Radial(g) => {
                processor.process_radial_gradient(rect, g, rounded_clips, gradient_clip(&rect))
            }
            AnyGradientCommand::Conic(g) => {
                processor.process_conic_gradient(rect, g, rounded_clips, gradient_clip(&rect))
            }
        },
    );

    let color = match fill {
        BrushFill::Solid(color) => PremultipliedRgbaColor::from(color),
        BrushFill::Gradient => PremultipliedRgbaColor::default(),
    };
    if border_color.alpha > 0 && border_color.alpha < 255 {
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
            alpha: (color.alpha as u16 + b_alpha_16 - (color.alpha as u16 * b_alpha_16) / 255)
                as u8,
        }
    }

    if !radius.is_zero() {
        processor.process_rounded_rectangle(
            clipped.round().cast(),
            RoundedRectangle {
                shape: rounded_shape,
                width: border,
                border_color,
                inner_color: color,
            },
            rounded_clips,
        );
        return;
    }

    // `cast` panics past the `i16` range, while `as` saturates and keeps a far edge far away.
    let b = border.get();
    let g = geom.round().to_box2d();
    let outer = euclid::Box2D::<i16, PhysicalPx>::new(
        euclid::point2(g.min.x as i16, g.min.y as i16),
        euclid::point2(g.max.x as i16, g.max.y as i16),
    );
    let inner = euclid::Box2D::new(
        euclid::point2(outer.min.x.saturating_add(b), outer.min.y.saturating_add(b)),
        euclid::point2(outer.max.x.saturating_sub(b), outer.max.y.saturating_sub(b)),
    );
    let clip = clip.to_box2d();
    let mut fill = |r: euclid::Box2D<i16, PhysicalPx>, color| {
        if let Some(r) = r.intersection(&clip) {
            processor.process_simple_rectangle(r.to_rect(), color, rounded_clips);
        }
    };
    if color.alpha > 0 {
        fill(inner, color);
    }
    if border_color.alpha > 0 {
        fill(euclid::Box2D::new(outer.min, euclid::point2(outer.max.x, inner.min.y)), border_color);
        fill(euclid::Box2D::new(euclid::point2(outer.min.x, inner.max.y), outer.max), border_color);
        fill(
            euclid::Box2D::new(
                euclid::point2(outer.min.x, inner.min.y),
                euclid::point2(inner.min.x, inner.max.y),
            ),
            border_color,
        );
        fill(
            euclid::Box2D::new(
                euclid::point2(inner.max.x, inner.min.y),
                euclid::point2(outer.max.x, inner.max.y),
            ),
            border_color,
        );
    }
}

/// What [`gradient_commands`] fills a shape with.
enum BrushFill {
    Solid(Color),
    /// The gradient's commands went to the sink.
    Gradient,
}

fn gradient_commands(
    brush: &Brush,
    alpha: u8,
    rotation: RenderingRotation,
    gradient_box: euclid::Rect<f32, PhysicalPx>,
    draw_rect: euclid::Rect<f32, PhysicalPx>,
    scale_factor: ScaleFactor,
    sink: &mut dyn FnMut(PhysicalRect, AnyGradientCommand),
) -> BrushFill {
    let box_w = gradient_box.width();
    let box_h = gradient_box.height();
    let (item_w, item_h) = if rotation.is_transpose() { (box_h, box_w) } else { (box_w, box_h) };
    let rect: PhysicalRect = draw_rect.round().cast();
    let to_rect_center = |x: f32, y: f32| {
        let (cx, cy) = match rotation {
            RenderingRotation::NoRotation => (x, y),
            RenderingRotation::Rotate90 => (box_w - y, x),
            RenderingRotation::Rotate180 => (box_w - x, box_h - y),
            RenderingRotation::Rotate270 => (y, box_h - x),
        };
        (
            gradient_box.min_x() + cx - rect.min_x() as f32,
            gradient_box.min_y() + cy - rect.min_y() as f32,
        )
    };

    match brush {
        Brush::LinearGradient(g) if g.stops().nth(1).is_some() => {
            let (start, end) = i_slint_core::graphics::line_for_angle(
                g.angle() + rotation.angle(),
                euclid::size2(box_w, box_h),
            );
            let (axis_x, axis_y) = (end.x - start.x, end.y - start.y);
            let length_squared = axis_x * axis_x + axis_y * axis_y;
            let (ux, uy) = (axis_x / length_squared, axis_y / length_squared);
            // Pixels are sampled at their centers.
            let px = rect.min_x() as f32 + 0.5 - gradient_box.min_x() - start.x;
            let py = rect.min_y() as f32 + 0.5 - gradient_box.min_y() - start.y;
            // `None` for a gradient line of zero length, or one too short for the fixed point.
            if let Some(position) = LinearGradientPosition::from_f32(px * ux + py * uy)
                && let Some(dx) = LinearGradientPosition::from_f32(ux)
                && let Some(dy) = LinearGradientPosition::from_f32(uy)
            {
                let stops = LinearGradientStop::collect(
                    g.stops().map(|s| (alpha_color(s.color, alpha).into(), s.position)),
                    dx,
                );
                let gr = LinearGradientCommand { stops, position, dx, dy };
                sink(rect, AnyGradientCommand::Linear(gr));
            }
            BrushFill::Gradient
        }
        Brush::RadialGradient(g) if g.stops().nth(1).is_some() => {
            let (cx, cy) = g.center_or_default_scaled(item_w, item_h, scale_factor.get());
            let (center_x, center_y) = to_rect_center(cx, cy);
            let (rx, ry) = g.radii_or_default_scaled(item_w, item_h, scale_factor.get());
            let (radius_x, radius_y) = if rotation.is_transpose() { (ry, rx) } else { (rx, ry) };

            let radial_grad = RadialGradientCommand {
                stops: PremultipliedGradientStop::collect(
                    g.stops().map(|s| (alpha_color(s.color, alpha).into(), s.position)),
                ),
                center_x,
                center_y,
                radius_x,
                radius_y,
            };

            sink(rect, AnyGradientCommand::Radial(radial_grad));
            BrushFill::Gradient
        }
        Brush::ConicGradient(g) if g.stops().nth(1).is_some() => {
            let (cx, cy) = g.center_or_default_scaled(item_w, item_h, scale_factor.get());
            let (center_x, center_y) = to_rect_center(cx, cy);
            let conic_grad = ConicGradientCommand {
                stops: PremultipliedGradientStop::collect(
                    g.stops().map(|s| (alpha_color(s.color, alpha).into(), s.position)),
                ),
                center_x,
                center_y,
                rotation: rotation.angle().to_radians(),
            };

            sink(rect, AnyGradientCommand::Conic(conic_grad));
            BrushFill::Gradient
        }
        Brush::SolidColor(color) => BrushFill::Solid(alpha_color(*color, alpha)),
        _ => BrushFill::Solid(alpha_color(brush.color(), alpha)),
    }
}

struct RenderToBuffer<'a, TargetPixelBuffer> {
    buffer: &'a mut TargetPixelBuffer,
    dirty_range_cache: Vec<core::ops::Range<i16>>,
    dirty_region: PhysicalRegion,
    scale_factor: ScaleFactor,
    /// Reused by every command, so drawing doesn't allocate.
    clips: Vec<ShapeClip>,
}

impl<B: target_pixel_buffer::TargetPixelBuffer> RenderToBuffer<'_, B> {
    fn foreach_ranges(
        &mut self,
        geometry: &PhysicalRect,
        mut f: impl FnMut(i16, &mut [B::TargetPixel], i16, i16),
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

                for l in region.y_range() {
                    f(
                        l,
                        &mut self.buffer.line_slice(l as usize)
                            [region.min_x() as usize..region.max_x() as usize],
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

    fn draw_clipped(
        &mut self,
        geometry: PhysicalRect,
        rounded_clips: &[RoundedClip],
        own_clip: Option<ShapeClip>,
        cmd: &impl draw_functions::LineCommand<B::TargetPixel>,
    ) {
        if rounded_clips.is_empty() && own_clip.is_none() {
            self.foreach_ranges(&geometry, |line, buffer, extra_left_clip, extra_right_clip| {
                cmd.draw_line(
                    &geometry,
                    PhysicalLength::new(line),
                    buffer,
                    extra_left_clip,
                    extra_right_clip,
                );
            });
            return;
        }
        let mut clips = core::mem::take(&mut self.clips);
        clips.clear();
        clips.extend(shape_clips(rounded_clips, &geometry).chain(own_clip));
        self.foreach_ranges(&geometry, |line, buffer, extra_left_clip, extra_right_clip| {
            draw_functions::draw_clipped_line(
                &geometry,
                PhysicalLength::new(line),
                &clips,
                cmd,
                buffer,
                extra_left_clip,
                extra_right_clip,
            );
        });
        self.clips = clips;
    }
}

impl<B: target_pixel_buffer::TargetPixelBuffer> ProcessScene for RenderToBuffer<'_, B> {
    fn process_scene_texture(
        &mut self,
        geometry: PhysicalRect,
        texture: SceneTexture<'static>,
        rounded_clips: &[RoundedClip],
    ) {
        self.draw_clipped(geometry, rounded_clips, None, &texture);
    }

    fn process_shared_buffer(
        &mut self,
        geometry: PhysicalRect,
        command: SharedBufferCommand,
        rounded_clips: &[RoundedClip],
    ) {
        self.draw_clipped(geometry, rounded_clips, None, &command.as_texture());
    }

    fn process_target_texture(
        &mut self,
        texture: &target_pixel_buffer::DrawTextureArgs,
        clip: PhysicalRect,
        rounded_clips: &[RoundedClip],
    ) {
        if hook_can_draw(rounded_clips, || {
            let geometry = euclid::rect::<i32, PhysicalPx>(
                texture.dst_x as _,
                texture.dst_y as _,
                texture.dst_width as _,
                texture.dst_height as _,
            );
            geometry.intersection(&clip.cast()).map_or_else(Default::default, |r| r.cast())
        }) && self.buffer.draw_texture(texture, &self.dirty_region.intersection(&clip))
        {
            return;
        }

        let Some((texture, geometry)) = SceneTexture::from_target_texture(texture, &clip) else {
            return;
        };

        self.draw_clipped(geometry, rounded_clips, None, &texture);
    }

    fn process_rectangle(
        &mut self,
        args: &target_pixel_buffer::DrawRectangleArgs,
        clip: PhysicalRect,
        rounded_clips: &[RoundedClip],
    ) {
        if hook_can_draw(rounded_clips, || {
            args.geometry().round_out().cast().intersection(&clip).unwrap_or_default()
        }) && self.buffer.draw_rectangle(args, &self.dirty_region.intersection(&clip))
        {
            return;
        }

        let scale_factor = self.scale_factor;
        process_rectangle_impl(self, args, &clip, scale_factor, rounded_clips);
    }

    fn process_rounded_rectangle(
        &mut self,
        geometry: PhysicalRect,
        rr: RoundedRectangle,
        rounded_clips: &[RoundedClip],
    ) {
        self.draw_clipped(geometry, rounded_clips, None, &rr);
    }

    fn process_simple_rectangle(
        &mut self,
        geometry: PhysicalRect,
        color: PremultipliedRgbaColor,
        rounded_clips: &[RoundedClip],
    ) {
        self.draw_clipped(geometry, rounded_clips, None, &draw_functions::SolidColor(color));
    }

    fn process_linear_gradient(
        &mut self,
        geometry: PhysicalRect,
        g: LinearGradientCommand,
        rounded_clips: &[RoundedClip],
        clip: Option<ShapeClip>,
    ) {
        self.draw_clipped(geometry, rounded_clips, clip, &g);
    }
    fn process_radial_gradient(
        &mut self,
        geometry: PhysicalRect,
        g: RadialGradientCommand,
        rounded_clips: &[RoundedClip],
        clip: Option<ShapeClip>,
    ) {
        self.draw_clipped(geometry, rounded_clips, clip, &g);
    }
    fn process_conic_gradient(
        &mut self,
        geometry: PhysicalRect,
        g: ConicGradientCommand,
        rounded_clips: &[RoundedClip],
        clip: Option<ShapeClip>,
    ) {
        self.draw_clipped(geometry, rounded_clips, clip, &g);
    }
    fn process_box_shadow(
        &mut self,
        geometry: PhysicalRect,
        shadow: BoxShadowCommand,
        rounded_clips: &[RoundedClip],
    ) {
        self.draw_clipped(geometry, rounded_clips, None, &shadow);
    }

    #[cfg(feature = "path")]
    fn process_filled_path(
        &mut self,
        path_geometry: PhysicalRect,
        clip_geometry: PhysicalRect,
        commands: alloc::vec::Vec<path::Command>,
        paint: path::Paint,
        rounded_clips: &[RoundedClip],
    ) {
        self.clips.clear();
        self.clips.extend(shape_clips(rounded_clips, &clip_geometry));
        path::render_filled_path(
            &commands,
            &path_geometry,
            &clip_geometry,
            &self.clips,
            &paint,
            self.buffer,
        );
    }

    #[cfg(feature = "path")]
    fn process_stroked_path(
        &mut self,
        path_geometry: PhysicalRect,
        clip_geometry: PhysicalRect,
        commands: alloc::vec::Vec<path::Command>,
        paint: path::Paint,
        stroke_style: path::StrokeStyle<'_>,
        rounded_clips: &[RoundedClip],
    ) {
        self.clips.clear();
        self.clips.extend(shape_clips(rounded_clips, &clip_geometry));
        path::render_stroked_path(
            &commands,
            &path_geometry,
            &clip_geometry,
            &self.clips,
            &paint,
            stroke_style,
            self.buffer,
        );
    }
}

#[derive(Default)]
struct PrepareScene {
    items: Vec<SceneItem>,
    vectors: SceneVectors,
    scale_factor: ScaleFactor,
}

impl PrepareScene {
    fn push_item(
        &mut self,
        geometry: PhysicalRect,
        command: SceneCommand,
        rounded_clips: &[RoundedClip],
        own_clip: Option<ShapeClip>,
    ) {
        let clip_start = self.vectors.clips.len();
        self.vectors.clips.extend(shape_clips(rounded_clips, &geometry).chain(own_clip));
        debug_assert!(self.vectors.clips.len() <= u16::MAX as usize, "too many clips in the scene");
        self.items.push(SceneItem {
            pos: geometry.origin,
            size: geometry.size,
            z: self.items.len() as u16,
            command,
            clip_start: clip_start as u16,
            clip_len: (self.vectors.clips.len() - clip_start) as u16,
        });
    }
}

impl ProcessScene for PrepareScene {
    fn process_scene_texture(
        &mut self,
        geometry: PhysicalRect,
        texture: SceneTexture<'static>,
        rounded_clips: &[RoundedClip],
    ) {
        let texture_index = self.vectors.textures.len() as u16;
        self.vectors.textures.push(texture);
        self.push_item(geometry, SceneCommand::Texture { texture_index }, rounded_clips, None);
    }

    fn process_shared_buffer(
        &mut self,
        geometry: PhysicalRect,
        command: SharedBufferCommand,
        rounded_clips: &[RoundedClip],
    ) {
        let shared_buffer_index = self.vectors.shared_buffers.len() as u16;
        self.vectors.shared_buffers.push(command);
        self.push_item(
            geometry,
            SceneCommand::SharedBuffer { shared_buffer_index },
            rounded_clips,
            None,
        );
    }

    fn process_target_texture(
        &mut self,
        texture: &target_pixel_buffer::DrawTextureArgs,
        clip: PhysicalRect,
        rounded_clips: &[RoundedClip],
    ) {
        let Some((extra, geometry)) = SceneTextureExtra::from_target_texture(texture, &clip) else {
            return;
        };
        match &texture.data {
            target_pixel_buffer::TextureDataContainer::Static(texture_data) => {
                let pixel_stride =
                    (texture_data.byte_stride / texture_data.pixel_format.bpp()) as u16;
                self.process_scene_texture(
                    geometry,
                    SceneTexture {
                        data: texture_data.data,
                        format: texture_data.pixel_format,
                        pixel_stride,
                        extra,
                    },
                    rounded_clips,
                );
            }
            target_pixel_buffer::TextureDataContainer::Shared { buffer, source_rect } => self
                .process_shared_buffer(
                    geometry,
                    SharedBufferCommand {
                        buffer: buffer.clone(),
                        source_rect: *source_rect,
                        extra,
                    },
                    rounded_clips,
                ),
        }
    }

    fn process_rectangle(
        &mut self,
        args: &target_pixel_buffer::DrawRectangleArgs,
        clip: PhysicalRect,
        rounded_clips: &[RoundedClip],
    ) {
        let scale_factor = self.scale_factor;
        process_rectangle_impl(self, args, &clip, scale_factor, rounded_clips);
    }

    fn process_simple_rectangle(
        &mut self,
        geometry: PhysicalRect,
        color: PremultipliedRgbaColor,
        rounded_clips: &[RoundedClip],
    ) {
        if !geometry.is_empty() {
            self.push_item(geometry, SceneCommand::Rectangle { color }, rounded_clips, None);
        }
    }

    fn process_rounded_rectangle(
        &mut self,
        geometry: PhysicalRect,
        data: RoundedRectangle,
        rounded_clips: &[RoundedClip],
    ) {
        if !geometry.is_empty() {
            let rectangle_index = self.vectors.rounded_rectangles.len() as u16;
            self.vectors.rounded_rectangles.push(data);
            self.push_item(
                geometry,
                SceneCommand::RoundedRectangle { rectangle_index },
                rounded_clips,
                None,
            );
        }
    }

    fn process_linear_gradient(
        &mut self,
        geometry: PhysicalRect,
        gradient: LinearGradientCommand,
        rounded_clips: &[RoundedClip],
        clip: Option<ShapeClip>,
    ) {
        if !geometry.is_empty() {
            let linear_gradient_index = self.vectors.linear_gradients.len() as u16;
            self.vectors.linear_gradients.push(gradient);
            self.push_item(
                geometry,
                SceneCommand::LinearGradient { linear_gradient_index },
                rounded_clips,
                clip,
            );
        }
    }
    fn process_radial_gradient(
        &mut self,
        geometry: PhysicalRect,
        gradient: RadialGradientCommand,
        rounded_clips: &[RoundedClip],
        clip: Option<ShapeClip>,
    ) {
        if !geometry.is_empty() {
            let radial_gradient_index = self.vectors.radial_gradients.len() as u16;
            self.vectors.radial_gradients.push(gradient);
            self.push_item(
                geometry,
                SceneCommand::RadialGradient { radial_gradient_index },
                rounded_clips,
                clip,
            );
        }
    }
    fn process_conic_gradient(
        &mut self,
        geometry: PhysicalRect,
        gradient: ConicGradientCommand,
        rounded_clips: &[RoundedClip],
        clip: Option<ShapeClip>,
    ) {
        if !geometry.is_empty() {
            let conic_gradient_index = self.vectors.conic_gradients.len() as u16;
            self.vectors.conic_gradients.push(gradient);
            self.push_item(
                geometry,
                SceneCommand::ConicGradient { conic_gradient_index },
                rounded_clips,
                clip,
            );
        }
    }
    fn process_box_shadow(
        &mut self,
        geometry: PhysicalRect,
        shadow: BoxShadowCommand,
        rounded_clips: &[RoundedClip],
    ) {
        if !geometry.is_empty() {
            let box_shadow_index = self.vectors.box_shadows.len() as u16;
            self.vectors.box_shadows.push(shadow);
            self.push_item(
                geometry,
                SceneCommand::BoxShadow { box_shadow_index },
                rounded_clips,
                None,
            );
        }
    }

    #[cfg(feature = "path")]
    fn process_filled_path(
        &mut self,
        _path_geometry: PhysicalRect,
        _clip_geometry: PhysicalRect,
        _commands: alloc::vec::Vec<path::Command>,
        _paint: path::Paint,
        _rounded_clips: &[RoundedClip],
    ) {
        // Path rendering is not supported in line-by-line mode (PrepareScene/render_by_line)
        // Only works with buffer-based rendering (RenderToBuffer)
    }

    #[cfg(feature = "path")]
    fn process_stroked_path(
        &mut self,
        _path_geometry: PhysicalRect,
        _clip_geometry: PhysicalRect,
        _commands: alloc::vec::Vec<path::Command>,
        _paint: path::Paint,
        _stroke_style: path::StrokeStyle<'_>,
        _rounded_clips: &[RoundedClip],
    ) {
        // Path rendering is not supported in line-by-line mode (PrepareScene/render_by_line)
        // Only works with buffer-based rendering (RenderToBuffer)
    }
}

struct SceneBuilder<'a, T> {
    processor: T,
    state_stack: Vec<RenderState>,
    current_state: RenderState,
    rounded_clips: Vec<RoundedClip>,
    scale_factor: ScaleFactor,
    window: &'a WindowInner,
    rotation: RotationInfo,
    #[cfg(feature = "systemfonts")]
    text_layout_cache: &'a sharedparley::TextLayoutCache,
}

impl<'a, T: ProcessScene> SceneBuilder<'a, T> {
    fn new(
        screen_size: PhysicalSize,
        scale_factor: ScaleFactor,
        window: &'a WindowInner,
        processor: T,
        orientation: RenderingRotation,
        #[cfg(feature = "systemfonts")] text_layout_cache: &'a sharedparley::TextLayoutCache,
    ) -> Self {
        Self {
            processor,
            state_stack: Vec::new(),
            current_state: RenderState {
                alpha: 1.,
                offset: LogicalPoint::default(),
                clip: LogicalRect::new(
                    LogicalPoint::default(),
                    (screen_size.cast() / scale_factor).cast(),
                ),
                rounded_clip_count: 0,
            },
            rounded_clips: Vec::new(),
            scale_factor,
            window,
            rotation: RotationInfo { orientation, screen_size },
            #[cfg(feature = "systemfonts")]
            text_layout_cache,
        }
    }

    fn draw_window_background_gradient(&mut self, gradient: &Brush) {
        let screen = PhysicalRect::from_size(self.rotation.screen_size).transformed(self.rotation);
        let mut args =
            target_pixel_buffer::DrawRectangleArgs::from_rect(screen.cast(), gradient.clone());
        args.rotation = self.rotation.orientation;
        self.processor.process_rectangle(&args, screen, &[]);
    }

    fn should_draw(&self, rect: &LogicalRect) -> bool {
        !rect.size.is_empty()
            && self.current_state.alpha > 0.01
            && self.current_state.clip.intersects(rect)
    }

    fn clip_to_rounded_clips(&self, rect: PhysicalRect) -> Option<PhysicalRect> {
        clip_to_rounded_clips(&self.rounded_clips, rect)
    }

    /// Like [`Self::clip_to_rounded_clips`], for a rect in unrotated physical coordinates,
    /// relative to `offset`.
    fn clip_to_rounded_clips_at(
        &self,
        rect: euclid::Rect<f32, PhysicalPx>,
        offset: euclid::Vector2D<f32, PhysicalPx>,
    ) -> Option<euclid::Rect<f32, PhysicalPx>> {
        let inverse = self.rotation.inverse();
        self.rounded_clips.iter().try_fold(rect, |rect, clip| {
            rect.intersection(&clip.rect.transformed(inverse).cast().translate(-offset))
        })
    }

    /// `rect`, relative to `offset`, clipped to `physical_clip` and in physical screen coordinates.
    fn clip_text_rect(
        &self,
        rect: PhysicalRect,
        physical_clip: euclid::Rect<f32, PhysicalPx>,
        offset: euclid::Vector2D<f32, PhysicalPx>,
    ) -> Option<PhysicalRect> {
        let clipped = rect.intersection(&physical_clip.cast())?;
        // Truncating the float clip can reach past a rounded clip's rect.
        self.clip_to_rounded_clips(clipped.translate(offset.cast()).transformed(self.rotation))
    }

    fn physical_clip(&self) -> Option<PhysicalRect> {
        self.clip_to_rounded_clips(
            (self.current_state.clip.translate(self.current_state.offset.to_vector()).cast()
                * self.scale_factor)
                .round()
                .cast()
                .transformed(self.rotation),
        )
    }

    fn physical_rounded_rect(
        &self,
        size: LogicalSize,
        radius: LogicalBorderRadius,
    ) -> (euclid::Rect<f32, PhysicalPx>, BorderRadius<f32, PhysicalPx>) {
        let geom =
            (LogicalRect::from(size).translate(self.current_state.offset.to_vector()).cast()
                * self.scale_factor)
                .transformed(self.rotation);
        let radius = scale_overlapping_radii(
            (radius.cast() * self.scale_factor).transformed(self.rotation),
            geom.size,
        );
        (geom, radius)
    }

    /// `None` if the border leaves no rounded corner.
    fn rounded_clip(
        &self,
        size: LogicalSize,
        radius: LogicalBorderRadius,
        border_width: LogicalLength,
    ) -> Option<RoundedClip> {
        // Inset in physical pixels, so the clip follows the inner edge of the border that
        // `draw_border_rectangle` draws for the same geometry.
        let (geom, radius) = self.physical_rounded_rect(size, radius);
        let border = ((border_width.cast() * self.scale_factor).get() as i16).max(0);
        let inset = |r: f32| (r as i16).saturating_sub(border).max(0);
        let radius = PhysicalBorderRadius::new(
            inset(radius.top_left),
            inset(radius.top_right),
            inset(radius.bottom_right),
            inset(radius.bottom_left),
        );
        (!radius.is_zero())
            .then(|| RoundedClip { rect: geom.round().cast().inflate(-border, -border), radius })
    }

    fn draw_image_impl(
        &mut self,
        image_inner: &ImageInner,
        i_slint_core::graphics::FitResult {
            clip_rect: source_rect,
            source_to_target_x,
            source_to_target_y,
            size: fit_size,
            offset: image_fit_offset,
            tiled,
        }: i_slint_core::graphics::FitResult,
        colorize: Color,
    ) {
        let global_alpha_u16 = (self.current_state.alpha * 255.) as u16;
        let offset =
            self.current_state.offset.cast() * self.scale_factor + image_fit_offset.to_vector();

        let Some(physical_clip) = self.physical_clip() else { return };
        let physical_clip = physical_clip.cast::<i32>();

        enum Pixels {
            Static(&'static [u8]),
            #[cfg(feature = "owned-pre-rendered-resources")]
            Owned(Rc<StaticTextures<OwnedStorage>>),
        }
        let mut draw_textures = |pixels: Pixels,
                                 textures: &[StaticTexture],
                                 size: IntSize,
                                 original_size: IntSize| {
            let adjust_x = size.width as f32 / original_size.width as f32;
            let adjust_y = size.height as f32 / original_size.height as f32;
            let source_to_target_x = source_to_target_x / adjust_x;
            let source_to_target_y = source_to_target_y / adjust_y;
            let source_rect =
                source_rect.cast::<f32>().scale(adjust_x, adjust_y).round().to_box2d().cast();

            for t in textures {
                let t_rect = t.rect.to_box2d();
                // That's the source rect in the whole image coordinate
                let Some(src_rect) = t_rect.intersection(&source_rect) else { continue };

                let target_rect = if tiled.is_some() {
                    euclid::Rect::new(offset, fit_size).round().cast::<i32>()
                } else {
                    // The slice maps onto the fit rect `offset ..= offset + fit_size`;
                    // this texture only covers `src_rect` of the slice's `source_rect`, so
                    // inset each edge by the uncovered source amount. Edges reaching the
                    // slice boundary keep the exact fit rect, so abutting slices share a
                    // seamless edge (scaling each from its source extent drifted apart).
                    let inset = |a: i32, b: i32, s2t: f32| (a - b) as f32 * s2t;
                    euclid::Box2D::<f32, PhysicalPx>::new(
                        euclid::point2(
                            offset.x + inset(src_rect.min.x, source_rect.min.x, source_to_target_x),
                            offset.y + inset(src_rect.min.y, source_rect.min.y, source_to_target_y),
                        ),
                        euclid::point2(
                            offset.x + fit_size.width
                                - inset(source_rect.max.x, src_rect.max.x, source_to_target_x),
                            offset.y + fit_size.height
                                - inset(source_rect.max.y, src_rect.max.y, source_to_target_y),
                        ),
                    )
                    .round()
                    .to_rect()
                    .cast::<i32>()
                };
                let target_rect = target_rect.transformed(self.rotation).round();

                let Some(clipped_target) = physical_clip.intersection(&target_rect) else {
                    continue;
                };

                let pixel_stride = t.rect.width() as usize;
                let texture_rect = PhysicalRect::from_untyped(
                    &src_rect.to_rect().translate(-t.rect.origin.to_vector()).cast(),
                );

                let color =
                    if colorize.alpha() > 0 { colorize } else { Color::from_argb_encoded(t.color) };
                let alpha = if colorize.alpha() > 0 || t.format == TexturePixelFormat::AlphaMap {
                    color.alpha() as u16 * global_alpha_u16 / 255
                } else {
                    global_alpha_u16
                } as u8;

                let tiling = tiled.map(|tile_o| {
                    let src_o = src_rect.min - source_rect.min;
                    let gap = (src_o) + (source_rect.max - src_rect.max);
                    target_pixel_buffer::TilingInfo {
                        offset_x: ((src_o.x as f32 - tile_o.x as f32) * source_to_target_x).round()
                            as _,
                        offset_y: ((src_o.y as f32 - tile_o.y as f32) * source_to_target_y).round()
                            as _,
                        scale_x: 1. / source_to_target_x,
                        scale_y: 1. / source_to_target_y,
                        gap_x: (gap.x as f32 * source_to_target_x).round() as _,
                        gap_y: (gap.y as f32 * source_to_target_y).round() as _,
                    }
                });

                let t = target_pixel_buffer::DrawTextureArgs {
                    data: match &pixels {
                        Pixels::Static(data) => target_pixel_buffer::TextureDataContainer::Static(
                            target_pixel_buffer::TextureData::new(
                                texture_pixels(
                                    data,
                                    t,
                                    compute_range_in_buffer(&texture_rect, pixel_stride),
                                ),
                                t.format,
                                pixel_stride * t.format.bpp(),
                                src_rect.size().cast(),
                            ),
                        ),
                        #[cfg(feature = "owned-pre-rendered-resources")]
                        Pixels::Owned(textures) => {
                            target_pixel_buffer::TextureDataContainer::Shared {
                                buffer: SharedBufferData::Texture {
                                    textures: textures.clone(),
                                    texture: t.clone(),
                                },
                                source_rect: texture_rect,
                            }
                        }
                    },
                    colorize: (color.alpha() > 0).then_some(color),
                    alpha,
                    dst_x: target_rect.origin.x as _,
                    dst_y: target_rect.origin.y as _,
                    dst_width: target_rect.size.width as _,
                    dst_height: target_rect.size.height as _,
                    rotation: self.rotation.orientation,
                    tiling,
                };

                self.processor.process_target_texture(
                    &t,
                    clipped_target.cast(),
                    &self.rounded_clips,
                );
            }
        };

        match image_inner {
            ImageInner::None => (),
            ImageInner::StaticTextures(StaticTextures { data, textures, size, original_size }) => {
                draw_textures(Pixels::Static(data), textures, *size, *original_size)
            }
            #[cfg(feature = "owned-pre-rendered-resources")]
            ImageInner::OwnedTextures(owned) => {
                let StaticTextures { textures, size, original_size, .. } = &**owned;
                draw_textures(Pixels::Owned(owned.clone()), textures, *size, *original_size)
            }

            ImageInner::NineSlice(..) => unreachable!(),
            _ => {
                let target_rect =
                    euclid::Rect::new(offset, fit_size).round().cast().transformed(self.rotation);
                let Some(clipped_target) = physical_clip.intersection(&target_rect) else {
                    return;
                };

                let orig = image_inner.size().cast::<f32>();
                let svg_target_size = if tiled.is_some() {
                    euclid::size2(orig.width * source_to_target_x, orig.height * source_to_target_y)
                        .round()
                        .cast()
                } else {
                    target_rect.size.cast()
                };
                if let Some(buffer) = image_inner.render_to_buffer(Some(svg_target_size)) {
                    let buf_size = buffer.size().cast::<f32>();

                    let alpha = if colorize.alpha() > 0 {
                        colorize.alpha() as u16 * global_alpha_u16 / 255
                    } else {
                        global_alpha_u16
                    } as u8;

                    let tiling = tiled.map(|tile_o| target_pixel_buffer::TilingInfo {
                        offset_x: (tile_o.x as f32 * -source_to_target_x).round() as _,
                        offset_y: (tile_o.y as f32 * -source_to_target_y).round() as _,
                        scale_x: 1. / source_to_target_x,
                        scale_y: 1. / source_to_target_y,
                        gap_x: 0,
                        gap_y: 0,
                    });

                    let t = target_pixel_buffer::DrawTextureArgs {
                        data: target_pixel_buffer::TextureDataContainer::Shared {
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
                        },
                        colorize: (colorize.alpha() > 0).then_some(colorize),
                        alpha,
                        dst_x: target_rect.origin.x as _,
                        dst_y: target_rect.origin.y as _,
                        dst_width: target_rect.size.width as _,
                        dst_height: target_rect.size.height as _,
                        rotation: self.rotation.orientation,
                        tiling,
                    };

                    self.processor.process_target_texture(
                        &t,
                        clipped_target.cast(),
                        &self.rounded_clips,
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
        Font: AbstractFont
            + i_slint_core::textlayout::TextShaper<Length = PhysicalLength>
            + GlyphRenderer,
    {
        let slint_context = self.window.context();
        paragraph.layout_lines(
            &mut |glyphs, line_x, line_y, _, sel| {
                let baseline_y =
                    line_y + paragraph.layout.half_leading() + paragraph.layout.font.ascent();
                if let (Some(sel), Some(selection)) = (sel, &selection) {
                    let (band_offset, band_height) = paragraph.layout.cursor_band();
                    let geometry = euclid::rect(
                        line_x.get() + sel.start.get(),
                        (line_y + band_offset).get(),
                        (sel.end - sel.start).get(),
                        band_height.get(),
                    );
                    if let Some(geometry) = self.clip_text_rect(geometry, physical_clip, offset) {
                        let args = target_pixel_buffer::DrawRectangleArgs::from_rect(
                            geometry.cast(),
                            selection.selection_background.into(),
                        );
                        self.processor.process_rectangle(&args, geometry, &self.rounded_clips);
                    }
                }
                let scale_delta = paragraph.layout.font.scale_delta();
                for positioned_glyph in glyphs {
                    let Some(glyph) = paragraph
                        .layout
                        .font
                        .render_glyph(positioned_glyph.glyph_id, slint_context)
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

                    if glyph.sdf {
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

                        let delta32 = Fixed::<i32, 8>::from_fixed(scale_delta);
                        let normalize = |x: Fixed<i32, 8>| {
                            if x < Fixed::from_integer(0) { x + Fixed::from_integer(1) } else { x }
                        };
                        let fract_x = normalize((-glyph.x) - Fixed::from_integer(gl_x.get() as _));
                        let off_x = delta32 * off_x as i32 + fract_x;
                        let fract_y = normalize(glyph.y - Fixed::from_integer(gl_y.get() as _));
                        let off_y = delta32 * off_y as i32 + fract_y;
                        let extra = SceneTextureExtra {
                            colorize: color,
                            // color already is mixed with global alpha
                            alpha: color.alpha(),
                            rotation: self.rotation.orientation,
                            dx: scale_delta,
                            dy: scale_delta,
                            off_x: Fixed::try_from_fixed(off_x).unwrap(),
                            off_y: Fixed::try_from_fixed(off_y).unwrap(),
                        };
                        let geometry = geometry.transformed(self.rotation);
                        match &glyph.alpha_map {
                            fonts::GlyphAlphaMap::Static(data) => {
                                self.processor.process_scene_texture(
                                    geometry,
                                    SceneTexture {
                                        data,
                                        pixel_stride,
                                        format: TexturePixelFormat::SignedDistanceField,
                                        extra,
                                    },
                                    &self.rounded_clips,
                                )
                            }
                            _ => {
                                let buffer = SharedBufferData::AlphaMap {
                                    data: glyph.alpha_map.clone(),
                                    width: pixel_stride,
                                    sdf: true,
                                };
                                let source_rect = euclid::rect(
                                    0,
                                    0,
                                    buffer.width() as i16,
                                    buffer.height() as i16,
                                );
                                self.processor.process_shared_buffer(
                                    geometry,
                                    SharedBufferCommand { buffer, source_rect, extra },
                                    &self.rounded_clips,
                                )
                            }
                        }
                        continue;
                    }
                    let data = match &glyph.alpha_map {
                        fonts::GlyphAlphaMap::Static(data) => {
                            target_pixel_buffer::TextureDataContainer::Static(
                                target_pixel_buffer::TextureData::new(
                                    data,
                                    TexturePixelFormat::AlphaMap,
                                    glyph.pixel_stride as usize,
                                    euclid::size2(glyph.width.get(), glyph.height.get()).cast(),
                                ),
                            )
                        }
                        _ => {
                            let source_rect = euclid::rect(0, 0, glyph.width.0, glyph.height.0);
                            target_pixel_buffer::TextureDataContainer::Shared {
                                buffer: SharedBufferData::AlphaMap {
                                    data: glyph.alpha_map.clone(),
                                    width: glyph.pixel_stride,
                                    sdf: false,
                                },
                                source_rect,
                            }
                        }
                    };
                    let clipped_target =
                        clipped_target.translate(offset).round().transformed(self.rotation);
                    let target_rect =
                        target_rect.translate(offset).round().transformed(self.rotation);
                    let t = target_pixel_buffer::DrawTextureArgs {
                        data,
                        colorize: Some(color),
                        // color already is mixed with global alpha
                        alpha: color.alpha(),
                        dst_x: target_rect.origin.x as _,
                        dst_y: target_rect.origin.y as _,
                        dst_width: target_rect.size.width as _,
                        dst_height: target_rect.size.height as _,
                        rotation: self.rotation.orientation,
                        tiling: None,
                    };

                    self.processor.process_target_texture(
                        &t,
                        clipped_target.cast(),
                        &self.rounded_clips,
                    );
                }
                core::ops::ControlFlow::Continue(())
            },
            selection.as_ref().map(|s| s.selection.clone()),
        );
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

/// CSS Backgrounds 3 "Overlapping Curves"
fn scale_overlapping_radii(
    radius: BorderRadius<f32, PhysicalPx>,
    size: euclid::Size2D<f32, PhysicalPx>,
) -> BorderRadius<f32, PhysicalPx> {
    let radius = radius.max(Default::default());
    let top = radius.top_left + radius.top_right;
    let bottom = radius.bottom_left + radius.bottom_right;
    let left = radius.top_left + radius.bottom_left;
    let right = radius.top_right + radius.bottom_right;

    // Skip divisions when nothing overflows
    if top > size.width || bottom > size.width || left > size.height || right > size.height {
        let scale =
            [(size.width, top), (size.width, bottom), (size.height, left), (size.height, right)]
                .into_iter()
                .map(|(side, sum)| side / sum)
                .fold(1.0, |acc, s| if s < acc { s } else { acc });
        radius * scale
    } else {
        radius
    }
}

fn alpha_color(color: Color, alpha: u8) -> Color {
    if alpha < 255 {
        Color::from_argb_u8(
            ((color.alpha() as u32 * alpha as u32) / 255) as u8,
            color.red(),
            color.green(),
            color.blue(),
        )
    } else {
        color
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
    rounded_clip_count: u16,
}

impl<T: ProcessScene> i_slint_core::item_rendering::ItemRenderer for SceneBuilder<'_, T> {
    fn global_alpha_transparent(&self) -> bool {
        self.current_state.alpha == 0.0
    }

    fn draw_rectangle(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        _: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let geom = LogicalRect::from(size);
        if self.should_draw(&geom)
            && let Some(clipped) = self.physical_clip()
        {
            let geom = (geom.translate(self.current_state.offset.to_vector()).cast()
                * self.scale_factor)
                .transformed(self.rotation);

            let mut args =
                target_pixel_buffer::DrawRectangleArgs::from_rect(geom, rect.background());
            args.alpha = (self.current_state.alpha * 255.) as u8;
            args.rotation = self.rotation.orientation;
            self.processor.process_rectangle(&args, clipped, &self.rounded_clips);
        }
    }

    fn draw_border_rectangle(
        &mut self,
        rect: Pin<&dyn RenderBorderRectangle>,
        _: &ItemRc,
        size: LogicalSize,
        _: &CachedRenderingData,
    ) {
        let geom = LogicalRect::from(size);
        if self.should_draw(&geom)
            && let Some(clipped) = self.physical_clip()
        {
            let border = rect.border_width().cast() * self.scale_factor;
            let border_color =
                if border.get() > 0.01 { rect.border_color() } else { Default::default() };
            let background = rect.background();
            if background.is_transparent() && border_color.is_transparent() {
                return;
            }

            let (geom, radius) = self.physical_rounded_rect(size, rect.border_radius());
            let args = target_pixel_buffer::DrawRectangleArgs {
                x: geom.origin.x,
                y: geom.origin.y,
                width: geom.size.width,
                height: geom.size.height,
                top_left_radius: radius.top_left,
                top_right_radius: radius.top_right,
                bottom_right_radius: radius.bottom_right,
                bottom_left_radius: radius.bottom_left,
                border_width: border.get(),
                background,
                border: border_color,
                alpha: (self.current_state.alpha * 255.) as u8,
                rotation: self.rotation.orientation,
            };

            self.processor.process_rectangle(&args, clipped, &self.rounded_clips);
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
                for fit in i_slint_core::graphics::fit9slice(
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
            let fit = i_slint_core::graphics::fit(
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
        text: Pin<&dyn i_slint_core::item_rendering::RenderText>,
        self_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let font_request = text.font_request(self_rc);

        let font = {
            #[cfg(feature = "systemfonts")]
            let mut font_ctx = self.window.context().font_context().borrow_mut();
            fonts::match_font(
                &font_request,
                self.scale_factor,
                #[cfg(feature = "systemfonts")]
                &mut font_ctx,
            )
        };

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            sharedparley::draw_text(self, text, Some(self_rc), size, Some(self.text_layout_cache));
            return;
        }

        let content = text.text();
        let string = match &content {
            PlainOrStyledText::Plain(string) => alloc::borrow::Cow::Borrowed(string.as_str()),
            PlainOrStyledText::Styled(styled_text) => {
                i_slint_core::styled_text::get_raw_text(styled_text)
            }
        };

        if string.trim().is_empty() {
            return;
        }

        let geom = LogicalRect::from(size);
        if !self.should_draw(&geom) {
            return;
        }

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
        let Some(physical_clip) = self.clip_to_rounded_clips_at(physical_clip, offset) else {
            return;
        };

        let (horizontal_alignment, vertical_alignment) = text.alignment();
        let max_lines = text.line_limit();
        let selection = text.selection().map(|selection| SelectionInfo {
            selection_background: self.alpha_color(selection.background),
            selection_color: self.alpha_color(selection.foreground),
            selection: selection.range,
        });

        with_font!(&font, |font| {
            let layout = fonts::text_layout_for_font(
                font,
                &font_request,
                text.line_height_factor(),
                self.scale_factor,
            );
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
                max_lines,
            };

            self.draw_text_paragraph(&paragraph, physical_clip, offset, color, selection);
        });
    }

    fn draw_text_input(
        &mut self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) {
        let font_request = text_input.font_request(self_rc);
        let font = {
            #[cfg(feature = "systemfonts")]
            let mut font_ctx = self.window.context().font_context().borrow_mut();
            fonts::match_font(
                &font_request,
                self.scale_factor,
                #[cfg(feature = "systemfonts")]
                &mut font_ctx,
            )
        };

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            sharedparley::draw_text_input(self, text_input, self_rc, size, self.text_layout_cache);
            return;
        }

        let geom = LogicalRect::from(size);
        if !self.should_draw(&geom) {
            return;
        }

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
        let Some(physical_clip) = self.clip_to_rounded_clips_at(physical_clip, offset) else {
            return;
        };

        let text_visual_representation = text_input.visual_representation();
        let color = self.alpha_color(text_visual_representation.text_color.color());

        let selection =
            (!text_visual_representation.selection_range.is_empty()).then_some(SelectionInfo {
                selection_background: self.alpha_color(text_input.selection_background_color()),
                selection_color: self.alpha_color(text_input.selection_foreground_color()),
                selection: text_visual_representation.selection_range.clone(),
            });

        let cursor_pos_and_height = with_font!(&font, |font| {
            let paragraph = TextParagraphLayout {
                string: &text_visual_representation.text,
                layout: fonts::text_layout_for_font(
                    font,
                    &font_request,
                    i_slint_core::item_rendering::RenderString::line_height_factor(text_input),
                    self.scale_factor,
                ),
                max_width: max_size.width_length(),
                max_height: max_size.height_length(),
                horizontal_alignment: text_input.horizontal_alignment(),
                vertical_alignment: text_input.vertical_alignment(),
                wrap: text_input.wrap(),
                overflow: TextOverflow::Clip,
                single_line: text_input.single_line(),
                max_lines: None,
            };

            self.draw_text_paragraph(&paragraph, physical_clip, offset, color, selection);

            text_visual_representation.cursor_position.map(|cursor_offset| {
                let (band_offset, band_height) = paragraph.layout.cursor_band();
                let (cursor_x, cursor_y) = paragraph.cursor_pos_for_byte_offset(cursor_offset);
                ((cursor_x, cursor_y + band_offset), band_height)
            })
        });

        // Nothing below depends on the concrete font, so keep it out of the monomorphized body.
        if let Some(((cursor_x, cursor_y), cursor_height)) = cursor_pos_and_height {
            let cursor_rect = PhysicalRect::new(
                PhysicalPoint::from_lengths(cursor_x, cursor_y),
                PhysicalSize::from_lengths(
                    (text_input.text_cursor_width().cast() * self.scale_factor).cast(),
                    cursor_height,
                ),
            );

            if let Some(geometry) = self.clip_text_rect(cursor_rect, physical_clip, offset) {
                let args = target_pixel_buffer::DrawRectangleArgs::from_rect(
                    geometry.cast(),
                    self.alpha_color(text_visual_representation.cursor_color).into(),
                );
                self.processor.process_rectangle(&args, geometry, &self.rounded_clips);
            }
        }
    }

    #[cfg(all(feature = "std", not(feature = "path")))]
    fn draw_path(
        &mut self,
        _path: Pin<&i_slint_core::items::Path>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) {
        // Path rendering is disabled without the path feature
    }

    #[cfg(feature = "path")]
    fn draw_path(
        &mut self,
        path: Pin<&i_slint_core::items::Path>,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) {
        let geom = LogicalRect::from(size);
        if !self.should_draw(&geom) {
            return;
        }
        let alpha = (self.current_state.alpha * 255.) as u8;
        if alpha == 0 {
            return;
        }

        // Get the fitted path events from the Path item
        let Some((offset, path_iterator)) = path.fitted_path_events(self_rc) else {
            return;
        };

        let physical_geom_f32 =
            geom.translate(self.current_state.offset.to_vector()).cast() * self.scale_factor;
        let gradient_box = physical_geom_f32
            .translate(offset.cast() * self.scale_factor)
            .transformed(self.rotation);
        let rounded_geom = physical_geom_f32.round();
        let physical_geom = rounded_geom.cast().transformed(self.rotation);

        let rotation = RotationInfo {
            orientation: self.rotation.orientation,
            screen_size: rounded_geom.size.cast::<i16>() + euclid::size2(1, 1),
        };

        let offset =
            offset.cast() * self.scale_factor + (physical_geom_f32.origin - rounded_geom.origin);

        // Clip the geometry - early return if nothing to draw
        let Some(clipped_geom) =
            self.physical_clip().and_then(|clip| physical_geom.intersection(&clip))
        else {
            return;
        };

        let zeno_commands =
            path::convert_path_data_to_zeno(path_iterator, rotation, self.scale_factor, offset);

        let orientation = self.rotation.orientation;
        let scale_factor = self.scale_factor;
        let resolve_paint = |brush: Brush| {
            let mut gradients = Vec::new();
            let fill = gradient_commands(
                &brush,
                alpha,
                orientation,
                gradient_box,
                clipped_geom.cast(),
                scale_factor,
                &mut |rect, gradient| gradients.push((rect, gradient)),
            );
            match fill {
                BrushFill::Solid(color) => {
                    let color = PremultipliedRgbaColor::from(color);
                    (color.alpha > 0).then_some(path::Paint::Solid(color))
                }
                BrushFill::Gradient => Some(path::Paint::Gradient(gradients)),
            }
        };

        // Draw fill if specified
        let fill_brush = path.fill();
        if !fill_brush.is_transparent()
            && let Some(paint) = resolve_paint(fill_brush)
        {
            self.processor.process_filled_path(
                physical_geom,
                clipped_geom,
                zeno_commands.clone(),
                paint,
                &self.rounded_clips,
            );
        }

        // Draw stroke if specified
        let stroke_brush = path.stroke();
        let stroke_width = path.stroke_width();
        if !stroke_brush.is_transparent()
            && stroke_width.get() > 0 as Coord
            && let Some(paint) = resolve_paint(stroke_brush)
        {
            let physical_stroke_width = (stroke_width.cast() * self.scale_factor).get();
            let stroke_line_cap = path.stroke_line_cap();
            let stroke_line_join = path.stroke_line_join();
            let stroke_miter_limit = path.stroke_miter_limit();
            let stroke_dash_array: Vec<f32> = path
                .stroke_dash_array()
                .iter()
                .map(|&x| (LogicalLength::new(x).cast() * self.scale_factor).get())
                .collect();
            let stroke_dash_offset = (path.stroke_dash_offset().cast() * self.scale_factor).get();
            let stroke_style = path::StrokeStyle {
                width: physical_stroke_width,
                line_cap: stroke_line_cap,
                line_join: stroke_line_join,
                miter_limit: stroke_miter_limit,
                dash_array: &stroke_dash_array,
                dash_offset: stroke_dash_offset,
            };
            self.processor.process_stroked_path(
                physical_geom,
                clipped_geom,
                zeno_commands,
                paint,
                stroke_style,
                &self.rounded_clips,
            );
        }
    }

    fn draw_box_shadow(
        &mut self,
        box_shadow: Pin<&i_slint_core::items::BoxShadow>,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) {
        if box_shadow.inset() {
            return;
        }
        let offset = LogicalVector::from_lengths(box_shadow.offset_x(), box_shadow.offset_y());
        if !self.should_draw(&drop_shadow_bounding_rect(
            size.into(),
            offset,
            box_shadow.blur(),
            box_shadow.spread(),
        )) {
            return;
        }
        let Some(options) = BoxShadowOptions::new(self_rc, box_shadow, self.scale_factor) else {
            return;
        };
        let Some(clip) = self.physical_clip() else { return };
        let color = self.alpha_color(options.color);
        process_drop_shadow(
            &mut self.processor,
            &options,
            (self.current_state.offset + offset).cast() * self.scale_factor,
            color,
            &clip,
            &self.rounded_clips,
            self.rotation,
        );
    }

    fn visit_clip(
        &mut self,
        clip_item: Pin<&i_slint_core::items::Clip>,
        _: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        if !clip_item.clip() {
            return RenderingResult::ContinueRenderingChildren;
        }
        let radius = clip_item.logical_border_radius();
        let border_width = clip_item.border_width();
        let (clip_rect, inner_radius) = clip_content_box(size, radius, border_width);
        let rounded_clip = if inner_radius.is_zero() {
            None
        } else {
            self.rounded_clip(size, radius, border_width)
        };
        // The rounded clip's rect is the border's inner edge in physical pixels;
        // the logical inset rounds differently at a fractional scale factor.
        let clip_rect = if rounded_clip.is_some() { LogicalRect::from(size) } else { clip_rect };
        if !self.combine_clip(clip_rect, LogicalBorderRadius::default()) {
            return RenderingResult::ContinueRenderingWithoutChildren;
        }
        if let Some(rounded_clip) = rounded_clip {
            if rounded_clip.rect.is_empty() {
                return RenderingResult::ContinueRenderingWithoutChildren;
            }
            debug_assert_eq!(
                self.rounded_clips.len(),
                self.current_state.rounded_clip_count as usize
            );
            self.rounded_clips.push(rounded_clip);
            self.current_state.rounded_clip_count += 1;
        }
        RenderingResult::ContinueRenderingChildren
    }

    fn combine_clip(&mut self, other: LogicalRect, radius: LogicalBorderRadius) -> bool {
        debug_assert!(radius.is_zero(), "rounded clips go through visit_clip");
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
    }

    fn get_current_clip(&self) -> LogicalRect {
        self.current_state.clip
    }

    fn translate(&mut self, distance: LogicalVector) {
        self.current_state.offset += distance;
        self.current_state.clip = self.current_state.clip.translate(-distance)
    }

    fn current_transform(&self) -> i_slint_core::lengths::ItemTransform {
        let v = self.current_state.offset.to_vector().cast::<f32>();
        i_slint_core::lengths::ItemTransform::translation(v.x, v.y)
    }

    fn rotate(&mut self, _angle_in_degrees: f32) {
        // TODO (#6068)
    }

    fn scale(&mut self, _x_factor: f32, _y_factor: f32) {
        // TODO
    }

    fn apply_opacity(&mut self, opacity: f32) {
        self.current_state.alpha *= opacity;
    }

    fn save_state(&mut self) {
        self.state_stack.push(self.current_state);
    }

    fn restore_state(&mut self) {
        self.current_state = self.state_stack.pop().unwrap();
        let count = self.current_state.rounded_clip_count as usize;
        debug_assert!(self.rounded_clips.len() >= count);
        self.rounded_clips.truncate(count);
    }

    fn scale_factor(&self) -> ScaleFactor {
        self.scale_factor
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
                let offset = self.current_state.offset.cast() * self.scale_factor;
                let geometry = clipped_src.translate(offset.to_vector().cast()).round_in();

                let t = target_pixel_buffer::DrawTextureArgs {
                    data: target_pixel_buffer::TextureDataContainer::Shared {
                        buffer: SharedBufferData::SharedImage(img),
                        source_rect,
                    },
                    colorize: None,
                    alpha: (self.current_state.alpha * 255.) as u8,
                    dst_x: offset.x as _,
                    dst_y: offset.y as _,
                    dst_width: width as _,
                    dst_height: height as _,
                    rotation: self.rotation.orientation,
                    tiling: None,
                };
                if let Some(clip) =
                    self.clip_to_rounded_clips(geometry.cast().transformed(self.rotation))
                {
                    self.processor.process_target_texture(&t, clip, &self.rounded_clips);
                }
            }
        });
    }

    fn draw_string(&mut self, string: &str, color: Color) {
        let font_request = Default::default();
        let font = {
            #[cfg(feature = "systemfonts")]
            let mut font_ctx = self.window.context().font_context().borrow_mut();
            fonts::match_font(
                &font_request,
                self.scale_factor,
                #[cfg(feature = "systemfonts")]
                &mut font_ctx,
            )
        };

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            sharedparley::draw_text(
                self,
                std::pin::pin!((i_slint_core::SharedString::from(string), Brush::from(color))),
                None,
                self.current_state.clip.size.cast(),
                None,
            );
            return;
        }

        let clip = self.current_state.clip.cast() * self.scale_factor;
        let Some(clip) = self.clip_to_rounded_clips_at(clip, Default::default()) else {
            return;
        };

        with_font!(&font, |font| {
            let layout = fonts::text_layout_for_font(font, &font_request, None, self.scale_factor);

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
                max_lines: None,
            };

            self.draw_text_paragraph(&paragraph, clip, Default::default(), color, None);
        });
    }

    fn draw_image_direct(&mut self, image: i_slint_core::graphics::Image) {
        let image_inner: &ImageInner = (&image).into();
        let source_size = image.size();
        if source_size.is_empty() {
            return;
        }
        let target_size = euclid::Size2D::<f32, i_slint_core::lengths::LogicalPx>::from_untyped(
            source_size.cast(),
        ) * self.scale_factor;
        let fit = i_slint_core::graphics::fit(
            i_slint_core::items::ImageFit::Fill,
            target_size,
            i_slint_core::graphics::IntRect::from_size(source_size.cast()),
            self.scale_factor,
            Default::default(),
            Default::default(),
        );
        self.draw_image_impl(image_inner, fit, i_slint_core::Color::default());
    }

    fn window(&self) -> &i_slint_core::window::WindowInner {
        self.window
    }

    fn as_any(&mut self) -> Option<&mut dyn core::any::Any> {
        None
    }
}

impl<T: ProcessScene> i_slint_core::item_rendering::ItemRendererFeatures for SceneBuilder<'_, T> {
    const SUPPORTS_TRANSFORMATIONS: bool = false;
}

#[cfg(feature = "systemfonts")]
use i_slint_core::textlayout::sharedparley::{self, fontique};

#[cfg(feature = "systemfonts")]
#[derive(Clone, Copy)]
struct GlyphBrush {
    color: Color,
    /// `None` fills the glyphs.
    stroke: Option<fonts::vectorfont::StrokeWidth>,
}

/// Snaps a fill from the text layout (a text decoration, a selection highlight, a cursor) to
/// whole pixels. The edges are rounded instead of truncated, so that they quantize the way
/// `draw_glyph_run` quantizes the glyph origin and the clip it cuts glyphs with. Truncating would
/// put a selection highlight edge a whole pixel away from the glyph clip meant to line up with
/// it, as soon as the item's own offset lands on a fractional device pixel.
/// A fill thinner than a pixel, such as an underline at small font sizes, keeps one pixel
/// centered on it: rounding both of its edges would make it vanish depending on where it lands.
#[cfg(feature = "systemfonts")]
fn snap_fill_rect(rect: sharedparley::PhysicalRect) -> PhysicalRect {
    let mut snapped = rect.round();
    if rect.size.height > 0. && snapped.size.height < 1. {
        snapped.origin.y = (rect.center().y - 0.5).round();
        snapped.size.height = 1.;
    }
    if rect.size.width > 0. && snapped.size.width < 1. {
        snapped.origin.x = (rect.center().x - 0.5).round();
        snapped.size.width = 1.;
    }
    snapped.cast()
}

#[cfg(feature = "systemfonts")]
#[test]
fn snap_fill_rect_keeps_thin_fills() {
    let r = |x: f32, y: f32, w: f32, h: f32| euclid::rect(x, y, w, h);
    // An underline about 0.7px thick: rounding both edges used to drop it whenever its
    // top landed in the upper half of a pixel.
    for y in [10.0, 10.2, 10.4, 10.6, 10.8] {
        let snapped = snap_fill_rect(r(5.3, y, 40.4, 0.7));
        assert_eq!(snapped.size.height, 1, "y = {y}");
        assert_eq!(snapped.origin.y, (y + 0.35 - 0.5f32).round() as i16, "y = {y}");
        assert_eq!((snapped.origin.x, snapped.size.width), (5, 41));
    }
    // A thin vertical fill, like a text cursor at a fractional scale factor.
    let snapped = snap_fill_rect(r(7.6, 2.0, 0.8, 14.0));
    assert_eq!((snapped.origin.x, snapped.size.width), (8, 1));
    // Fills of a pixel or more are rounded edge by edge, empty ones stay empty.
    assert_eq!(snap_fill_rect(r(1.4, 2.6, 3.4, 4.2)), euclid::rect(1, 3, 4, 4));
    assert!(snap_fill_rect(r(1.0, 2.0, 0.0, 5.0)).is_empty());
}

#[cfg(feature = "systemfonts")]
impl<T: ProcessScene> sharedparley::GlyphRenderer for SceneBuilder<'_, T> {
    type PlatformBrush = GlyphBrush;

    fn snap_y(&self, y: f32) -> f32 {
        let origin = (self.current_state.offset.to_vector().cast::<f32>() * self.scale_factor).y;
        (origin + y).round() - origin
    }

    fn platform_brush_for_color(&mut self, color: &Color) -> Option<Self::PlatformBrush> {
        Some(GlyphBrush { color: *color, stroke: None })
    }

    fn platform_text_fill_brush(
        &mut self,
        brush: Brush,
        _size: LogicalSize,
    ) -> Option<Self::PlatformBrush> {
        Some(GlyphBrush { color: brush.color(), stroke: None })
    }

    fn platform_text_stroke_brush(
        &mut self,
        brush: Brush,
        physical_stroke_width: f32,
        _size: LogicalSize,
    ) -> Option<Self::PlatformBrush> {
        Some(GlyphBrush {
            color: brush.color(),
            stroke: Some(fonts::vectorfont::StrokeWidth::from_physical(physical_stroke_width)),
        })
    }

    fn fill_rectangle(
        &mut self,
        mut physical_rect: sharedparley::PhysicalRect,
        brush: GlyphBrush,
        radius: sharedparley::PhysicalLength,
        border: Option<sharedparley::RectangleBorder<GlyphBrush>>,
    ) {
        let color = brush.color;
        let has_visible_border =
            border.as_ref().is_some_and(|b| b.width.get() > 0.0 && b.brush.color.alpha() > 0);
        if color.alpha() == 0 && !has_visible_border {
            return;
        }

        let global_offset =
            (self.current_state.offset.to_vector().cast() * self.scale_factor).cast();

        physical_rect.origin += global_offset;
        let geometry: PhysicalRect = snap_fill_rect(physical_rect).transformed(self.rotation);
        // These fills reach the processor directly rather than through `draw_rectangle`, so they
        // have to bring the clip along themselves. Without it a text decoration, a selection
        // highlight or a cursor taller than the item it belongs to paints right over its
        // surroundings, while the glyphs beside it are clipped.
        let Some(clip) = self.physical_clip() else { return };
        let mut args = target_pixel_buffer::DrawRectangleArgs::from_rect(
            geometry.cast(),
            Brush::SolidColor(color),
        );

        if radius.get() > 0.0 {
            let r = radius.get().min(args.width / 2.0).min(args.height / 2.0);
            args.top_left_radius = r;
            args.top_right_radius = r;
            args.bottom_right_radius = r;
            args.bottom_left_radius = r;
        }

        if let Some(sharedparley::RectangleBorder { brush: border_brush, width: border_width }) =
            border
            && border_width.get() > 0.0
            && border_brush.color.alpha() > 0
        {
            args.border_width = border_width.get();
            args.border = Brush::SolidColor(border_brush.color);
        }

        self.processor.process_rectangle(&args, clip, &self.rounded_clips);
    }

    fn draw_glyph_run(
        &mut self,
        font: &sharedparley::parley::FontData,
        font_size: sharedparley::PhysicalLength,
        normalized_coords: &[i16],
        synthesis: &fontique::Synthesis,
        brush: Self::PlatformBrush,
        y_offset: sharedparley::PhysicalLength,
        glyphs_it: &mut dyn Iterator<Item = sharedparley::parley::layout::Glyph>,
    ) {
        let slint_context = self.window.context();
        let (swash_key, swash_offset) =
            fonts::systemfonts::get_swash_font_info(&font.data, font.index);
        let font = fonts::vectorfont::VectorFont::new_from_blob_and_index_with_coords(
            font.data.clone(),
            font.index,
            swash_key,
            swash_offset,
            font_size.cast(),
            normalized_coords,
        )
        .with_synthesis(*synthesis);

        let global_offset: euclid::Vector2D<f32, PhysicalPx> =
            self.current_state.offset.to_vector().cast() * self.scale_factor;

        const SUBPIXEL_BINS: i32 = fonts::vectorfont::SUBPIXEL_BIN_COUNT;

        let color = self.alpha_color(brush.color);
        let text_rgb = [brush.color.red(), brush.color.green(), brush.color.blue()];
        let Some(physical_clip) = self.physical_clip() else { return };
        let physical_clip = physical_clip.cast::<i32>();

        for positioned_glyph in glyphs_it {
            let Some(id) = std::num::NonZero::new(positioned_glyph.id as u16) else {
                continue;
            };

            // Absolute, sub-pixel-accurate device position of the pen for this glyph.
            let abs_x = global_offset.x + positioned_glyph.x;
            let abs_y = global_offset.y + positioned_glyph.y + y_offset.get();

            // Quantize the horizontal position to SUBPIXEL_BINS positions per pixel.
            // The integer part is the blit origin; the fractional part is rendered
            // into the glyph coverage (see `render_vector_glyph`) instead of being
            // discarded. This keeps inter-glyph spacing even: snapping the pen to a
            // whole pixel (truncate or round) redistributes the sub-pixel advances
            // unevenly between neighboring glyph pairs.
            let quantized_x = (abs_x * SUBPIXEL_BINS as f32).round() as i32;
            let dst_int_x = quantized_x.div_euclid(SUBPIXEL_BINS);
            let subpixel_bin = quantized_x.rem_euclid(SUBPIXEL_BINS) as u8;

            let Some(glyph) = font.render_vector_glyph(
                id,
                subpixel_bin,
                brush.stroke,
                Some(text_rgb),
                slint_context,
            ) else {
                continue;
            };

            let gl_y = PhysicalLength::new(glyph.y.truncate() as i16);
            // i32 so a glyph past the i16 range survives until it is clipped below.
            let target_rect: euclid::Rect<i32, PhysicalPx> = euclid::Rect::<f32, PhysicalPx>::new(
                euclid::Point2D::new(
                    dst_int_x as f32 + glyph.glyph_origin_x,
                    abs_y.round() + (-gl_y - glyph.height).get() as f32,
                ),
                glyph.size().cast(),
            )
            .cast()
            .transformed(self.rotation);

            let Some(clipped_target) = physical_clip.intersection(&target_rect) else {
                continue;
            };

            let source_rect = euclid::rect(0, 0, glyph.width.0, glyph.height.0);
            // A color glyph (an emoji) keeps its own colors; the text's color only lends it its
            // alpha, as with Skia.
            let (buffer, colorize) = match glyph.pixels {
                fonts::VectorGlyphPixels::AlphaMap(alpha_map) => (
                    SharedBufferData::AlphaMap {
                        data: alpha_map.into(),
                        width: glyph.pixel_stride,
                        sdf: false,
                    },
                    Some(color),
                ),
                fonts::VectorGlyphPixels::Color(pixels) => (
                    SharedBufferData::SharedImage(SharedImageBuffer::RGBA8Premultiplied(pixels)),
                    None,
                ),
            };
            let data = target_pixel_buffer::TextureDataContainer::Shared { buffer, source_rect };

            let t = target_pixel_buffer::DrawTextureArgs {
                data,
                colorize,
                // color already is mixed with global alpha
                alpha: color.alpha(),
                dst_x: target_rect.origin.x as _,
                dst_y: target_rect.origin.y as _,
                dst_width: target_rect.size.width as _,
                dst_height: target_rect.size.height as _,
                rotation: self.rotation.orientation,
                tiling: None,
            };

            self.processor.process_target_texture(&t, clipped_target.cast(), &self.rounded_clips);
        }
    }
}

/// Renders `draw` into a buffer of `screen_size`, limited to the dirty `region`.
#[cfg(test)]
fn render_region(
    screen_size: PhysicalSize,
    region: &[PhysicalRect],
    scale_factor: ScaleFactor,
    draw: impl FnOnce(&mut dyn ProcessScene),
) -> Vec<PremultipliedRgbaColor> {
    let pixel_count = screen_size.width as usize * screen_size.height as usize;
    let mut data = alloc::vec![PremultipliedRgbaColor::default(); pixel_count];
    let mut buffer = TargetPixelSlice { data: &mut data, pixel_stride: screen_size.width as usize };
    let mut rectangles = [euclid::Box2D::default(); PHYSICAL_REGION_MAX_SIZE];
    for (r, dst) in region.iter().zip(rectangles.iter_mut()) {
        *dst = r.to_box2d();
    }
    let mut processor = RenderToBuffer {
        buffer: &mut buffer,
        dirty_range_cache: Vec::new(),
        dirty_region: PhysicalRegion { rectangles, count: region.len() },
        scale_factor,
        clips: Vec::new(),
    };
    draw(&mut processor);
    data
}

/// Asserts that `draw` renders the same pixels within `clip` and the dirty `region`
/// as it does unclipped on the whole screen.
#[cfg(test)]
fn assert_partial_render_is_identical(
    screen_size: PhysicalSize,
    clip: PhysicalRect,
    region: &[PhysicalRect],
    draw: impl Fn(&mut dyn ProcessScene, &PhysicalRect),
) {
    let screen = PhysicalRect::from_size(screen_size);
    let scale_factor = ScaleFactor::new(1.);
    let full =
        render_region(screen_size, &[screen], scale_factor, |processor| draw(processor, &screen));
    let partial =
        render_region(screen_size, region, scale_factor, |processor| draw(processor, &clip));
    let mut drawn = false;
    for (i, (a, b)) in full.iter().zip(partial.iter()).enumerate() {
        let p = PhysicalPoint::new(
            (i % screen_size.width as usize) as i16,
            (i / screen_size.width as usize) as i16,
        );
        if clip.contains(p) && region.iter().any(|r| r.contains(p)) {
            assert_eq!(bytemuck::bytes_of(a), bytemuck::bytes_of(b), "{p:?}");
            drawn |= a.alpha > 0;
        }
    }
    assert!(drawn);
}

#[test]
fn rounded_rectangle_partial_render_is_identical() {
    let screen_size = PhysicalSize::new(100, 80);
    // The clip and the regions cut through the anti-aliasing of the corner curves.
    let clip = euclid::rect(17, 5, 60, 70);
    let region = [euclid::rect(12, 8, 21, 19), euclid::rect(48, 33, 31, 30)];
    for border_width in [0., 4.3] {
        let args = target_pixel_buffer::DrawRectangleArgs {
            x: 10.,
            y: 11.,
            width: 62.,
            height: 45.,
            top_left_radius: 17.,
            top_right_radius: 9.5,
            bottom_right_radius: 21.2,
            bottom_left_radius: 6.,
            border_width,
            background: Brush::SolidColor(Color::from_argb_u8(200, 10, 20, 30)),
            border: Brush::SolidColor(Color::from_argb_u8(230, 200, 100, 0)),
            alpha: 255,
            rotation: RenderingRotation::NoRotation,
        };
        assert_partial_render_is_identical(screen_size, clip, &region, |processor, clip| {
            processor.process_rectangle(&args, *clip, &[])
        });
    }
}
