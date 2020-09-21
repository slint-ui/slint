/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
This module contains the list of builtin items.

When adding an item or a property, it needs to be kept in sync with different place.
(This is less than ideal and maybe we can have some automation later)

 - It needs to be changed in this module
 - The ItemVTable_static at the end of datastructures.rs (new items only)
 - In the compiler: typeregister.rs
 - In the vewer: main.rs
 - For the C++ code (new item only): the build.rs to export the new item, and the `using` declaration in sixtyfps.h

*/

#![allow(unsafe_code)]
#![allow(non_upper_case_globals)]
#![allow(missing_docs)] // because documenting each property of items is redundent

use super::eventloop::ComponentWindow;
use super::graphics::{Color, HighLevelRenderingPrimitive, PathData, Rect, Resource};
use super::input::{InputEventResult, KeyEvent, KeyEventResult, MouseEvent, MouseEventType};
use super::item_rendering::CachedRenderingData;
use super::layout::LayoutInfo;
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::{Property, SharedString, Signal};
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use sixtyfps_corelib_macros::*;
use vtable::*;

/// Items are the nodes in the render tree.
#[vtable]
#[repr(C)]
pub struct ItemVTable {
    /// Returns the geometry of this item (relative to its parent item)
    pub geometry: extern "C" fn(core::pin::Pin<VRef<ItemVTable>>) -> Rect,

    /// offset in bytes fromthe *const ItemImpl.
    /// isize::MAX  means None
    #[allow(non_upper_case_globals)]
    #[field_offset(CachedRenderingData)]
    pub cached_rendering_data_offset: usize,

    /// Return the rendering primitive used to display this item. This should depend on only
    /// rarely changed properties as it typically contains data uploaded to the GPU.
    pub rendering_primitive: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive,

    /// Return the variables needed to render the graphical primitives of this item. These
    /// are typically variables that do not require uploading any data sets to the GPU and
    /// can instead be represented using uniforms.
    pub rendering_variables: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable>,

    /// We would need max/min/preferred size, and all layout info
    pub layouting_info:
        extern "C" fn(core::pin::Pin<VRef<ItemVTable>>, window: &ComponentWindow) -> LayoutInfo,

    /// input event
    pub input_event: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        MouseEvent,
        window: &ComponentWindow,
    ) -> InputEventResult,

    pub key_event: extern "C" fn(
        core::pin::Pin<VRef<ItemVTable>>,
        &KeyEvent,
        window: &ComponentWindow,
    ) -> KeyEventResult,
}

/// Alias for `vtable::VRef<ItemVTable>` which represent a pointer to a `dyn Item` with
/// the associated vtable
pub type ItemRef<'a> = vtable::VRef<'a, ItemVTable>;

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
/// The implementation of the `Rectangle` element
pub struct Rectangle {
    pub color: Property<Color>,
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Rectangle {
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        let width = Self::FIELD_OFFSETS.width.apply_pin(self).get();
        let height = Self::FIELD_OFFSETS.height.apply_pin(self).get();
        if width > 0. && height > 0. {
            HighLevelRenderingPrimitive::Rectangle { width, height }
        } else {
            HighLevelRenderingPrimitive::NoContents
        }
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::from([RenderingVariable::Color(
            Self::FIELD_OFFSETS.color.apply_pin(self).get(),
        )])
    }

    fn layouting_info(self: Pin<&Self>, _window: &crate::eventloop::ComponentWindow) -> LayoutInfo {
        Default::default()
    }

    fn input_event(self: Pin<&Self>, _: MouseEvent, _window: &ComponentWindow) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
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
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
/// The implementation of the `BorderRectangle` element
pub struct BorderRectangle {
    pub color: Property<Color>,
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub border_width: Property<f32>,
    pub border_radius: Property<f32>,
    pub border_color: Property<Color>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for BorderRectangle {
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        let width = Self::FIELD_OFFSETS.width.apply_pin(self).get();
        let height = Self::FIELD_OFFSETS.height.apply_pin(self).get();
        if width > 0. && height > 0. {
            HighLevelRenderingPrimitive::BorderRectangle {
                width,
                height,
                border_width: Self::FIELD_OFFSETS.border_width.apply_pin(self).get(),
                border_radius: Self::FIELD_OFFSETS.border_radius.apply_pin(self).get(),
            }
        } else {
            HighLevelRenderingPrimitive::NoContents
        }
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::from([
            RenderingVariable::Color(Self::FIELD_OFFSETS.color.apply_pin(self).get()),
            RenderingVariable::Color(Self::FIELD_OFFSETS.border_color.apply_pin(self).get()),
        ])
    }

    fn layouting_info(self: Pin<&Self>, _window: &crate::eventloop::ComponentWindow) -> LayoutInfo {
        Default::default()
    }

    fn input_event(self: Pin<&Self>, _: MouseEvent, _window: &ComponentWindow) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
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

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
/// The implementation of the `Image` element
pub struct Image {
    pub source: Property<Resource>,
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Image {
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        HighLevelRenderingPrimitive::Image {
            source: Self::FIELD_OFFSETS.source.apply_pin(self).get(),
        }
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        let mut vars = SharedArray::default();

        let width = Self::FIELD_OFFSETS.width.apply_pin(self).get();
        let height = Self::FIELD_OFFSETS.height.apply_pin(self).get();

        if width > 0. {
            vars.push(RenderingVariable::ScaledWidth(width));
        }
        if height > 0. {
            vars.push(RenderingVariable::ScaledHeight(height));
        }

        vars
    }

    fn layouting_info(self: Pin<&Self>, _window: &crate::eventloop::ComponentWindow) -> LayoutInfo {
        // FIXME: should we use the image size here
        Default::default()
    }

    fn input_event(self: Pin<&Self>, _: MouseEvent, _window: &ComponentWindow) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }
}

impl ItemConsts for Image {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Image,
        CachedRenderingData,
    > = Image::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! {
    /// The VTable for `Image`
    #[no_mangle]
    pub static ImageVTable for Image
}

#[derive(Copy, Clone, Debug, PartialEq, strum_macros::EnumString, strum_macros::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum TextHorizontalAlignment {
    align_left,
    align_center,
    align_right,
}

impl Default for TextHorizontalAlignment {
    fn default() -> Self {
        Self::align_left
    }
}

#[derive(Copy, Clone, Debug, PartialEq, strum_macros::EnumString, strum_macros::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum TextVerticalAlignment {
    align_top,
    align_center,
    align_bottom,
}

impl Default for TextVerticalAlignment {
    fn default() -> Self {
        Self::align_top
    }
}

/// The implementation of the `Text` element
#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct Text {
    pub text: Property<SharedString>,
    pub font_family: Property<SharedString>,
    pub font_size: Property<f32>,
    pub color: Property<Color>,
    pub horizontal_alignment: Property<TextHorizontalAlignment>,
    pub vertical_alignment: Property<TextVerticalAlignment>,
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Text {
    // FIXME: width / height.  or maybe it doesn't matter?  (
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        HighLevelRenderingPrimitive::Text {
            text: Self::FIELD_OFFSETS.text.apply_pin(self).get(),
            font_family: Self::FIELD_OFFSETS.font_family.apply_pin(self).get(),
            font_size: Text::font_pixel_size(self, window),
        }
    }

    fn rendering_variables(
        self: Pin<&Self>,
        window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        let layout_info = self.layouting_info(window);
        let rect = self.geometry();

        let hor_alignment = Self::FIELD_OFFSETS.horizontal_alignment.apply_pin(self).get();
        let translate_x = match hor_alignment {
            TextHorizontalAlignment::align_left => 0.,
            TextHorizontalAlignment::align_center => rect.width() / 2. - layout_info.min_width / 2.,
            TextHorizontalAlignment::align_right => rect.width() - layout_info.min_width,
        };

        let ver_alignment = Self::FIELD_OFFSETS.vertical_alignment.apply_pin(self).get();
        let translate_y = match ver_alignment {
            TextVerticalAlignment::align_top => 0.,
            TextVerticalAlignment::align_center => rect.height() / 2. - layout_info.min_height / 2.,
            TextVerticalAlignment::align_bottom => rect.height() - layout_info.min_height,
        };

        SharedArray::from([
            RenderingVariable::Translate(translate_x, translate_y),
            RenderingVariable::Color(Self::FIELD_OFFSETS.color.apply_pin(self).get()),
        ])
    }

    fn layouting_info(self: Pin<&Self>, window: &ComponentWindow) -> LayoutInfo {
        let font_family = Self::FIELD_OFFSETS.font_family.apply_pin(self).get();
        let font_size = Text::font_pixel_size(self, window);
        let text = Self::FIELD_OFFSETS.text.apply_pin(self).get();

        crate::font::FONT_CACHE.with(|fc| {
            let font = fc.find_font(&font_family, font_size);
            let width = font.text_width(&text);
            let height = font.height();
            LayoutInfo {
                min_width: width,
                max_width: f32::MAX,
                min_height: height,
                max_height: height,
            }
        })
    }

    fn input_event(self: Pin<&Self>, _: MouseEvent, _window: &ComponentWindow) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }
}

impl ItemConsts for Text {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Text, CachedRenderingData> =
        Text::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

impl Text {
    fn font_pixel_size(self: Pin<&Self>, window: &ComponentWindow) -> f32 {
        let font_size = Self::FIELD_OFFSETS.font_size.apply_pin(self).get();
        if font_size == 0.0 {
            16. * window.scale_factor()
        } else {
            font_size
        }
    }
}

ItemVTable_static! {
    /// The VTable for `Text`
    #[no_mangle]
    pub static TextVTable for Text
}

/// The implementation of the `TouchArea` element
#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct TouchArea {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    /// FIXME: We should anotate this as an "output" property.
    pub pressed: Property<bool>,
    /// FIXME: there should be just one property for the point istead of two.
    /// Could even be merged with pressed in a Property<Option<Point>> (of course, in the
    /// implementation item only, for the compiler it would stay separate properties)
    pub pressed_x: Property<f32>,
    pub pressed_y: Property<f32>,
    /// FIXME: should maybe be as parameter to the mouse event instead. Or at least just one property
    pub mouse_x: Property<f32>,
    pub mouse_y: Property<f32>,
    pub clicked: Signal<()>,
    /// FIXME: remove this
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for TouchArea {
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        HighLevelRenderingPrimitive::NoContents
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &ComponentWindow,
    ) -> InputEventResult {
        Self::FIELD_OFFSETS.mouse_x.apply_pin(self).set(event.pos.x);
        Self::FIELD_OFFSETS.mouse_y.apply_pin(self).set(event.pos.y);

        let result = if matches!(event.what, MouseEventType::MouseReleased) {
            Self::FIELD_OFFSETS.clicked.apply_pin(self).emit(&());
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
                return if Self::FIELD_OFFSETS.pressed.apply_pin(self).get() {
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventIgnored
                }
            }
        });
        result
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }
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

/// The implementation of the `Path` element
#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct Path {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub elements: Property<PathData>,
    pub fill_color: Property<Color>,
    pub stroke_color: Property<Color>,
    pub stroke_width: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Path {
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            0.,
            0.,
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        HighLevelRenderingPrimitive::Path {
            width: Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            height: Self::FIELD_OFFSETS.height.apply_pin(self).get(),
            elements: Self::FIELD_OFFSETS.elements.apply_pin(self).get(),
            stroke_width: Self::FIELD_OFFSETS.stroke_width.apply_pin(self).get(),
        }
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::from([
            RenderingVariable::Color(Self::FIELD_OFFSETS.fill_color.apply_pin(self).get()),
            RenderingVariable::Color(Self::FIELD_OFFSETS.stroke_color.apply_pin(self).get()),
        ])
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn input_event(self: Pin<&Self>, _: MouseEvent, _window: &ComponentWindow) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
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
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct Flickable {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub viewport: Rectangle,
    data: FlickableDataBox,

    /// FIXME: remove this
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Flickable {
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        HighLevelRenderingPrimitive::NoContents
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &ComponentWindow,
    ) -> InputEventResult {
        self.data.handle_mouse(self, event);
        // FIXME
        InputEventResult::EventAccepted
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
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

pub use crate::{graphics::RenderingVariable, SharedArray};

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
#[derive(FieldOffsets, Default, BuiltinItem, Clone)]
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
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct Window {
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Window {
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            0.,
            0.,
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        HighLevelRenderingPrimitive::NoContents
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn input_event(
        self: Pin<&Self>,
        _event: MouseEvent,
        _window: &ComponentWindow,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
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

/// The implementation of the `TextInput` element
#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct TextInput {
    pub text: Property<SharedString>,
    pub font_family: Property<SharedString>,
    pub font_size: Property<f32>,
    pub color: Property<Color>,
    pub horizontal_alignment: Property<TextHorizontalAlignment>,
    pub vertical_alignment: Property<TextVerticalAlignment>,
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cursor_position: Property<i32>,
    pub text_cursor_width: Property<f32>,
    pub cursor_visible: Property<bool>,
    pub accepted: Signal<()>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for TextInput {
    // FIXME: width / height.  or maybe it doesn't matter?  (
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        HighLevelRenderingPrimitive::Text {
            text: Self::FIELD_OFFSETS.text.apply_pin(self).get(),
            font_family: Self::FIELD_OFFSETS.font_family.apply_pin(self).get(),
            font_size: TextInput::font_pixel_size(self, window),
        }
    }

    fn rendering_variables(
        self: Pin<&Self>,
        window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        let layout_info = self.layouting_info(window);
        let rect = self.geometry();

        let hor_alignment = Self::FIELD_OFFSETS.horizontal_alignment.apply_pin(self).get();
        let translate_x = match hor_alignment {
            TextHorizontalAlignment::align_left => 0.,
            TextHorizontalAlignment::align_center => rect.width() / 2. - layout_info.min_width / 2.,
            TextHorizontalAlignment::align_right => rect.width() - layout_info.min_width,
        };

        let ver_alignment = Self::FIELD_OFFSETS.vertical_alignment.apply_pin(self).get();
        let translate_y = match ver_alignment {
            TextVerticalAlignment::align_top => 0.,
            TextVerticalAlignment::align_center => rect.height() / 2. - layout_info.min_height / 2.,
            TextVerticalAlignment::align_bottom => rect.height() - layout_info.min_height,
        };

        let mut variables = SharedArray::from([
            RenderingVariable::Translate(translate_x, translate_y),
            RenderingVariable::Color(Self::FIELD_OFFSETS.color.apply_pin(self).get()),
        ]);

        if Self::FIELD_OFFSETS.cursor_visible.apply_pin(self).get() {
            let cursor_pos = Self::FIELD_OFFSETS.cursor_position.apply_pin(self).get();
            let text = Self::FIELD_OFFSETS.text.apply_pin(self).get();
            let (cursor_x_pos, font_height) = TextInput::with_font(self, window, |font| {
                (font.text_width(text.split_at(cursor_pos as _).0), font.height())
            });

            let cursor_width =
                Self::FIELD_OFFSETS.text_cursor_width.apply_pin(self).get() * window.scale_factor();

            variables.push(RenderingVariable::TextCursor(cursor_x_pos, cursor_width, font_height));
        }

        variables
    }

    fn layouting_info(self: Pin<&Self>, window: &ComponentWindow) -> LayoutInfo {
        let text = Self::FIELD_OFFSETS.text.apply_pin(self).get();

        let (width, height) =
            TextInput::with_font(self, window, |font| (font.text_width(&text), font.height()));

        LayoutInfo { min_width: width, max_width: f32::MAX, min_height: height, max_height: height }
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        window: &ComponentWindow,
    ) -> InputEventResult {
        if matches!(event.what, MouseEventType::MouseReleased) {
            self.as_ref().show_cursor(window);
            InputEventResult::EventAccepted
        } else {
            InputEventResult::EventIgnored
        }
    }

    fn key_event(self: Pin<&Self>, event: &KeyEvent, window: &ComponentWindow) -> KeyEventResult {
        use std::convert::TryFrom;
        match event {
            KeyEvent::CharacterInput { unicode_scalar, .. } => {
                let mut text: String = Self::FIELD_OFFSETS.text.apply_pin(self).get().into();

                // FIXME: respect grapheme boundaries
                let insert_pos = Self::FIELD_OFFSETS
                    .cursor_position
                    .apply_pin(self)
                    .get()
                    .max(0)
                    .min(text.len() as i32) as usize;
                let ch = char::try_from(*unicode_scalar).unwrap().to_string();
                text.insert_str(insert_pos, &ch);

                Self::FIELD_OFFSETS.text.apply_pin(self).set(text.into());
                Self::FIELD_OFFSETS
                    .cursor_position
                    .apply_pin(self)
                    .set((insert_pos + ch.len()) as i32);

                // Keep the cursor visible when inserting text. Blinking should only occur when
                // nothing is entered or the cursor isn't moved.
                self.as_ref().show_cursor(window);

                KeyEventResult::EventAccepted
            }
            KeyEvent::KeyPressed { code, .. } if *code == crate::input::KeyCode::Right => {
                TextInput::move_cursor(self, TextCursorDirection::Forward, window);
                KeyEventResult::EventAccepted
            }
            KeyEvent::KeyPressed { code, .. } if *code == crate::input::KeyCode::Left => {
                TextInput::move_cursor(self, TextCursorDirection::Backward, window);
                KeyEventResult::EventAccepted
            }
            KeyEvent::KeyPressed { code, .. } if *code == crate::input::KeyCode::Back => {
                TextInput::delete_previous(self, window);
                KeyEventResult::EventAccepted
            }
            KeyEvent::KeyPressed { code, .. } if *code == crate::input::KeyCode::Delete => {
                TextInput::delete_char(self);
                KeyEventResult::EventAccepted
            }
            KeyEvent::KeyPressed { code, .. } if *code == crate::input::KeyCode::Return => {
                Self::FIELD_OFFSETS.accepted.apply_pin(self).emit(&());
                KeyEventResult::EventAccepted
            }
            _ => KeyEventResult::EventIgnored,
        }
    }
}

impl TextInput {
    fn font_pixel_size(self: Pin<&Self>, window: &ComponentWindow) -> f32 {
        let font_size = Self::FIELD_OFFSETS.font_size.apply_pin(self).get();
        if font_size == 0.0 {
            16. * window.scale_factor()
        } else {
            font_size
        }
    }
}

impl ItemConsts for TextInput {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        TextInput,
        CachedRenderingData,
    > = TextInput::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

enum TextCursorDirection {
    Forward,
    Backward,
}

impl TextInput {
    fn show_cursor(&self, window: &ComponentWindow) {
        window.set_cursor_blink_binding(&self.cursor_visible);
    }

    fn move_cursor(
        self: Pin<&Self>,
        direction: TextCursorDirection,
        window: &ComponentWindow,
    ) -> bool {
        let text = Self::FIELD_OFFSETS.text.apply_pin(self).get();
        if text.len() == 0 {
            return false;
        }

        let mut cursor_pos = Self::FIELD_OFFSETS.cursor_position.apply_pin(self).get() as i32;

        let delta: i32 = match direction {
            TextCursorDirection::Forward => 1,
            TextCursorDirection::Backward => -1,
        };

        let mut moved = false;

        loop {
            if (delta < 0 && cursor_pos == 0) || (delta > 0 && cursor_pos == (text.len() as i32)) {
                break;
            }
            cursor_pos += delta;
            moved = true;
            if text.is_char_boundary(cursor_pos as usize) {
                break;
            }
        }

        self.as_ref().cursor_position.set(cursor_pos);

        // Keep the cursor visible when moving. Blinking should only occur when
        // nothing is entered or the cursor isn't moved.
        self.as_ref().show_cursor(window);

        moved
    }

    fn delete_char(self: Pin<&Self>) {
        let mut text: String = Self::FIELD_OFFSETS.text.apply_pin(self).get().into();
        if text.len() == 0 {
            return;
        }

        let cursor_pos = Self::FIELD_OFFSETS.cursor_position.apply_pin(self).get();
        if cursor_pos == text.len() as i32 {
            return;
        }
        text.remove(cursor_pos as usize);

        self.cursor_position.set(cursor_pos.min(text.len() as i32));
        self.text.set(text.into());
    }

    fn delete_previous(self: Pin<&Self>, window: &ComponentWindow) {
        if self.move_cursor(TextCursorDirection::Backward, window) {
            self.delete_char();
        }
    }

    fn with_font<R>(
        self: Pin<&Self>,
        window: &ComponentWindow,
        callback: impl FnOnce(&crate::font::Font) -> R,
    ) -> R {
        let font_family = Self::FIELD_OFFSETS.font_family.apply_pin(self).get();
        let font_size = TextInput::font_pixel_size(self, window);
        crate::font::FONT_CACHE.with(|fc| {
            let font = fc.find_font(&font_family, font_size);
            callback(&font)
        })
    }
}

ItemVTable_static! {
    /// The VTable for `TextInput`
    #[no_mangle]
    pub static TextInputVTable for TextInput
}
