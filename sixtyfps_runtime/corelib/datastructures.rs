
/// The opaque component type
type ComponentImpl = ();

#[repr(C)]
struct ComponentType {
    /// Allocate an instance of this component
    create: fn(*const ComponentType)-> *mut ComponentImpl,

    /// destruct this component
    destroy: fn(*const ComponentType, *mut ComponentImpl),

    /// Returns an array that represent the item tree
    /// FIXME: dynamic items
    item_tree: fn(*const ComponentType)-> *const ItemTreeNode,
}

#[repr(C)]
struct ItemTreeNode {
    /// byte offset where we can find the item
    offset: isize,
    /// virtual table of the item
    vtable : *const ItemVTable,

    /// number of children
    chilren_count : u32,

    /// index of the fisrt children
    children_index : u32,
}

#[repr(C)]
struct ItemVTable {
    //???

    // an offset of where to find this property
    x: isize,

    // or a function
    y: fn(*const ()) -> f32,

    // ???
    rendering_primitive: (),

    // ???
    layouting_info: ()
}




