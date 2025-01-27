// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
 This module enable runtime type information for the builtin items and
 property so that the viewer can handle them
*/

#![allow(clippy::result_unit_err)] // We have nothing better to report

pub type FieldOffset<T, U> = const_field_offset::FieldOffset<T, U, const_field_offset::AllowPin>;
use crate::items::PropertyAnimation;
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::convert::{TryFrom, TryInto};
use core::pin::Pin;

macro_rules! declare_ValueType {
    ($($ty:ty,)*) => {
        pub trait ValueType: 'static + Default + Clone $(+ TryInto<$ty> + TryFrom<$ty>)* {}
    };
}

macro_rules! declare_ValueType_2 {
    ($( $(#[$enum_doc:meta])* enum $Name:ident { $($body:tt)* })*) => {
        declare_ValueType![
            (),
            bool,
            u32,
            u64,
            i32,
            i64,
            f32,
            f64,
            crate::SharedString,
            crate::graphics::Image,
            crate::Color,
            crate::PathData,
            crate::animations::EasingCurve,
            crate::model::StandardListViewItem,
            crate::model::TableColumn,
            crate::input::KeyEvent,
            crate::Brush,
            crate::graphics::Point,
            crate::items::PointerEvent,
            crate::items::PointerScrollEvent,
            crate::lengths::LogicalLength,
            crate::component_factory::ComponentFactory,
            crate::api::LogicalPosition,
            crate::items::FontMetrics,
            crate::items::MenuEntry,
            crate::model::ModelRc<crate::items::MenuEntry>,
            $(crate::items::$Name,)*
        ];
    };
}

i_slint_common::for_each_enums!(declare_ValueType_2);

/// What kind of animation is on a binding
pub enum AnimatedBindingKind {
    /// No animation is on the binding
    NotAnimated,
    /// Single animation
    Animation(PropertyAnimation),
    /// Transition
    Transition(Box<dyn Fn() -> (PropertyAnimation, crate::animations::Instant)>),
}

impl AnimatedBindingKind {
    /// return a PropertyAnimation if self contains AnimatedBindingKind::Animation
    pub fn as_animation(self) -> Option<PropertyAnimation> {
        match self {
            AnimatedBindingKind::NotAnimated => None,
            AnimatedBindingKind::Animation(a) => Some(a),
            AnimatedBindingKind::Transition(_) => None,
        }
    }
}

pub trait PropertyInfo<Item, Value> {
    fn get(&self, item: Pin<&Item>) -> Result<Value, ()>;
    fn set(
        &self,
        item: Pin<&Item>,
        value: Value,
        animation: Option<PropertyAnimation>,
    ) -> Result<(), ()>;
    fn set_binding(
        &self,
        item: Pin<&Item>,
        binding: Box<dyn Fn() -> Value>,
        animation: AnimatedBindingKind,
    ) -> Result<(), ()>;

    /// The offset of the property in the item.
    /// The use of this is unsafe
    fn offset(&self) -> usize;

    /// Returns self. This is just a trick to get auto-deref specialization of
    /// MaybeAnimatedPropertyInfoWrapper working.
    fn as_property_info(&'static self) -> &'static dyn PropertyInfo<Item, Value>
    where
        Self: Sized,
    {
        self
    }

    /// Calls Property::link_two_ways with the property represented here and the property pointer
    ///
    /// # Safety
    /// the property2 must be a pinned pointer to a Property of the same type
    #[allow(unsafe_code)]
    unsafe fn link_two_ways(&self, item: Pin<&Item>, property2: *const ());
}

impl<Item, T: PartialEq + Clone + 'static, Value: 'static> PropertyInfo<Item, Value>
    for FieldOffset<Item, crate::Property<T>>
where
    Value: TryInto<T>,
    T: TryInto<Value>,
{
    fn get(&self, item: Pin<&Item>) -> Result<Value, ()> {
        self.apply_pin(item).get().try_into().map_err(|_| ())
    }
    fn set(
        &self,
        item: Pin<&Item>,
        value: Value,
        animation: Option<PropertyAnimation>,
    ) -> Result<(), ()> {
        if animation.is_some() {
            Err(())
        } else {
            self.apply_pin(item).set(value.try_into().map_err(|_| ())?);
            Ok(())
        }
    }
    fn set_binding(
        &self,
        item: Pin<&Item>,
        binding: Box<dyn Fn() -> Value>,
        animation: AnimatedBindingKind,
    ) -> Result<(), ()> {
        if !matches!(animation, AnimatedBindingKind::NotAnimated) {
            Err(())
        } else {
            self.apply_pin(item).set_binding(move || {
                binding().try_into().map_err(|_| ()).expect("binding was of the wrong type")
            });
            Ok(())
        }
    }
    fn offset(&self) -> usize {
        self.get_byte_offset()
    }

    #[allow(unsafe_code)]
    unsafe fn link_two_ways(&self, item: Pin<&Item>, property2: *const ()) {
        let p1 = self.apply_pin(item);
        // Safety: that's the invariant of this function
        let p2 = Pin::new_unchecked((property2 as *const crate::Property<T>).as_ref().unwrap());
        crate::Property::link_two_way(p1, p2);
    }
}

/// Wrapper for a field offset that optionally implement PropertyInfo and uses
/// the auto deref specialization trick
#[derive(derive_more::Deref)]
pub struct MaybeAnimatedPropertyInfoWrapper<T, U>(pub FieldOffset<T, U>);

impl<Item, T: Clone + 'static, Value: 'static> PropertyInfo<Item, Value>
    for MaybeAnimatedPropertyInfoWrapper<Item, crate::Property<T>>
where
    Value: TryInto<T>,
    T: TryInto<Value>,
    T: crate::properties::InterpolatedPropertyValue,
{
    fn get(&self, item: Pin<&Item>) -> Result<Value, ()> {
        self.0.get(item)
    }
    fn set(
        &self,
        item: Pin<&Item>,
        value: Value,
        animation: Option<PropertyAnimation>,
    ) -> Result<(), ()> {
        if let Some(animation) = animation {
            self.apply_pin(item).set_animated_value(value.try_into().map_err(|_| ())?, animation);
            Ok(())
        } else {
            self.0.set(item, value, None)
        }
    }
    fn set_binding(
        &self,
        item: Pin<&Item>,
        binding: Box<dyn Fn() -> Value>,
        animation: AnimatedBindingKind,
    ) -> Result<(), ()> {
        // Put in a function that does not depends on Item to avoid code bloat
        fn set_binding_impl<T, Value>(
            p: Pin<&crate::Property<T>>,
            binding: Box<dyn Fn() -> Value>,
            animation: AnimatedBindingKind,
        ) -> Result<(), ()>
        where
            T: Clone + TryInto<Value> + crate::properties::InterpolatedPropertyValue + 'static,
            Value: TryInto<T> + 'static,
        {
            match animation {
                AnimatedBindingKind::NotAnimated => {
                    p.set_binding(move || {
                        binding().try_into().map_err(|_| ()).expect("binding was of the wrong type")
                    });
                    Ok(())
                }
                AnimatedBindingKind::Animation(animation) => {
                    p.set_animated_binding(
                        move || {
                            binding()
                                .try_into()
                                .map_err(|_| ())
                                .expect("binding was of the wrong type")
                        },
                        animation,
                    );
                    Ok(())
                }
                AnimatedBindingKind::Transition(tr) => {
                    p.set_animated_binding_for_transition(
                        move || {
                            binding()
                                .try_into()
                                .map_err(|_| ())
                                .expect("binding was of the wrong type")
                        },
                        tr,
                    );
                    Ok(())
                }
            }
        }
        set_binding_impl(self.apply_pin(item), binding, animation)
    }
    fn offset(&self) -> usize {
        self.get_byte_offset()
    }

    #[allow(unsafe_code)]
    unsafe fn link_two_ways(&self, item: Pin<&Item>, property2: *const ()) {
        let p1 = self.apply_pin(item);
        // Safety: that's the invariant of this function
        let p2 = Pin::new_unchecked((property2 as *const crate::Property<T>).as_ref().unwrap());
        crate::Property::link_two_way(p1, p2);
    }
}

pub trait CallbackInfo<Item, Value> {
    fn call(&self, item: Pin<&Item>, args: &[Value]) -> Result<Value, ()>;
    fn set_handler(
        &self,
        item: Pin<&Item>,
        handler: Box<dyn Fn(&[Value]) -> Value>,
    ) -> Result<(), ()>;
}

impl<Item, Value: Default + 'static, Ret: Default> CallbackInfo<Item, Value>
    for FieldOffset<Item, crate::Callback<(), Ret>>
where
    Value: TryInto<Ret>,
    Ret: TryInto<Value>,
{
    fn call(&self, item: Pin<&Item>, _args: &[Value]) -> Result<Value, ()> {
        self.apply_pin(item).call(&()).try_into().map_err(|_| ())
    }

    fn set_handler(
        &self,
        item: Pin<&Item>,
        handler: Box<dyn Fn(&[Value]) -> Value>,
    ) -> Result<(), ()> {
        self.apply_pin(item).set_handler(move |()| handler(&[]).try_into().ok().unwrap());
        Ok(())
    }
}

impl<Item, Value: Clone + Default + 'static, T: Clone, Ret: Default> CallbackInfo<Item, Value>
    for FieldOffset<Item, crate::Callback<(T,), Ret>>
where
    Value: TryInto<T>,
    T: TryInto<Value>,
    Value: TryInto<Ret>,
    Ret: TryInto<Value>,
{
    fn call(&self, item: Pin<&Item>, args: &[Value]) -> Result<Value, ()> {
        let value = args.first().ok_or(())?;
        let value = value.clone().try_into().map_err(|_| ())?;
        self.apply_pin(item).call(&(value,)).try_into().map_err(|_| ())
    }

    fn set_handler(
        &self,
        item: Pin<&Item>,
        handler: Box<dyn Fn(&[Value]) -> Value>,
    ) -> Result<(), ()> {
        self.apply_pin(item).set_handler(move |(val,)| {
            let val: Value = val.clone().try_into().ok().unwrap();
            handler(&[val]).try_into().ok().unwrap()
        });
        Ok(())
    }
}

pub trait FieldInfo<Item, Value> {
    fn set_field(&self, item: &mut Item, value: Value) -> Result<(), ()>;
}

impl<Item, T, Value: 'static> FieldInfo<Item, Value> for FieldOffset<Item, T>
where
    Value: TryInto<T>,
    T: TryInto<Value>,
{
    fn set_field(&self, item: &mut Item, value: Value) -> Result<(), ()> {
        *self.apply_mut(item) = value.try_into().map_err(|_| ())?;
        Ok(())
    }
}

pub trait BuiltinItem: Sized {
    fn name() -> &'static str;
    fn properties<Value: ValueType>() -> Vec<(&'static str, &'static dyn PropertyInfo<Self, Value>)>;
    fn fields<Value: ValueType>() -> Vec<(&'static str, &'static dyn FieldInfo<Self, Value>)>;
    fn callbacks<Value: ValueType>() -> Vec<(&'static str, &'static dyn CallbackInfo<Self, Value>)>;
}

/// Trait implemented by builtin globals
pub trait BuiltinGlobal: BuiltinItem {
    fn new() -> Pin<Rc<Self>>;
}
