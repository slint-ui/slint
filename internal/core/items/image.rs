// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains the builtin image related items.

When adding an item or a property, it needs to be kept in sync with different place.
Lookup the [`crate::items`] module documentation.
*/
use super::{
    ImageFit, ImageHorizontalAlignment, ImageRendering, ImageTiling, ImageVerticalAlignment, Item,
    ItemConsts, ItemRc, RenderingResult,
};
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, KeyEvent,
    KeyEventResult, MouseEvent,
};
use crate::item_rendering::ItemRenderer;
use crate::item_rendering::{CachedRenderingData, RenderImage};
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{LogicalLength, LogicalRect, LogicalSize};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowAdapter;
use crate::{Brush, Coord, Property};
use alloc::rc::Rc;
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use i_slint_core_macros::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of the `Image` element
pub struct ImageItem {
    pub source: Property<crate::graphics::Image>,
    pub width: Property<LogicalLength>,
    pub height: Property<LogicalLength>,
    pub image_fit: Property<ImageFit>,
    pub image_rendering: Property<ImageRendering>,
    pub colorize: Property<Brush>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for ImageItem {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        let natural_size = self.source().size();
        LayoutInfo {
            preferred: match orientation {
                _ if natural_size.width == 0 || natural_size.height == 0 => 0 as Coord,
                Orientation::Horizontal => natural_size.width as Coord,
                Orientation::Vertical => {
                    natural_size.height as Coord * self.width().get() / natural_size.width as Coord
                }
            },
            ..Default::default()
        }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(
        self: Pin<&Self>,
        _: &KeyEvent,
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
        backend: &mut &mut dyn ItemRenderer,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        (*backend).draw_image(self, self_rc, size, &self.cached_rendering_data);
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

impl RenderImage for ImageItem {
    fn target_size(self: Pin<&Self>) -> LogicalSize {
        LogicalSize::from_lengths(self.width(), self.height())
    }

    fn source(self: Pin<&Self>) -> crate::graphics::Image {
        self.source()
    }

    fn source_clip(self: Pin<&Self>) -> Option<crate::graphics::IntRect> {
        None
    }

    fn image_fit(self: Pin<&Self>) -> ImageFit {
        self.image_fit()
    }

    fn rendering(self: Pin<&Self>) -> ImageRendering {
        self.image_rendering()
    }

    fn colorize(self: Pin<&Self>) -> Brush {
        self.colorize()
    }

    fn alignment(self: Pin<&Self>) -> (ImageHorizontalAlignment, ImageVerticalAlignment) {
        Default::default()
    }

    fn tiling(self: Pin<&Self>) -> (ImageTiling, ImageTiling) {
        Default::default()
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
    pub width: Property<LogicalLength>,
    pub height: Property<LogicalLength>,
    pub image_fit: Property<ImageFit>,
    pub image_rendering: Property<ImageRendering>,
    pub colorize: Property<Brush>,
    pub source_clip_x: Property<i32>,
    pub source_clip_y: Property<i32>,
    pub source_clip_width: Property<i32>,
    pub source_clip_height: Property<i32>,

    pub horizontal_alignment: Property<ImageHorizontalAlignment>,
    pub vertical_alignment: Property<ImageVerticalAlignment>,
    pub horizontal_tiling: Property<ImageTiling>,
    pub vertical_tiling: Property<ImageTiling>,

    pub cached_rendering_data: CachedRenderingData,
}

impl Item for ClippedImage {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        LayoutInfo {
            preferred: match orientation {
                Orientation::Horizontal => self.source_clip_width() as Coord,
                Orientation::Vertical => {
                    let source_clip_width = self.source_clip_width();
                    if source_clip_width == 0 {
                        0 as Coord
                    } else {
                        self.source_clip_height() as Coord * self.width().get()
                            / source_clip_width as Coord
                    }
                }
            },
            ..Default::default()
        }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(
        self: Pin<&Self>,
        _: &KeyEvent,
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
        backend: &mut &mut dyn ItemRenderer,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        (*backend).draw_image(self, self_rc, size, &self.cached_rendering_data);
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

impl RenderImage for ClippedImage {
    fn target_size(self: Pin<&Self>) -> LogicalSize {
        LogicalSize::from_lengths(self.width(), self.height())
    }

    fn source(self: Pin<&Self>) -> crate::graphics::Image {
        self.source()
    }

    fn source_clip(self: Pin<&Self>) -> Option<crate::graphics::IntRect> {
        Some(euclid::rect(
            self.source_clip_x(),
            self.source_clip_y(),
            self.source_clip_width(),
            self.source_clip_height(),
        ))
    }

    fn image_fit(self: Pin<&Self>) -> ImageFit {
        self.image_fit()
    }

    fn rendering(self: Pin<&Self>) -> ImageRendering {
        self.image_rendering()
    }

    fn colorize(self: Pin<&Self>) -> Brush {
        self.colorize()
    }

    fn alignment(self: Pin<&Self>) -> (ImageHorizontalAlignment, ImageVerticalAlignment) {
        (self.horizontal_alignment(), self.vertical_alignment())
    }

    fn tiling(self: Pin<&Self>) -> (ImageTiling, ImageTiling) {
        (self.horizontal_tiling(), self.vertical_tiling())
    }
}

impl ItemConsts for ClippedImage {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        ClippedImage,
        CachedRenderingData,
    > = ClippedImage::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}
