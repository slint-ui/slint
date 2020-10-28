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
use sixtyfps_corelib::{Property, Signal};

use crate::eval;

/// For the global component, we don't use the dynamic_type optimisation, and we don't try to to optimize the property to their real type
pub struct GlobalComponent {
    properties: HashMap<String, Pin<Box<Property<eval::Value>>>>,
    signals: HashMap<String, Pin<Box<Signal<[eval::Value]>>>>,
    pub component: Rc<Component>,
}

impl GlobalComponent {
    /// Create a new instance of a GlobalComponent, for the given component
    pub fn instantiate(component: &Rc<Component>) -> Rc<Self> {
        assert!(component.is_global());

        let mut instance = GlobalComponent {
            properties: Default::default(),
            signals: Default::default(),
            component: component.clone(),
        };
        for (name, decl) in &component.root_element.borrow().property_declarations {
            if matches!(decl.property_type, Type::Signal{..}) {
                instance.signals.insert(name.clone(), Box::pin(Default::default()));
            } else {
                instance.properties.insert(name.clone(), Box::pin(Default::default()));
            }
        }
        let rc = Rc::new(instance);
        for (k, expr) in &component.root_element.borrow().bindings {
            if expr.expression.is_constant() {
                rc.properties[k].as_ref().set(eval::eval_expression(
                    &expr.expression,
                    &mut eval::EvalLocalContext::from_global(&rc),
                ));
            } else {
                let wk = Rc::downgrade(&rc);
                let e = expr.expression.clone();
                rc.properties[k].as_ref().set_binding(move || {
                    eval::eval_expression(
                        &e,
                        &mut eval::EvalLocalContext::from_global(&wk.upgrade().unwrap()),
                    )
                });
            }
        }
        rc
    }

    pub fn emit_signal(self: &Rc<Self>, _signal_name: &str, _args: &[eval::Value]) {
        todo!("emit signal")
    }

    pub fn set_property(self: &Rc<Self>, prop_name: &str, value: eval::Value) {
        self.properties[prop_name].as_ref().set(value);
    }

    pub fn get_property(self: &Rc<Self>, prop_name: &str) -> eval::Value {
        self.properties[prop_name].as_ref().get()
    }
}
