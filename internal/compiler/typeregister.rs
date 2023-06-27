// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore imum

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::expression_tree::BuiltinFunction;
use crate::langtype::{
    BuiltinElement, BuiltinPropertyInfo, ElementType, Enumeration, PropertyLookupResult, Type,
};
use crate::object_tree::Component;

pub const RESERVED_GEOMETRY_PROPERTIES: &[(&str, Type)] = &[
    ("x", Type::LogicalLength),
    ("y", Type::LogicalLength),
    ("width", Type::LogicalLength),
    ("height", Type::LogicalLength),
    ("z", Type::Float32),
];

pub const RESERVED_LAYOUT_PROPERTIES: &[(&str, Type)] = &[
    ("min-width", Type::LogicalLength),
    ("min-height", Type::LogicalLength),
    ("max-width", Type::LogicalLength),
    ("max-height", Type::LogicalLength),
    ("padding", Type::LogicalLength),
    ("padding-left", Type::LogicalLength),
    ("padding-right", Type::LogicalLength),
    ("padding-top", Type::LogicalLength),
    ("padding-bottom", Type::LogicalLength),
    ("preferred-width", Type::LogicalLength),
    ("preferred-height", Type::LogicalLength),
    ("horizontal-stretch", Type::Float32),
    ("vertical-stretch", Type::Float32),
    ("col", Type::Int32),
    ("row", Type::Int32),
    ("colspan", Type::Int32),
    ("rowspan", Type::Int32),
];

macro_rules! declare_enums {
    ($( $(#[$enum_doc:meta])* enum $Name:ident { $( $(#[$value_doc:meta])* $Value:ident,)* })*) => {
        pub struct BuiltinEnums {
            $(pub $Name : Rc<Enumeration>),*
        }
        impl BuiltinEnums {
            fn new() -> Self {
                Self {
                    $($Name : Rc::new(Enumeration {
                        name: stringify!($Name).replace('_', "-"),
                        values: vec![$(crate::generator::to_kebab_case(stringify!($Value).trim_start_matches("r#"))),*],
                        default_value: 0,
                        node: None,
                    })),*
                }
            }
            fn fill_register(&self, register: &mut TypeRegister) {
                $(if stringify!($Name) != "PathEvent" {
                    register.insert_type_with_name(
                        Type::Enumeration(self.$Name.clone()),
                        stringify!($Name).replace('_', "-")
                    );
                })*
            }
        }
    };
}

i_slint_common::for_each_enums!(declare_enums);

thread_local! {
    pub static BUILTIN_ENUMS: BuiltinEnums = BuiltinEnums::new();
}

const RESERVED_OTHER_PROPERTIES: &[(&str, Type)] = &[
    ("clip", Type::Bool),
    ("opacity", Type::Float32),
    ("cache-rendering-hint", Type::Bool),
    ("visible", Type::Bool), // ("enabled", Type::Bool),
];

pub const RESERVED_DROP_SHADOW_PROPERTIES: &[(&str, Type)] = &[
    ("drop-shadow-offset-x", Type::LogicalLength),
    ("drop-shadow-offset-y", Type::LogicalLength),
    ("drop-shadow-blur", Type::LogicalLength),
    ("drop-shadow-color", Type::Color),
];

pub const RESERVED_ROTATION_PROPERTIES: &[(&str, Type)] = &[
    ("rotation-angle", Type::Angle),
    ("rotation-origin-x", Type::LogicalLength),
    ("rotation-origin-y", Type::LogicalLength),
];

pub const RESERVED_ACCESSIBILITY_PROPERTIES: &[(&str, Type)] = &[
    //("accessible-role", ...)
    ("accessible-checkable", Type::Bool),
    ("accessible-checked", Type::Bool),
    ("accessible-delegate-focus", Type::Int32),
    ("accessible-description", Type::String),
    ("accessible-label", Type::String),
    ("accessible-value", Type::String),
    ("accessible-value-maximum", Type::Float32),
    ("accessible-value-minimum", Type::Float32),
    ("accessible-value-step", Type::Float32),
];

/// list of reserved property injected in every item
pub fn reserved_properties() -> impl Iterator<Item = (&'static str, Type)> {
    RESERVED_GEOMETRY_PROPERTIES
        .iter()
        .chain(RESERVED_LAYOUT_PROPERTIES.iter())
        .chain(RESERVED_OTHER_PROPERTIES.iter())
        .chain(RESERVED_DROP_SHADOW_PROPERTIES.iter())
        .chain(RESERVED_ROTATION_PROPERTIES.iter())
        .chain(RESERVED_ACCESSIBILITY_PROPERTIES.iter())
        .map(|(k, v)| (*k, v.clone()))
        .chain(IntoIterator::into_iter([("absolute-position", logical_point_type())]))
        .chain(IntoIterator::into_iter([
            ("forward-focus", Type::ElementReference),
            ("focus", BuiltinFunction::SetFocusItem.ty()),
            (
                "dialog-button-role",
                Type::Enumeration(BUILTIN_ENUMS.with(|e| e.DialogButtonRole.clone())),
            ),
            (
                "accessible-role",
                Type::Enumeration(BUILTIN_ENUMS.with(|e| e.AccessibleRole.clone())),
            ),
        ]))
        .chain(std::iter::once(("init", Type::Callback { return_type: None, args: vec![] })))
}

/// lookup reserved property injected in every item
pub fn reserved_property(name: &str) -> PropertyLookupResult {
    for (p, t) in reserved_properties() {
        if p == name {
            return PropertyLookupResult {
                property_type: t,
                resolved_name: name.into(),
                is_local_to_component: false,
                property_visibility: crate::object_tree::PropertyVisibility::InOut,
                declared_pure: None,
            };
        }
    }

    // Report deprecated known reserved properties (maximum_width, minimum_height, ...)
    for pre in &["min", "max"] {
        if let Some(a) = name.strip_prefix(pre) {
            for suf in &["width", "height"] {
                if let Some(b) = a.strip_suffix(suf) {
                    if b == "imum-" {
                        return PropertyLookupResult {
                            property_type: Type::LogicalLength,
                            resolved_name: format!("{}-{}", pre, suf).into(),
                            is_local_to_component: false,
                            property_visibility: crate::object_tree::PropertyVisibility::InOut,
                            declared_pure: None,
                        };
                    }
                }
            }
        }
    }
    PropertyLookupResult {
        resolved_name: name.into(),
        property_type: Type::Invalid,
        is_local_to_component: false,
        property_visibility: crate::object_tree::PropertyVisibility::Private,
        declared_pure: None,
    }
}

/// These member functions are injected in every time
pub fn reserved_member_function(name: &str) -> Option<BuiltinFunction> {
    for (m, e) in [
        ("focus", BuiltinFunction::SetFocusItem), // match for callable "focus" property
    ] {
        if m == name {
            return Some(e);
        }
    }
    None
}

#[derive(Debug, Default)]
pub struct TypeRegister {
    /// The set of property types.
    types: HashMap<String, Type>,
    /// The set of element types
    elements: HashMap<String, ElementType>,
    supported_property_animation_types: HashSet<String>,
    pub(crate) property_animation_type: ElementType,
    pub(crate) empty_type: ElementType,
    /// Map from a context restricted type to the list of contexts (parent type) it is allowed in. This is
    /// used to construct helpful error messages, such as "Row can only be within a GridLayout element".
    context_restricted_types: HashMap<String, HashSet<String>>,
    parent_registry: Option<Rc<RefCell<TypeRegister>>>,
    /// If the lookup function should return types that are marked as internal
    pub(crate) expose_internal_types: bool,
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
        register.insert_type(Type::PhysicalLength);
        register.insert_type(Type::LogicalLength);
        register.insert_type(Type::Color);
        register.insert_type(Type::Duration);
        register.insert_type(Type::Image);
        register.insert_type(Type::Bool);
        register.insert_type(Type::Model);
        register.insert_type(Type::Percent);
        register.insert_type(Type::Easing);
        register.insert_type(Type::Angle);
        register.insert_type(Type::Brush);
        register.insert_type(Type::Rem);
        register.types.insert("Point".into(), logical_point_type());

        BUILTIN_ENUMS.with(|e| e.fill_register(&mut register));

        register.supported_property_animation_types.insert(Type::Float32.to_string());
        register.supported_property_animation_types.insert(Type::Int32.to_string());
        register.supported_property_animation_types.insert(Type::Color.to_string());
        register.supported_property_animation_types.insert(Type::PhysicalLength.to_string());
        register.supported_property_animation_types.insert(Type::LogicalLength.to_string());
        register.supported_property_animation_types.insert(Type::Brush.to_string());
        register.supported_property_animation_types.insert(Type::Angle.to_string());

        crate::load_builtins::load_builtins(&mut register);

        let mut context_restricted_types = HashMap::new();
        register
            .elements
            .values()
            .for_each(|ty| ty.collect_contextual_types(&mut context_restricted_types));
        register.context_restricted_types = context_restricted_types;

        match &mut register.elements.get_mut("PopupWindow").unwrap() {
            ElementType::Builtin(ref mut b) => {
                Rc::get_mut(b).unwrap().properties.insert(
                    "show".into(),
                    BuiltinPropertyInfo::new(BuiltinFunction::ShowPopupWindow.ty()),
                );
                Rc::get_mut(b)
                    .unwrap()
                    .member_functions
                    .insert("show".into(), BuiltinFunction::ShowPopupWindow);
                Rc::get_mut(b).unwrap().properties.insert(
                    "close".into(),
                    BuiltinPropertyInfo::new(BuiltinFunction::ClosePopupWindow.ty()),
                );
                Rc::get_mut(b)
                    .unwrap()
                    .member_functions
                    .insert("close".into(), BuiltinFunction::ClosePopupWindow);
            }

            _ => unreachable!(),
        };

        Rc::new(RefCell::new(register))
    }

    pub fn new(parent: &Rc<RefCell<TypeRegister>>) -> Self {
        Self {
            parent_registry: Some(parent.clone()),
            expose_internal_types: parent.borrow().expose_internal_types,
            ..Default::default()
        }
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
    ) -> Result<ElementType, HashMap<String, HashSet<String>>> {
        match self.elements.get(name).cloned() {
            Some(ty) => Ok(ty),
            None => match &self.parent_registry {
                Some(r) => r.borrow().lookup_element_as_result(name),
                None => Err(self.context_restricted_types.clone()),
            },
        }
    }

    pub fn lookup_element(&self, name: &str) -> Result<ElementType, String> {
        self.lookup_element_as_result(name).map_err(|context_restricted_types| {
            if let Some(permitted_parent_types) = context_restricted_types.get(name) {
                if permitted_parent_types.len() == 1 {
                    format!(
                        "{} can only be within a {} element",
                        name,
                        permitted_parent_types.iter().next().unwrap()
                    )
                } else {
                    let mut elements = permitted_parent_types.iter().cloned().collect::<Vec<_>>();
                    elements.sort();
                    format!(
                        "{} can only be within the following elements: {}",
                        name,
                        elements.join(", ")
                    )
                }
            } else if let Some(ty) = self.types.get(name) {
                format!("'{}' cannot be used as an element", ty)
            } else {
                format!("Unknown type {}", name)
            }
        })
    }

    pub fn lookup_builtin_element(&self, name: &str) -> Option<ElementType> {
        self.parent_registry.as_ref().map_or_else(
            || self.elements.get(name).cloned(),
            |p| p.borrow().lookup_builtin_element(name),
        )
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
        self.elements.insert(name, ElementType::Component(comp));
    }

    pub fn add_builtin(&mut self, builtin: Rc<BuiltinElement>) {
        self.elements.insert(builtin.name.clone(), ElementType::Builtin(builtin));
    }

    pub fn property_animation_type_for_property(&self, property_type: Type) -> ElementType {
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

    /// Return a hashmap with all the registered type
    pub fn all_types(&self) -> HashMap<String, Type> {
        let mut all =
            self.parent_registry.as_ref().map(|r| r.borrow().all_types()).unwrap_or_default();
        for (k, v) in &self.types {
            all.insert(k.clone(), v.clone());
        }
        all
    }

    /// Return a hashmap with all the registered element type
    pub fn all_elements(&self) -> HashMap<String, ElementType> {
        let mut all =
            self.parent_registry.as_ref().map(|r| r.borrow().all_elements()).unwrap_or_default();
        for (k, v) in &self.elements {
            all.insert(k.clone(), v.clone());
        }
        all
    }

    pub fn empty_type(&self) -> ElementType {
        match self.parent_registry.as_ref() {
            Some(parent) => parent.borrow().empty_type(),
            None => self.empty_type.clone(),
        }
    }
}

pub fn logical_point_type() -> Type {
    Type::Struct {
        fields: IntoIterator::into_iter([
            ("x".to_owned(), Type::LogicalLength),
            ("y".to_owned(), Type::LogicalLength),
        ])
        .collect(),
        name: Some("slint::LogicalPosition".into()),
        node: None,
        rust_attributes: None,
    }
}
