use corelib::SharedString;
use sixtyfps_compiler::expression_tree::Expression;
use sixtyfps_compiler::typeregister::Type;

#[derive(Debug)]
pub enum Value {
    Number(f64),
    String(SharedString),
}

pub fn eval_expression(e: &Expression, ctx: &crate::ComponentImpl) -> Value {
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
                if let Some(x) = ctx.custom_properties.get(name) {
                    return unsafe { (x.get)(ctx.mem.offset(x.offset as isize)) };
                }
            };
            let item = &ctx.items[element.borrow().id.as_str()];
            let (offset, _set, get) = item.rtti.properties[name.as_str()];
            unsafe { get(ctx.mem.offset(offset as isize)) }
        }
        Expression::Cast { from, to } => {
            let v = eval_expression(&*from, ctx);
            match (v, to) {
                (Value::Number(n), Type::Int32) => Value::Number(n.round()),
                (Value::Number(n), Type::String) => {
                    Value::String(SharedString::from(format!("{}", n).as_str()))
                }
                (v, _) => v,
            }
        }
    }
}
