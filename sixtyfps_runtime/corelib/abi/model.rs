/// Opaque type of a model, represented bith a ModelType
pub struct ModelImpl;

/// Virtual table for a model.
///
/// TODO: how to represent the data
///
/// TODO: how to get notification when it changes
#[repr(C)]
pub struct ModelType {
    /// Number of items
    count: unsafe fn(*const ModelType, *const ModelImpl) -> u32,

    /// Returns the data. (FIXME: find out what this returns exactly)
    data: unsafe fn(*const ModelType, *const ModelImpl, n: u32) -> *const (),
}

/// This structure will hold a vector of the component instaces
#[repr(C)]
pub struct ComponentVec {
    /// Should be some kind of smart pointer that deletes the component
    /// Should also be a repr(c) thing with some kind of init method
    _todo: Vec<*mut dyn super::datastructures::Component>,
}
