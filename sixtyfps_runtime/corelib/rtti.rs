/*!
 This module enable runtime type information for the builtin items and
 property so that the viewer can handle them
*/

pub use const_field_offset::FieldOffset;
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

    /// The offset of the property in the item.
    /// The use of this is unsafe
    fn offset(&self) -> usize;
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

pub trait BuiltinItem: Sized {
    fn name() -> &'static str;
    fn properties<Value: ValueType>() -> Vec<(&'static str, &'static dyn PropertyInfo<Self, Value>)>;
    fn signals() -> Vec<(&'static str, FieldOffset<Self, crate::Signal<()>>)>;
}
