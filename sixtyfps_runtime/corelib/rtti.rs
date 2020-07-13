/*!
 This module enable runtime type information for the builtin items and
 property so that the viewer can handle them
*/

pub type FieldOffset<T, U> = const_field_offset::FieldOffset<T, U, const_field_offset::PinnedFlag>;
use core::convert::{TryFrom, TryInto};
use core::pin::Pin;

macro_rules! declare_ValueType {
    ($($ty:ty),*) => {
        pub trait ValueType: 'static $(+ TryInto<$ty> + TryFrom<$ty>)* {}
    };
}
declare_ValueType![
    bool,
    u32,
    u64,
    i32,
    i64,
    f32,
    f64,
    crate::SharedString,
    crate::Resource,
    crate::Color,
    crate::PathData
];

pub trait PropertyInfo<Item, Value> {
    fn get(&self, item: Pin<&Item>) -> Result<Value, ()>;
    fn set(
        &self,
        item: Pin<&Item>,
        value: Value,
        animation: Option<crate::abi::primitives::PropertyAnimation>,
    ) -> Result<(), ()>;
    fn set_binding(
        &self,
        item: Pin<&Item>,
        binding: Box<dyn Fn() -> Value>,
        animation: Option<crate::abi::primitives::PropertyAnimation>,
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
}

impl<Item, T: Clone, Value: 'static> PropertyInfo<Item, Value>
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
        animation: Option<crate::abi::primitives::PropertyAnimation>,
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
        animation: Option<crate::abi::primitives::PropertyAnimation>,
    ) -> Result<(), ()> {
        if animation.is_some() {
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
}

/// Wraper for a field offset that optonally implement PropertyInfo and uses
/// the auto deref specialization trick
#[derive(derive_more::Deref)]
pub struct MaybeAnimatedPropertyInfoWrapper<T, U>(pub FieldOffset<T, U>);

impl<Item, T: Clone, Value: 'static> PropertyInfo<Item, Value>
    for MaybeAnimatedPropertyInfoWrapper<Item, crate::Property<T>>
where
    Value: TryInto<T>,
    T: TryInto<Value>,
    T: crate::abi::properties::InterpolatedPropertyValue,
{
    fn get(&self, item: Pin<&Item>) -> Result<Value, ()> {
        self.0.get(item)
    }
    fn set(
        &self,
        item: Pin<&Item>,
        value: Value,
        animation: Option<crate::abi::primitives::PropertyAnimation>,
    ) -> Result<(), ()> {
        if let Some(animation) = &animation {
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
        animation: Option<crate::abi::primitives::PropertyAnimation>,
    ) -> Result<(), ()> {
        if let Some(animation) = &animation {
            self.apply_pin(item).set_animated_binding(
                move || {
                    binding().try_into().map_err(|_| ()).expect("binding was of the wrong type")
                },
                animation,
            );
            Ok(())
        } else {
            self.0.set_binding(item, binding, None)
        }
    }
    fn offset(&self) -> usize {
        self.get_byte_offset()
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
    fn signals() -> Vec<(&'static str, FieldOffset<Self, crate::Signal<()>>)>;
}
