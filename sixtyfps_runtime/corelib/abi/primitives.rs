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
    render_node: super::datastructures::RenderNode,
}

/// TODO
#[allow(non_upper_case_globals)]
#[no_mangle]
pub static RectangleVTable: super::datastructures::ItemVTable = super::datastructures::ItemVTable {
    geometry: None,
    // offset_of!(Rectangle, render_node),    is not const on stable rust
    render_node_index_offset: Rectangle::field_offsets().render_node as isize,
    rendering_info: None,
    layouting_info: None,
    input_event: None,
};
