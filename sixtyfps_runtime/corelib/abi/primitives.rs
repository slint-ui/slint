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

// FIXME: remove  (or use the libc one)
#[allow(non_camel_case_types)]
type c_char = i8;

#[repr(C)]
#[derive(const_field_offset::FieldOffsets)]
pub struct Image {
    /// FIXME: make it a image source
    pub source: *const c_char,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub cached_rendering_data: super::datastructures::CachedRenderingData,
}

impl Default for Image {
    fn default() -> Self {
        Image {
            source: (b"\0").as_ptr() as *const _,
            x: 0.,
            y: 0.,
            width: 0.,
            height: 0.,
            cached_rendering_data: Default::default(),
        }
    }
}

impl Item for Image {
    fn geometry(&self) {}
    fn rendering_info(&self) -> RenderingInfo {
        unsafe { RenderingInfo::Image(std::ffi::CStr::from_ptr(self.source).to_str().unwrap()) }
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
