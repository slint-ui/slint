pub use sixtyfps_rs_macro::sixtyfps;

/// internal re_exports used by the macro generated
pub mod re_exports {
    pub use const_field_offset::FieldOffsets;
    pub use corelib::abi::datastructures::{Component, ComponentVTable, ItemTreeNode};
    pub use corelib::abi::primitives::{Image, ImageVTable, Rectangle, RectangleVTable};
    pub use gl::sixtyfps_runtime_run_component_with_gl_renderer;
    pub use once_cell::sync::Lazy;
}
