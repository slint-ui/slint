// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::Value;
use core::pin::Pin;
use i_slint_core::items::{ItemRef, PropertyAnimation};
use i_slint_core::rtti::AnimatedBindingKind;
use i_slint_core::{Callback, Property, rtti};
use std::rc::Rc;

pub trait ItemInstance {
    fn get_property_or_callback<'a>(
        self: Pin<&'a Self>,
        name: &str,
    ) -> Option<PropertyOrCallback<'a>>;

    fn as_item_ref(&self) -> Pin<ItemRef>;
}

pub trait ErasedProperty {
    fn get(self: Pin<&Self>) -> Value;
    fn set(self: Pin<&Self>, value: Value, animation: Option<PropertyAnimation>);
    fn set_binding(
        self: Pin<&Self>,
        binding: Box<dyn Fn() -> Value>,
        animation: rtti::AnimatedBindingKind,
    );
    fn set_constant(self: Pin<&Self>) {}

    fn prepare_for_two_way_binding(self: Pin<&Self>) -> Pin<Rc<Property<Value>>>;
    fn link_two_way_with_map(
        self: Pin<&Self>,
        property2: Pin<Rc<Property<Value>>>,
        map: Option<Rc<dyn rtti::TwoWayBindingMapping<Value>>>,
    );
}

pub trait ErasedCallback {
    fn call(self: Pin<&Self>, args: &[Value]) -> Value;
    fn set_handler(self: Pin<&Self>, handler: Box<dyn Fn(&[Value]) -> Value>);
}

pub type PropertyOrCallback<'a> =
    itertools::Either<Pin<&'a dyn ErasedProperty>, Pin<&'a dyn ErasedCallback>>;

impl rtti::ValueType for Value {}

impl<T> ErasedProperty for Property<T>
where
    T: Clone + TryFrom<Value> + PartialEq,
    Value: From<T>,
{
    fn get(self: Pin<&Self>) -> Value {
        Property::get(self).try_into().unwrap()
    }
    fn set(self: Pin<&Self>, value: Value, animation: Option<PropertyAnimation>) {
        match animation {
            None => Property::set(&*self, value.try_into().ok().unwrap()),
            Some(a) => self.set_animated_value(value.try_into().ok().unwrap(), move || (a, None)),
        }
    }
    fn set_binding(
        self: Pin<&Self>,
        binding: Box<dyn Fn() -> Value>,
        animation: AnimatedBindingKind,
    ) {
        match animation {
            AnimatedBindingKind::NotAnimated => Property::set_binding(&*self, binding),
            AnimatedBindingKind::Animation(a) => {
                Property::set_animated_binding(&*self, binding, move || (a(), None))
            }
            AnimatedBindingKind::Transition(t) => {
                Property::set_animated_binding(&*self, binding, t)
            }
        }
    }

    fn prepare_for_two_way_binding(self: Pin<&Self>) -> Pin<Rc<Property<Value>>> {
        todo!()
    }

    fn link_two_way_with_map(
        self: Pin<&Self>,
        property2: Pin<Rc<Property<Value>>>,
        map: Option<Rc<dyn rtti::TwoWayBindingMapping<Value>>>,
    ) {
    }
}

impl ErasedCallback for Callback<[Value], Value> {
    fn call(self: Pin<&Self>, args: &[Value]) -> Value {
        Callback::call(self.get_ref(), args)
    }

    fn set_handler(self: Pin<&Self>, handler: Box<dyn Fn(&[Value]) -> Value>) {
        Callback::set_handler(self.get_ref(), handler)
    }
}
