/*!
# SixtyFPS interpreter library

**NOTE:** This library is an internal crate for the SixtyFPS project.
This crate should not be used directly by application using SixtyFPS.
You should use the `sixtyfps` crate instead
*/

mod dynamic_component;
mod dynamic_type;
mod eval;

pub use dynamic_component::load;
pub use dynamic_component::ComponentDescription;
pub use eval::Value;

pub use dynamic_component::ComponentBox;
use sixtyfps_corelib::abi::datastructures::{ComponentRef, ComponentRefMut};
use sixtyfps_corelib::{ComponentRefPin, EvaluationContext, Signal};
use std::{collections::HashMap, pin::Pin, rc::Rc};

impl ComponentDescription {
    /// The name of this Component as written in the .60 file
    pub fn id(&self) -> &str {
        self.original.id.as_str()
    }

    /// List of publicly declared properties or signal
    pub fn properties(&self) -> HashMap<String, sixtyfps_compilerlib::typeregister::Type> {
        self.original
            .root_element
            .borrow()
            .property_declarations
            .iter()
            .map(|(s, v)| (s.clone(), v.property_type.clone()))
            .collect()
    }

    /// Instantiate a runtime component from this ComponentDescription
    pub fn create(self: Rc<Self>) -> ComponentBox {
        dynamic_component::instentiate(self, None)
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
        let x = self.custom_properties.get(name).ok_or(())?;
        unsafe { x.prop.set(&*component.as_ptr().add(x.offset), value) }
    }

    /// Set a binding to a property
    ///
    /// Returns an error if the component is not an instance corresponding to this ComponentDescription,
    /// or if the property with this name does not exist in this component
    pub fn set_binding(
        &self,
        component: ComponentRef,
        name: &str,
        binding: Box<dyn Fn(&EvaluationContext) -> Value>,
    ) -> Result<(), ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        let x = self.custom_properties.get(name).ok_or(())?;
        unsafe { x.prop.set_binding(&*component.as_ptr().add(x.offset), binding) };
        Ok(())
    }

    /// Return the value of a property
    ///
    /// Returns an error if the component is not an instance corresponding to this ComponentDescription,
    /// or if a signal with this name does not exist in this component
    pub fn get_property(&self, eval_context: &EvaluationContext, name: &str) -> Result<Value, ()> {
        if !core::ptr::eq((&self.ct) as *const _, eval_context.component.get_vtable() as *const _) {
            return Err(());
        }
        let x = self.custom_properties.get(name).ok_or(())?;
        unsafe { x.prop.get(&*eval_context.component.as_ptr().add(x.offset), eval_context) }
    }

    /// Sets an handler for a signal
    ///
    /// Returns an error if the component is not an instance corresponding to this ComponentDescription,
    /// or if the property with this name does not exist in this component
    pub fn set_signal_handler(
        &self,
        component: Pin<ComponentRefMut>,
        name: &str,
        handler: Box<dyn Fn(&EvaluationContext, ())>,
    ) -> Result<(), ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        let x = self.custom_signals.get(name).ok_or(())?;
        let sig = unsafe { &mut *(component.as_ptr().add(*x) as *mut Signal<()>) };
        sig.set_handler(handler);
        Ok(())
    }

    /// Emits the specified signal
    ///
    /// Returns an error if the component is not an instance corresponding to this ComponentDescription,
    /// or if the signal with this name does not exist in this component
    pub fn emit_signal(&self, eval_context: &EvaluationContext, name: &str) -> Result<(), ()> {
        let component = eval_context.component;
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        let x = self.custom_signals.get(name).ok_or(())?;
        let sig = unsafe { &mut *(component.as_ptr().add(*x) as *mut Signal<()>) };
        sig.emit(eval_context, ());
        Ok(())
    }
}
