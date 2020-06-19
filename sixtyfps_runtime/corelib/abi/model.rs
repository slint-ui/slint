//! Models

/*
use super::datastructures::ComponentVTable;

/// Virtual table for a model.
///
/// TODO: how to represent the data
///
/// TODO: how to get notification when it changes
#[repr(C)]
#[vtable]
pub struct ModelVTable {
    /// Number of items
    count: unsafe fn(VRef<ModelVTable>) -> u32,

    /// Returns the data. (FIXME: find out what this returns exactly)
    data: unsafe fn(VRef<ModelVTable>, n: u32) -> *const (),
}*/
/*
/// This structure will hold a vector of the component instaces
#[repr(C)]
pub struct ComponentVecHolder {
    mode: vtable::VBox<ModelType>
    // Possible optimization: all the VBox should have the same VTable kown to the parent component
    _todo: Vec<vtable::VBox<super::datastructures::ComponentVTable>>,
}
*/
