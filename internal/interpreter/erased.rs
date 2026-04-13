// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Trait object for manipulating native items without knowing their type.
//!
//! Replaces the old `dynamic_type` arena.
//! Every item, property and callback is heap-allocated in its own `Pin<Rc<…>>`.
//! The trait methods dispatch by name to the item's `rtti::BuiltinItem` metadata.

use crate::Value;
use core::pin::Pin;
use i_slint_core::graphics::Brush;
use i_slint_core::items::{ItemVTable, PropertyAnimation};
use i_slint_core::properties::InterpolatedPropertyValue;
use i_slint_core::rtti::{AnimatedBindingKind, TwoWayBindingMapping};
use i_slint_core::{Callback, Property};
use std::rc::Rc;
use vtable::VRef;

/// Erased access to a boxed native item such as `Rectangle`, `Text` or `TouchArea`.
pub trait ErasedItem {
    fn get_property(self: Pin<&Self>, name: &str) -> Result<Value, ()>;

    fn set_property(
        self: Pin<&Self>,
        name: &str,
        value: Value,
        animation: Option<PropertyAnimation>,
    ) -> Result<(), ()>;

    fn set_property_binding(
        self: Pin<&Self>,
        name: &str,
        binding: Box<dyn Fn() -> Value>,
        animation: AnimatedBindingKind,
    ) -> Result<(), ()>;

    fn prepare_property_for_two_way_binding(
        self: Pin<&Self>,
        name: &str,
    ) -> Option<Pin<Rc<Property<Value>>>>;

    fn link_property_two_way_with_map(
        self: Pin<&Self>,
        name: &str,
        other: Pin<Rc<Property<Value>>>,
        mapper: Option<Rc<dyn TwoWayBindingMapping<Value>>>,
    );

    fn call_callback(self: Pin<&Self>, name: &str, args: &[Value]) -> Result<Value, ()>;

    fn set_callback_handler(
        self: Pin<&Self>,
        name: &str,
        handler: Box<dyn Fn(&[Value]) -> Value>,
    ) -> Result<(), ()>;

    /// Write a plain (non-`Property`) field, e.g. the fields updated by layouts.
    fn set_field(self: Pin<&Self>, name: &str, value: Value) -> Result<(), ()>;

    fn as_item_ref(self: Pin<&Self>) -> Pin<VRef<'_, ItemVTable>>;
}

pub type ErasedItemRc = Pin<Rc<dyn ErasedItem>>;

/// A sub-component's own `Property<Value>`.
pub type SubComponentProperty = Pin<Rc<Property<Value>>>;

/// A sub-component's own callback, with a `Vec<Value>` argument tuple.
pub type SubComponentCallback = Pin<Rc<Callback<(Vec<Value>,), Value>>>;

/// Animated bindings on sub-component properties need `Value` to be
/// interpolable.
/// Numeric and brush variants interpolate; everything else snaps.
impl InterpolatedPropertyValue for Value {
    fn interpolate(&self, target: &Self, t: f32) -> Self {
        match (self, target) {
            (Value::Number(a), Value::Number(b)) => Value::Number(a + (b - a) * t as f64),
            (Value::Brush(a), Value::Brush(b)) => Value::Brush(Brush::interpolate(a, b, t)),
            _ => target.clone(),
        }
    }
}
