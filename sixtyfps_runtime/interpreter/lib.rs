mod dynamic_component;
mod dynamic_type;
mod eval;

pub use dynamic_component::load;
pub use dynamic_component::MyComponentType as ComponentDescription;
pub use eval::Value;

use corelib::abi::datastructures::{ComponentBox, ComponentRef, ComponentRefMut};
pub(crate) use dynamic_component::ComponentImpl;
use std::{collections::HashMap, rc::Rc};

impl ComponentDescription {
    /// The name of this Component as written in the .60 file
    pub fn id(&self) -> &str {
        self.original.root_component.id.as_str()
    }

    /// List of publicly declared properties or signal
    pub fn properties(&self) -> HashMap<String, sixtyfps_compilerlib::typeregister::Type> {
        self.original
            .root_component
            .root_element
            .borrow()
            .property_declarations
            .iter()
            .map(|(s, v)| (s.clone(), v.property_type.clone()))
            .collect()
    }

    pub fn create(self: Rc<Self>) -> ComponentBox {
        dynamic_component::instentiate(self)
    }

    pub fn set_property(
        &self,
        component: ComponentRef,
        name: &str,
        value: Value,
    ) -> Result<(), ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        let x = self.custom_properties.get(name).ok_or(())?;
        unsafe { x.prop.set(&*component.as_ptr().add(x.offset), value) }
    }

    pub fn set_binding(
        &self,
        component: ComponentRef,
        name: &str,
        binding: Box<dyn Fn(&corelib::EvaluationContext) -> Value>,
    ) -> Result<(), ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        let x = self.custom_properties.get(name).ok_or(())?;
        unsafe { x.prop.set_binding(&*component.as_ptr().add(x.offset), binding) };
        Ok(())
    }

    pub fn get_property(&self, component: ComponentRef, name: &str) -> Result<Value, ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        let x = self.custom_properties.get(name).ok_or(())?;
        let eval_context = corelib::EvaluationContext { component };
        unsafe { x.prop.get(&*component.as_ptr().add(x.offset), &eval_context) }
    }

    pub fn set_signal_handler(
        &self,
        component: ComponentRefMut,
        name: &str,
        handler: Box<dyn Fn(&corelib::EvaluationContext, ())>,
    ) -> Result<(), ()> {
        if !core::ptr::eq((&self.ct) as *const _, component.get_vtable() as *const _) {
            return Err(());
        }
        let x = self.custom_signals.get(name).ok_or(())?;
        let sig = unsafe { &mut *(component.as_ptr().add(*x) as *mut corelib::Signal<()>) };
        sig.set_handler(handler);
        Ok(())
    }
}
