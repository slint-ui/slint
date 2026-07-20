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
    LogicalBorderRadius, LogicalLength, LogicalPx, LogicalRect, LogicalSize, LogicalVector,
    RectLengths,
};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowAdapter;
use crate::{Coord, Property};
use alloc::boxed::Box;
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
    pub viewbox_x: Property<f32>,
    pub viewbox_y: Property<f32>,
    pub viewbox_width: Property<f32>,
    pub viewbox_height: Property<f32>,
    pub fit: Property<ImageFit>,
    pub clip: Property<bool>,
    pub anti_alias: Property<bool>,
    pub cached_rendering_data: CachedRenderingData,
    fitted_cache: PathFittedCacheBox,
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
            (*backend).combine_clip(
                size.into(),
                LogicalBorderRadius::zero(),
                LogicalLength::zero(),
            );
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
        _self_rc: &ItemRc,
        geometry: LogicalRect,
    ) -> LogicalRect {
        geometry
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

        elements_iter.fit(
            bounds_width.get() as _,
            bounds_height.get() as _,
            maybe_viewbox,
            self.fit(),
        );
        (offset, elements_iter).into()
    }

    /// Returns the path and measurements as long as none of the properties are dirty
    fn cached_fitted_path(self: Pin<&Self>, self_rc: &ItemRc) -> Option<Rc<FittedPath>> {
        if let Some(new_data) =
            self.fitted_cache.tracker.as_ref().evaluate_if_dirty(|| self.build_fitted_path(self_rc))
        {
            *self.fitted_cache.data.borrow_mut() = new_data.clone();
            new_data
        } else {
            self.fitted_cache.data.borrow().clone()
        }
    }

    fn build_fitted_path(self: Pin<&Self>, self_rc: &ItemRc) -> Option<Rc<FittedPath>> {
        let (_, elements_iter) = self.fitted_path_events(self_rc)?;
        Some(Rc::new(elements_iter.to_fitted_path()))
    }

    pub fn point_at_percent(
        self: Pin<&Self>,
        self_rc: &ItemRc,
        percent: f32,
    ) -> Point2D<f32, LogicalPx> {
        self.cached_fitted_path(self_rc)
            .and_then(|fitted| fitted.position_at(percent))
            .map(|pos| Point2D::new(pos.x, pos.y))
            .unwrap_or_default()
    }
    pub fn angle_at_percent(self: Pin<&Self>, self_rc: &ItemRc, percent: f32) -> f32 {
        self.cached_fitted_path(self_rc).and_then(|fitted| fitted.angle_at(percent)).unwrap_or(0.0)
    }
}

impl ItemConsts for Path {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Path, CachedRenderingData> =
        Path::FIELD_OFFSETS.cached_rendering_data().as_unpinned_projection();
}

pub struct PathFittedCacheInner {
    data: RefCell<Option<Rc<FittedPath>>>,
    /// Tracks the properties read while building data to rebuild data when the cache is
    /// invalidated
    tracker: Pin<Box<crate::properties::PropertyTracker>>,
}

impl Default for PathFittedCacheInner {
    fn default() -> Self {
        Self { data: Default::default(), tracker: Box::pin(Default::default()) }
    }
}

/// Opaque box holding the cached path
#[repr(C)]
pub struct PathFittedCacheBox(core::ptr::NonNull<PathFittedCacheInner>);

impl Default for PathFittedCacheBox {
    fn default() -> Self {
        PathFittedCacheBox(Box::leak(Box::<PathFittedCacheInner>::default()).into())
    }
}
impl Drop for PathFittedCacheBox {
    fn drop(&mut self) {
        // Safety: self.0 was constructed from a Box::leak in PathFittedCacheBox::default
        drop(unsafe { Box::from_raw(self.0.as_ptr()) });
    }
}
impl core::ops::Deref for PathFittedCacheBox {
    type Target = PathFittedCacheInner;
    fn deref(&self) -> &Self::Target {
        // Safety: initialized in PathFittedCacheBox::default
        unsafe { self.0.as_ref() }
    }
}

/// # Safety
/// This must be called using a non-null pointer pointing to a chunk of memory big enough to
/// hold a PathFittedCacheBox
#[cfg(feature = "ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_path_fitted_cache_init(cache: *mut PathFittedCacheBox) {
    unsafe { core::ptr::write(cache, PathFittedCacheBox::default()) };
}

/// # Safety
/// This must be called using a non-null pointer pointing to an initialized PathFittedCacheBox
#[cfg(feature = "ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_path_fitted_cache_free(cache: *mut PathFittedCacheBox) {
    unsafe {
        core::ptr::drop_in_place(cache);
    }
}

#[cfg(feature = "ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_path_point_at_percent(
    self_component: &vtable::VRc<crate::item_tree::ItemTreeVTable>,
    self_index: u32,
    percent: f32,
) -> crate::lengths::LogicalPoint {
    let self_rc = ItemRc::new(self_component.clone(), self_index);
    self_rc.downcast::<Path>().unwrap().as_pin_ref().point_at_percent(&self_rc, percent)
}

#[cfg(feature = "ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_path_angle_at_percent(
    self_component: &vtable::VRc<crate::item_tree::ItemTreeVTable>,
    self_index: u32,
    percent: f32,
) -> f32 {
    let self_rc = ItemRc::new(self_component.clone(), self_index);
    self_rc.downcast::<Path>().unwrap().as_pin_ref().angle_at_percent(&self_rc, percent)
}
