// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Blanket `ErasedItem` impl for any native item type.
//!
//! Items are stored as `Pin<Rc<dyn ErasedItem>>`.
//! Trait methods look up `PropertyInfo`/`CallbackInfo` by name via `BuiltinItem`.

use crate::Value;
use crate::erased::{ErasedItem, ErasedItemRc};
use i_slint_core::Property;
use i_slint_core::items::{ItemVTable, PropertyAnimation};
use i_slint_core::rtti::{
    AnimatedBindingKind, BuiltinItem, CallbackInfo, PropertyInfo, TwoWayBindingMapping,
};
use std::pin::Pin;
use std::rc::Rc;
use vtable::{HasStaticVTable, VRef};

fn find_property<T: 'static + BuiltinItem>(
    name: &str,
) -> Option<&'static dyn PropertyInfo<T, Value>> {
    T::properties::<Value>().into_iter().find_map(|(n, info)| (n == name).then_some(info))
}

fn find_callback<T: 'static + BuiltinItem>(
    name: &str,
) -> Option<&'static dyn CallbackInfo<T, Value>> {
    T::callbacks::<Value>().into_iter().find_map(|(n, info)| (n == name).then_some(info))
}

impl<T> ErasedItem for T
where
    T: 'static + BuiltinItem + HasStaticVTable<ItemVTable>,
{
    fn get_property(self: Pin<&Self>, name: &str) -> Result<Value, ()> {
        find_property::<T>(name).ok_or(())?.get(self)
    }

    fn set_property(
        self: Pin<&Self>,
        name: &str,
        value: Value,
        animation: Option<PropertyAnimation>,
    ) -> Result<(), ()> {
        find_property::<T>(name).ok_or(())?.set(self, value, animation)
    }

    fn set_property_binding(
        self: Pin<&Self>,
        name: &str,
        binding: Box<dyn Fn() -> Value>,
        animation: AnimatedBindingKind,
    ) -> Result<(), ()> {
        find_property::<T>(name).ok_or(())?.set_binding(self, binding, animation)
    }

    fn prepare_property_for_two_way_binding(
        self: Pin<&Self>,
        name: &str,
    ) -> Option<Pin<Rc<Property<Value>>>> {
        Some(find_property::<T>(name)?.prepare_for_two_way_binding(self))
    }

    fn link_property_two_way_with_map(
        self: Pin<&Self>,
        name: &str,
        other: Pin<Rc<Property<Value>>>,
        mapper: Option<Rc<dyn TwoWayBindingMapping<Value>>>,
    ) {
        if let Some(info) = find_property::<T>(name) {
            info.link_two_way_with_map(self, other, mapper);
        }
    }

    fn call_callback(self: Pin<&Self>, name: &str, args: &[Value]) -> Result<Value, ()> {
        find_callback::<T>(name).ok_or(())?.call(self, args)
    }

    fn set_callback_handler(
        self: Pin<&Self>,
        name: &str,
        handler: Box<dyn Fn(&[Value]) -> Value>,
    ) -> Result<(), ()> {
        find_callback::<T>(name).ok_or(())?.set_handler(self, handler)
    }

    fn set_field(self: Pin<&Self>, _name: &str, _value: Value) -> Result<(), ()> {
        // `FieldInfo::set_field` needs `&mut T`, which a `Pin<&Self>` can't give.
        // Layouts write geometric fields via the corresponding `Property<Length>`,
        // so this stays unused until a concrete caller forces the design.
        Err(())
    }

    fn as_item_ref(self: Pin<&Self>) -> Pin<VRef<'_, ItemVTable>> {
        VRef::new_pin(self)
    }
}

/// Allocate a default-constructed item of type `T` as a boxed `ErasedItemRc`.
pub fn make_item<T>() -> ErasedItemRc
where
    T: 'static + Default + BuiltinItem + HasStaticVTable<ItemVTable>,
{
    Rc::pin(T::default())
}
