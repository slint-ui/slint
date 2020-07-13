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

#![allow(non_upper_case_globals)]
#![allow(missing_docs)] // because documenting each property of items is redundent

use super::datastructures::{
    CachedRenderingData, Color, Item, ItemConsts, LayoutInfo, PathData, Rect, RenderingPrimitive,
    Resource,
};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::{EvaluationContext, Property, SharedString, Signal};
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use corelib_macro::*;

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
    fn geometry(self: Pin<&Self>, context: &EvaluationContext) -> Rect {
        euclid::rect(
            Self::field_offsets().x.apply_pin(self).get(context),
            Self::field_offsets().y.apply_pin(self).get(context),
            Self::field_offsets().width.apply_pin(self).get(context),
            Self::field_offsets().height.apply_pin(self).get(context),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        context: &crate::EvaluationContext,
    ) -> RenderingPrimitive {
        let width = Self::field_offsets().width.apply_pin(self).get(context);
        let height = Self::field_offsets().height.apply_pin(self).get(context);
        if width > 0. && height > 0. {
            RenderingPrimitive::Rectangle {
                x: Self::field_offsets().x.apply_pin(self).get(context),
                y: Self::field_offsets().y.apply_pin(self).get(context),
                width,
                height,
                color: Self::field_offsets().color.apply_pin(self).get(context),
            }
        } else {
            RenderingPrimitive::NoContents
        }
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        Default::default()
    }

    fn input_event(
        self: Pin<&Self>,
        _: super::datastructures::MouseEvent,
        _: &crate::EvaluationContext,
    ) {
    }
}

impl ItemConsts for Rectangle {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Rectangle,
        CachedRenderingData,
    > = Rectangle::field_offsets().cached_rendering_data.as_unpinned_projection();
}

pub use crate::abi::datastructures::RectangleVTable;

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
    fn geometry(self: Pin<&Self>, context: &crate::EvaluationContext) -> Rect {
        euclid::rect(
            Self::field_offsets().x.apply_pin(self).get(context),
            Self::field_offsets().y.apply_pin(self).get(context),
            Self::field_offsets().width.apply_pin(self).get(context),
            Self::field_offsets().height.apply_pin(self).get(context),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        context: &crate::EvaluationContext,
    ) -> RenderingPrimitive {
        RenderingPrimitive::Image {
            x: Self::field_offsets().x.apply_pin(self).get(context),
            y: Self::field_offsets().y.apply_pin(self).get(context),
            source: Self::field_offsets().source.apply_pin(self).get(context),
        }
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        // FIXME: should we use the image size here
        Default::default()
    }

    fn input_event(
        self: Pin<&Self>,
        _: super::datastructures::MouseEvent,
        _: &crate::EvaluationContext,
    ) {
    }
}

impl ItemConsts for Image {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Image,
        CachedRenderingData,
    > = Image::field_offsets().cached_rendering_data.as_unpinned_projection();
}

pub use crate::abi::datastructures::ImageVTable;

/// The implementation of the `Text` element
#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct Text {
    pub text: Property<SharedString>,
    pub font_family: Property<SharedString>,
    pub font_pixel_size: Property<f32>,
    pub color: Property<Color>,
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Text {
    // FIXME: width / height.  or maybe it doesn't matter?  (
    fn geometry(self: Pin<&Self>, context: &crate::EvaluationContext) -> Rect {
        euclid::rect(
            Self::field_offsets().x.apply_pin(self).get(context),
            Self::field_offsets().y.apply_pin(self).get(context),
            0.,
            0.,
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        context: &crate::EvaluationContext,
    ) -> RenderingPrimitive {
        RenderingPrimitive::Text {
            x: Self::field_offsets().x.apply_pin(self).get(context),
            y: Self::field_offsets().y.apply_pin(self).get(context),
            text: Self::field_offsets().text.apply_pin(self).get(context),
            font_family: Self::field_offsets().font_family.apply_pin(self).get(context),
            font_pixel_size: Self::field_offsets().font_pixel_size.apply_pin(self).get(context),
            color: Self::field_offsets().color.apply_pin(self).get(context),
        }
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        todo!()
    }

    fn input_event(
        self: Pin<&Self>,
        _: super::datastructures::MouseEvent,
        _: &crate::EvaluationContext,
    ) {
    }
}

impl ItemConsts for Text {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Text, CachedRenderingData> =
        Text::field_offsets().cached_rendering_data.as_unpinned_projection();
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
    fn geometry(self: Pin<&Self>, context: &crate::EvaluationContext) -> Rect {
        euclid::rect(
            Self::field_offsets().x.apply_pin(self).get(context),
            Self::field_offsets().y.apply_pin(self).get(context),
            Self::field_offsets().width.apply_pin(self).get(context),
            Self::field_offsets().height.apply_pin(self).get(context),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        _context: &crate::EvaluationContext,
    ) -> RenderingPrimitive {
        RenderingPrimitive::NoContents
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        todo!()
    }

    fn input_event(
        self: Pin<&Self>,
        event: super::datastructures::MouseEvent,
        context: &crate::EvaluationContext,
    ) {
        println!("Touch Area Event {:?}", event);
        Self::field_offsets().pressed.apply_pin(self).set(match event.what {
            super::datastructures::MouseEventType::MousePressed => true,
            super::datastructures::MouseEventType::MouseReleased => false,
            super::datastructures::MouseEventType::MouseMoved => return,
        });
        if matches!(event.what, super::datastructures::MouseEventType::MouseReleased) {
            Self::field_offsets().clicked.apply_pin(self).emit(context, ())
        }
    }
}

impl ItemConsts for TouchArea {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        TouchArea,
        CachedRenderingData,
    > = TouchArea::field_offsets().cached_rendering_data.as_unpinned_projection();
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
    fn geometry(self: Pin<&Self>, context: &crate::EvaluationContext) -> Rect {
        euclid::rect(
            Self::field_offsets().x.apply_pin(self).get(context),
            Self::field_offsets().y.apply_pin(self).get(context),
            0.,
            0.,
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        context: &crate::EvaluationContext,
    ) -> RenderingPrimitive {
        RenderingPrimitive::Path {
            x: Self::field_offsets().x.apply_pin(self).get(context),
            y: Self::field_offsets().y.apply_pin(self).get(context),
            width: Self::field_offsets().width.apply_pin(self).get(context),
            height: Self::field_offsets().height.apply_pin(self).get(context),
            elements: Self::field_offsets().elements.apply_pin(self).get(context),
            fill_color: Self::field_offsets().fill_color.apply_pin(self).get(context),
            stroke_color: Self::field_offsets().stroke_color.apply_pin(self).get(context),
            stroke_width: Self::field_offsets().stroke_width.apply_pin(self).get(context),
        }
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        todo!()
    }

    fn input_event(
        self: Pin<&Self>,
        _: super::datastructures::MouseEvent,
        _: &crate::EvaluationContext,
    ) {
    }
}

impl ItemConsts for Path {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Path, CachedRenderingData> =
        Path::field_offsets().cached_rendering_data.as_unpinned_projection();
}

pub use crate::abi::datastructures::PathVTable;

/// The implementation of the `PropertyAnimation` element
#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem, Clone)]
#[pin]
pub struct PropertyAnimation {
    #[rtti_field]
    pub duration: i32,
}
