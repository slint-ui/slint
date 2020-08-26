/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
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
