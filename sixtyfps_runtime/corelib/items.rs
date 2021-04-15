/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
This module contains the builtin items, either in this file or in sub-modules.

When adding an item or a property, it needs to be kept in sync with different place.
(This is less than ideal and maybe we can have some automation later)

 - It needs to be changed in this module
 - In the compiler: builtins.60
 - In the interpreter (new item only): dynamic_component.rs
 - For the C++ code (new item only):
   - the cbindgen.rs to export the new item
   - the `using` declaration in sixtyfps.h for the item and its vtable
 - Don't forget to update the documentation
*/

#![allow(unsafe_code)]
#![allow(non_upper_case_globals)]
#![allow(missing_docs)] // because documenting each property of items is redundent

use crate::component::ComponentVTable;
use crate::graphics::PathDataIterator;
use crate::graphics::{Brush, Color, PathData, Rect, Size};
use crate::input::{
    FocusEvent, InputEventFilterResult, InputEventResult, KeyEvent, KeyEventResult, KeyEventType,
    MouseEvent, MouseEventType,
};
use crate::item_rendering::CachedRenderingData;
use crate::layout::LayoutInfo;
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::ComponentWindow;
use crate::{Callback, Property, SharedString};
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use sixtyfps_corelib_macros::*;
use vtable::*;

mod text;
pub use text::*;
mod image;
pub use self::image::*;

/// Alias for `&mut dyn ItemRenderer`. Required so cbingen generates the ItemVTable
/// despite the presence of trait object
type ItemRendererRef<'a> = &'a mut dyn crate::item_rendering::ItemRenderer;

/// Workarounds for cbindgen
pub type VoidArg = ();
type KeyEventArg = (KeyEvent,);

/// Items are the nodes in the render tree.
#[vtable]
#[repr(C)]
pub struct ItemVTable {
    /// This function is called by the run-time after the memory for the item
    /// has been allocated and initialized. It will be called before any user specified
    /// bindings are set.
    pub init: extern "C" fn(core::pin::Pin<VRef<ItemVTable>>, window: &ComponentWindow),

    /// Returns the geometry of this item (relative to its parent item)
    pub geometry: extern "C" fn(core::pin::Pin<VRef<ItemVTable>>) -> Rect,

    /// offset in bytes fromthe *const ItemImpl.
    /// isize::MAX  means None
    #[allow(non_upper_case_globals)]
    #[field_offset(CachedRenderingData)]
    pub cached_rendering_data_offset: usize,

    /// We would need max/min/preferred size, and all layout info
    pub layouting_info:
        extern "C" fn(core::pin::Pin<VRef<ItemVTable>>, window: &ComponentWindow) -> LayoutInfo,

    pub implicit_size:
        extern "C" fn(core::pin::Pin<VRef<ItemVTable>>, window: &ComponentWindow) -> Size,

    /// Event handler for mouse and touch event. This function is called before being called on children.
    /// Then, depending on the return value, it is called for the children, and their children, then
    /// [`Self::input_event`] is called on the children, and finaly [`Self::input_event`] is called
    /// on this item again.
    pub input_event_filter_before_children: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        MouseEvent,
        window: &ComponentWindow,
        self_rc: &ItemRc,
    ) -> InputEventFilterResult,

    /// Handle input event for mouse and touch event
    pub input_event: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        MouseEvent,
        window: &ComponentWindow,
        self_rc: &ItemRc,
    ) -> InputEventResult,

    pub focus_event:
        extern "C" fn(core::pin::Pin<VRef<ItemVTable>>, &FocusEvent, window: &ComponentWindow),

    pub key_event: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        &KeyEvent,
        window: &ComponentWindow,
    ) -> KeyEventResult,

    pub render: extern "C" fn(core::pin::Pin<VRef<ItemVTable>>, backend: &mut ItemRendererRef),
}

/// Alias for `vtable::VRef<ItemVTable>` which represent a pointer to a `dyn Item` with
/// the associated vtable
pub type ItemRef<'a> = vtable::VRef<'a, ItemVTable>;

/// A ItemRc is holding a reference to a component containing the item, and the index of this item
#[repr(C)]
#[derive(Clone)]
pub struct ItemRc {
    component: vtable::VRc<ComponentVTable>,
    index: usize,
}

impl ItemRc {
    /// Create an ItemRc from a component and an index
    pub fn new(component: vtable::VRc<ComponentVTable>, index: usize) -> Self {
        Self { component, index }
    }
    /// Return a `Pin<ItemRef<'a>>`
    pub fn borrow<'a>(&'a self) -> Pin<ItemRef<'a>> {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.component);
        let result = comp_ref_pin.as_ref().get_item_ref(self.index);
        // Safety: we can expand the lifetime of the ItemRef because we know it lives for at least the
        // lifetime of the component, which is 'a.  Pin::as_ref removes the lifetime, but we can just put it back.
        unsafe { core::mem::transmute::<Pin<ItemRef<'_>>, Pin<ItemRef<'a>>>(result) }
    }
    pub fn downgrade(&self) -> ItemWeak {
        ItemWeak { component: VRc::downgrade(&self.component), index: self.index }
    }
    /// Return the parent Item in the item tree.
    /// This is weak because it can be null if there is no parent
    pub fn parent_item(&self) -> ItemWeak {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.component);
        let mut r = ItemWeak::default();
        comp_ref_pin.as_ref().parent_item(self.index, &mut r);
        r
    }

    /// Return the index of the item within the component
    pub fn index(&self) -> usize {
        self.index
    }
    /// Returns a reference to the component holding this item
    pub fn component(&self) -> vtable::VRc<ComponentVTable> {
        self.component.clone()
    }
}

/// A Weak reference to an item that can be constructed from an ItemRc.
#[derive(Default, Clone)]
#[repr(C)]
pub struct ItemWeak {
    component: crate::component::ComponentWeak,
    index: usize,
}

impl ItemWeak {
    pub fn upgrade(&self) -> Option<ItemRc> {
        self.component.upgrade().map(|c| ItemRc::new(c, self.index))
    }
}

#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
/// The implementation of the `Rectangle` element
pub struct Rectangle {
    pub background: Property<Brush>,
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Rectangle {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        LayoutInfo { horizontal_stretch: 1., vertical_stretch: 1., ..LayoutInfo::default() }
    }

    fn implicit_size(self: Pin<&Self>, _window: &ComponentWindow) -> Size {
        Default::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}

    fn render(self: Pin<&Self>, backend: &mut ItemRendererRef) {
        (*backend).draw_rectangle(self)
    }
}

impl ItemConsts for Rectangle {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Rectangle,
        CachedRenderingData,
    > = Rectangle::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! {
    /// The VTable for `Rectangle`
    #[no_mangle]
    pub static RectangleVTable for Rectangle
}

#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
/// The implementation of the `BorderRectangle` element
pub struct BorderRectangle {
    pub background: Property<Brush>,
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub border_width: Property<f32>,
    pub border_radius: Property<f32>,
    pub border_color: Property<Brush>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for BorderRectangle {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        LayoutInfo { horizontal_stretch: 1., vertical_stretch: 1., ..LayoutInfo::default() }
    }

    fn implicit_size(self: Pin<&Self>, _window: &ComponentWindow) -> Size {
        Default::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}

    fn render(self: Pin<&Self>, backend: &mut ItemRendererRef) {
        (*backend).draw_border_rectangle(self)
    }
}

impl ItemConsts for BorderRectangle {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        BorderRectangle,
        CachedRenderingData,
    > = BorderRectangle::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! {
    /// The VTable for `BorderRectangle`
    #[no_mangle]
    pub static BorderRectangleVTable for BorderRectangle
}

ItemVTable_static! {
    /// The VTable for `Image`
    #[no_mangle]
    pub static ImageVTable for Image
}

ItemVTable_static! {
    /// The VTable for `ClippedImage`
    #[no_mangle]
    pub static ClippedImageVTable for ClippedImage
}

/// The implementation of the `TouchArea` element
#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
pub struct TouchArea {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub enabled: Property<bool>,
    /// FIXME: We should anotate this as an "output" property.
    pub pressed: Property<bool>,
    pub has_hover: Property<bool>,
    /// FIXME: there should be just one property for the point istead of two.
    /// Could even be merged with pressed in a Property<Option<Point>> (of course, in the
    /// implementation item only, for the compiler it would stay separate properties)
    pub pressed_x: Property<f32>,
    pub pressed_y: Property<f32>,
    /// FIXME: should maybe be as parameter to the mouse event instead. Or at least just one property
    pub mouse_x: Property<f32>,
    pub mouse_y: Property<f32>,
    pub clicked: Callback<VoidArg>,
    /// FIXME: remove this
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for TouchArea {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn implicit_size(self: Pin<&Self>, _window: &ComponentWindow) -> Size {
        Default::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        if !self.enabled() {
            return InputEventFilterResult::ForwardAndIgnore;
        }
        Self::FIELD_OFFSETS.mouse_x.apply_pin(self).set(event.pos.x);
        Self::FIELD_OFFSETS.mouse_y.apply_pin(self).set(event.pos.y);
        Self::FIELD_OFFSETS.has_hover.apply_pin(self).set(event.what != MouseEventType::MouseExit);
        InputEventFilterResult::ForwardAndInterceptGrab
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        if event.what == MouseEventType::MouseExit {
            Self::FIELD_OFFSETS.has_hover.apply_pin(self).set(false)
        }
        if !self.enabled() {
            return InputEventResult::EventIgnored;
        }
        let result = if matches!(event.what, MouseEventType::MouseReleased) {
            Self::FIELD_OFFSETS.clicked.apply_pin(self).call(&());
            InputEventResult::EventAccepted
        } else {
            InputEventResult::GrabMouse
        };

        Self::FIELD_OFFSETS.pressed.apply_pin(self).set(match event.what {
            MouseEventType::MousePressed => {
                Self::FIELD_OFFSETS.pressed_x.apply_pin(self).set(event.pos.x);
                Self::FIELD_OFFSETS.pressed_y.apply_pin(self).set(event.pos.y);
                true
            }
            MouseEventType::MouseExit | MouseEventType::MouseReleased => false,
            MouseEventType::MouseMoved => {
                return if self.pressed() {
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventAccepted
                }
            }
        });
        result
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}

    fn render(self: Pin<&Self>, _backend: &mut ItemRendererRef) {}
}

impl ItemConsts for TouchArea {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        TouchArea,
        CachedRenderingData,
    > = TouchArea::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! {
    /// The VTable for `TouchArea`
    #[no_mangle]
    pub static TouchAreaVTable for TouchArea
}

#[derive(Copy, Clone, Debug, PartialEq, strum_macros::EnumString, strum_macros::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
/// What is returned from the event handler
pub enum EventResult {
    reject,
    accept,
}

impl Default for EventResult {
    fn default() -> Self {
        Self::reject
    }
}

/// A runtime item that exposes key
#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
pub struct FocusScope {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub has_focus: Property<bool>,
    pub key_pressed: Callback<KeyEventArg, EventResult>,
    pub key_released: Callback<KeyEventArg, EventResult>,
    /// FIXME: remove this
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for FocusScope {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn implicit_size(self: Pin<&Self>, _window: &ComponentWindow) -> Size {
        Default::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        window: &ComponentWindow,
        self_rc: &ItemRc,
    ) -> InputEventResult {
        /*if !self.enabled() {
            return InputEventResult::EventIgnored;
        }*/
        if matches!(event.what, MouseEventType::MousePressed) {
            if !self.has_focus() {
                window.set_focus_item(self_rc);
            }
        }
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, event: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        let r = match event.event_type {
            KeyEventType::KeyPressed => {
                Self::FIELD_OFFSETS.key_pressed.apply_pin(self).call(&(event.clone(),))
            }
            KeyEventType::KeyReleased => {
                Self::FIELD_OFFSETS.key_released.apply_pin(self).call(&(event.clone(),))
            }
        };
        match r {
            EventResult::accept => KeyEventResult::EventAccepted,
            EventResult::reject => KeyEventResult::EventIgnored,
        }
    }

    fn focus_event(self: Pin<&Self>, event: &FocusEvent, _window: &ComponentWindow) {
        match event {
            FocusEvent::FocusIn | FocusEvent::WindowReceivedFocus => {
                self.has_focus.set(true);
            }
            FocusEvent::FocusOut | FocusEvent::WindowLostFocus => {
                self.has_focus.set(false);
            }
        }
    }

    fn render(self: Pin<&Self>, _backend: &mut ItemRendererRef) {}
}

impl ItemConsts for FocusScope {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        FocusScope,
        CachedRenderingData,
    > = FocusScope::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! {
    /// The VTable for `FocusScope`
    #[no_mangle]
    pub static FocusScopeVTable for FocusScope
}

#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
/// The implementation of the `Clip` element
pub struct Clip {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub border_radius: Property<f32>,
    pub border_width: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Clip {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        LayoutInfo { horizontal_stretch: 1., vertical_stretch: 1., ..LayoutInfo::default() }
    }

    fn implicit_size(self: Pin<&Self>, _window: &ComponentWindow) -> Size {
        Default::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}

    fn render(self: Pin<&Self>, backend: &mut ItemRendererRef) {
        let geometry = self.geometry();
        (*backend).combine_clip(
            euclid::rect(0., 0., geometry.width(), geometry.height()),
            self.border_radius(),
            self.border_width(),
        )
    }
}

impl ItemConsts for Clip {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Clip, CachedRenderingData> =
        Clip::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! {
    /// The VTable for `Clip`
    #[no_mangle]
    pub static ClipVTable for Clip
}

#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
/// The Opacity Item is not meant to be used directly by the .60 code, instead, the `opacity: xxx` or `visible: false` should be used
pub struct Opacity {
    // FIXME: this element shouldn't need these geometry property
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub opacity: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Opacity {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        LayoutInfo { horizontal_stretch: 1., vertical_stretch: 1., ..LayoutInfo::default() }
    }

    fn implicit_size(self: Pin<&Self>, _window: &ComponentWindow) -> Size {
        Default::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}

    fn render(self: Pin<&Self>, backend: &mut ItemRendererRef) {
        backend.apply_opacity(self.opacity());
    }
}

impl ItemConsts for Opacity {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Opacity,
        CachedRenderingData,
    > = Opacity::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! {
    /// The VTable for `Opacity`
    #[no_mangle]
    pub static OpacityVTable for Opacity
}

#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
/// The implementation of the `Rotate` element
pub struct Rotate {
    pub angle: Property<f32>,
    pub origin_x: Property<f32>,
    pub origin_y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Rotate {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(0., 0., 0., 0.)
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        LayoutInfo { horizontal_stretch: 1., vertical_stretch: 1., ..LayoutInfo::default() }
    }

    fn implicit_size(self: Pin<&Self>, _window: &ComponentWindow) -> Size {
        Default::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}

    fn render(self: Pin<&Self>, backend: &mut ItemRendererRef) {
        (*backend).translate(self.origin_x(), self.origin_y());
        (*backend).rotate(self.angle());
        (*backend).translate(-self.origin_x(), -self.origin_y());
    }
}

impl ItemConsts for Rotate {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Rotate,
        CachedRenderingData,
    > = Rotate::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! {
    /// The VTable for `Rotate`
    #[no_mangle]
    pub static RotateVTable for Rotate
}

#[derive(Copy, Clone, Debug, PartialEq, strum_macros::EnumString, strum_macros::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum FillRule {
    nonzero,
    evenodd,
}

impl Default for FillRule {
    fn default() -> Self {
        Self::nonzero
    }
}

/// The implementation of the `Path` element
#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
pub struct Path {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub elements: Property<PathData>,
    pub fill: Property<Brush>,
    pub fill_rule: Property<FillRule>,
    pub stroke: Property<Brush>,
    pub stroke_width: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Path {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), 0., 0.)
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn implicit_size(self: Pin<&Self>, _window: &ComponentWindow) -> Size {
        Default::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}

    fn render(self: Pin<&Self>, backend: &mut ItemRendererRef) {
        (*backend).draw_path(self)
    }
}

impl Path {
    /// Returns an iterator of the events of the path and an offset, so that the
    /// shape fits into the width/height of the path while respecting the stroke
    /// width.
    pub fn fitted_path_events(
        self: Pin<&Self>,
    ) -> (euclid::default::Vector2D<f32>, PathDataIterator) {
        let stroke_width = self.stroke_width();
        let bounds_width = (self.width() - stroke_width).max(0.);
        let bounds_height = (self.height() - stroke_width).max(0.);
        let offset = euclid::default::Vector2D::new(stroke_width / 2., stroke_width / 2.);
        let event_iterator = self.elements().iter_fitted(bounds_width, bounds_height);
        (offset, event_iterator)
    }
}

impl ItemConsts for Path {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Path, CachedRenderingData> =
        Path::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! {
    /// The VTable for `Path`
    #[no_mangle]
    pub static PathVTable for Path
}

/// The implementation of the `Flickable` element
#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
pub struct Flickable {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub viewport: Rectangle,
    pub interactive: Property<bool>,
    data: FlickableDataBox,

    /// FIXME: remove this
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Flickable {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        LayoutInfo { horizontal_stretch: 1., vertical_stretch: 1., ..LayoutInfo::default() }
    }

    fn implicit_size(self: Pin<&Self>, _window: &ComponentWindow) -> Size {
        Default::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        if !self.interactive() {
            return InputEventFilterResult::ForwardAndIgnore;
        }
        self.data.handle_mouse_filter(self, event)
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        if !self.interactive() {
            return InputEventResult::EventIgnored;
        }
        self.data.handle_mouse(self, event)
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}

    fn render(self: Pin<&Self>, backend: &mut ItemRendererRef) {
        let geometry = self.geometry();
        (*backend).combine_clip(euclid::rect(0., 0., geometry.width(), geometry.height()), 0., 0.)
    }
}

impl ItemConsts for Flickable {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! {
    /// The VTable for `Flickable`
    #[no_mangle]
    pub static FlickableVTable for Flickable
}

pub use crate::SharedVector;

#[repr(C)]
/// Wraps the internal datastructure for the Flickable
pub struct FlickableDataBox(core::ptr::NonNull<crate::flickable::FlickableData>);

impl Default for FlickableDataBox {
    fn default() -> Self {
        FlickableDataBox(Box::leak(Box::new(crate::flickable::FlickableData::default())).into())
    }
}
impl Drop for FlickableDataBox {
    fn drop(&mut self) {
        // Safety: the self.0 was constructed from a Box::leak in FlickableDataBox::default
        unsafe {
            Box::from_raw(self.0.as_ptr());
        }
    }
}
impl core::ops::Deref for FlickableDataBox {
    type Target = crate::flickable::FlickableData;
    fn deref(&self) -> &Self::Target {
        // Safety: initialized in FlickableDataBox::default
        unsafe { self.0.as_ref() }
    }
}

#[no_mangle]
pub unsafe extern "C" fn sixtyfps_flickable_data_init(data: *mut FlickableDataBox) {
    std::ptr::write(data, FlickableDataBox::default());
}
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_flickable_data_free(data: *mut FlickableDataBox) {
    std::ptr::read(data);
}

/// The implementation of the `PropertyAnimation` element
#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement, Clone, Debug)]
#[pin]
pub struct PropertyAnimation {
    #[rtti_field]
    pub duration: i32,
    #[rtti_field]
    pub loop_count: i32,
    #[rtti_field]
    pub easing: crate::animations::EasingCurve,
}

/// The implementation of the `Window` element
#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
pub struct Window {
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub background: Property<Color>,
    pub title: Property<SharedString>,
    pub default_font_family: Property<SharedString>,
    pub default_font_size: Property<f32>,
    pub default_font_weight: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Window {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(0., 0., self.width(), self.height())
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn implicit_size(self: Pin<&Self>, _window: &ComponentWindow) -> Size {
        Default::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _event: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}

    fn render(self: Pin<&Self>, _backend: &mut ItemRendererRef) {}
}

impl Window {
    /// Returns the font properties that can be used as defaults for child items
    pub fn default_font_properties(self: Pin<&Self>) -> crate::graphics::FontRequest {
        crate::graphics::FontRequest {
            family: {
                let maybe_family = self.default_font_family();
                if !maybe_family.is_empty() {
                    Some(maybe_family)
                } else {
                    None
                }
            },
            pixel_size: {
                let font_size = self.default_font_size();
                if font_size == 0.0 {
                    None
                } else {
                    Some(font_size)
                }
            },
            weight: {
                let font_weight = self.default_font_weight();
                if font_weight == 0 {
                    None
                } else {
                    Some(font_weight)
                }
            },
            ..Default::default()
        }
    }
}

impl ItemConsts for Window {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! {
    /// The VTable for `Window`
    #[no_mangle]
    pub static WindowVTable for Window
}

/// The implementation of the `BoxShadow` element
#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
pub struct BoxShadow {
    // Rectangle properties
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub border_radius: Property<f32>,
    // Shadow specific properties
    pub offset_x: Property<f32>,
    pub offset_y: Property<f32>,
    pub color: Property<Color>,
    pub blur: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for BoxShadow {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        LayoutInfo { horizontal_stretch: 1., vertical_stretch: 1., ..LayoutInfo::default() }
    }

    fn implicit_size(self: Pin<&Self>, _window: &ComponentWindow) -> Size {
        Default::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _event: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}

    fn render(self: Pin<&Self>, backend: &mut ItemRendererRef) {
        (*backend).draw_box_shadow(self)
    }
}

impl ItemConsts for BoxShadow {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! {
    /// The VTable for `BoxShadow`
    #[no_mangle]
    pub static BoxShadowVTable for BoxShadow
}

ItemVTable_static! {
    /// The VTable for `Text`
    #[no_mangle]
    pub static TextVTable for Text
}

ItemVTable_static! {
    /// The VTable for `TextInput`
    #[no_mangle]
    pub static TextInputVTable for TextInput
}
