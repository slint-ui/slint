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

pub enum PropertyInfoOption<'a, Item, Value> {
    SimpleProperty(&'a dyn PropertyInfo<Item, Value>),
    AnimatedProperty(&'a dyn AnimatedPropertyInfo<Item, Value>),
}

impl<'a, Item, Value> PropertyInfoOption<'a, Item, Value> {
    pub fn get(&self, item: &Item, context: &crate::EvaluationContext) -> Result<Value, ()> {
        match self {
            PropertyInfoOption::SimpleProperty(pi) => pi.get(item, context),
            PropertyInfoOption::AnimatedProperty(pi) => pi.get(item, context),
        }
    }
    pub fn set(&self, item: &Item, value: Value) -> Result<(), ()> {
        match self {
            PropertyInfoOption::SimpleProperty(pi) => pi.set(item, value),
            PropertyInfoOption::AnimatedProperty(pi) => pi.set(item, value),
        }
    }
    pub fn set_binding(
        &self,
        item: &Item,
        binding: Box<dyn Fn(&crate::EvaluationContext) -> Value>,
    ) {
        match self {
            PropertyInfoOption::SimpleProperty(pi) => pi.set_binding(item, binding),
            PropertyInfoOption::AnimatedProperty(pi) => pi.set_binding(item, binding),
        }
    }
    pub fn set_animated_value(
        &self,
        item: &Item,
        value: Value,
        animation: &crate::abi::primitives::PropertyAnimation,
    ) -> Result<(), ()> {
        match self {
            PropertyInfoOption::SimpleProperty(_) => Err(()),
            PropertyInfoOption::AnimatedProperty(pi) => {
                pi.set_animated_value(item, value, animation)
            }
        }
    }
    pub fn set_animated_binding(
        &self,
        item: &Item,
        binding: Box<dyn Fn(&crate::EvaluationContext) -> Value>,
        animation: &crate::abi::primitives::PropertyAnimation,
    ) -> bool {
        match self {
            PropertyInfoOption::SimpleProperty(_) => false,
            PropertyInfoOption::AnimatedProperty(pi) => {
                pi.set_animated_binding(item, binding, animation);
                true
            }
        }
    }
    /// The offset of the property in the item.
    /// The use of this is unsafe
    pub fn offset(&self) -> usize {
        match self {
            PropertyInfoOption::SimpleProperty(pi) => pi.offset(),
            PropertyInfoOption::AnimatedProperty(pi) => pi.offset(),
        }
    }
}

pub trait PropertyInfo<Item, Value> {
    fn get(&self, item: &Item, context: &crate::EvaluationContext) -> Result<Value, ()>;
    fn set(&self, item: &Item, value: Value) -> Result<(), ()>;
    fn set_binding(&self, item: &Item, binding: Box<dyn Fn(&crate::EvaluationContext) -> Value>);

    /// The offset of the property in the item.
    /// The use of this is unsafe
    fn offset(&self) -> usize;
}

pub trait AnimatedPropertyInfo<Item, Value>: PropertyInfo<Item, Value> {
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
    );
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
    fn offset(&self) -> usize {
        self.get_byte_offset()
    }
}

impl<Item, T: Clone, Value: 'static> AnimatedPropertyInfo<Item, Value>
    for FieldOffset<Item, crate::Property<T>>
where
    Value: TryInto<T>,
    T: TryInto<Value>,
    T: crate::abi::properties::InterpolatedPropertyValue,
{
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
    ) {
        self.apply(item).set_animated_binding(
            move |context| {
                binding(context).try_into().map_err(|_| ()).expect("binding was of the wrong type")
            },
            animation,
        );
    }
}

pub trait BuiltinItem: Sized {
    fn name() -> &'static str;
    fn properties<Value: ValueType>(
    ) -> Vec<(&'static str, PropertyInfoOption<'static, Self, Value>)>;
    fn signals() -> Vec<(&'static str, FieldOffset<Self, crate::Signal<()>>)>;
}
