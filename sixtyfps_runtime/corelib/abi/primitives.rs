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
    CachedRenderingData, Item, ItemConsts, ItemVTable, LayoutInfo, RenderingInfo,
};
use crate::{Property, SharedString};
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
    fn geometry(&self) {}
    fn rendering_info(&self) -> RenderingInfo {
        RenderingInfo::Rectangle(
            self.x.get(),
            self.y.get(),
            self.width.get(),
            self.height.get(),
            self.color.get(),
        )
    }

    fn layouting_info(&self) -> LayoutInfo {
        todo!()
    }

    fn input_event(&self, _: super::datastructures::MouseEvent) {}
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
    fn geometry(&self) {}
    fn rendering_info(&self) -> RenderingInfo {
        RenderingInfo::Image(self.x.get(), self.y.get(), self.source.get())
    }

    fn layouting_info(&self) -> LayoutInfo {
        todo!()
    }

    fn input_event(&self, _: super::datastructures::MouseEvent) {}
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
    pub color: Property<u32>,
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Text {
    fn geometry(&self) {}
    fn rendering_info(&self) -> RenderingInfo {
        RenderingInfo::Text(self.x.get(), self.y.get(), self.text.get(), self.color.get())
    }

    fn layouting_info(&self) -> LayoutInfo {
        todo!()
    }

    fn input_event(&self, _: super::datastructures::MouseEvent) {}
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
    // FIXME: remove this
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for TouchArea {
    fn geometry(&self) {}
    fn rendering_info(&self) -> RenderingInfo {
        RenderingInfo::NoContents
    }

    fn layouting_info(&self) -> LayoutInfo {
        todo!()
    }

    fn input_event(&self, _: super::datastructures::MouseEvent) {}
}

impl ItemConsts for TouchArea {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<TouchArea, CachedRenderingData> =
        TouchArea::field_offsets().cached_rendering_data;
}

#[no_mangle]
pub static TouchAreaVTable: ItemVTable = TouchArea::VTABLE;
