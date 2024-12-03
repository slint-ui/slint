// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::api::Value;
use crate::dynamic_item_tree::{
    ErasedItemTreeBox, ErasedItemTreeDescription, PopupMenuDescription,
};
use crate::SetPropertyError;
use core::cell::RefCell;
use core::pin::Pin;
use i_slint_compiler::langtype::ElementType;
use i_slint_compiler::namedreference::NamedReference;
use i_slint_compiler::object_tree::{Component, Document, PropertyDeclaration};
use i_slint_core::item_tree::ItemTreeVTable;
use i_slint_core::rtti;
use smol_str::SmolStr;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

pub struct CompiledGlobalCollection {
    /// compiled globals
    pub compiled_globals: Vec<CompiledGlobal>,
    /// Map of all exported global singletons and their index in the compiled_globals vector. The key
    /// is the normalized name of the global.
    pub exported_globals_by_name: BTreeMap<SmolStr, usize>,
}

impl CompiledGlobalCollection {
    pub fn compile(doc: &Document) -> Self {
        let mut exported_globals_by_name = BTreeMap::new();
        let compiled_globals = doc
            .used_types
            .borrow()
            .globals
            .iter()
            .enumerate()
            .map(|(index, component)| {
                let mut global = generate(component);

                if !component.exported_global_names.borrow().is_empty() {
                    global.extend_public_properties(
                        component.root_element.borrow().property_declarations.clone(),
                    );

                    exported_globals_by_name.extend(
                        component
                            .exported_global_names
                            .borrow()
                            .iter()
                            .map(|exported_name| (exported_name.name.clone(), index)),
                    )
                }

                global
            })
            .collect();
        Self { compiled_globals, exported_globals_by_name }
    }
}

#[derive(Clone)]
pub enum GlobalStorage {
    Strong(Rc<RefCell<HashMap<String, Pin<Rc<dyn GlobalComponent>>>>>),
    /// When the storage is held by another global
    Weak(std::rc::Weak<RefCell<HashMap<String, Pin<Rc<dyn GlobalComponent>>>>>),
}

impl GlobalStorage {
    pub fn get(&self, name: &str) -> Option<Pin<Rc<dyn GlobalComponent>>> {
        match self {
            GlobalStorage::Strong(storage) => storage.borrow().get(name).cloned(),
            GlobalStorage::Weak(storage) => storage.upgrade().unwrap().borrow().get(name).cloned(),
        }
    }
}

impl Default for GlobalStorage {
    fn default() -> Self {
        GlobalStorage::Strong(Default::default())
    }
}

pub enum CompiledGlobal {
    Builtin {
        name: SmolStr,
        element: Rc<i_slint_compiler::langtype::BuiltinElement>,
        // dummy needed for iterator accessor
        public_properties: BTreeMap<SmolStr, PropertyDeclaration>,
        /// keep the Component alive as it is boing referenced by `NamedReference`s
        _original: Rc<Component>,
    },
    Component {
        component: ErasedItemTreeDescription,
        public_properties: BTreeMap<SmolStr, PropertyDeclaration>,
    },
}

impl CompiledGlobal {
    pub fn names(&self) -> Vec<SmolStr> {
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
                let is_exported = !component.original.exported_global_names.borrow().is_empty();
                is_exported
            }
        }
    }

    pub fn public_properties(&self) -> impl Iterator<Item = (&SmolStr, &PropertyDeclaration)> + '_ {
        match self {
            CompiledGlobal::Builtin { public_properties, .. } => public_properties.iter(),
            CompiledGlobal::Component { public_properties, .. } => public_properties.iter(),
        }
    }

    pub fn extend_public_properties(
        &mut self,
        iter: impl IntoIterator<Item = (SmolStr, PropertyDeclaration)>,
    ) {
        match self {
            CompiledGlobal::Builtin { public_properties, .. } => public_properties.extend(iter),
            CompiledGlobal::Component { public_properties, .. } => public_properties.extend(iter),
        }
    }
}

pub trait GlobalComponent {
    fn invoke_callback(
        self: Pin<&Self>,
        callback_name: &SmolStr,
        args: &[Value],
    ) -> Result<Value, ()>;

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

    fn get_property_ptr(self: Pin<&Self>, prop_name: &SmolStr) -> *const ();

    fn eval_function(self: Pin<&Self>, fn_name: &str, args: Vec<Value>) -> Result<Value, ()>;
}

/// Instantiate the global singleton and store it in `globals`
pub fn instantiate(
    description: &CompiledGlobal,
    globals: &mut GlobalStorage,
    root: vtable::VWeak<ItemTreeVTable, ErasedItemTreeBox>,
) {
    let GlobalStorage::Strong(ref mut globals) = globals else {
        panic!("Global storage is not strong")
    };

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
            let inst = crate::dynamic_item_tree::instantiate(
                description.clone(),
                None,
                Some(root),
                None,
                GlobalStorage::Weak(Rc::downgrade(&globals)),
            );
            inst.run_setup_code();
            Rc::pin(GlobalComponentInstance(inst))
        }
    };

    globals.borrow_mut().extend(
        description
            .names()
            .iter()
            .map(|name| (crate::normalize_identifier(name).to_string(), instance.clone())),
    );
}

/// For the global components, we don't use the dynamic_type optimization,
/// and we don't try to optimize the property to their real type
pub struct GlobalComponentInstance(vtable::VRc<ItemTreeVTable, ErasedItemTreeBox>);

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

    fn get_property_ptr(self: Pin<&Self>, prop_name: &SmolStr) -> *const () {
        generativity::make_guard!(guard);
        let comp = self.0.unerase(guard);
        crate::dynamic_item_tree::get_property_ptr(
            &NamedReference::new(&comp.description().original.root_element, prop_name.clone()),
            comp.borrow_instance(),
        )
    }

    fn invoke_callback(
        self: Pin<&Self>,
        callback_name: &SmolStr,
        args: &[Value],
    ) -> Result<Value, ()> {
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

    fn get_property_ptr(self: Pin<&Self>, prop_name: &SmolStr) -> *const () {
        let prop: &dyn rtti::PropertyInfo<Self, Value> =
            Self::properties().into_iter().find(|(k, _)| *k == prop_name).unwrap().1;
        unsafe { (self.get_ref() as *const Self as *const u8).add(prop.offset()) as *const () }
    }

    fn invoke_callback(
        self: Pin<&Self>,
        callback_name: &SmolStr,
        args: &[Value],
    ) -> Result<Value, ()> {
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

fn generate(component: &Rc<Component>) -> CompiledGlobal {
    debug_assert!(component.is_global());
    match &component.root_element.borrow().base_type {
        ElementType::Global => {
            generativity::make_guard!(guard);
            CompiledGlobal::Component {
                component: crate::dynamic_item_tree::generate_item_tree(
                    component,
                    None,
                    PopupMenuDescription::Weak(Default::default()),
                    false,
                    guard,
                )
                .into(),
                public_properties: Default::default(),
            }
        }
        ElementType::Builtin(b) => CompiledGlobal::Builtin {
            name: component.id.clone(),
            element: b.clone(),
            public_properties: Default::default(),
            _original: component.clone(),
        },
        ElementType::Error | ElementType::Native(_) | ElementType::Component(_) => unreachable!(),
    }
}
