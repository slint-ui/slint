// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use core::pin::Pin;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::api::Value;
use crate::dynamic_component::{ErasedComponentBox, ErasedComponentDescription};
use crate::SetPropertyError;
use sixtyfps_compilerlib::namedreference::NamedReference;
use sixtyfps_compilerlib::object_tree::PropertyDeclaration;
use sixtyfps_compilerlib::{langtype::Type, object_tree::Component};
use sixtyfps_corelib::component::ComponentVTable;
use sixtyfps_corelib::rtti;

pub enum CompiledGlobal {
    Builtin {
        name: String,
        element: Rc<sixtyfps_compilerlib::langtype::BuiltinElement>,
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
}

pub fn instantiate(description: &CompiledGlobal) -> (String, Pin<Rc<dyn GlobalComponent>>) {
    match description {
        CompiledGlobal::Builtin { name, element, .. } => {
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
            let g = sixtyfps_rendering_backend_default::NativeGlobals::instantiate(
                element.native_class.class_name.as_ref(),
            );
            (name.clone(), g)
        }
        CompiledGlobal::Component { component, .. } => {
            generativity::make_guard!(guard);
            let description = component.unerase(guard);
            let component = &description.original;
            let g = Rc::pin(GlobalComponentInstance(crate::dynamic_component::instantiate(
                description.clone(),
                None,
                None,
            )));
            let id = if component.is_global() {
                component.root_element.borrow().id.clone()
            } else {
                component.id.clone()
            };
            (id, g)
        }
    }
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
        comp.description().invoke_callback(comp.borrow(), callback_name, args)
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
}

pub(crate) fn generate(component: &Rc<Component>) -> CompiledGlobal {
    debug_assert!(component.is_global());
    match &component.root_element.borrow().base_type {
        Type::Void => {
            generativity::make_guard!(guard);
            CompiledGlobal::Component {
                component: crate::dynamic_component::generate_component(component, guard).into(),
                public_properties: Default::default(),
            }
        }
        Type::Builtin(b) => CompiledGlobal::Builtin {
            name: component.id.clone(),
            element: b.clone(),
            public_properties: Default::default(),
        },
        _ => unreachable!(),
    }
}
