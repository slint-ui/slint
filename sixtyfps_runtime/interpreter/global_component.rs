/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use core::pin::Pin;
use std::collections::HashMap;
use std::iter::FromIterator;
use std::rc::Rc;

use crate::api::{Struct, Value};
use crate::eval;
use sixtyfps_compilerlib::{langtype::Type, object_tree::Component};
use sixtyfps_corelib::{rtti, Callback, Property};

pub trait GlobalComponent {
    fn invoke_callback(self: Pin<&Self>, _callback_name: &str, _args: &[Value]) -> Value {
        todo!("call callback")
    }

    fn set_property(self: Pin<&Self>, prop_name: &str, value: Value);
    fn get_property(self: Pin<&Self>, prop_name: &str) -> Value;
}

pub fn instantiate(component: &Rc<Component>) -> Pin<Rc<dyn GlobalComponent>> {
    debug_assert!(component.is_global());
    match &component.root_element.borrow().base_type {
        Type::Void => GlobalComponentInstance::instantiate(component),
        Type::Builtin(b) => {
            trait Helper {
                fn instantiate(name: &str) -> Pin<Rc<dyn GlobalComponent>> {
                    panic!("Cannot find native global {}", name)
                }
            }
            impl Helper for () {}
            impl<T: rtti::BuiltinItem + Default + 'static, Next: Helper> Helper for (T, Next) {
                fn instantiate(name: &str) -> Pin<Rc<dyn GlobalComponent>> {
                    if name == T::name() {
                        Rc::pin(T::default())
                    } else {
                        Next::instantiate(name)
                    }
                }
            }
            sixtyfps_rendering_backend_default::NativeGlobals::instantiate(
                b.native_class.class_name.as_ref(),
            )
        }
        _ => unreachable!(),
    }
}

/// For the global components, we don't use the dynamic_type optimisation,
/// and we don't try to to optimize the property to their real type
pub struct GlobalComponentInstance {
    properties: HashMap<String, Pin<Box<Property<Value>>>>,
    callbacks: HashMap<String, Pin<Box<Callback<[Value]>>>>,
    pub component: Rc<Component>,
}
impl Unpin for GlobalComponentInstance {}

impl GlobalComponentInstance {
    /// Create a new instance of a GlobalComponent, for the given component
    fn instantiate(component: &Rc<Component>) -> Pin<Rc<Self>> {
        assert!(component.is_global());

        let mut instance = Self {
            properties: Default::default(),
            callbacks: Default::default(),
            component: component.clone(),
        };
        for (name, decl) in &component.root_element.borrow().property_declarations {
            if matches!(decl.property_type, Type::Callback { .. }) {
                instance.callbacks.insert(name.clone(), Box::pin(Default::default()));
            } else {
                instance.properties.insert(
                    name.clone(),
                    Box::pin(Property::new(default_value_for_type(&decl.property_type))),
                );
            }
        }
        let rc = Rc::pin(instance);
        for (k, expr) in &component.root_element.borrow().bindings {
            if expr.expression.is_constant() {
                rc.properties[k].as_ref().set(eval::eval_expression(
                    &expr.expression,
                    &mut eval::EvalLocalContext::from_global(&(rc.clone() as _)),
                ));
            } else {
                let wk = Rc::<Self>::downgrade(&Pin::into_inner(rc.clone()));
                let e = expr.expression.clone();
                rc.properties[k].as_ref().set_binding(move || {
                    eval::eval_expression(
                        &e,
                        &mut eval::EvalLocalContext::from_global(
                            &(Pin::<Rc<Self>>::new(wk.upgrade().unwrap()) as _),
                        ),
                    )
                });
            }
        }
        rc
    }
}
impl GlobalComponent for GlobalComponentInstance {
    fn set_property(self: Pin<&Self>, prop_name: &str, value: Value) {
        self.properties[prop_name].as_ref().set(value);
    }

    fn get_property(self: Pin<&Self>, prop_name: &str) -> Value {
        self.properties[prop_name].as_ref().get()
    }
}

impl<T: rtti::BuiltinItem + 'static> GlobalComponent for T {
    fn set_property(self: Pin<&Self>, prop_name: &str, value: Value) {
        let prop = Self::properties().into_iter().find(|(k, _)| *k == prop_name).unwrap().1;
        prop.set(self, value, None).unwrap()
    }

    fn get_property(self: Pin<&Self>, prop_name: &str) -> Value {
        let prop = Self::properties().into_iter().find(|(k, _)| *k == prop_name).unwrap().1;
        prop.get(self).unwrap()
    }
}

/// Create a value suitable as the default value of a given type
fn default_value_for_type(ty: &Type) -> Value {
    match ty {
        Type::Float32 | Type::Int32 => Value::Number(0.),
        Type::String => Value::String(Default::default()),
        Type::Color | Type::Brush => Value::Brush(Default::default()),
        Type::Duration | Type::Angle | Type::PhysicalLength | Type::LogicalLength => {
            Value::Number(0.)
        }
        Type::Image => Value::Image(Default::default()),
        Type::Bool => Value::Bool(false),
        Type::Callback { .. } => Value::Void,
        Type::Struct { fields, .. } => Value::Struct(Struct::from_iter(
            fields.iter().map(|(n, t)| (n.clone(), default_value_for_type(t))),
        )),
        Type::Array(_) => Value::Array(Default::default()),
        Type::Percent => Value::Number(0.),
        Type::Enumeration(e) => {
            Value::EnumerationValue(e.name.clone(), e.values.get(e.default_value).unwrap().clone())
        }
        Type::Easing => Value::EasingCurve(Default::default()),
        Type::Void | Type::Invalid => Value::Void,
        Type::Model => Value::Void,
        Type::UnitProduct(_) => Value::Number(0.),
        Type::PathElements => Value::PathElements(Default::default()),
        Type::LayoutCache => Value::LayoutCache(Default::default()),
        Type::ElementReference
        | Type::Builtin(_)
        | Type::Component(_)
        | Type::Native(_)
        | Type::Function { .. } => {
            panic!("There can't be such property")
        }
    }
}
