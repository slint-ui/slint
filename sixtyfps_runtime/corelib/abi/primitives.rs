#![allow(non_upper_case_globals)]

use super::datastructures::{
    CachedRenderingData, Item, ItemConsts, ItemVTable, LayoutInfo, RenderingInfo,
};
use vtable::HasStaticVTable;

/// FIXME:  more properties
#[repr(C)]
#[derive(const_field_offset::FieldOffsets, Default)]
pub struct Rectangle {
    /// FIXME: make it a color
    pub color: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Rectangle {
    fn geometry(&self) {}
    fn rendering_info(&self) -> RenderingInfo {
        RenderingInfo::Rectangle(self.x, self.y, self.width, self.height, self.color)
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

#[repr(C)]
#[derive(const_field_offset::FieldOffsets, Default)]
pub struct Image {
    /// FIXME: make it a image source
    pub source: crate::SharedString,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub cached_rendering_data: super::datastructures::CachedRenderingData,
}

impl Item for Image {
    fn geometry(&self) {}
    fn rendering_info(&self) -> RenderingInfo {
        RenderingInfo::Image(self.x, self.y, self.source.clone())
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
pub static RectangleVTable: ItemVTable = Rectangle::VTABLE;

#[no_mangle]
pub static ImageVTable: ItemVTable = Image::VTABLE;
