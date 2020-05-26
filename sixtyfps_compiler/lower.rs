//! This module contains the code that lower the tree to the datastructure that that the runtime understand
use crate::{
    expression_tree::Expression,
    object_tree::{Component, Element},
    typeregister::Type,
};
use std::cell::RefCell;
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

pub type LoweredPropertyDeclarationIndex = usize;

#[derive(Default, Debug)]
pub struct LoweredItem {
    pub id: String,
    pub native_type: Rc<NativeItemType>,
    pub init_properties: HashMap<String, Expression>,
    /// Right now we only allow forwarding and this connect with the signal in the root
    pub connect_signals: HashMap<String, String>,
    pub property_declarations: HashMap<String, LoweredPropertyDeclarationIndex>,
    pub children: Vec<LoweredItem>,
}

#[derive(Default, Debug)]
pub struct LoweredComponent {
    pub id: String,
    pub root_item: LoweredItem,

    pub signals_declarations: Vec<String>,
    pub property_declarations: Vec<LoweredPropertyDeclaration>,
}

// I guess this should actually be in the generator for the given language?
fn format_name(name: &str, elem: &Rc<RefCell<Element>>, component: &Rc<Component>) -> String {
    if Rc::ptr_eq(elem, &component.root_element) {
        name.to_owned()
    } else {
        // FIXME: using the pointer name will not lead to reproducable output
        format!("{}_{:p}", name, *elem)
    }
}

impl LoweredComponent {
    pub fn lower(component: &Rc<Component>) -> Self {
        let mut state = LowererState::default();
        state.component = component.clone();
        LoweredComponent {
            id: component.id.clone(),
            root_item: LoweredComponent::lower_item(&component.root_element, &mut state),
            signals_declarations: state.signals,
            property_declarations: state.property_declarations,
        }
    }

    fn lower_item(elem: &Rc<RefCell<Element>>, state: &mut LowererState) -> LoweredItem {
        let element = elem.borrow();
        state.count += 1;

        let id =
            format!("{}_{}", if element.id.is_empty() { "id" } else { &*element.id }, state.count);

        let mut lowered = match &element.base_type {
            Type::Component(_) => {
                panic!("This should not happen because of inlining");
            }
            Type::Builtin(b) => {
                let native_type = Rc::new(NativeItemType {
                    vtable: b.vtable_symbol.clone(),
                    class_name: b.class_name.clone(),
                });

                LoweredItem { id: id.clone(), native_type, ..Default::default() }
            }
            _ => panic!("Invalid type"),
        };

        let component = state.component.clone();
        state.signals.extend(
            element.signals_declaration.iter().map(|name| format_name(name, elem, &component)),
        );

        for (prop_name, property_decl) in element.property_declarations.iter() {
            let component_global_index = state.property_declarations.len();
            lowered.property_declarations.insert(prop_name.clone(), component_global_index);
            state.property_declarations.push(LoweredPropertyDeclaration {
                property_type: property_decl.property_type.clone(),
                name_hint: prop_name.clone(),
                type_location: property_decl.type_location.clone(),
            });
        }

        for (k, e) in element.bindings.iter() {
            if let Expression::SignalReference { name, component, element } = e {
                lowered.connect_signals.insert(
                    if elem.borrow().signals_declaration.contains(k) {
                        format_name(k, elem, &state.component)
                    } else {
                        format!("{}.{}", id, k)
                    },
                    format_name(name, &element.upgrade().unwrap(), &component.upgrade().unwrap()),
                );
            } else {
                lowered.init_properties.insert(k.clone(), e.clone());
            }
        }
        lowered
            .children
            .extend(element.children.iter().map(|e| LoweredComponent::lower_item(&e, state)));
        lowered
    }
}

#[derive(Debug)]
pub struct LoweredPropertyDeclaration {
    pub property_type: Type,
    pub name_hint: String,
    pub type_location: crate::diagnostics::Span,
}

#[derive(Default)]
struct LowererState {
    /// The count of item to create the ids
    count: usize,

    signals: Vec<String>,
    property_declarations: Vec<LoweredPropertyDeclaration>,
    component: Rc<Component>,
}
