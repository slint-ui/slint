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
    pub init_properties: HashMap<String, crate::expressions::Expression>,
    /// Right now we only allow forwarding and this connect with the signal in the root
    pub connect_signals: HashMap<String, String>,
    pub children: Vec<LoweredItem>,
}

#[derive(Default, Debug)]
pub struct LoweredComponent {
    pub id: String,
    pub root_item: LoweredItem,

    pub signals_declarations: Vec<String>,
}

impl LoweredComponent {
    pub fn lower(component: &crate::object_tree::Component) -> Self {
        let mut state = LowererState::default();
        LoweredComponent {
            id: component.id.clone(),
            root_item: LoweredComponent::lower_item(&*component.root_element.borrow(), &mut state),
            signals_declarations: state.signals,
        }
    }

    fn lower_item(element: &crate::object_tree::Element, state: &mut LowererState) -> LoweredItem {
        state.count += 1;

        let id =
            format!("{}_{}", if element.id.is_empty() { "id" } else { &*element.id }, state.count);

        let (mut lowered, is_builtin) = match &element.base_type {
            Type::Component(c) => {
                let mut current_component_id = id.clone();
                std::mem::swap(&mut current_component_id, &mut state.current_component_id);
                let r = LoweredComponent::lower_item(&*c.root_element.borrow(), state);
                std::mem::swap(&mut current_component_id, &mut state.current_component_id);
                (r, false)
            }
            Type::Builtin(_) => {
                // FIXME: that information should be in the BuiltType, i guess
                let native_type = Rc::new(NativeItemType {
                    vtable: format!("{}VTable", element.base),
                    class_name: element.base.to_string(),
                });

                (LoweredItem { id: id.clone(), native_type, ..Default::default() }, true)
            }
            _ => panic!("Invalid type"),
        };

        let current_component_id = state.current_component_id.clone();
        let format_signal = |name| format!("{}_{}", current_component_id, name);
        state.signals.extend(element.signals_declaration.iter().map(format_signal));

        for (k, e) in element.bindings.iter() {
            if let crate::expressions::Expression::Identifier(x) = e {
                let value: u32 = match &**x {
                    "blue" => 0xff0000ff,
                    "red" => 0xffff0000,
                    "green" => 0xff00ff00,
                    "yellow" => 0xffffff00,
                    "black" => 0xff000000,
                    "white" => 0xffffffff,
                    _ => {
                        lowered.connect_signals.insert(
                            if is_builtin {
                                format!("{}.{}", id, k)
                            } else {
                                format!("{}_{}", id, k)
                            },
                            format_signal(x),
                        );
                        continue;
                    }
                };
                lowered
                    .init_properties
                    .insert(k.clone(), crate::expressions::Expression::NumberLiteral(value.into()));
            } else {
                lowered.init_properties.insert(k.clone(), e.clone());
            }
        }
        lowered.children.extend(
            element.children.iter().map(|e| LoweredComponent::lower_item(&*e.borrow(), state)),
        );
        lowered
    }
}

#[derive(Default)]
struct LowererState {
    /// The count of item to create the ids
    count: usize,
    /// The ID of the current component
    current_component_id: String,

    signals: Vec<String>,
}
