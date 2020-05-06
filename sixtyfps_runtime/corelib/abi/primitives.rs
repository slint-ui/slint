/// FIXME: if we have a use of that, take from clib
#[allow(non_camel_case_types)]
type c_char = u8;

/// FIXME:  more properties
#[repr(C)]
pub struct Rectangle {
    /// FIXME! this is not supposed to be a String
    color: &'static c_char,
}

/// TODO
#[allow(non_upper_case_globals)]
#[no_mangle]
pub static RectangleVTable: crate::datastructures::ItemVTable = crate::datastructures::ItemVTable {
    geometry: None,
    render_node_index_offset: isize::MAX,
    rendering_info: None,
    layouting_info: None,
    input_event: None,
};
