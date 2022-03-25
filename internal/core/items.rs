// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
This module contains the builtin items, either in this file or in sub-modules.

When adding an item or a property, it needs to be kept in sync with different place.
(This is less than ideal and maybe we can have some automation later)

 - It needs to be changed in this module
 - In the compiler: builtins.slint
 - In the interpreter (new item only): dynamic_component.rs
 - For the C++ code (new item only): the cbindgen.rs to export the new item
 - Don't forget to update the documentation
*/

#![allow(unsafe_code)]
#![allow(non_upper_case_globals)]
#![allow(missing_docs)] // because documenting each property of items is redundant

use crate::component::ComponentVTable;
use crate::graphics::{Brush, Color, Point, Rect};
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, KeyEvent,
    KeyEventResult, KeyEventType, MouseEvent,
};
use crate::item_rendering::CachedRenderingData;
use crate::layout::{LayoutInfo, Orientation};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowRc;
use crate::{Callback, Property, SharedString};
use alloc::boxed::Box;
use const_field_offset::FieldOffsets;
use core::cell::Cell;
use core::pin::Pin;
use i_slint_core_macros::*;
use vtable::*;

mod text;
pub use text::*;
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
type PointArg = (Point,);

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

// Returned by the `render()` function on items to indicate whether the rendering of
// children should be handled by the caller, of if the item took care of that (for example
// through layer indirection)
#[repr(C)]
pub enum RenderingResult {
    ContinueRenderingChildren,
    ContinueRenderingWithoutChildren,
}

impl Default for RenderingResult {
    fn default() -> Self {
        Self::ContinueRenderingChildren
    }
}

/// Items are the nodes in the render tree.
#[vtable]
#[repr(C)]
pub struct ItemVTable {
    /// This function is called by the run-time after the memory for the item
    /// has been allocated and initialized. It will be called before any user specified
    /// bindings are set.
    pub init: extern "C" fn(core::pin::Pin<VRef<ItemVTable>>, window: &WindowRc),

    /// Returns the geometry of this item (relative to its parent item)
    pub geometry: extern "C" fn(core::pin::Pin<VRef<ItemVTable>>) -> Rect,

    /// offset in bytes from the *const ItemImpl.
    /// isize::MAX  means None
    #[allow(non_upper_case_globals)]
    #[field_offset(CachedRenderingData)]
    pub cached_rendering_data_offset: usize,

    /// We would need max/min/preferred size, and all layout info
    pub layout_info: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        orientation: Orientation,
        window: &WindowRc,
    ) -> LayoutInfo,

    /// Event handler for mouse and touch event. This function is called before being called on children.
    /// Then, depending on the return value, it is called for the children, and their children, then
    /// [`Self::input_event`] is called on the children, and finally [`Self::input_event`] is called
    /// on this item again.
    pub input_event_filter_before_children: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        MouseEvent,
        window: &WindowRc,
        self_rc: &ItemRc,
    ) -> InputEventFilterResult,

    /// Handle input event for mouse and touch event
    pub input_event: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        MouseEvent,
        window: &WindowRc,
        self_rc: &ItemRc,
    ) -> InputEventResult,

    pub focus_event: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        &FocusEvent,
        window: &WindowRc,
    ) -> FocusEventResult,

    pub key_event: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        &KeyEvent,
        window: &WindowRc,
    ) -> KeyEventResult,

    pub render: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        backend: &mut ItemRendererRef,
        self_rc: &ItemRc,
    ) -> RenderingResult,
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

    /// Returns the number of child items for this item. Returns None if
    /// the number is dynamically determined.
    /// TODO: Remove the option when the Subtree trait exists and allows querying
    pub fn children_count(&self) -> Option<u32> {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.component);
        let item_tree = comp_ref_pin.as_ref().get_item_tree();
        match item_tree.as_slice()[self.index] {
            crate::item_tree::ItemTreeNode::Item { children_count, .. } => Some(children_count),
            crate::item_tree::ItemTreeNode::DynamicTree { .. } => None,
        }
    }
}

/// A Weak reference to an item that can be constructed from an ItemRc.
#[derive(Clone, Default)]
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

impl PartialEq for ItemWeak {
    fn eq(&self, other: &Self) -> bool {
        VWeak::ptr_eq(&self.component, &other.component) && self.index == other.index
    }
}

impl Eq for ItemWeak {}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
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
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, _orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
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
        backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
    ) -> RenderingResult {
        (*backend).draw_rectangle(self);
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
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, _orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
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
        backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
    ) -> RenderingResult {
        (*backend).draw_border_rectangle(self);
        RenderingResult::ContinueRenderingChildren
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

#[derive(Copy, Clone, Debug, PartialEq, strum::EnumString, strum::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum MouseCursor {
    default,
    none,
    //context_menu,
    help,
    pointer,
    progress,
    wait,
    //cell,
    crosshair,
    text,
    //vertical_text,
    alias,
    copy,
    //r#move,
    no_drop,
    not_allowed,
    grab,
    grabbing,
    //all_scroll,
    col_resize,
    row_resize,
    n_resize,
    e_resize,
    s_resize,
    w_resize,
    ne_resize,
    nw_resize,
    se_resize,
    sw_resize,
    ew_resize,
    ns_resize,
    nesw_resize,
    nwse_resize,
    //zoom_in,
    //zoom_out,
}

impl Default for MouseCursor {
    fn default() -> Self {
        Self::default
    }
}

/// The implementation of the `TouchArea` element
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct TouchArea {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub enabled: Property<bool>,
    /// FIXME: We should annotate this as an "output" property.
    pub pressed: Property<bool>,
    pub has_hover: Property<bool>,
    /// FIXME: there should be just one property for the point instead of two.
    /// Could even be merged with pressed in a Property<Option<Point>> (of course, in the
    /// implementation item only, for the compiler it would stay separate properties)
    pub pressed_x: Property<f32>,
    pub pressed_y: Property<f32>,
    /// FIXME: should maybe be as parameter to the mouse event instead. Or at least just one property
    pub mouse_x: Property<f32>,
    pub mouse_y: Property<f32>,
    pub mouse_cursor: Property<MouseCursor>,
    pub clicked: Callback<VoidArg>,
    pub moved: Callback<VoidArg>,
    pub pointer_event: Callback<PointerEventArg>,
    /// FIXME: remove this
    pub cached_rendering_data: CachedRenderingData,
    /// true when we are currently grabbing the mouse
    grabbed: Cell<bool>,
}

impl Item for TouchArea {
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
        window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        if !self.enabled() {
            return InputEventFilterResult::ForwardAndIgnore;
        }
        if let Some(pos) = event.pos() {
            Self::FIELD_OFFSETS.mouse_x.apply_pin(self).set(pos.x);
            Self::FIELD_OFFSETS.mouse_y.apply_pin(self).set(pos.y);
        }
        let hovering = !matches!(event, MouseEvent::MouseExit);
        Self::FIELD_OFFSETS.has_hover.apply_pin(self).set(hovering);
        if hovering {
            window.set_mouse_cursor(self.mouse_cursor());
        }
        InputEventFilterResult::ForwardAndInterceptGrab
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        if matches!(event, MouseEvent::MouseExit) {
            Self::FIELD_OFFSETS.has_hover.apply_pin(self).set(false);
            window.set_mouse_cursor(MouseCursor::default);
        }
        if !self.enabled() {
            return InputEventResult::EventIgnored;
        }
        let result = if let MouseEvent::MouseReleased { pos, button } = event {
            if button == PointerEventButton::left
                && euclid::rect(0., 0., self.width(), self.height()).contains(pos)
            {
                Self::FIELD_OFFSETS.clicked.apply_pin(self).call(&());
            }
            InputEventResult::EventAccepted
        } else {
            InputEventResult::GrabMouse
        };

        match event {
            MouseEvent::MousePressed { pos, button } => {
                self.grabbed.set(true);
                if button == PointerEventButton::left {
                    Self::FIELD_OFFSETS.pressed_x.apply_pin(self).set(pos.x);
                    Self::FIELD_OFFSETS.pressed_y.apply_pin(self).set(pos.y);
                    Self::FIELD_OFFSETS.pressed.apply_pin(self).set(true);
                }
                Self::FIELD_OFFSETS
                    .pointer_event
                    .apply_pin(self)
                    .call(&(PointerEvent { button, kind: PointerEventKind::down },));
            }
            MouseEvent::MouseExit => {
                Self::FIELD_OFFSETS.pressed.apply_pin(self).set(false);
                if self.grabbed.replace(false) {
                    Self::FIELD_OFFSETS.pointer_event.apply_pin(self).call(&(PointerEvent {
                        button: PointerEventButton::none,
                        kind: PointerEventKind::cancel,
                    },));
                }
            }
            MouseEvent::MouseReleased { button, .. } => {
                self.grabbed.set(false);
                if button == PointerEventButton::left {
                    Self::FIELD_OFFSETS.pressed.apply_pin(self).set(false);
                }
                Self::FIELD_OFFSETS
                    .pointer_event
                    .apply_pin(self)
                    .call(&(PointerEvent { button, kind: PointerEventKind::up },));
            }
            MouseEvent::MouseMoved { .. } => {
                return if self.grabbed.get() {
                    Self::FIELD_OFFSETS.moved.apply_pin(self).call(&());
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventAccepted
                }
            }
            MouseEvent::MouseWheel { .. } => {
                return if self.grabbed.get() {
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventAccepted
                }
            }
        };
        result
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &WindowRc) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &WindowRc) -> FocusEventResult {
        FocusEventResult::FocusIgnored
    }

    fn render(
        self: Pin<&Self>,
        _backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
    ) -> RenderingResult {
        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for TouchArea {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        TouchArea,
        CachedRenderingData,
    > = TouchArea::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_TouchAreaVTable() -> TouchAreaVTable for TouchArea
}

#[derive(Copy, Clone, Debug, PartialEq, strum::EnumString, strum::Display)]
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
#[derive(FieldOffsets, Default, SlintElement)]
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
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, _orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        window: &WindowRc,
        self_rc: &ItemRc,
    ) -> InputEventResult {
        /*if !self.enabled() {
            return InputEventResult::EventIgnored;
        }*/
        if matches!(event, MouseEvent::MousePressed { .. }) && !self.has_focus() {
            window.clone().set_focus_item(self_rc);
        }
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, event: &KeyEvent, _window: &WindowRc) -> KeyEventResult {
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

    fn focus_event(self: Pin<&Self>, event: &FocusEvent, _window: &WindowRc) -> FocusEventResult {
        match event {
            FocusEvent::FocusIn | FocusEvent::WindowReceivedFocus => {
                self.has_focus.set(true);
            }
            FocusEvent::FocusOut | FocusEvent::WindowLostFocus => {
                self.has_focus.set(false);
            }
        }
        FocusEventResult::FocusAccepted
    }

    fn render(
        self: Pin<&Self>,
        _backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
    ) -> RenderingResult {
        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for FocusScope {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        FocusScope,
        CachedRenderingData,
    > = FocusScope::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_FocusScopeVTable() -> FocusScopeVTable for FocusScope
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
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
    pub clip: Property<bool>,
}

impl Item for Clip {
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, _orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        if let Some(pos) = event.pos() {
            if self.clip()
                && (pos.x < 0. || pos.y < 0. || pos.x > self.width() || pos.y > self.height())
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
        (*backend).visit_clip(self, self_rc)
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
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub opacity: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Opacity {
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, _orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
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
        backend: &mut ItemRendererRef,
        self_rc: &ItemRc,
    ) -> RenderingResult {
        backend.visit_opacity(self, self_rc)
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
        let component_rc = self_rc.component();
        let component_ref = vtable::VRc::borrow_pin(&component_rc);
        let self_index = self_rc.index();
        // TODO: use first_child() once it exists
        let item_tree = component_ref.as_ref().get_item_tree();
        let target_item_index = match item_tree.as_slice()[self_index] {
            crate::item_tree::ItemTreeNode::Item { children_count, children_index, .. }
                if children_count == 1 =>
            {
                children_index as usize
            }
            _ => return true, // Dynamic tree or multiple children -> need layer
        };
        let target_item = ItemRc::new(component_rc.clone(), target_item_index);
        // any children? Then we need a layer
        target_item.children_count() != Some(0)
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
    // FIXME: this element shouldn't need these geometry property
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cache_rendering_hint: Property<bool>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Layer {
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, _orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
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
        backend: &mut ItemRendererRef,
        self_rc: &ItemRc,
    ) -> RenderingResult {
        backend.visit_layer(self, self_rc)
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
    pub angle: Property<f32>,
    pub origin_x: Property<f32>,
    pub origin_y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Rotate {
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(0., 0., 0., 0.)
    }

    fn layout_info(self: Pin<&Self>, _orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
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
        backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
    ) -> RenderingResult {
        (*backend).translate(self.origin_x(), self.origin_y());
        (*backend).rotate(self.angle());
        (*backend).translate(-self.origin_x(), -self.origin_y());
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

/// The implementation of the `Flickable` element
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
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
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, _orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        if let Some(pos) = event.pos() {
            if pos.x < 0. || pos.y < 0. || pos.x > self.width() || pos.y > self.height() {
                return InputEventFilterResult::Intercept;
            }
        }
        if !self.interactive() && !matches!(event, MouseEvent::MouseWheel { .. }) {
            return InputEventFilterResult::ForwardAndIgnore;
        }
        self.data.handle_mouse_filter(self, event)
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        if !self.interactive() && !matches!(event, MouseEvent::MouseWheel { .. }) {
            return InputEventResult::EventIgnored;
        }
        if let Some(pos) = event.pos() {
            if matches!(event, MouseEvent::MouseWheel { .. } | MouseEvent::MousePressed { .. })
                && (pos.x < 0. || pos.y < 0. || pos.x > self.width() || pos.y > self.height())
            {
                return InputEventResult::EventIgnored;
            }
        }

        self.data.handle_mouse(self, event)
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
        _self_rc: &ItemRc,
    ) -> RenderingResult {
        let geometry = self.geometry();
        (*backend).combine_clip(euclid::rect(0., 0., geometry.width(), geometry.height()), 0., 0.);
        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for Flickable {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_FlickableVTable() -> FlickableVTable for Flickable
}

pub use crate::SharedVector;

#[repr(C)]
/// Wraps the internal data structure for the Flickable
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

/// # Safety
/// This must be called using a non-null pointer pointing to a chunk of memory big enough to
/// hold a FlickableDataBox
#[no_mangle]
pub unsafe extern "C" fn slint_flickable_data_init(data: *mut FlickableDataBox) {
    core::ptr::write(data, FlickableDataBox::default());
}

/// # Safety
/// This must be called using a non-null pointer pointing to an initialized FlickableDataBox
#[no_mangle]
pub unsafe extern "C" fn slint_flickable_data_free(data: *mut FlickableDataBox) {
    core::ptr::drop_in_place(data);
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
    pub easing: crate::animations::EasingCurve,
}

impl Default for PropertyAnimation {
    fn default() -> Self {
        // Defaults for PropertyAnimation are defined here (for internal Rust code doing programmatic animations)
        // as well as in `builtins.slint` (for generated C++ and Rust code)
        Self { delay: 0, duration: 0, iteration_count: 1., easing: Default::default() }
    }
}

/// The implementation of the `Window` element
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct WindowItem {
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub background: Property<Color>,
    pub title: Property<SharedString>,
    pub no_frame: Property<bool>,
    pub icon: Property<crate::graphics::Image>,
    pub default_font_family: Property<SharedString>,
    pub default_font_size: Property<f32>,
    pub default_font_weight: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for WindowItem {
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(0., 0., self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, _orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        LayoutInfo::default()
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
        _event: MouseEvent,
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
        _backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
    ) -> RenderingResult {
        RenderingResult::ContinueRenderingChildren
    }
}

impl WindowItem {
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

impl ItemConsts for WindowItem {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_WindowItemVTable() -> WindowItemVTable for WindowItem
}

/// The implementation of the `BoxShadow` element
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
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
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, _orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
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
        _event: MouseEvent,
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
        _self_rc: &ItemRc,
    ) -> RenderingResult {
        (*backend).draw_box_shadow(self);
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
    fn slint_get_TextVTable() -> TextVTable for Text
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

#[derive(Copy, Clone, Debug, PartialEq, strum::EnumString, strum::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum StandardButtonKind {
    ok,
    cancel,
    apply,
    close,
    reset,
    help,
    yes,
    no,
    abort,
    retry,
    ignore,
}

impl Default for StandardButtonKind {
    fn default() -> Self {
        Self::ok
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, strum::EnumString, strum::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum DialogButtonRole {
    none,
    accept,
    reject,
    apply,
    reset,
    action,
    help,
}

impl Default for DialogButtonRole {
    fn default() -> Self {
        Self::none
    }
}

#[derive(Copy, Clone, Debug, PartialEq, strum::EnumString, strum::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum PointerEventKind {
    cancel,
    down,
    up,
}

impl Default for PointerEventKind {
    fn default() -> Self {
        Self::cancel
    }
}

#[derive(Copy, Clone, Debug, PartialEq, strum::EnumString, strum::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum PointerEventButton {
    none,
    left,
    right,
    middle,
}

impl Default for PointerEventButton {
    fn default() -> Self {
        Self::none
    }
}

/// Represents a key event sent by the windowing system.
#[derive(Debug, Clone, PartialEq, Default)]
#[repr(C)]
pub struct PointerEvent {
    pub button: PointerEventButton,
    pub kind: PointerEventKind,
}
