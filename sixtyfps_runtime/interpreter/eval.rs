use sixtyfps_compilerlib::expression_tree::Expression;
use sixtyfps_compilerlib::typeregister::Type;
use sixtyfps_corelib as corelib;
use sixtyfps_corelib::{abi::datastructures::ItemRef, EvaluationContext, Resource, SharedString};
use std::convert::{TryFrom, TryInto};

pub trait ErasedPropertyInfo {
    fn get(&self, item: ItemRef, context: &EvaluationContext) -> Value;
    fn set(&self, item: ItemRef, value: Value);
    fn set_binding(&self, item: ItemRef, binding: Box<dyn Fn(&EvaluationContext) -> Value>);
}

impl<Item: vtable::HasStaticVTable<corelib::abi::datastructures::ItemVTable>> ErasedPropertyInfo
    for &'static dyn corelib::rtti::PropertyInfo<Item, Value>
{
    fn get(&self, item: ItemRef, context: &EvaluationContext) -> Value {
        (*self).get(item.downcast().unwrap(), context).unwrap()
    }
    fn set(&self, item: ItemRef, value: Value) {
        (*self).set(item.downcast().unwrap(), value).unwrap()
    }
    fn set_binding(&self, item: ItemRef, binding: Box<dyn Fn(&EvaluationContext) -> Value>) {
        (*self).set_binding(item.downcast().unwrap(), binding);
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Void,
    Number(f64),
    String(SharedString),
    Bool(bool),
    Resource(Resource),
}

impl corelib::rtti::ValueType for Value {}
macro_rules! declare_value_conversion {
    ( $value:ident => [$($ty:ty),*] ) => {
        $(
            impl TryFrom<$ty> for Value {
                type Error = ();
                fn try_from(v: $ty) -> Result<Self, ()> {
                    //Ok(Value::$value(v.try_into().map_err(|_|())?))
                    Ok(Value::$value(v as _))
                }
            }
            impl TryInto<$ty> for Value {
                type Error = ();
                fn try_into(self) -> Result<$ty, ()> {
                    match self {
                        //Self::$value(x) => x.try_into().map_err(|_|()),
                        Self::$value(x) => Ok(x as _),
                        _ => Err(())
                    }
                }
            }
        )*
    };
}
declare_value_conversion!(Number => [u32, u64, i32, i64, f32, f64] );
declare_value_conversion!(String => [SharedString] );
declare_value_conversion!(Bool => [bool] );
declare_value_conversion!(Resource => [Resource] );

pub fn eval_expression(
    e: &Expression,
    ctx: &crate::ComponentImpl,
    eval_context: &corelib::EvaluationContext,
) -> Value {
    match e {
        Expression::Invalid => panic!("invalid expression while evaluating"),
        Expression::Uncompiled(_) => panic!("uncompiled expression while evaluating"),
        Expression::StringLiteral(s) => Value::String(s.as_str().into()),
        Expression::NumberLiteral(n) => Value::Number(*n),
        Expression::SignalReference { .. } => panic!("signal in expression"),
        Expression::PropertyReference { component, element, name } => {
            let component = component.upgrade().unwrap();
            let element = element.upgrade().unwrap();
            if element.borrow().id == component.root_element.borrow().id {
                if let Some(x) = ctx.component_type.custom_properties.get(name) {
                    return unsafe {
                        x.prop.get(&*ctx.mem.offset(x.offset as isize), &eval_context).unwrap()
                    };
                }
            };
            let item_info = &ctx.component_type.items[element.borrow().id.as_str()];
            let item = unsafe { item_info.item_from_component(ctx.mem) };
            item_info.rtti.properties[name.as_str()].get(item, &eval_context)
        }
        Expression::Cast { from, to } => {
            let v = eval_expression(&*from, ctx, eval_context);
            match (v, to) {
                (Value::Number(n), Type::Int32) => Value::Number(n.round()),
                (Value::Number(n), Type::String) => {
                    Value::String(SharedString::from(format!("{}", n).as_str()))
                }
                (v, _) => v,
            }
        }
        Expression::CodeBlock(sub) => {
            let mut v = Value::Void;
            for e in sub {
                v = eval_expression(e, ctx, eval_context);
            }
            v
        }
        Expression::FunctionCall { function, .. } => {
            if let Expression::SignalReference { component: _, element, name } = &**function {
                let element = element.upgrade().unwrap();
                let item_info = &ctx.component_type.items[element.borrow().id.as_str()];
                let item = unsafe { item_info.item_from_component(ctx.mem) };
                let signal = unsafe {
                    &mut *(item_info
                        .rtti
                        .signals
                        .get(name.as_str())
                        .map(|o| item.as_ptr().add(*o) as *mut u8)
                        .or_else(|| {
                            ctx.component_type
                                .custom_signals
                                .get(name.as_str())
                                .map(|o| ctx.mem.add(*o))
                        })
                        .unwrap_or_else(|| panic!("unkown signal {}", name))
                        as *mut corelib::Signal<()>)
                };
                signal.emit(eval_context, ());
                Value::Void
            } else {
                panic!("call of something not a signal")
            }
        }
        Expression::SelfAssignement { lhs, rhs, op } => match &**lhs {
            Expression::PropertyReference { component, element, name, .. } => {
                let eval = |lhs| {
                    let rhs = eval_expression(&**rhs, ctx, eval_context);
                    match (lhs, rhs, op) {
                        (Value::Number(a), Value::Number(b), '+') => Value::Number(a + b),
                        (Value::Number(a), Value::Number(b), '-') => Value::Number(a - b),
                        (Value::Number(a), Value::Number(b), '/') => Value::Number(a / b),
                        (Value::Number(a), Value::Number(b), '*') => Value::Number(a * b),
                        (lhs, rhs, op) => panic!("unsupported {:?} {} {:?}", lhs, op, rhs),
                    }
                };

                let component = component.upgrade().unwrap();
                let element = element.upgrade().unwrap();
                if element.borrow().id == component.root_element.borrow().id {
                    if let Some(x) = ctx.component_type.custom_properties.get(name) {
                        unsafe {
                            let p = &*ctx.mem.offset(x.offset as isize);
                            x.prop.set(p, eval(x.prop.get(p, &eval_context).unwrap())).unwrap();
                        }
                        return Value::Void;
                    }
                };
                let item_info = &ctx.component_type.items[element.borrow().id.as_str()];
                let item = unsafe { item_info.item_from_component(ctx.mem) };
                let p = &item_info.rtti.properties[name.as_str()];
                p.set(item, eval(p.get(item, &eval_context)));
                Value::Void
            }
            _ => panic!("typechecking should make sure this was a PropertyReference"),
        },
        Expression::ResourceReference { absolute_source_path } => {
            Value::Resource(Resource::AbsoluteFilePath(absolute_source_path.as_str().into()))
        }
    }
}
