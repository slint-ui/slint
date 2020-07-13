/*!

# SixtyFPS runtime library

**NOTE:** This library is an internal crate for the SixtyFPS project.
This crate should not be used directly by application using SixtyFPS.
You should use the `sixtyfps` crate instead
*/

#![deny(unsafe_code)]

/// The animation system
pub mod animations;
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
    pub mod primitives;
    pub mod properties;
    pub mod sharedarray;
    pub mod signals;
    pub mod slice;
    pub mod string;
}

#[doc(inline)]
pub use abi::string::SharedString;

#[doc(inline)]
pub use abi::sharedarray::SharedArray;

#[doc(inline)]
pub use abi::datastructures::Resource;

#[doc(inline)]
pub use abi::properties::Property;

#[doc(inline)]
pub use abi::signals::Signal;

#[doc(inline)]
pub use abi::datastructures::Color;

#[doc(inline)]
pub use abi::datastructures::PathData;

/// Type alias to the commonly use `Pin<VRef<ComponentVTable>>>`
pub type ComponentRefPin<'a> = core::pin::Pin<abi::datastructures::ComponentRef<'a>>;

pub mod eventloop;
mod item_rendering;
