// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
This module contains the builtin image related items.

When adding an item or a property, it needs to be kept in sync with different place.
Lookup the [`crate::items`] module documentation.
*/
use super::{ImageFit, ImageRendering, Item, ItemConsts, ItemRc, RenderingResult};
use crate::graphics::Rect;
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, KeyEvent,
    KeyEventResult, MouseEvent,
};
use crate::item_rendering::CachedRenderingData;
use crate::item_rendering::ItemRenderer;
use crate::layout::{LayoutInfo, Orientation};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowRc;
use crate::{Brush, Coord, Property};
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use i_slint_core_macros::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of the `Image` element
pub struct ImageItem {
    pub source: Property<crate::graphics::Image>,
    pub x: Property<Coord>,
    pub y: Property<Coord>,
    pub width: Property<Coord>,
    pub height: Property<Coord>,
    pub image_fit: Property<ImageFit>,
    pub image_rendering: Property<ImageRendering>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for ImageItem {
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        let natural_size = self.source().size();
        LayoutInfo {
            preferred: match orientation {
                _ if natural_size.width == 0 || natural_size.height == 0 => 0 as Coord,
                Orientation::Horizontal => natural_size.width as Coord,
                Orientation::Vertical => {
                    natural_size.height as Coord * self.width() / natural_size.width as Coord
                }
            },
            ..Default::default()
        }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
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
        backend: &mut &mut dyn ItemRenderer,
        self_rc: &ItemRc,
    ) -> RenderingResult {
        (*backend).draw_image(self, self_rc);
        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for ImageItem {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        ImageItem,
        CachedRenderingData,
    > = ImageItem::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of the `ClippedImage` element
pub struct ClippedImage {
    pub source: Property<crate::graphics::Image>,
    pub x: Property<Coord>,
    pub y: Property<Coord>,
    pub width: Property<Coord>,
    pub height: Property<Coord>,
    pub image_fit: Property<ImageFit>,
    pub image_rendering: Property<ImageRendering>,
    pub colorize: Property<Brush>,
    pub source_clip_x: Property<i32>,
    pub source_clip_y: Property<i32>,
    pub source_clip_width: Property<i32>,
    pub source_clip_height: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for ClippedImage {
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        let natural_size = self.source().size();
        LayoutInfo {
            preferred: match orientation {
                _ if natural_size.width == 0 || natural_size.height == 0 => 0 as Coord,
                Orientation::Horizontal => natural_size.width as Coord,
                Orientation::Vertical => {
                    natural_size.height as Coord * self.width() / natural_size.width as Coord
                }
            },
            ..Default::default()
        }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
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
        backend: &mut &mut dyn ItemRenderer,
        self_rc: &ItemRc,
    ) -> RenderingResult {
        (*backend).draw_clipped_image(self, self_rc);
        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for ClippedImage {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        ClippedImage,
        CachedRenderingData,
    > = ClippedImage::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}
