/*!

# SixtyFPS runtime library

**NOTE:** This library is an internal crate for the SixtyFPS project.
This crate should not be used directly by application using SixtyFPS.
You should use the `sixtyfps` crate instead
*/

#![deny(unsafe_code)]

/// The animation system
pub mod animations;
pub(crate) mod flickable;
pub mod font;
pub mod graphics;
pub mod input;
pub mod item_tree;
pub mod layout;

#[cfg(feature = "rtti")]
pub mod rtti;

/// Things that are exposed to the C ABI
pub mod abi {
    #![warn(missing_docs)]
    // We need to allow unsafe functions because of FFI
    #![allow(unsafe_code)]
    pub mod datastructures;
    pub mod model;
    pub mod sharedarray;
    pub mod signals;
    pub mod tests;
}

pub mod items;
pub mod properties;
pub mod string;

#[doc(inline)]
pub use string::SharedString;

#[doc(inline)]
pub use abi::sharedarray::SharedArray;

#[doc(inline)]
pub use graphics::Resource;

#[doc(inline)]
pub use properties::Property;

#[doc(inline)]
pub use abi::signals::Signal;

#[doc(inline)]
pub use graphics::Color;

#[doc(inline)]
pub use graphics::PathData;

pub mod slice;

/// Type alias to the commonly use `Pin<VRef<ComponentVTable>>>`
pub type ComponentRefPin<'a> = core::pin::Pin<abi::datastructures::ComponentRef<'a>>;

pub mod eventloop;
pub mod item_rendering;

/// One need to use at least one function in each module in order to get them
/// exported in the final binary.
/// This only use functions from modules which are not otherwise used.
#[doc(hidden)]
#[cold]
pub fn use_modules() -> usize {
    abi::tests::sixtyfps_test_ellapse_time as usize
        + abi::signals::sixtyfps_signal_init as usize
        + abi::sharedarray::sixtyfps_shared_array_drop as usize
        + layout::solve_grid_layout as usize
        + item_tree::ffi::sixtyfps_visit_item_tree as usize
        + graphics::ffi::sixtyfps_new_path_elements as usize
        + properties::ffi::sixtyfps_property_init as usize
        + string::ffi::sixtyfps_shared_string_bytes as usize
}
