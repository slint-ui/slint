/// FIXME:  more properties
#[repr(C)]
pub struct Rectangle {
    /// FIXME: make it a color
    color: u32,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

/// TODO
#[allow(non_upper_case_globals)]
#[no_mangle]
pub static RectangleVTable: super::datastructures::ItemVTable = super::datastructures::ItemVTable {
    geometry: None,
    render_node_index_offset: isize::MAX,
    rendering_info: None,
    layouting_info: None,
    input_event: None,
};
