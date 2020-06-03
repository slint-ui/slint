mod dynamic_component;
mod dynamic_type;
mod eval;

pub(crate) use dynamic_component::ComponentImpl;
pub use dynamic_component::MyComponentType as ComponentDescription;
pub use dynamic_component::{instentiate, load};

impl ComponentDescription {
    /// The name of this Component as written in the .60 file
    pub fn id(&self) -> &str {
        self.original.root_component.id.as_str()
    }

    /// List of publicly declared properties or signal
    pub fn properties(&self) -> Vec<(String, sixtyfps_compiler::typeregister::Type)> {
        self.original
            .root_component
            .root_element
            .borrow()
            .property_declarations
            .iter()
            .map(|(s, v)| (s.clone(), v.property_type.clone()))
            .collect()
    }
}
