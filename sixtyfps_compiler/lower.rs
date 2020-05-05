//! This module contains the code that lower the tree to the datastructure that that the runtime understand
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Default, Debug)]
pub struct NativeItemType {
    /*render_function: String,
    geometry_function: String,
    imput_function: String,*/
    /// The C symbol of the VTable
    pub vtable: String,

    /// The class name
    pub class_name: String,
}

#[derive(Default, Debug)]
pub struct LoweredItem {
    pub native_type: Rc<NativeItemType>,
    pub init_properties: HashMap<String, String>,
    pub children: Vec<LoweredItem>,
}

#[derive(Default, Debug)]
pub struct LoweredComponent {
    pub id: String,
    pub root_item: LoweredItem,
}

impl LoweredComponent {
    pub fn lower(component: &crate::object_tree::Component) -> Self {
        LoweredComponent {
            id: component.id.clone(),
            root_item: LoweredComponent::lower_item(&*component.root_element),
        }
    }

    fn lower_item(element: &crate::object_tree::Element) -> LoweredItem {
        // FIXME: lookup base instead of assuming
        let native_type = Rc::new(NativeItemType {
            vtable: format!("{}VTable", element.base),
            class_name: element.base.clone(),
        });
        LoweredItem {
            native_type,
            init_properties: element
                .bindings
                .iter()
                .map(|(s, c)| (s.clone(), c.value.clone()))
                .collect(),
            children: vec![],
        }
    }
}
