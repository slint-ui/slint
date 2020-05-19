//! This module contains the code that lower the tree to the datastructure that that the runtime understand
use crate::typeregister::Type;
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
        let id = format!("{}_{}", if element.id.is_empty() { "id" } else { &*element.id }, count);
        *count += 1;

        let mut lowered = match &element.base_type {
            Type::Component(c) => LoweredComponent::lower_item(&*c.root_element, count),
            Type::Builtin(_) => {
                // FIXME: that information should be in the BuiltType, i guess
                let native_type = Rc::new(NativeItemType {
                    vtable: format!("{}VTable", element.base),
                    class_name: element.base.clone(),
                });

                LoweredItem { id, native_type, ..Default::default() }
            }
            _ => panic!("Invalid type"),
        };

        lowered.init_properties.extend(element.bindings.iter().map(|(k, e)| {
            if let crate::object_tree::Expression::Identifier(x) = e {
                let value: u32 = match &**x {
                    "blue" => 0xff0000ff,
                    "red" => 0xffff0000,
                    "green" => 0xff00ff00,
                    "yellow" => 0xffffff00,
                    "black" => 0xff000000,
                    "white" => 0xffffffff,
                    _ => return (k.clone(), e.clone()),
                };
                (k.clone(), crate::object_tree::Expression::NumberLiteral(value.into()))
            } else {
                (k.clone(), e.clone())
            }
        }));
        lowered
            .children
            .extend(element.children.iter().map(|e| LoweredComponent::lower_item(&*e, count)));
        lowered
    }
}
