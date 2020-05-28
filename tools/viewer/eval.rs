use corelib::{abi::datastructures::ComponentRefMut, EvaluationContext, SharedString};
use sixtyfps_compiler::expression_tree::Expression;
use sixtyfps_compiler::typeregister::Type;

#[derive(Debug)]
pub enum Value {
    Void,
    Number(f64),
    String(SharedString),
}

pub fn eval_expression(
    e: &Expression,
    ctx: &crate::ComponentImpl,
    component_ref: &ComponentRefMut,
) -> Value {
    match e {
        Expression::Invalid => panic!("invalid expression while evaluating"),
        Expression::Uncompiled(_) => panic!("uncompiled expression while evaluating"),
        Expression::StringLiteral(s) => Value::String(s.as_str().into()),
        Expression::NumberLiteral(n) => Value::Number(*n),
        Expression::SignalReference { .. } => panic!("signal in expression"),
        Expression::PropertyReference { component, element, name } => {
            let eval_context = EvaluationContext { component: component_ref.borrow() };
            let component = component.upgrade().unwrap();
            let element = element.upgrade().unwrap();
            if element.borrow().id == component.root_element.borrow().id {
                if let Some(x) = ctx.custom_properties.get(name) {
                    return unsafe { (x.get)(ctx.mem.offset(x.offset as isize), &eval_context) };
                }
            };
            let item = &ctx.items[element.borrow().id.as_str()];
            let (offset, _set, get) = item.rtti.properties[name.as_str()];
            unsafe { get(ctx.mem.offset(offset as isize), &eval_context) }
        }
        Expression::Cast { from, to } => {
            let v = eval_expression(&*from, ctx, component_ref);
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
                v = eval_expression(e, ctx, component_ref);
            }
            v
        }
        Expression::FunctionCall { .. } => todo!("function call"),
        Expression::SelfAssignement { lhs, rhs, op } => match &**lhs {
            Expression::PropertyReference { component, element, name, .. } => {
                let eval = |lhs| {
                    let rhs = eval_expression(&**rhs, ctx, component_ref);
                    match (lhs, rhs, op) {
                        (Value::Number(a), Value::Number(b), '+') => Value::Number(a + b),
                        (Value::Number(a), Value::Number(b), '-') => Value::Number(a + b),
                        (Value::Number(a), Value::Number(b), '/') => Value::Number(a + b),
                        (Value::Number(a), Value::Number(b), '*') => Value::Number(a + b),
                        (lhs, rhs, op) => panic!("unsupported {:?} {} {:?}", lhs, op, rhs),
                    }
                };

                let eval_context = EvaluationContext { component: component_ref.borrow() };
                let component = component.upgrade().unwrap();
                let element = element.upgrade().unwrap();
                if element.borrow().id == component.root_element.borrow().id {
                    if let Some(x) = ctx.custom_properties.get(name) {
                        unsafe {
                            let p = ctx.mem.offset(x.offset as isize);
                            (x.set)(p, eval((x.get)(p, &eval_context)));
                        }
                        return Value::Void;
                    }
                };
                let item = &ctx.items[element.borrow().id.as_str()];
                let (offset, set, get) = item.rtti.properties[name.as_str()];
                unsafe {
                    let p = ctx.mem.offset(offset as isize);
                    set(p, eval(get(p, &eval_context)));
                }
                Value::Void
            }
            _ => panic!("typechecking should make sure this was a PropertyReference"),
        },
    }
}
