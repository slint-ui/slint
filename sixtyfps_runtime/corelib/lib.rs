/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
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

pub mod component;
pub mod items;
pub mod model;
pub mod properties;
pub mod sharedarray;
pub mod signals;
pub mod string;

#[doc(inline)]
pub use string::SharedString;

#[doc(inline)]
pub use sharedarray::SharedArray;

#[doc(inline)]
pub use graphics::Resource;

#[doc(inline)]
pub use properties::Property;

#[doc(inline)]
pub use signals::Signal;

#[doc(inline)]
pub use graphics::Color;

#[doc(inline)]
pub use graphics::ARGBColor;

#[doc(inline)]
pub use graphics::PathData;

pub mod slice;

pub mod eventloop;
pub mod item_rendering;
pub mod tests;
pub mod timers;

/// One need to use at least one function in each module in order to get them
/// exported in the final binary.
/// This only use functions from modules which are not otherwise used.
#[doc(hidden)]
#[cold]
pub fn use_modules() -> usize {
    tests::sixtyfps_mock_elapsed_time as usize
        + signals::ffi::sixtyfps_signal_init as usize
        + sharedarray::ffi::sixtyfps_shared_array_empty as usize
        + layout::solve_grid_layout as usize
        + item_tree::ffi::sixtyfps_visit_item_tree as usize
        + graphics::ffi::sixtyfps_new_path_elements as usize
        + properties::ffi::sixtyfps_property_init as usize
        + string::ffi::sixtyfps_shared_string_bytes as usize
        + eventloop::ffi::sixtyfps_component_window_drop as usize
        + input::ffi::sixtyfps_process_ungrabbed_mouse_event as usize
        + component::ffi::sixtyfps_component_init_items as usize
}
