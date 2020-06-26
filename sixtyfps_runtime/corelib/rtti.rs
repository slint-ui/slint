/*!
 This module enable runtime type information for the builtin items and
 property so that the viewer can handle them
*/

pub type FieldOffset<T, U> = const_field_offset::FieldOffset<T, U, const_field_offset::PinnedFlag>;
use std::convert::{TryFrom, TryInto};

macro_rules! declare_ValueType {
    ($($ty:ty),*) => {
        pub trait ValueType: 'static $(+ TryInto<$ty> + TryFrom<$ty>)* {}
    };
}
declare_ValueType![bool, u32, u64, i32, i64, f32, f64, crate::SharedString, crate::Resource];

pub trait PropertyInfo<Item, Value> {
    fn get(&self, item: &Item, context: &crate::EvaluationContext) -> Result<Value, ()>;
    fn set(&self, item: &Item, value: Value) -> Result<(), ()>;
    fn set_binding(&self, item: &Item, binding: Box<dyn Fn(&crate::EvaluationContext) -> Value>);

    fn set_animated_value(
        &self,
        item: &Item,
        value: Value,
        animation: &crate::abi::primitives::PropertyAnimation,
    ) -> Result<(), ()>;
    fn set_animated_binding(
        &self,
        item: &Item,
        binding: Box<dyn Fn(&crate::EvaluationContext) -> Value>,
        animation: &crate::abi::primitives::PropertyAnimation,
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
    fn get(&self, item: &Item, context: &crate::EvaluationContext) -> Result<Value, ()> {
        self.apply(item).get(context).try_into().map_err(|_| ())
    }
    fn set(&self, item: &Item, value: Value) -> Result<(), ()> {
        self.apply(item).set(value.try_into().map_err(|_| ())?);
        Ok(())
    }
    fn set_binding(&self, item: &Item, binding: Box<dyn Fn(&crate::EvaluationContext) -> Value>) {
        self.apply(item).set_binding(move |context| {
            binding(context).try_into().map_err(|_| ()).expect("binding was of the wrong type")
        });
    }
    fn set_animated_value(
        &self,
        _item: &Item,
        _value: Value,
        _animation: &crate::abi::primitives::PropertyAnimation,
    ) -> Result<(), ()> {
        Err(())
    }
    fn set_animated_binding(
        &self,
        _item: &Item,
        _binding: Box<dyn Fn(&crate::EvaluationContext) -> Value>,
        _animation: &crate::abi::primitives::PropertyAnimation,
    ) -> Result<(), ()> {
        Err(())
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
    fn get(&self, item: &Item, context: &crate::EvaluationContext) -> Result<Value, ()> {
        self.0.get(item, context)
    }
    fn set(&self, item: &Item, value: Value) -> Result<(), ()> {
        self.0.set(item, value)
    }
    fn set_binding(&self, item: &Item, binding: Box<dyn Fn(&crate::EvaluationContext) -> Value>) {
        self.0.set_binding(item, binding)
    }
    fn set_animated_value(
        &self,
        item: &Item,
        value: Value,
        animation: &crate::abi::primitives::PropertyAnimation,
    ) -> Result<(), ()> {
        self.apply(item).set_animated_value(value.try_into().map_err(|_| ())?, animation);
        Ok(())
    }
    fn set_animated_binding(
        &self,
        item: &Item,
        binding: Box<dyn Fn(&crate::EvaluationContext) -> Value>,
        animation: &crate::abi::primitives::PropertyAnimation,
    ) -> Result<(), ()> {
        self.apply(item).set_animated_binding(
            move |context| {
                binding(context).try_into().map_err(|_| ()).expect("binding was of the wrong type")
            },
            animation,
        );
        Ok(())
    }
    fn offset(&self) -> usize {
        self.get_byte_offset()
    }
}

pub trait BuiltinItem: Sized {
    fn name() -> &'static str;
    fn properties<Value: ValueType>() -> Vec<(&'static str, &'static dyn PropertyInfo<Self, Value>)>;
    fn signals() -> Vec<(&'static str, FieldOffset<Self, crate::Signal<()>>)>;
}
