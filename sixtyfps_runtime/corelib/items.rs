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

use super::abi::datastructures::{Item, ItemConsts};
use super::graphics::{Color, HighLevelRenderingPrimitive, PathData, Rect, Resource};
use super::input::{InputEventResult, MouseEvent, MouseEventType};
use super::item_rendering::CachedRenderingData;
use super::layout::LayoutInfo;
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::{Property, SharedString, Signal};
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use sixtyfps_corelib_macros::*;

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
    fn rendering_primitive(self: Pin<&Self>) -> HighLevelRenderingPrimitive {
        let width = Self::FIELD_OFFSETS.width.apply_pin(self).get();
        let height = Self::FIELD_OFFSETS.height.apply_pin(self).get();
        if width > 0. && height > 0. {
            HighLevelRenderingPrimitive::Rectangle { width, height }
        } else {
            HighLevelRenderingPrimitive::NoContents
        }
    }

    fn rendering_variables(self: Pin<&Self>) -> SharedArray<RenderingVariable> {
        SharedArray::from(&[RenderingVariable::Color(
            Self::FIELD_OFFSETS.color.apply_pin(self).get(),
        )])
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        Default::default()
    }

    fn input_event(self: Pin<&Self>, _: MouseEvent) -> InputEventResult {
        InputEventResult::EventIgnored
    }
}

impl ItemConsts for Rectangle {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Rectangle,
        CachedRenderingData,
    > = Rectangle::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

pub use crate::abi::datastructures::RectangleVTable;

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
    fn rendering_primitive(self: Pin<&Self>) -> HighLevelRenderingPrimitive {
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

    fn rendering_variables(self: Pin<&Self>) -> SharedArray<RenderingVariable> {
        SharedArray::from(&[
            RenderingVariable::Color(Self::FIELD_OFFSETS.color.apply_pin(self).get()),
            RenderingVariable::Color(Self::FIELD_OFFSETS.border_color.apply_pin(self).get()),
        ])
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        Default::default()
    }

    fn input_event(self: Pin<&Self>, _: MouseEvent) -> InputEventResult {
        InputEventResult::EventIgnored
    }
}

impl ItemConsts for BorderRectangle {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        BorderRectangle,
        CachedRenderingData,
    > = BorderRectangle::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

pub use crate::abi::datastructures::BorderRectangleVTable;

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
    fn rendering_primitive(self: Pin<&Self>) -> HighLevelRenderingPrimitive {
        HighLevelRenderingPrimitive::Image {
            source: Self::FIELD_OFFSETS.source.apply_pin(self).get(),
        }
    }

    fn rendering_variables(self: Pin<&Self>) -> SharedArray<RenderingVariable> {
        let mut vars = Vec::new();

        let width = Self::FIELD_OFFSETS.width.apply_pin(self).get();
        let height = Self::FIELD_OFFSETS.height.apply_pin(self).get();

        if width > 0. {
            vars.push(RenderingVariable::ScaledWidth(width));
        }
        if height > 0. {
            vars.push(RenderingVariable::ScaledHeight(height));
        }

        SharedArray::from_iter(vars.into_iter())
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        // FIXME: should we use the image size here
        Default::default()
    }

    fn input_event(self: Pin<&Self>, _: MouseEvent) -> InputEventResult {
        InputEventResult::EventIgnored
    }
}

impl ItemConsts for Image {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Image,
        CachedRenderingData,
    > = Image::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

pub use crate::abi::datastructures::ImageVTable;

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
    fn rendering_primitive(self: Pin<&Self>) -> HighLevelRenderingPrimitive {
        HighLevelRenderingPrimitive::Text {
            text: Self::FIELD_OFFSETS.text.apply_pin(self).get(),
            font_family: Self::FIELD_OFFSETS.font_family.apply_pin(self).get(),
            font_size: Self::FIELD_OFFSETS.font_size.apply_pin(self).get(),
            color: Self::FIELD_OFFSETS.color.apply_pin(self).get(),
        }
    }

    fn rendering_variables(self: Pin<&Self>) -> SharedArray<RenderingVariable> {
        let layout_info = self.layouting_info();
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

        SharedArray::from(&[RenderingVariable::Translate(translate_x, translate_y)])
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        let font_family = Self::FIELD_OFFSETS.font_family.apply_pin(self).get();
        let font_size = Self::FIELD_OFFSETS.font_size.apply_pin(self).get();
        let text = Self::FIELD_OFFSETS.text.apply_pin(self).get();

        crate::font::FONT_CACHE.with(|fc| {
            let font = fc.find_font(&font_family, font_size);
            let width = font.text_width(&text);
            let height = font.font_height();
            LayoutInfo {
                min_width: width,
                max_width: width,
                min_height: height,
                max_height: height,
            }
        })
    }

    fn input_event(self: Pin<&Self>, _: MouseEvent) -> InputEventResult {
        InputEventResult::EventIgnored
    }
}

impl ItemConsts for Text {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Text, CachedRenderingData> =
        Text::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

pub use crate::abi::datastructures::TextVTable;

/// The implementation of the `TouchArea` element
#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct TouchArea {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    /// FIXME: We should anotate this as an "output" property
    pub pressed: Property<bool>,
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
    fn rendering_primitive(self: Pin<&Self>) -> HighLevelRenderingPrimitive {
        HighLevelRenderingPrimitive::NoContents
    }

    fn rendering_variables(self: Pin<&Self>) -> SharedArray<RenderingVariable> {
        SharedArray::from(&[])
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn input_event(self: Pin<&Self>, event: MouseEvent) -> InputEventResult {
        Self::FIELD_OFFSETS.pressed.apply_pin(self).set(match event.what {
            MouseEventType::MousePressed => true,
            MouseEventType::MouseExit | MouseEventType::MouseReleased => false,
            MouseEventType::MouseMoved => return InputEventResult::EventAccepted,
        });
        if matches!(event.what, MouseEventType::MouseReleased) {
            Self::FIELD_OFFSETS.clicked.apply_pin(self).emit(())
        }
        InputEventResult::GrabMouse
    }
}

impl ItemConsts for TouchArea {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        TouchArea,
        CachedRenderingData,
    > = TouchArea::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}
pub use crate::abi::datastructures::TouchAreaVTable;

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
    fn rendering_primitive(self: Pin<&Self>) -> HighLevelRenderingPrimitive {
        HighLevelRenderingPrimitive::Path {
            width: Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            height: Self::FIELD_OFFSETS.height.apply_pin(self).get(),
            elements: Self::FIELD_OFFSETS.elements.apply_pin(self).get(),
            stroke_width: Self::FIELD_OFFSETS.stroke_width.apply_pin(self).get(),
        }
    }

    fn rendering_variables(self: Pin<&Self>) -> SharedArray<RenderingVariable> {
        SharedArray::from(&[
            RenderingVariable::Color(Self::FIELD_OFFSETS.fill_color.apply_pin(self).get()),
            RenderingVariable::Color(Self::FIELD_OFFSETS.stroke_color.apply_pin(self).get()),
        ])
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn input_event(self: Pin<&Self>, _: MouseEvent) -> InputEventResult {
        InputEventResult::EventIgnored
    }
}

impl ItemConsts for Path {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Path, CachedRenderingData> =
        Path::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

pub use crate::abi::datastructures::PathVTable;

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
    fn rendering_primitive(self: Pin<&Self>) -> HighLevelRenderingPrimitive {
        HighLevelRenderingPrimitive::NoContents
    }

    fn rendering_variables(self: Pin<&Self>) -> SharedArray<RenderingVariable> {
        SharedArray::from(&[])
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn input_event(self: Pin<&Self>, event: MouseEvent) -> InputEventResult {
        self.data.handle_mouse(self, event);
        // FIXME
        InputEventResult::EventAccepted
    }
}

impl ItemConsts for Flickable {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}
pub use crate::{abi::datastructures::FlickableVTable, graphics::RenderingVariable, SharedArray};

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
