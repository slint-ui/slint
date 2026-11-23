// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains the builtin Path related items.

When adding an item or a property, it needs to be kept in sync with different place.
Lookup the [`crate::items`] module documentation.
*/

use super::{
    FillRule, Item, ItemConsts, ItemRc, ItemRendererRef, LineCap, LineJoin, RenderingResult,
};
use crate::graphics::{Brush, FittedPath, PathData, PathDataIterator};
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, InternalKeyEvent,
    KeyEventResult, MouseEvent,
};
use crate::item_rendering::CachedRenderingData;

use crate::items::ImageFit;
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalPx, LogicalRect, LogicalSize,
    LogicalVector, RectLengths,
};
use crate::once_box::OnceBox;
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowAdapter;
use crate::{Coord, Property};
use alloc::rc::Rc;
use const_field_offset::FieldOffsets;
use core::cell::RefCell;
use core::pin::Pin;
use euclid::Point2D;
use euclid::num::Zero;
use i_slint_core_macros::*;

/// The implementation of the `Path` element
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct Path {
    pub elements: Property<PathData>,
    pub fill: Property<Brush>,
    pub fill_rule: Property<FillRule>,
    pub stroke: Property<Brush>,
    pub stroke_width: Property<LogicalLength>,
    pub stroke_line_cap: Property<LineCap>,
    pub stroke_line_join: Property<LineJoin>,
    pub stroke_miter_limit: Property<f32>,
    pub stroke_dash_array: Property<crate::SharedVector<crate::Coord>>,
    pub stroke_dash_offset: Property<LogicalLength>,
    pub viewbox_x: Property<f32>,
    pub viewbox_y: Property<f32>,
    pub viewbox_width: Property<f32>,
    pub viewbox_height: Property<f32>,
    pub fit: Property<ImageFit>,
    pub clip: Property<bool>,
    pub anti_alias: Property<bool>,
    pub cached_rendering_data: CachedRenderingData,
    fitted_path: OnceBox<FittedPathInner>,
    tracker: crate::properties::PropertyTracker,
}

impl Item for Path {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn deinit(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _cross_axis_constraint: Coord,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut super::MouseCursorInner,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut super::MouseCursorInner,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn capture_key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(
        self: Pin<&Self>,
        _: &FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> FocusEventResult {
        FocusEventResult::FocusIgnored
    }

    fn render(
        self: Pin<&Self>,
        backend: &mut ItemRendererRef,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        let clip = self.clip();
        if clip {
            (*backend).save_state();
            (*backend).combine_clip(size.into(), LogicalBorderRadius::zero());
        }
        (*backend).draw_path(self, self_rc, size);
        if clip {
            (*backend).restore_state();
        }
        RenderingResult::ContinueRenderingChildren
    }

    fn bounding_rect(
        self: core::pin::Pin<&Self>,
        _window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
        geometry: LogicalRect,
    ) -> LogicalRect {
        if self.clip() {
            return geometry;
        }
        let outset = self.stroke_outset();
        let path_rect = self.fit_rect(geometry.size).or_else(|| {
            let (offset, elements_iter) = self.fitted_path_events(self_rc)?;
            let bounds = lyon_algorithms::aabb::bounding_box(elements_iter.iter());
            Some(LogicalRect::from_untyped(&bounds.to_rect().cast()).translate(offset))
        });
        path_rect
            .map_or(LogicalRect::zero(), |rect| rect.inflate(outset, outset))
            .union(&LogicalRect::from_size(geometry.size))
            .translate(geometry.origin.to_vector())
    }

    fn clips_children(self: core::pin::Pin<&Self>) -> bool {
        false
    }
}

impl Path {
    /// Returns an iterator of the events of the path and an offset, so that the
    /// shape fits into the width/height of the path while respecting the stroke
    /// width.
    pub fn fitted_path_events(
        self: Pin<&Self>,
        self_rc: &ItemRc,
    ) -> Option<(LogicalVector, PathDataIterator)> {
        let mut elements_iter = self.elements().iter()?;

        let fit = self.fit();
        if fit == ImageFit::Preserve {
            return (LogicalVector::zero(), elements_iter).into();
        }

        let stroke_width = self.stroke_width();
        let geometry = self_rc.geometry();
        let bounds_width = (geometry.width_length() - stroke_width).max(LogicalLength::zero());
        let bounds_height = (geometry.height_length() - stroke_width).max(LogicalLength::zero());
        let offset =
            LogicalVector::from_lengths(stroke_width / 2 as Coord, stroke_width / 2 as Coord);

        let viewbox_width = self.viewbox_width();
        let viewbox_height = self.viewbox_height();

        let maybe_viewbox = if viewbox_width > 0. && viewbox_height > 0. {
            Some(
                euclid::rect(self.viewbox_x(), self.viewbox_y(), viewbox_width, viewbox_height)
                    .to_box2d(),
            )
        } else {
            None
        };

        elements_iter.fit(bounds_width.get() as _, bounds_height.get() as _, maybe_viewbox, fit);
        (offset, elements_iter).into()
    }

    /// The rectangle that `fitted_path_events` fits the path's outline into, or `None` when
    /// the outline may lie outside of it, so that only its own bounds tell where it is.
    fn fit_rect(self: Pin<&Self>, size: LogicalSize) -> Option<LogicalRect> {
        if !matches!(self.fit(), ImageFit::Contain | ImageFit::Fill)
            || (self.viewbox_width() > 0. && self.viewbox_height() > 0.)
        {
            return None;
        }
        let stroke_width = self.stroke_width().get();
        let fit_size =
            (size - LogicalSize::new(stroke_width, stroke_width)).max(LogicalSize::zero());
        (fit_size.width > 0 as Coord || fit_size.height > 0 as Coord).then(|| {
            LogicalRect::new(
                LogicalPoint::new(stroke_width / 2 as Coord, stroke_width / 2 as Coord),
                fit_size,
            )
        })
    }

    /// How far the stroke reaches past the path's outline.
    fn stroke_outset(self: Pin<&Self>) -> Coord {
        if self.stroke().is_transparent() {
            return 0 as Coord;
        }
        // Skia draws a zero width stroke as a one pixel wide hairline.
        let half_width = self.stroke_width().get().max(1 as Coord) / 2 as Coord;
        let join_factor = if self.stroke_line_join() == LineJoin::Miter {
            self.stroke_miter_limit().max(1.)
        } else {
            1.
        };
        let cap_factor =
            if self.stroke_line_cap() == LineCap::Square { core::f32::consts::SQRT_2 } else { 1. };
        half_width * join_factor.max(cap_factor) as Coord
    }

    fn sample_at(
        self: Pin<&Self>,
        self_rc: &ItemRc,
        t: f32,
    ) -> Option<(Point2D<f32, LogicalPx>, f32)> {
        let fitted_path = &self.fitted_path.0;
        if let Some(new_path) =
            Path::FIELD_OFFSETS.tracker().apply_pin(self).evaluate_if_dirty(|| {
                let (offset, elements_iter) = self.fitted_path_events(self_rc)?;
                Some(elements_iter.to_fitted_path(offset))
            })
        {
            *fitted_path.borrow_mut() = new_path;
        }
        fitted_path.borrow().as_ref()?.sample_at(t)
    }

    pub fn point_at(self: Pin<&Self>, self_rc: &ItemRc, t: f32) -> Point2D<f32, LogicalPx> {
        self.sample_at(self_rc, t).map(|(pos, _)| pos).unwrap_or_default()
    }
    pub fn angle_at(self: Pin<&Self>, self_rc: &ItemRc, t: f32) -> f32 {
        self.sample_at(self_rc, t).map(|(_, tangent)| tangent).unwrap_or_default()
    }
}

impl ItemConsts for Path {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Path, CachedRenderingData> =
        Path::FIELD_OFFSETS.cached_rendering_data().as_unpinned_projection();
}

#[derive(Default)]
struct FittedPathInner(RefCell<Option<FittedPath>>);
