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
    pub id: String,
    pub native_type: Rc<NativeItemType>,
    pub init_properties: HashMap<String, crate::object_tree::Expression>,
    pub children: Vec<LoweredItem>,
}

#[derive(Default, Debug)]
pub struct LoweredComponent {
    pub id: String,
    pub root_item: LoweredItem,
}

impl LoweredComponent {
    pub fn lower(component: &crate::object_tree::Component) -> Self {
        let mut count = 0;
        LoweredComponent {
            id: component.id.clone(),
            root_item: LoweredComponent::lower_item(&*component.root_element, &mut count),
        }
    }

    fn lower_item(element: &crate::object_tree::Element, count: &mut usize) -> LoweredItem {
        // FIXME: lookup base instead of assuming
        let native_type = Rc::new(NativeItemType {
            vtable: format!("{}VTable", element.base),
            class_name: element.base.clone(),
        });
        let mut id = element.id.clone();
        if id.is_empty() {
            id = format!("id_{}", count);
        }
        *count += 1;

        let mut init_properties = element.bindings.clone();
        for (_, e) in &mut init_properties {
            if let crate::object_tree::Expression::Identifier(x) = e {
                let value: u32 = match &**x {
                    "blue" => 0xff0000ff,
                    "red" => 0xffff0000,
                    "green" => 0xff00ff00,
                    "yellow" => 0xffffff00,
                    "black" => 0xff000000,
                    "white" => 0xffffffff,
                    _ => continue,
                };
                *e = crate::object_tree::Expression::NumberLiteral(value.into())
            }
        }

        LoweredItem {
            id,
            native_type,
            init_properties,
            // FIXME: we should only keep element that can be lowered.
            children: element
                .children
                .iter()
                .map(|e| LoweredComponent::lower_item(&*e, count))
                .collect(),
        }
    }
}
