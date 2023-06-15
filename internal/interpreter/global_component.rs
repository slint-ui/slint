// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

use core::pin::Pin;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use crate::api::Value;
use crate::dynamic_component::{ErasedComponentBox, ErasedComponentDescription};
use crate::SetPropertyError;
use i_slint_compiler::langtype::ElementType;
use i_slint_compiler::namedreference::NamedReference;
use i_slint_compiler::object_tree::Component;
use i_slint_compiler::object_tree::PropertyDeclaration;
use i_slint_core::component::ComponentVTable;
use i_slint_core::rtti;

pub type GlobalStorage = HashMap<String, Pin<Rc<dyn GlobalComponent>>>;

pub enum CompiledGlobal {
    Builtin {
        name: String,
        element: Rc<i_slint_compiler::langtype::BuiltinElement>,
        // dummy needed for iterator accessor
        public_properties: BTreeMap<String, PropertyDeclaration>,
    },
    Component {
        component: ErasedComponentDescription,
        public_properties: BTreeMap<String, PropertyDeclaration>,
    },
}

impl CompiledGlobal {
    pub fn names(&self) -> Vec<String> {
        match self {
            CompiledGlobal::Builtin { name, .. } => vec![name.clone()],
            CompiledGlobal::Component { component, .. } => {
                generativity::make_guard!(guard);
                let component = component.unerase(guard);
                let mut names = component.original.global_aliases();
                names.push(component.original.root_element.borrow().original_name());
                names
            }
        }
    }

    pub fn visible_in_public_api(&self) -> bool {
        match self {
            CompiledGlobal::Builtin { .. } => false,
            CompiledGlobal::Component { component, .. } => {
                generativity::make_guard!(guard);
                let component = component.unerase(guard);
                component.original.visible_in_public_api()
            }
        }
    }

    pub fn public_properties(&self) -> impl Iterator<Item = (&String, &PropertyDeclaration)> + '_ {
        match self {
            CompiledGlobal::Builtin { public_properties, .. } => public_properties.iter(),
            CompiledGlobal::Component { public_properties, .. } => public_properties.iter(),
        }
    }

    pub fn extend_public_properties(
        &mut self,
        iter: impl IntoIterator<Item = (String, PropertyDeclaration)>,
    ) {
        match self {
            CompiledGlobal::Builtin { public_properties, .. } => public_properties.extend(iter),
            CompiledGlobal::Component { public_properties, .. } => public_properties.extend(iter),
        }
    }
}

pub trait GlobalComponent {
    fn invoke_callback(self: Pin<&Self>, callback_name: &str, args: &[Value]) -> Result<Value, ()>;

    fn set_callback_handler(
        self: Pin<&Self>,
        callback_name: &str,
        handler: Box<dyn Fn(&[Value]) -> Value>,
    ) -> Result<(), ()>;

    fn set_property(
        self: Pin<&Self>,
        prop_name: &str,
        value: Value,
    ) -> Result<(), SetPropertyError>;
    fn get_property(self: Pin<&Self>, prop_name: &str) -> Result<Value, ()>;

    fn get_property_ptr(self: Pin<&Self>, prop_name: &str) -> *const ();

    fn eval_function(self: Pin<&Self>, fn_name: &str, args: Vec<Value>) -> Result<Value, ()>;
}

/// Instantiate the global singleton and store it in `globals`
pub fn instantiate(
    description: &CompiledGlobal,
    globals: &mut GlobalStorage,
    root: vtable::VWeak<ComponentVTable, ErasedComponentBox>,
) {
    let instance = match description {
        CompiledGlobal::Builtin { element, .. } => {
            trait Helper {
                fn instantiate(name: &str) -> Pin<Rc<dyn GlobalComponent>> {
                    panic!("Cannot find native global {}", name)
                }
            }
            impl Helper for () {}
            impl<T: rtti::BuiltinGlobal + 'static, Next: Helper> Helper for (T, Next) {
                fn instantiate(name: &str) -> Pin<Rc<dyn GlobalComponent>> {
                    if name == T::name() {
                        T::new()
                    } else {
                        Next::instantiate(name)
                    }
                }
            }
            i_slint_backend_selector::NativeGlobals::instantiate(
                element.native_class.class_name.as_ref(),
            )
        }
        CompiledGlobal::Component { component, .. } => {
            generativity::make_guard!(guard);
            let description = component.unerase(guard);
            Rc::pin(GlobalComponentInstance(crate::dynamic_component::instantiate(
                description.clone(),
                None,
                Some(root),
                None,
                globals.clone(),
            )))
        }
    };
    globals.extend(
        description
            .names()
            .iter()
            .map(|name| (crate::normalize_identifier(name).to_string(), instance.clone())),
    );
}

/// For the global components, we don't use the dynamic_type optimization,
/// and we don't try to optimize the property to their real type
pub struct GlobalComponentInstance(vtable::VRc<ComponentVTable, ErasedComponentBox>);

impl GlobalComponent for GlobalComponentInstance {
    fn set_property(
        self: Pin<&Self>,
        prop_name: &str,
        value: Value,
    ) -> Result<(), SetPropertyError> {
        generativity::make_guard!(guard);
        let comp = self.0.unerase(guard);
        comp.description().set_property(comp.borrow(), prop_name, value)
    }

    fn get_property(self: Pin<&Self>, prop_name: &str) -> Result<Value, ()> {
        generativity::make_guard!(guard);
        let comp = self.0.unerase(guard);
        comp.description().get_property(comp.borrow(), prop_name)
    }

    fn get_property_ptr(self: Pin<&Self>, prop_name: &str) -> *const () {
        generativity::make_guard!(guard);
        let comp = self.0.unerase(guard);
        crate::dynamic_component::get_property_ptr(
            &NamedReference::new(&comp.description().original.root_element, prop_name),
            comp.borrow_instance(),
        )
    }

    fn invoke_callback(self: Pin<&Self>, callback_name: &str, args: &[Value]) -> Result<Value, ()> {
        generativity::make_guard!(guard);
        let comp = self.0.unerase(guard);
        comp.description().invoke(comp.borrow(), callback_name, args)
    }

    fn set_callback_handler(
        self: Pin<&Self>,
        callback_name: &str,
        handler: Box<dyn Fn(&[Value]) -> Value>,
    ) -> Result<(), ()> {
        generativity::make_guard!(guard);
        let comp = self.0.unerase(guard);
        comp.description().set_callback_handler(comp.borrow(), callback_name, handler)
    }

    fn eval_function(self: Pin<&Self>, fn_name: &str, args: Vec<Value>) -> Result<Value, ()> {
        generativity::make_guard!(guard);
        let comp = self.0.unerase(guard);
        let mut ctx =
            crate::eval::EvalLocalContext::from_function_arguments(comp.borrow_instance(), args);
        let result = crate::eval::eval_expression(
            &comp
                .description()
                .original
                .root_element
                .borrow()
                .bindings
                .get(fn_name)
                .ok_or(())?
                .borrow()
                .expression,
            &mut ctx,
        );
        Ok(result)
    }
}

impl<T: rtti::BuiltinItem + 'static> GlobalComponent for T {
    fn set_property(
        self: Pin<&Self>,
        prop_name: &str,
        value: Value,
    ) -> Result<(), SetPropertyError> {
        let prop = Self::properties()
            .into_iter()
            .find(|(k, _)| *k == prop_name)
            .ok_or(SetPropertyError::NoSuchProperty)?
            .1;
        prop.set(self, value, None).map_err(|()| SetPropertyError::WrongType)
    }

    fn get_property(self: Pin<&Self>, prop_name: &str) -> Result<Value, ()> {
        let prop = Self::properties().into_iter().find(|(k, _)| *k == prop_name).ok_or(())?.1;
        prop.get(self)
    }

    fn get_property_ptr(self: Pin<&Self>, prop_name: &str) -> *const () {
        let prop: &dyn rtti::PropertyInfo<Self, Value> =
            Self::properties().into_iter().find(|(k, _)| *k == prop_name).unwrap().1;
        unsafe { (self.get_ref() as *const Self as *const u8).add(prop.offset()) as *const () }
    }

    fn invoke_callback(self: Pin<&Self>, callback_name: &str, args: &[Value]) -> Result<Value, ()> {
        let cb = Self::callbacks().into_iter().find(|(k, _)| *k == callback_name).ok_or(())?.1;
        cb.call(self, args)
    }

    fn set_callback_handler(
        self: Pin<&Self>,
        callback_name: &str,
        handler: Box<dyn Fn(&[Value]) -> Value>,
    ) -> Result<(), ()> {
        let cb = Self::callbacks().into_iter().find(|(k, _)| *k == callback_name).ok_or(())?.1;
        cb.set_handler(self, handler)
    }

    fn eval_function(self: Pin<&Self>, _fn_name: &str, _args: Vec<Value>) -> Result<Value, ()> {
        Err(())
    }
}

pub(crate) fn generate(component: &Rc<Component>) -> CompiledGlobal {
    debug_assert!(component.is_global());
    match &component.root_element.borrow().base_type {
        ElementType::Global => {
            generativity::make_guard!(guard);
            CompiledGlobal::Component {
                component: crate::dynamic_component::generate_component(component, guard).into(),
                public_properties: Default::default(),
            }
        }
        ElementType::Builtin(b) => CompiledGlobal::Builtin {
            name: component.id.clone(),
            element: b.clone(),
            public_properties: Default::default(),
        },
        ElementType::Error | ElementType::Native(_) | ElementType::Component(_) => unreachable!(),
    }
}
