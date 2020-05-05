// FIXME:  more properties
#[repr(C)]
pub struct Rectangle {
    // FIXME! this is not suppàosed to be a String
    color: &'static str,
}

#[allow(non_upper_case_globals)]
// FIXME
pub static RectangleVTable: crate::datastructures::ItemVTable = crate::datastructures::ItemVTable {
    geometry: None,
    render_node_index_offset: None,
    rendering_info: None,
    layouting_info: None,
    input_event: None,
};
