// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore nesw

/*!
This module contains the builtin items, either in this file or in sub-modules.

When adding an item or a property, it needs to be kept in sync with different place.
(This is less than ideal and maybe we can have some automation later)

 - It needs to be changed in this module
 - In the compiler: builtins.slint
 - In the interpreter (new item only): dynamic_item_tree.rs
 - For the C++ code (new item only): the cbindgen.rs to export the new item
 - Don't forget to update the documentation
*/

#![allow(unsafe_code)]
#![allow(non_upper_case_globals)]
#![allow(missing_docs)] // because documenting each property of items is redundant

use crate::graphics::{Brush, Color, Point};
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, KeyEventResult,
    KeyEventType, MouseEvent,
};
use crate::item_rendering::{CachedRenderingData, RenderBorderRectangle};
pub use crate::item_tree::ItemRc;
use crate::layout::LayoutInfo;
use crate::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalSize, LogicalVector, PointLengths, RectLengths,
};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::{WindowAdapter, WindowAdapterRc};
use crate::{Callback, Coord, Property, SharedString};
use alloc::rc::Rc;
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use i_slint_core_macros::*;
use vtable::*;

mod component_container;
pub use self::component_container::*;
mod flickable;
pub use flickable::Flickable;
mod text;
pub use text::*;
mod input_items;
pub use input_items::*;
mod image;
pub use self::image::*;
#[cfg(feature = "std")]
mod path;
#[cfg(feature = "std")]
pub use path::*;

/// Alias for `&mut dyn ItemRenderer`. Required so cbindgen generates the ItemVTable
/// despite the presence of trait object
type ItemRendererRef<'a> = &'a mut dyn crate::item_rendering::ItemRenderer;

/// Workarounds for cbindgen
pub type VoidArg = ();
pub type KeyEventArg = (KeyEvent,);
type PointerEventArg = (PointerEvent,);
type PointerScrollEventArg = (PointerScrollEvent,);
type PointArg = (Point,);
type MenuEntryArg = (MenuEntry,);
type MenuEntryModel = crate::model::ModelRc<MenuEntry>;

#[cfg(all(feature = "ffi", windows))]
#[macro_export]
macro_rules! declare_item_vtable {
    (fn $getter:ident() -> $item_vtable_ty:ident for $item_ty:ty) => {
        ItemVTable_static! {
            #[no_mangle]
            pub static $item_vtable_ty for $item_ty
        }
        #[no_mangle]
        pub extern "C" fn $getter() -> *const ItemVTable {
            use vtable::HasStaticVTable;
            <$item_ty>::static_vtable()
        }
    };
}
#[cfg(not(all(feature = "ffi", windows)))]
#[macro_export]
macro_rules! declare_item_vtable {
    (fn $getter:ident() -> $item_vtable_ty:ident for $item_ty:ty) => {
        ItemVTable_static! {
            #[no_mangle]
            pub static $item_vtable_ty for $item_ty
        }
    };
}

/// Returned by the `render()` function on items to indicate whether the rendering of
/// children should be handled by the caller, of if the item took care of that (for example
/// through layer indirection)
#[repr(C)]
#[derive(Default)]
pub enum RenderingResult {
    #[default]
    ContinueRenderingChildren,
    ContinueRenderingWithoutChildren,
}

/// Items are the nodes in the render tree.
#[vtable]
#[repr(C)]
pub struct ItemVTable {
    /// This function is called by the run-time after the memory for the item
    /// has been allocated and initialized. It will be called before any user specified
    /// bindings are set.
    pub init: extern "C" fn(core::pin::Pin<VRef<ItemVTable>>, my_item: &ItemRc),

    /// offset in bytes from the *const ItemImpl.
    /// isize::MAX  means None
    #[allow(non_upper_case_globals)]
    #[field_offset(CachedRenderingData)]
    pub cached_rendering_data_offset: usize,

    /// We would need max/min/preferred size, and all layout info
    pub layout_info: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        orientation: Orientation,
        window_adapter: &WindowAdapterRc,
    ) -> LayoutInfo,

    /// Event handler for mouse and touch event. This function is called before being called on children.
    /// Then, depending on the return value, it is called for the children, and their children, then
    /// [`Self::input_event`] is called on the children, and finally [`Self::input_event`] is called
    /// on this item again.
    pub input_event_filter_before_children: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        MouseEvent,
        window_adapter: &WindowAdapterRc,
        self_rc: &ItemRc,
    ) -> InputEventFilterResult,

    /// Handle input event for mouse and touch event
    pub input_event: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        MouseEvent,
        window_adapter: &WindowAdapterRc,
        self_rc: &ItemRc,
    ) -> InputEventResult,

    pub focus_event: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        &FocusEvent,
        window_adapter: &WindowAdapterRc,
        self_rc: &ItemRc,
    ) -> FocusEventResult,

    pub key_event: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        &KeyEvent,
        window_adapter: &WindowAdapterRc,
        self_rc: &ItemRc,
    ) -> KeyEventResult,

    pub render: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        backend: &mut ItemRendererRef,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult,
}

/// Alias for `vtable::VRef<ItemVTable>` which represent a pointer to a `dyn Item` with
/// the associated vtable
pub type ItemRef<'a> = vtable::VRef<'a, ItemVTable>;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of an empty items that does nothing
pub struct Empty {
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Empty {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
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
        _backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for Empty {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Empty,
        CachedRenderingData,
    > = Empty::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_EmptyVTable() -> EmptyVTable for Empty
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of the `Rectangle` element
pub struct Rectangle {
    pub background: Property<Brush>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Rectangle {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
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
        backend: &mut ItemRendererRef,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        (*backend).draw_rectangle(self, self_rc, size);
        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for Rectangle {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Rectangle,
        CachedRenderingData,
    > = Rectangle::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_RectangleVTable() -> RectangleVTable for Rectangle
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of the `BasicBorderRectangle` element
pub struct BasicBorderRectangle {
    pub background: Property<Brush>,
    pub border_width: Property<LogicalLength>,
    pub border_radius: Property<LogicalLength>,
    pub border_color: Property<Brush>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for BasicBorderRectangle {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
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
        backend: &mut ItemRendererRef,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        (*backend).draw_border_rectangle(self, self_rc, size, &self.cached_rendering_data);
        RenderingResult::ContinueRenderingChildren
    }
}

impl RenderBorderRectangle for BasicBorderRectangle {
    fn background(self: Pin<&Self>) -> Brush {
        self.background()
    }
    fn border_width(self: Pin<&Self>) -> LogicalLength {
        self.border_width()
    }
    fn border_radius(self: Pin<&Self>) -> LogicalBorderRadius {
        LogicalBorderRadius::from_length(self.border_radius())
    }
    fn border_color(self: Pin<&Self>) -> Brush {
        self.border_color()
    }
}

impl ItemConsts for BasicBorderRectangle {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        BasicBorderRectangle,
        CachedRenderingData,
    > = BasicBorderRectangle::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_BasicBorderRectangleVTable() -> BasicBorderRectangleVTable for BasicBorderRectangle
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of the `BorderRectangle` element
pub struct BorderRectangle {
    pub background: Property<Brush>,
    pub border_width: Property<LogicalLength>,
    pub border_radius: Property<LogicalLength>,
    pub border_top_left_radius: Property<LogicalLength>,
    pub border_top_right_radius: Property<LogicalLength>,
    pub border_bottom_left_radius: Property<LogicalLength>,
    pub border_bottom_right_radius: Property<LogicalLength>,
    pub border_color: Property<Brush>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for BorderRectangle {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
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
        backend: &mut ItemRendererRef,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        (*backend).draw_border_rectangle(self, self_rc, size, &self.cached_rendering_data);
        RenderingResult::ContinueRenderingChildren
    }
}

impl RenderBorderRectangle for BorderRectangle {
    fn background(self: Pin<&Self>) -> Brush {
        self.background()
    }
    fn border_width(self: Pin<&Self>) -> LogicalLength {
        self.border_width()
    }
    fn border_radius(self: Pin<&Self>) -> LogicalBorderRadius {
        LogicalBorderRadius::from_lengths(
            self.border_top_left_radius(),
            self.border_top_right_radius(),
            self.border_bottom_right_radius(),
            self.border_bottom_left_radius(),
        )
    }
    fn border_color(self: Pin<&Self>) -> Brush {
        self.border_color()
    }
}

impl ItemConsts for BorderRectangle {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        BorderRectangle,
        CachedRenderingData,
    > = BorderRectangle::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_BorderRectangleVTable() -> BorderRectangleVTable for BorderRectangle
}

declare_item_vtable! {
    fn slint_get_TouchAreaVTable() -> TouchAreaVTable for TouchArea
}

declare_item_vtable! {
    fn slint_get_FocusScopeVTable() -> FocusScopeVTable for FocusScope
}

declare_item_vtable! {
    fn slint_get_SwipeGestureHandlerVTable() -> SwipeGestureHandlerVTable for SwipeGestureHandler
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of the `Clip` element
pub struct Clip {
    pub border_top_left_radius: Property<LogicalLength>,
    pub border_top_right_radius: Property<LogicalLength>,
    pub border_bottom_left_radius: Property<LogicalLength>,
    pub border_bottom_right_radius: Property<LogicalLength>,
    pub border_width: Property<LogicalLength>,
    pub cached_rendering_data: CachedRenderingData,
    pub clip: Property<bool>,
}

impl Item for Clip {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        if let Some(pos) = event.position() {
            let geometry = self_rc.geometry();
            if self.clip()
                && (pos.x < 0 as Coord
                    || pos.y < 0 as Coord
                    || pos.x_length() > geometry.width_length()
                    || pos.y_length() > geometry.height_length())
            {
                return InputEventFilterResult::Intercept;
            }
        }
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
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
        backend: &mut ItemRendererRef,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        (*backend).visit_clip(self, self_rc, size)
    }
}

impl Clip {
    pub fn logical_border_radius(self: Pin<&Self>) -> LogicalBorderRadius {
        LogicalBorderRadius::from_lengths(
            self.border_top_left_radius(),
            self.border_top_right_radius(),
            self.border_bottom_right_radius(),
            self.border_bottom_left_radius(),
        )
    }
}

impl ItemConsts for Clip {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Clip, CachedRenderingData> =
        Clip::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_ClipVTable() -> ClipVTable for Clip
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The Opacity Item is not meant to be used directly by the .slint code, instead, the `opacity: xxx` or `visible: false` should be used
pub struct Opacity {
    // FIXME: this element shouldn't need these geometry property
    pub opacity: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Opacity {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
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
        backend: &mut ItemRendererRef,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        backend.visit_opacity(self, self_rc, size)
    }
}

impl Opacity {
    // This function determines the optimization opportunities for not having to render the
    // children of the Opacity element into a layer:
    //  *  The opacity item typically only one child (this is not guaranteed). If that item has
    //     no children, then we can skip the layer and apply the opacity directly. This is not perfect though,
    //     for example if the compiler inserts another synthetic element between the `Opacity` and the actual child,
    //     then this check will apply a layer even though it might not actually be necessary.
    //  * If the vale of the opacity is 1.0 then we don't need to do anything.
    pub fn need_layer(self_rc: &ItemRc, opacity: f32) -> bool {
        if opacity == 1.0 {
            return false;
        }

        let opacity_child = match self_rc.first_child() {
            Some(first_child) => first_child,
            None => return false, // No children? Don't need a layer then.
        };

        if opacity_child.next_sibling().is_some() {
            return true; // If the opacity item has more than one child, then we need a layer
        }

        // If the target of the opacity has any children then we need a layer
        opacity_child.first_child().is_some()
    }
}

impl ItemConsts for Opacity {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Opacity,
        CachedRenderingData,
    > = Opacity::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_OpacityVTable() -> OpacityVTable for Opacity
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The Layer Item is not meant to be used directly by the .slint code, instead, the `layer: xxx` property should be used
pub struct Layer {
    pub cache_rendering_hint: Property<bool>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Layer {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
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
        backend: &mut ItemRendererRef,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        backend.visit_layer(self, self_rc, size)
    }
}

impl ItemConsts for Layer {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Layer,
        CachedRenderingData,
    > = Layer::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_LayerVTable() -> LayerVTable for Layer
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of the `Rotate` element
pub struct Rotate {
    pub rotation_angle: Property<f32>,
    pub rotation_origin_x: Property<LogicalLength>,
    pub rotation_origin_y: Property<LogicalLength>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Rotate {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
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
        backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        let origin =
            LogicalVector::from_lengths(self.rotation_origin_x(), self.rotation_origin_y());
        (*backend).translate(origin);
        (*backend).rotate(self.rotation_angle());
        (*backend).translate(-origin);
        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for Rotate {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Rotate,
        CachedRenderingData,
    > = Rotate::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_RotateVTable() -> RotateVTable for Rotate
}

declare_item_vtable! {
    fn slint_get_FlickableVTable() -> FlickableVTable for Flickable
}

/// The implementation of the `PropertyAnimation` element
#[repr(C)]
#[derive(FieldOffsets, SlintElement, Clone, Debug)]
#[pin]
pub struct PropertyAnimation {
    #[rtti_field]
    pub delay: i32,
    #[rtti_field]
    pub duration: i32,
    #[rtti_field]
    pub iteration_count: f32,
    #[rtti_field]
    pub direction: AnimationDirection,
    #[rtti_field]
    pub easing: crate::animations::EasingCurve,
}

impl Default for PropertyAnimation {
    fn default() -> Self {
        // Defaults for PropertyAnimation are defined here (for internal Rust code doing programmatic animations)
        // as well as in `builtins.slint` (for generated C++ and Rust code)
        Self {
            delay: 0,
            duration: 0,
            iteration_count: 1.,
            direction: Default::default(),
            easing: Default::default(),
        }
    }
}

/// The implementation of the `Window` element
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct WindowItem {
    pub width: Property<LogicalLength>,
    pub height: Property<LogicalLength>,
    pub background: Property<Brush>,
    pub title: Property<SharedString>,
    pub no_frame: Property<bool>,
    pub resize_border_width: Property<LogicalLength>,
    pub always_on_top: Property<bool>,
    pub full_screen: Property<bool>,
    pub icon: Property<crate::graphics::Image>,
    pub default_font_family: Property<SharedString>,
    pub default_font_size: Property<LogicalLength>,
    pub default_font_weight: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for WindowItem {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {
        #[cfg(feature = "std")]
        self.full_screen.set(std::env::var("SLINT_FULLSCREEN").is_ok());
    }

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _event: MouseEvent,
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
        _backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        RenderingResult::ContinueRenderingChildren
    }
}

impl WindowItem {
    pub fn font_family(self: Pin<&Self>) -> Option<SharedString> {
        let maybe_family = self.default_font_family();
        if !maybe_family.is_empty() {
            Some(maybe_family)
        } else {
            None
        }
    }

    pub fn font_size(self: Pin<&Self>) -> Option<LogicalLength> {
        let font_size = self.default_font_size();
        if font_size.get() <= 0 as Coord {
            None
        } else {
            Some(font_size)
        }
    }

    pub fn font_weight(self: Pin<&Self>) -> Option<i32> {
        let font_weight = self.default_font_weight();
        if font_weight == 0 {
            None
        } else {
            Some(font_weight)
        }
    }
}

impl ItemConsts for WindowItem {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_WindowItemVTable() -> WindowItemVTable for WindowItem
}

/// The implementation of the `Window` element
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct ContextMenu {
    //pub entries: Property<crate::model::ModelRc<MenuEntry>>,
    pub sub_menu: Callback<MenuEntryArg, MenuEntryModel>,
    pub activated: Callback<MenuEntryArg>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for ContextMenu {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _event: MouseEvent,
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
        _backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        RenderingResult::ContinueRenderingChildren
    }
}

impl ContextMenu {}

impl ItemConsts for ContextMenu {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_ContextMenuVTable() -> ContextMenuVTable for ContextMenu
}

/// The implementation of the `BoxShadow` element
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct BoxShadow {
    pub border_radius: Property<LogicalLength>,
    // Shadow specific properties
    pub offset_x: Property<LogicalLength>,
    pub offset_y: Property<LogicalLength>,
    pub color: Property<Color>,
    pub blur: Property<LogicalLength>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for BoxShadow {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _event: MouseEvent,
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
        backend: &mut ItemRendererRef,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        (*backend).draw_box_shadow(self, self_rc, size);
        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for BoxShadow {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_BoxShadowVTable() -> BoxShadowVTable for BoxShadow
}

declare_item_vtable! {
    fn slint_get_ComponentContainerVTable() -> ComponentContainerVTable for ComponentContainer
}

declare_item_vtable! {
    fn slint_get_ComplexTextVTable() -> ComplexTextVTable for ComplexText
}

declare_item_vtable! {
    fn slint_get_SimpleTextVTable() -> SimpleTextVTable for SimpleText
}

declare_item_vtable! {
    fn slint_get_TextInputVTable() -> TextInputVTable for TextInput
}

declare_item_vtable! {
    fn slint_get_ImageItemVTable() -> ImageItemVTable for ImageItem
}

declare_item_vtable! {
    fn slint_get_ClippedImageVTable() -> ClippedImageVTable for ClippedImage
}

#[cfg(feature = "std")]
declare_item_vtable! {
    fn slint_get_PathVTable() -> PathVTable for Path
}

macro_rules! declare_enums {
    ($( $(#[$enum_doc:meta])* enum $Name:ident { $( $(#[$value_doc:meta])* $Value:ident,)* })*) => {
        $(
            #[derive(Copy, Clone, Debug, PartialEq, Eq, strum::EnumString, strum::Display, Hash)]
            #[repr(u32)]
            #[strum(serialize_all = "kebab-case")]
            $(#[$enum_doc])*
            pub enum $Name {
                $( $(#[$value_doc])* $Value),*
            }

            impl Default for $Name {
                fn default() -> Self {
                    // Always return the first value
                    ($(Self::$Value,)*).0
                }
            }
        )*
    };
}

i_slint_common::for_each_enums!(declare_enums);

macro_rules! declare_builtin_structs {
    ($(
        $(#[$struct_attr:meta])*
        struct $Name:ident {
            @name = $inner_name:literal
            export {
                $( $(#[$pub_attr:meta])* $pub_field:ident : $pub_type:ty, )*
            }
            private {
                $( $(#[$pri_attr:meta])* $pri_field:ident : $pri_type:ty, )*
            }
        }
    )*) => {
        $(
            #[derive(Clone, Debug, Default, PartialEq)]
            #[repr(C)]
            $(#[$struct_attr])*
            pub struct $Name {
                $(
                    $(#[$pub_attr])*
                    pub $pub_field : $pub_type,
                )*
                $(
                    $(#[$pri_attr])*
                    pub $pri_field : $pri_type,
                )*
            }
        )*
    };
}

i_slint_common::for_each_builtin_structs!(declare_builtin_structs);

#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn slint_item_absolute_position(
    self_component: &vtable::VRc<crate::item_tree::ItemTreeVTable>,
    self_index: u32,
) -> crate::lengths::LogicalPoint {
    let self_rc = ItemRc::new(self_component.clone(), self_index);
    self_rc.map_to_window(Default::default())
}
