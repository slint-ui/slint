/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::expression_tree::{BuiltinFunction, Expression};
use crate::langtype::{Enumeration, Type};
use crate::object_tree::Component;

/// reserved property injected in every item
pub fn reserved_property(name: &str) -> Type {
    for (p, t) in [
        ("x", Type::Length),
        ("y", Type::Length),
        ("width", Type::Length),
        ("height", Type::Length),
        ("minimum_width", Type::Length),
        ("minimum_height", Type::Length),
        ("maximum_width", Type::Length),
        ("maximum_height", Type::Length),
        ("padding", Type::Length),
        ("padding_left", Type::Length),
        ("padding_right", Type::Length),
        ("padding_top", Type::Length),
        ("padding_bottom", Type::Length),
        ("horizontal_stretch", Type::Float32),
        ("vertical_stretch", Type::Float32),
        ("clip", Type::Bool),
        ("opacity", Type::Float32),
        ("visible", Type::Bool),
        // ("enabled", Type::Bool),
        ("col", Type::Int32),
        ("row", Type::Int32),
        ("colspan", Type::Int32),
        ("rowspan", Type::Int32),
        ("initial_focus", Type::ElementReference),
    ]
    .iter()
    {
        if *p == name {
            return t.clone();
        }
    }
    Type::Invalid
}

#[derive(Debug, Default)]
pub struct TypeRegister {
    /// The set of types.
    types: HashMap<String, Type>,
    supported_property_animation_types: HashSet<String>,
    pub(crate) property_animation_type: Type,
    /// Map from a context restricted type to the list of contexts (parent type) it is allowed in. This is
    /// used to construct helpful error messages, such as "Row can only be within a GridLayout element".
    context_restricted_types: HashMap<String, HashSet<String>>,
    parent_registry: Option<Rc<RefCell<TypeRegister>>>,
}

impl TypeRegister {
    /// FIXME: same as 'add' ?
    pub fn insert_type(&mut self, t: Type) {
        self.types.insert(t.to_string(), t);
    }
    pub fn insert_type_with_name(&mut self, t: Type, name: String) {
        self.types.insert(name, t);
    }

    pub fn builtin() -> Rc<RefCell<Self>> {
        let mut register = TypeRegister::default();

        register.insert_type(Type::Float32);
        register.insert_type(Type::Int32);
        register.insert_type(Type::String);
        register.insert_type(Type::Length);
        register.insert_type(Type::LogicalLength);
        register.insert_type(Type::Color);
        register.insert_type(Type::Duration);
        register.insert_type(Type::Resource);
        register.insert_type(Type::Bool);
        register.insert_type(Type::Model);
        register.insert_type(Type::Percent);
        register.insert_type(Type::Easing);

        let mut declare_enum = |name: &str, values: &[&str]| {
            register.insert_type_with_name(
                Type::Enumeration(Rc::new(Enumeration {
                    name: name.to_owned(),
                    values: values.into_iter().cloned().map(String::from).collect(),
                    default_value: 0,
                })),
                name.to_owned(),
            );
        };

        declare_enum("TextHorizontalAlignment", &["align_left", "align_center", "align_right"]);
        declare_enum("TextVerticalAlignment", &["align_top", "align_center", "align_bottom"]);
        declare_enum(
            "LayoutAlignment",
            &["stretch", "center", "start", "end", "space_between", "space_around"],
        );

        register.supported_property_animation_types.insert(Type::Float32.to_string());
        register.supported_property_animation_types.insert(Type::Int32.to_string());
        register.supported_property_animation_types.insert(Type::Color.to_string());
        register.supported_property_animation_types.insert(Type::Length.to_string());
        register.supported_property_animation_types.insert(Type::LogicalLength.to_string());

        crate::load_builtins::load_builtins(&mut register);

        let mut context_restricted_types = HashMap::new();
        register
            .types
            .values()
            .for_each(|ty| ty.collect_contextual_types(&mut context_restricted_types));
        register.context_restricted_types = context_restricted_types;

        match &mut register.types.get_mut("TextInput").unwrap() {
            Type::Builtin(ref mut b) => {
                Rc::get_mut(b)
                    .unwrap()
                    .properties
                    .insert("focus".into(), BuiltinFunction::SetFocusItem.ty());
                Rc::get_mut(b).unwrap().member_functions.insert(
                    "focus".into(),
                    Expression::BuiltinFunctionReference(BuiltinFunction::SetFocusItem),
                );
            }
            _ => unreachable!(),
        };
        match &mut register.types.get_mut("PopupWindow").unwrap() {
            Type::Builtin(ref mut b) => {
                Rc::get_mut(b)
                    .unwrap()
                    .properties
                    .insert("show".into(), BuiltinFunction::ShowPopupWindow.ty());
                Rc::get_mut(b).unwrap().member_functions.insert(
                    "show".into(),
                    Expression::BuiltinFunctionReference(BuiltinFunction::ShowPopupWindow),
                );
            }
            _ => unreachable!(),
        };

        Rc::new(RefCell::new(register))
    }

    pub fn new(parent: &Rc<RefCell<TypeRegister>>) -> Self {
        Self { parent_registry: Some(parent.clone()), ..Default::default() }
    }

    pub fn lookup(&self, name: &str) -> Type {
        self.types
            .get(name)
            .cloned()
            .or_else(|| self.parent_registry.as_ref().map(|r| r.borrow().lookup(name)))
            .unwrap_or_default()
    }

    fn lookup_element_as_result(
        &self,
        name: &str,
    ) -> Result<Type, HashMap<String, HashSet<String>>> {
        match self.types.get(name).cloned() {
            Some(ty) => Ok(ty),
            None => match &self.parent_registry {
                Some(r) => r.borrow().lookup_element_as_result(name),
                None => Err(self.context_restricted_types.clone()),
            },
        }
    }

    pub fn lookup_element(&self, name: &str) -> Result<Type, String> {
        self.lookup_element_as_result(name).map_err(|context_restricted_types| {
            if let Some(permitted_parent_types) = context_restricted_types.get(name) {
                if permitted_parent_types.len() == 1 {
                    format!(
                        "{} can only be within a {} element",
                        name,
                        permitted_parent_types.iter().next().unwrap()
                    )
                    .to_owned()
                } else {
                    let mut elements = permitted_parent_types.iter().cloned().collect::<Vec<_>>();
                    elements.sort();
                    format!(
                        "{} can only be within the following elements: {}",
                        name,
                        elements.join(", ")
                    )
                    .to_owned()
                }
            } else {
                format!("Unknown type {}", name)
            }
        })
    }

    pub fn lookup_qualified<Member: AsRef<str>>(&self, qualified: &[Member]) -> Type {
        if qualified.len() != 1 {
            return Type::Invalid;
        }
        self.lookup(qualified[0].as_ref())
    }

    pub fn add(&mut self, comp: Rc<Component>) {
        self.add_with_name(comp.id.clone(), comp);
    }

    pub fn add_with_name(&mut self, name: String, comp: Rc<Component>) {
        self.types.insert(name, Type::Component(comp));
    }

    pub fn property_animation_type_for_property(&self, property_type: Type) -> Type {
        if self.supported_property_animation_types.contains(&property_type.to_string()) {
            self.property_animation_type.clone()
        } else {
            self.parent_registry
                .as_ref()
                .map(|registry| {
                    registry.borrow().property_animation_type_for_property(property_type)
                })
                .unwrap_or_default()
        }
    }
}
