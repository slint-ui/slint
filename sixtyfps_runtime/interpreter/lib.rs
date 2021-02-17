/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
# SixtyFPS interpreter library

**NOTE:** This library is an internal crate for the SixtyFPS project.
This crate should not be used directly by application using SixtyFPS.
You should use the `sixtyfps` crate instead
*/
#![doc(html_logo_url = "https://sixtyfps.io/resources/logo.drawio.svg")]

mod dynamic_component;
mod dynamic_type;
mod eval;
mod global_component;
mod value_model;

pub use eval::{ModelPtr, Value};

use dynamic_component::InstanceRef;
pub use sixtyfps_compilerlib::CompilerConfiguration;
use sixtyfps_corelib::component::{ComponentRef, ComponentRefPin, ComponentVTable};
use std::{collections::HashMap, pin::Pin, rc::Rc};

pub fn new_compiler_configuration() -> CompilerConfiguration {
    sixtyfps_compilerlib::CompilerConfiguration::new(
        sixtyfps_compilerlib::generator::OutputFormat::Interpreter,
    )
}

impl<'id> dynamic_component::ComponentDescription<'id> {
    /// The name of this Component as written in the .60 file
    pub fn id(&self) -> &str {
        self.original.id.as_str()
    }

    /// List of publicly declared properties or callback
    pub fn properties(&self) -> HashMap<String, sixtyfps_compilerlib::langtype::Type> {
        self.original
            .root_element
            .borrow()
            .property_declarations
            .iter()
            .map(|(s, v)| (s.clone(), v.property_type.clone()))
            .collect()
    }

    /// Instantiate a runtime component from this ComponentDescription
    pub fn create(
        self: Rc<Self>,
        #[cfg(target_arch = "wasm32")] canvas_id: String,
    ) -> vtable::VRc<ComponentVTable, dynamic_component::ErasedComponentBox> {
        let component_ref = dynamic_component::instantiate(
            self,
            None,
            #[cfg(target_arch = "wasm32")]
            canvas_id,
        );
        component_ref
            .as_pin_ref()
            .window()
            .set_component(&vtable::VRc::into_dyn(component_ref.clone()));
        component_ref
    }

    /// Set a value to property.
    ///
    /// Returns an error if the component is not an instance corresponding to this ComponentDescription,
    /// or if the property with this name does not exist in this component
    pub fn set_property(
        &self,
        component: ComponentRefPin,
        name: &str,
        value: Value,
    ) -> Result<(), ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        generativity::make_guard!(guard);
        let c = unsafe { InstanceRef::from_pin_ref(component, guard) };
        if let Some(alias) = self
            .original
            .root_element
            .borrow()
            .property_declarations
            .get(name)
            .and_then(|d| d.is_alias.as_ref())
        {
            eval::store_property(c, &alias.element.upgrade().unwrap(), &alias.name, value)
        } else {
            eval::store_property(c, &self.original.root_element, name, value)
        }
    }

    /// Set a binding to a property
    ///
    /// Returns an error if the component is not an instance corresponding to this ComponentDescription,
    /// or if the property with this name does not exist in this component
    pub fn set_binding(
        &self,
        component: ComponentRef,
        name: &str,
        binding: Box<dyn Fn() -> Value>,
    ) -> Result<(), ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        let x = self.custom_properties.get(name).ok_or(())?;
        unsafe {
            x.prop
                .set_binding(
                    Pin::new_unchecked(&*component.as_ptr().add(x.offset)),
                    binding,
                    sixtyfps_corelib::rtti::AnimatedBindingKind::NotAnimated,
                )
                .unwrap()
        };
        Ok(())
    }

    /// Return the value of a property
    ///
    /// Returns an error if the component is not an instance corresponding to this ComponentDescription,
    /// or if a callback with this name does not exist in this component
    pub fn get_property(&self, component: ComponentRefPin, name: &str) -> Result<Value, ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        generativity::make_guard!(guard);
        // Safety: we just verified that the component has the right vtable
        let c = unsafe { InstanceRef::from_pin_ref(component, guard) };
        if let Some(alias) = self
            .original
            .root_element
            .borrow()
            .property_declarations
            .get(name)
            .and_then(|d| d.is_alias.as_ref())
        {
            eval::load_property(c, &alias.element.upgrade().unwrap(), &alias.name)
        } else {
            eval::load_property(c, &self.original.root_element, name)
        }
    }

    /// Sets an handler for a callback
    ///
    /// Returns an error if the component is not an instance corresponding to this ComponentDescription,
    /// or if the property with this name does not exist in this component
    pub fn set_callback_handler(
        &self,
        component: Pin<ComponentRef>,
        name: &str,
        handler: Box<dyn Fn(&[Value]) -> Value>,
    ) -> Result<(), ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        let x = self.custom_callbacks.get(name).ok_or(())?;
        let sig = x.apply(unsafe { &*(component.as_ptr() as *const dynamic_type::Instance) });
        sig.set_handler(handler);
        Ok(())
    }

    /// Emits the specified callback
    ///
    /// Returns an error if the component is not an instance corresponding to this ComponentDescription,
    /// or if the callback with this name does not exist in this component
    pub fn call_callback(
        &self,
        component: ComponentRefPin,
        name: &str,
        args: &[Value],
    ) -> Result<Value, ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        let x = self.custom_callbacks.get(name).ok_or(())?;
        let sig = x.apply(unsafe { &*(component.as_ptr() as *const dynamic_type::Instance) });
        Ok(sig.call(args))
    }
}

pub type ComponentDescription = dynamic_component::ComponentDescription<'static>;
pub type ComponentBox = dynamic_component::ComponentBox<'static>;
pub type ComponentRc = vtable::VRc<ComponentVTable, dynamic_component::ErasedComponentBox>;
pub async fn load(
    source: String,
    path: std::path::PathBuf,
    mut compiler_config: CompilerConfiguration,
) -> (Result<Rc<ComponentDescription>, ()>, sixtyfps_compilerlib::diagnostics::BuildDiagnostics) {
    if compiler_config.style.is_none() && std::env::var("SIXTYFPS_STYLE").is_err() {
        // Defaults to native if it exists:
        compiler_config.style = Some(if sixtyfps_rendering_backend_default::HAS_NATIVE_STYLE {
            "native".to_owned()
        } else {
            "ugly".to_owned()
        });
    }
    dynamic_component::load(source, path, compiler_config, unsafe {
        generativity::Guard::new(generativity::Id::new())
    })
    .await
}

pub fn run_event_loop() {
    sixtyfps_rendering_backend_default::backend().run_event_loop();
}

pub fn register_font_from_path<P: AsRef<std::path::Path>>(
    path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    sixtyfps_rendering_backend_default::backend().register_font_from_path(path.as_ref())
}

pub fn register_font_from_memory(data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    sixtyfps_rendering_backend_default::backend().register_font_from_memory(data)
}
