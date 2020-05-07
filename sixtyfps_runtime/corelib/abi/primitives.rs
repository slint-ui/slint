use super::datastructures::{ItemImpl, ItemVTable, RenderNode, RenderingInfo};

/// FIXME:  more properties
#[repr(C)]
#[derive(const_field_offset::FieldOffsets)]
pub struct Rectangle {
    /// FIXME: make it a color
    color: u32,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    render_node: RenderNode,
}

unsafe fn render_rectangle(i: *const ItemImpl) -> RenderingInfo {
    let r = &*(i as *const Rectangle);
    RenderingInfo::Rectangle(r.x, r.y, r.width, r.height, r.color)
}

#[allow(non_upper_case_globals)]
#[no_mangle]
pub static RectangleVTable: ItemVTable = ItemVTable {
    geometry: None,
    // offset_of!(Rectangle, render_node),    is not const on stable rust
    render_node_index_offset: Rectangle::field_offsets().render_node as isize,
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
    source: *const c_char,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    render_node: super::datastructures::RenderNode,
}

unsafe fn render_image(i: *const ItemImpl) -> RenderingInfo {
    let i = &*(i as *const Image);
    RenderingInfo::Image(std::ffi::CStr::from_ptr(i.source).to_str().unwrap())
}

/// TODO
#[allow(non_upper_case_globals)]
#[no_mangle]
pub static ImageVTable: super::datastructures::ItemVTable = super::datastructures::ItemVTable {
    geometry: None,
    // offset_of!(Rectangle, render_node),    is not const on stable rust
    render_node_index_offset: Image::field_offsets().render_node as isize,
    rendering_info: Some(render_image),
    layouting_info: None,
    input_event: None,
};
