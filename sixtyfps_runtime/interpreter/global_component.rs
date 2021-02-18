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
use std::rc::Rc;

use sixtyfps_compilerlib::{langtype::Type, object_tree::Component};
use sixtyfps_corelib::{rtti, Callback, Property};

use crate::eval;

pub trait GlobalComponent {
    fn call_callback(self: Pin<&Self>, _callback_name: &str, _args: &[eval::Value]) -> eval::Value {
        todo!("call callback")
    }

    fn set_property(self: Pin<&Self>, prop_name: &str, value: eval::Value);
    fn get_property(self: Pin<&Self>, prop_name: &str) -> eval::Value;
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
    properties: HashMap<String, Pin<Box<Property<eval::Value>>>>,
    callbacks: HashMap<String, Pin<Box<Callback<[eval::Value]>>>>,
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
                instance.properties.insert(name.clone(), Box::pin(Default::default()));
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
    fn set_property(self: Pin<&Self>, prop_name: &str, value: eval::Value) {
        self.properties[prop_name].as_ref().set(value);
    }

    fn get_property(self: Pin<&Self>, prop_name: &str) -> eval::Value {
        self.properties[prop_name].as_ref().get()
    }
}

impl<T: rtti::BuiltinItem + 'static> GlobalComponent for T {
    fn set_property(self: Pin<&Self>, prop_name: &str, value: crate::Value) {
        let prop = Self::properties().into_iter().find(|(k, _)| *k == prop_name).unwrap().1;
        prop.set(self, value, None).unwrap()
    }

    fn get_property(self: Pin<&Self>, prop_name: &str) -> crate::Value {
        let prop = Self::properties().into_iter().find(|(k, _)| *k == prop_name).unwrap().1;
        prop.get(self).unwrap()
    }
}
