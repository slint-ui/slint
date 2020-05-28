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

use super::datastructures::{
    CachedRenderingData, Color, Item, ItemConsts, ItemVTable, LayoutInfo, Rect, RenderingPrimitive,
};
use crate::{EvaluationContext, Property, SharedString, Signal};
use vtable::HasStaticVTable;

#[repr(C)]
#[derive(const_field_offset::FieldOffsets, Default)]
pub struct Rectangle {
    /// FIXME: make it a color
    pub color: Property<u32>,
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Rectangle {
    fn geometry(&self, context: &EvaluationContext) -> Rect {
        euclid::rect(
            self.x.get(context),
            self.y.get(context),
            self.width.get(context),
            self.height.get(context),
        )
    }
    fn rendering_primitive(&self, context: &crate::EvaluationContext) -> RenderingPrimitive {
        let width = self.width.get(context);
        let height = self.height.get(context);
        if width > 0. && height > 0. {
            RenderingPrimitive::Rectangle {
                x: self.x.get(context),
                y: self.y.get(context),
                width,
                height,
                color: Color::from_argb_encoded(self.color.get(context)),
            }
        } else {
            RenderingPrimitive::NoContents
        }
    }

    fn layouting_info(&self) -> LayoutInfo {
        todo!()
    }

    fn input_event(&self, _: super::datastructures::MouseEvent, _: &crate::EvaluationContext) {}
}

impl ItemConsts for Rectangle {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Rectangle,
        CachedRenderingData,
    > = Rectangle::field_offsets().cached_rendering_data;
}

#[no_mangle]
pub static RectangleVTable: ItemVTable = Rectangle::VTABLE;

#[repr(C)]
#[derive(const_field_offset::FieldOffsets, Default)]
pub struct Image {
    /// FIXME: make it a image source
    pub source: Property<SharedString>,
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Image {
    fn geometry(&self, context: &crate::EvaluationContext) -> Rect {
        euclid::rect(
            self.x.get(context),
            self.y.get(context),
            self.width.get(context),
            self.height.get(context),
        )
    }
    fn rendering_primitive(&self, context: &crate::EvaluationContext) -> RenderingPrimitive {
        RenderingPrimitive::Image {
            x: self.x.get(context),
            y: self.y.get(context),
            source: self.source.get(context),
        }
    }

    fn layouting_info(&self) -> LayoutInfo {
        todo!()
    }

    fn input_event(&self, _: super::datastructures::MouseEvent, _: &crate::EvaluationContext) {}
}

impl ItemConsts for Image {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Image,
        CachedRenderingData,
    > = Image::field_offsets().cached_rendering_data;
}

#[no_mangle]
pub static ImageVTable: ItemVTable = Image::VTABLE;

#[repr(C)]
#[derive(const_field_offset::FieldOffsets, Default)]
pub struct Text {
    pub text: Property<SharedString>,
    pub font_family: Property<SharedString>,
    pub font_pixel_size: Property<f32>,
    pub color: Property<u32>,
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Text {
    // FIXME: width / height.  or maybe it doesn't matter?  (
    fn geometry(&self, context: &crate::EvaluationContext) -> Rect {
        euclid::rect(self.x.get(context), self.y.get(context), 0., 0.)
    }
    fn rendering_primitive(&self, context: &crate::EvaluationContext) -> RenderingPrimitive {
        RenderingPrimitive::Text {
            x: self.x.get(context),
            y: self.y.get(context),
            text: self.text.get(context),
            font_family: self.font_family.get(context),
            font_pixel_size: self.font_pixel_size.get(context),
            color: Color::from_argb_encoded(self.color.get(context)),
        }
    }

    fn layouting_info(&self) -> LayoutInfo {
        todo!()
    }

    fn input_event(&self, _: super::datastructures::MouseEvent, _: &crate::EvaluationContext) {}
}

impl ItemConsts for Text {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Text, CachedRenderingData> =
        Text::field_offsets().cached_rendering_data;
}

#[no_mangle]
pub static TextVTable: ItemVTable = Text::VTABLE;

#[repr(C)]
#[derive(const_field_offset::FieldOffsets, Default)]
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
    fn geometry(&self, context: &crate::EvaluationContext) -> Rect {
        euclid::rect(
            self.x.get(context),
            self.y.get(context),
            self.width.get(context),
            self.height.get(context),
        )
    }
    fn rendering_primitive(&self, _context: &crate::EvaluationContext) -> RenderingPrimitive {
        RenderingPrimitive::NoContents
    }

    fn layouting_info(&self) -> LayoutInfo {
        todo!()
    }

    fn input_event(
        &self,
        event: super::datastructures::MouseEvent,
        context: &crate::EvaluationContext,
    ) {
        println!("Touch Area Event {:?}", event);
        self.pressed.set(match event.what {
            super::datastructures::MouseEventType::MousePressed => true,
            super::datastructures::MouseEventType::MouseReleased => false,
            super::datastructures::MouseEventType::MouseMoved => return,
        });
        if matches!(event.what, super::datastructures::MouseEventType::MouseReleased) {
            self.clicked.emit(context, ())
        }
    }
}

impl ItemConsts for TouchArea {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        TouchArea,
        CachedRenderingData,
    > = TouchArea::field_offsets().cached_rendering_data;
}

#[no_mangle]
pub static TouchAreaVTable: ItemVTable = TouchArea::VTABLE;
