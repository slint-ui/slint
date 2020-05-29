/*!
 This module enable runtime type information for the builtin items and
 property so that the viewer can handle them
*/

use const_field_offset::FieldOffset;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};

/*
pub struct TypeInfo {
    /// required allignement
    align: usize,
    /// Size in byte of the type
    size: usize,
    construct_in_place: Option<unsafe fn(*mut u8)>,
    drop_in_place: Option<unsafe fn(*mut u8)>,
}*/

macro_rules! declare_ValueType {
    ($($ty:ty),*) => {
        pub trait ValueType: 'static $(+ TryInto<$ty> + TryFrom<$ty>)* {}
    };
}
declare_ValueType![bool, u32, u64, i32, i64, f32, f64, crate::SharedString];

pub trait PropertyInfo<Item, Value> {
    fn get(&self, item: &Item, context: &crate::EvaluationContext) -> Result<Value, ()>;
    fn set(&self, item: &Item, value: Value) -> Result<(), ()>;
    fn set_binding(&self, item: &Item, binding: Box<dyn Fn(&crate::EvaluationContext) -> Value>);
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
}

pub trait BuiltinItem {
    fn name() -> &'static str;
    fn properties<Value: ValueType>(
    ) -> HashMap<&'static str, &'static dyn PropertyInfo<Self, Value>>;
}
