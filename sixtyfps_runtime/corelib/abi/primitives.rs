use super::datastructures::{CachedRenderingData, ItemImpl, ItemVTable, RenderingInfo};

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

unsafe extern "C" fn render_rectangle(i: *const ItemImpl) -> RenderingInfo {
    let r = &*(i as *const Rectangle);
    RenderingInfo::Rectangle(r.x, r.y, r.width, r.height, r.color)
}

#[allow(non_upper_case_globals)]
#[no_mangle]
pub static RectangleVTable: ItemVTable = ItemVTable {
    geometry: None,
    // offset_of!(Rectangle, render_node),    is not const on stable rust
    cached_rendering_data_offset: Rectangle::field_offsets().cached_rendering_data as isize,
    rendering_info: Some(render_rectangle),
    layouting_info: None,
    input_event: None,
};

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

unsafe extern "C" fn render_image(i: *const ItemImpl) -> RenderingInfo {
    let i = &*(i as *const Image);
    RenderingInfo::Image(std::ffi::CStr::from_ptr(i.source).to_str().unwrap())
}

/// TODO
#[allow(non_upper_case_globals)]
#[no_mangle]
pub static ImageVTable: super::datastructures::ItemVTable = super::datastructures::ItemVTable {
    geometry: None,
    // offset_of!(Rectangle, render_node),    is not const on stable rust
    cached_rendering_data_offset: Image::field_offsets().cached_rendering_data as isize,
    rendering_info: Some(render_image),
    layouting_info: None,
    input_event: None,
};
