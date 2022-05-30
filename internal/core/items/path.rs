// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
This module contains the builtin Path related items.

When adding an item or a property, it needs to be kept in sync with different place.
Lookup the [`crate::items`] module documentation.
*/

use super::{FillRule, Item, ItemConsts, ItemRc, ItemRendererRef, RenderingResult};
use crate::graphics::{Brush, PathData, PathDataIterator, Rect};
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, KeyEvent,
    KeyEventResult, MouseEvent,
};
use crate::item_rendering::CachedRenderingData;

use crate::layout::{LayoutInfo, Orientation};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowRc;
use crate::{Coord, Property};
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use i_slint_core_macros::*;

/// The implementation of the `Path` element
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct Path {
    pub x: Property<Coord>,
    pub y: Property<Coord>,
    pub width: Property<Coord>,
    pub height: Property<Coord>,
    pub elements: Property<PathData>,
    pub fill: Property<Brush>,
    pub fill_rule: Property<FillRule>,
    pub stroke: Property<Brush>,
    pub stroke_width: Property<Coord>,
    pub viewbox_x: Property<f32>,
    pub viewbox_y: Property<f32>,
    pub viewbox_width: Property<f32>,
    pub viewbox_height: Property<f32>,
    pub clip: Property<bool>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Path {
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, _orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        if let Some(pos) = event.pos() {
            if self.clip()
                && (pos.x < 0 as _
                    || pos.y < 0 as _
                    || pos.x > self.width()
                    || pos.y > self.height())
            {
                return InputEventFilterResult::Intercept;
            }
        }
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &WindowRc) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &WindowRc) -> FocusEventResult {
        FocusEventResult::FocusIgnored
    }

    fn render(
        self: Pin<&Self>,
        backend: &mut ItemRendererRef,
        self_rc: &ItemRc,
    ) -> RenderingResult {
        let clip = self.clip();
        if clip {
            (*backend).save_state();
            (*backend).combine_clip(self.geometry(), 0 as _, 0 as _)
        }
        (*backend).draw_path(self, self_rc);
        if clip {
            (*backend).restore_state();
        }
        RenderingResult::ContinueRenderingChildren
    }
}

impl Path {
    /// Returns an iterator of the events of the path and an offset, so that the
    /// shape fits into the width/height of the path while respecting the stroke
    /// width.
    pub fn fitted_path_events(
        self: Pin<&Self>,
    ) -> (euclid::default::Vector2D<Coord>, PathDataIterator) {
        let stroke_width = self.stroke_width();
        let bounds_width = (self.width() - stroke_width).max(0 as _);
        let bounds_height = (self.height() - stroke_width).max(0 as _);
        let offset =
            euclid::default::Vector2D::new(stroke_width / 2 as Coord, stroke_width / 2 as Coord);

        let viewbox_width = self.viewbox_width();
        let viewbox_height = self.viewbox_height();

        let mut elements_iter = self.elements().iter();

        let maybe_viewbox = if viewbox_width > 0. && viewbox_height > 0. {
            Some(euclid::rect(self.viewbox_x(), self.viewbox_y(), viewbox_width, viewbox_height))
        } else {
            None
        };

        elements_iter.fit(bounds_width as _, bounds_height as _, maybe_viewbox);
        (offset, elements_iter)
    }
}

impl ItemConsts for Path {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Path, CachedRenderingData> =
        Path::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}
