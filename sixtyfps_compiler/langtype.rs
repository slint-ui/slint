/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Display;
use std::rc::Rc;

use itertools::Itertools;

use crate::expression_tree::{Expression, Unit};
use crate::object_tree::Component;
use crate::typeregister::TypeRegister;

#[derive(Debug, Clone)]
pub enum Type {
    /// Correspond to an uninitialized type, or an error
    Invalid,
    /// The type of an expression that return nothing
    Void,
    Component(Rc<Component>),
    Builtin(Rc<BuiltinElement>),
    Native(Rc<NativeClass>),

    Callback {
        return_type: Option<Box<Type>>,
        args: Vec<Type>,
    },
    Function {
        return_type: Box<Type>,
        args: Vec<Type>,
    },

    // Other property types:
    Float32,
    Int32,
    String,
    Color,
    Duration,
    Length,
    LogicalLength,
    Angle,
    Percent,
    Resource,
    Bool,
    Model,
    PathElements,
    Easing,
    Brush,

    Array(Box<Type>),
    Object {
        fields: BTreeMap<String, Type>,
        name: Option<String>,
    },
    Enumeration(Rc<Enumeration>),

    /// A type made up of the product of several "unit" types.
    /// The first parameter is the unit, and the second parameter is the power.
    /// The vector should be sorted by 1) the power, 2) the unit.
    UnitProduct(Vec<(Unit, i8)>),

    ElementReference,
}

impl core::cmp::PartialEq for Type {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Type::Invalid => matches!(other, Type::Invalid),
            Type::Void => matches!(other, Type::Void),
            Type::Component(a) => matches!(other, Type::Component(b) if Rc::ptr_eq(a, b)),
            Type::Builtin(a) => matches!(other, Type::Builtin(b) if Rc::ptr_eq(a, b)),
            Type::Native(a) => matches!(other, Type::Native(b) if Rc::ptr_eq(a, b)),
            Type::Callback { args: a, return_type: ra } => {
                matches!(other, Type::Callback { args: b, return_type: rb } if a == b && ra == rb)
            }
            Type::Function { return_type: lhs_rt, args: lhs_args } => {
                matches!(other, Type::Function { return_type: rhs_rt, args: rhs_args } if lhs_rt == rhs_rt && lhs_args == rhs_args)
            }
            Type::Float32 => matches!(other, Type::Float32),
            Type::Int32 => matches!(other, Type::Int32),
            Type::String => matches!(other, Type::String),
            Type::Color => matches!(other, Type::Color),
            Type::Duration => matches!(other, Type::Duration),
            Type::Angle => matches!(other, Type::Angle),
            Type::Length => matches!(other, Type::Length),
            Type::LogicalLength => matches!(other, Type::LogicalLength),
            Type::Percent => matches!(other, Type::Percent),
            Type::Resource => matches!(other, Type::Resource),
            Type::Bool => matches!(other, Type::Bool),
            Type::Model => matches!(other, Type::Model),
            Type::PathElements => matches!(other, Type::PathElements),
            Type::Easing => matches!(other, Type::Easing),
            Type::Brush => matches!(other, Type::Brush),
            Type::Array(a) => matches!(other, Type::Array(b) if a == b),
            Type::Object { fields, name } => {
                matches!(other, Type::Object{fields: f, name: n} if fields == f && name == n)
            }
            Type::Enumeration(lhs) => matches!(other, Type::Enumeration(rhs) if lhs == rhs),
            Type::UnitProduct(a) => matches!(other, Type::UnitProduct(b) if a == b),
            Type::ElementReference => matches!(other, Type::ElementReference),
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Invalid => write!(f, "<error>"),
            Type::Void => write!(f, "void"),
            Type::Component(c) => c.id.fmt(f),
            Type::Builtin(b) => b.name.fmt(f),
            Type::Native(b) => b.class_name.fmt(f),
            Type::Callback { args, return_type } => {
                write!(f, "callback")?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ",")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?
                }
                if let Some(rt) = return_type {
                    write!(f, "-> {}", rt)?;
                }
                Ok(())
            }
            Type::Function { return_type, args } => {
                write!(f, "function(")?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ") -> {}", return_type)
            }
            Type::Float32 => write!(f, "float"),
            Type::Int32 => write!(f, "int"),
            Type::String => write!(f, "string"),
            Type::Duration => write!(f, "duration"),
            Type::Angle => write!(f, "angle"),
            Type::Length => write!(f, "length"),
            Type::LogicalLength => write!(f, "logical_length"),
            Type::Percent => write!(f, "percent"),
            Type::Color => write!(f, "color"),
            Type::Resource => write!(f, "resource"),
            Type::Bool => write!(f, "bool"),
            Type::Model => write!(f, "model"),
            Type::Array(t) => write!(f, "[{}]", t),
            Type::Object { name: Some(name), .. } => write!(f, "{}", name),
            Type::Object { fields, name: None } => {
                write!(f, "{{ ")?;
                for (k, v) in fields {
                    write!(f, "{}: {},", k, v)?;
                }
                write!(f, "}}")
            }

            Type::PathElements => write!(f, "pathelements"),
            Type::Easing => write!(f, "easing"),
            Type::Brush => write!(f, "brush"),
            Type::Enumeration(enumeration) => write!(f, "enum {}", enumeration.name),
            Type::UnitProduct(vec) => {
                const POWERS: &[char] = &['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
                let mut x = vec.iter().map(|(unit, power)| {
                    if *power == 1 {
                        return unit.to_string();
                    }
                    let mut res = format!("{}{}", unit, if *power < 0 { "⁻" } else { "" });
                    let value = power.abs().to_string();
                    for x in value.as_bytes() {
                        res.push(POWERS[(x - b'0') as usize]);
                    }

                    res
                });
                write!(f, "({})", x.join("×"))
            }
            Type::ElementReference => write!(f, "element ref"),
        }
    }
}

impl Type {
    pub fn is_object_type(&self) -> bool {
        matches!(self, Self::Component(_) | Self::Builtin(_))
    }

    /// valid type for properties
    pub fn is_property_type(&self) -> bool {
        matches!(
            self,
            Self::Float32
                | Self::Int32
                | Self::String
                | Self::Color
                | Self::Duration
                | Self::Angle
                | Self::Length
                | Self::LogicalLength
                | Self::Percent
                | Self::Resource
                | Self::Bool
                | Self::Model
                | Self::Easing
                | Self::Enumeration(_)
                | Self::ElementReference
                | Self::Object { .. }
                | Self::Array(_)
                | Self::Brush
        )
    }

    pub fn ok_for_public_api(&self) -> bool {
        // Duration and Easing don't have good types for public API exposure yet.
        !matches!(self, Self::Duration | Self::Easing | Self::Angle)
    }

    pub fn lookup_property<'a>(&self, name: &'a str) -> PropertyLookupResult<'a> {
        match self {
            Type::Component(c) => c.root_element.borrow().lookup_property(name),
            Type::Builtin(b) => {
                let resolved_name =
                    if let Some(alias_name) = b.native_class.lookup_alias(name.as_ref()) {
                        Cow::Owned(alias_name.to_string())
                    } else {
                        Cow::Borrowed(name)
                    };
                let property_type =
                    b.properties.get(resolved_name.as_ref()).cloned().unwrap_or_else(|| {
                        if b.is_non_item_type {
                            Type::Invalid
                        } else {
                            crate::typeregister::reserved_property(resolved_name.as_ref())
                        }
                    });
                PropertyLookupResult { resolved_name, property_type }
            }
            Type::Native(n) => {
                let resolved_name = if let Some(alias_name) = n.lookup_alias(name.as_ref()) {
                    Cow::Owned(alias_name.to_string())
                } else {
                    Cow::Borrowed(name)
                };
                let property_type = n.lookup_property(resolved_name.as_ref()).unwrap_or_default();
                PropertyLookupResult { resolved_name, property_type }
            }
            _ => PropertyLookupResult {
                resolved_name: Cow::Borrowed(name),
                property_type: Type::Invalid,
            },
        }
    }

    pub fn lookup_type_for_child_element(
        &self,
        name: &str,
        tr: &TypeRegister,
    ) -> Result<Type, String> {
        match self {
            Type::Component(component) => {
                return component
                    .root_element
                    .borrow()
                    .base_type
                    .lookup_type_for_child_element(name, tr)
            }
            Type::Builtin(builtin) => {
                if let Some(child_type) = builtin.additional_accepted_child_types.get(name) {
                    return Ok(child_type.clone());
                }
                if builtin.disallow_global_types_as_child_elements {
                    let mut valid_children: Vec<_> =
                        builtin.additional_accepted_child_types.keys().cloned().collect();
                    valid_children.sort();

                    return Err(format!(
                        "{} is not allowed within {}. Only {} are valid children",
                        name,
                        builtin.native_class.class_name,
                        valid_children.join(" ")
                    ));
                }
            }
            _ => {}
        };
        tr.lookup_element(name)
    }

    pub fn lookup_member_function(&self, name: &str) -> Expression {
        match self {
            Type::Builtin(builtin) => builtin
                .member_functions
                .get(name)
                .cloned()
                .unwrap_or(crate::typeregister::reserved_member_function(name)),
            Type::Component(component) => {
                component.root_element.borrow().base_type.lookup_member_function(name)
            }
            _ => Expression::Invalid,
        }
    }

    /// Assume this is a builtin type, panic if it isn't
    pub fn as_builtin(&self) -> &BuiltinElement {
        match self {
            Type::Builtin(b) => &b,
            Type::Component(_) => panic!("This should not happen because of inlining"),
            _ => panic!("invalid type"),
        }
    }

    /// Assume this is a builtin type, panic if it isn't
    pub fn as_native(&self) -> &NativeClass {
        match self {
            Type::Native(b) => &b,
            Type::Component(_) => {
                panic!("This should not happen because of native class resolution")
            }
            _ => panic!("invalid type"),
        }
    }

    /// Assime it is a Component, panic if it isn't
    pub fn as_component(&self) -> &Rc<Component> {
        match self {
            Type::Component(c) => c,
            _ => panic!("should be a component because of the repeater_component pass"),
        }
    }

    /// Return true if the type can be converted to the other type
    pub fn can_convert(&self, other: &Self) -> bool {
        let can_convert_object = |a: &BTreeMap<String, Type>, b: &BTreeMap<String, Type>| {
            // the object `b` has property that the object `a` doesn't
            let mut has_more_property = false;
            for (k, v) in b {
                match a.get(k) {
                    Some(t) if !t.can_convert(v) => return false,
                    None => has_more_property = true,
                    _ => (),
                }
            }
            if has_more_property {
                // we should reject the conversion if `a` has property that `b` doesn't have
                if a.keys().any(|k| !b.contains_key(k)) {
                    return false;
                }
            }
            true
        };

        match (self, other) {
            (a, b) if a == b => true,
            (_, Type::Invalid)
            | (_, Type::Void)
            | (Type::Float32, Type::Int32)
            | (Type::Float32, Type::String)
            | (Type::Int32, Type::Float32)
            | (Type::Int32, Type::String)
            | (Type::Array(_), Type::Model)
            | (Type::Float32, Type::Model)
            | (Type::Int32, Type::Model)
            | (Type::Length, Type::LogicalLength)
            | (Type::LogicalLength, Type::Length)
            | (Type::Percent, Type::Float32)
            | (Type::Brush, Type::Color)
            | (Type::Color, Type::Brush) => true,
            (Type::Object { fields: a, .. }, Type::Object { fields: b, .. }) => {
                can_convert_object(a, b)
            }
            _ => false,
        }
    }

    pub fn collect_contextual_types(
        &self,
        context_restricted_types: &mut HashMap<String, HashSet<String>>,
    ) {
        let builtin = match self {
            Type::Builtin(ty) => ty,
            _ => return,
        };
        for (accepted_child_type_name, accepted_child_type) in
            builtin.additional_accepted_child_types.iter()
        {
            context_restricted_types
                .entry(accepted_child_type_name.clone())
                .or_default()
                .insert(builtin.native_class.class_name.clone());

            accepted_child_type.collect_contextual_types(context_restricted_types);
        }
    }

    /// If this is a number type which should be used with an unit, this returns the default unit
    /// otherwise, returns None
    pub fn default_unit(&self) -> Option<Unit> {
        match self {
            Type::Duration => Some(Unit::Ms),
            Type::Length => Some(Unit::Phx),
            Type::LogicalLength => Some(Unit::Px),
            // Unit::Percent is special that it does not combine with other units like
            Type::Percent => None,
            Type::Angle => Some(Unit::Deg),
            Type::Invalid => None,
            Type::Void => None,
            Type::Component(_) => None,
            Type::Builtin(_) => None,
            Type::Native(_) => None,
            Type::Callback { .. } => None,
            Type::Function { .. } => None,
            Type::Float32 => None,
            Type::Int32 => None,
            Type::String => None,
            Type::Color => None,
            Type::Resource => None,
            Type::Bool => None,
            Type::Model => None,
            Type::PathElements => None,
            Type::Easing => None,
            Type::Brush => None,
            Type::Array(_) => None,
            Type::Object { .. } => None,
            Type::Enumeration(_) => None,
            Type::UnitProduct(_) => None,
            Type::ElementReference => None,
        }
    }
}

impl Default for Type {
    fn default() -> Self {
        Self::Invalid
    }
}

#[derive(Debug, Clone, Default)]
pub struct NativeClass {
    pub parent: Option<Rc<NativeClass>>,
    pub class_name: String,
    pub vtable_symbol: String,
    pub properties: HashMap<String, Type>,
    pub deprecated_aliases: HashMap<String, String>,
    pub cpp_type: Option<String>,
    pub rust_type_constructor: Option<String>,
}

impl NativeClass {
    pub fn new(class_name: &str) -> Self {
        let vtable_symbol = format!("{}VTable", class_name);
        Self {
            class_name: class_name.into(),
            vtable_symbol,
            properties: Default::default(),
            ..Default::default()
        }
    }

    pub fn new_with_properties(
        class_name: &str,
        properties: impl IntoIterator<Item = (String, Type)>,
    ) -> Self {
        let mut class = Self::new(class_name);
        class.properties = properties.into_iter().collect();
        class
    }

    pub fn property_count(&self) -> usize {
        self.properties.len() + self.parent.clone().map(|p| p.property_count()).unwrap_or_default()
    }

    pub fn local_property_iter(&self) -> impl Iterator<Item = (&String, &Type)> {
        self.properties.iter()
    }

    pub fn visit_class_hierarchy(self: Rc<Self>, mut visitor: impl FnMut(&Rc<Self>)) {
        visitor(&self);
        if let Some(parent_class) = &self.parent {
            parent_class.clone().visit_class_hierarchy(visitor)
        }
    }

    pub fn lookup_property(&self, name: &str) -> Option<Type> {
        if let Some(ty) = self.properties.get(name) {
            Some(ty.clone())
        } else if let Some(parent_class) = &self.parent {
            parent_class.lookup_property(name)
        } else {
            None
        }
    }

    pub fn lookup_alias(&self, name: &str) -> Option<&str> {
        if let Some(alias_target) = self.deprecated_aliases.get(name) {
            Some(alias_target)
        } else if self.properties.contains_key(name) {
            None
        } else if let Some(parent_class) = &self.parent {
            parent_class.lookup_alias(name)
        } else {
            None
        }
    }

    fn lookup_property_distance(self: Rc<Self>, name: &str) -> (usize, Rc<Self>) {
        let mut distance = 0;
        let mut class = self;
        loop {
            if class.properties.contains_key(name) {
                return (distance, class);
            }
            distance += 1;
            class = class.parent.as_ref().unwrap().clone();
        }
    }

    pub fn select_minimal_class_based_on_property_usage<'a>(
        self: Rc<Self>,
        properties_used: impl Iterator<Item = &'a String>,
    ) -> Rc<Self> {
        let mut minimal_class = self.clone();
        while let Some(class) = minimal_class.parent.clone() {
            minimal_class = class;
        }
        let (_min_distance, minimal_class) = properties_used.fold(
            (std::usize::MAX, minimal_class),
            |(current_distance, current_class), prop_name| {
                let (prop_distance, prop_class) = self.clone().lookup_property_distance(&prop_name);

                if prop_distance < current_distance {
                    (prop_distance, prop_class)
                } else {
                    (current_distance, current_class)
                }
            },
        );
        minimal_class
    }
}

#[derive(Debug, Clone)]
pub enum DefaultSizeBinding {
    /// There should not be a default binding for the size
    None,
    /// The size should default to `width:100%; height:100%`
    ExpandsToParentGeometry,
    /// The size should default to the item's implicit size
    ImplicitSize,
}

impl Default for DefaultSizeBinding {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Default)]
pub struct BuiltinElement {
    pub name: String,
    pub native_class: Rc<NativeClass>,
    pub properties: HashMap<String, Type>,
    pub default_bindings: HashMap<String, Expression>,
    pub additional_accepted_child_types: HashMap<String, Type>,
    pub disallow_global_types_as_child_elements: bool,
    /// Non-item type do not have reserved properties (x/width/rowspan/...) added to them  (eg: PropertyAnimation)
    pub is_non_item_type: bool,
    pub accepts_focus: bool,
    pub member_functions: HashMap<String, Expression>,
    pub is_global: bool,
    pub default_size_binding: DefaultSizeBinding,
}

impl BuiltinElement {
    pub fn new(native_class: Rc<NativeClass>) -> Self {
        let mut properties = HashMap::new();
        native_class.clone().visit_class_hierarchy(|class| {
            for (prop_name, prop_type) in class.local_property_iter() {
                properties.insert(prop_name.clone(), prop_type.clone());
            }
        });
        Self {
            name: native_class.class_name.clone(),
            native_class,
            properties,
            ..Default::default()
        }
    }
}

#[test]
fn test_select_minimal_class_based_on_property_usage() {
    let first = Rc::new(NativeClass::new_with_properties(
        "first_class",
        [("first_prop".to_owned(), Type::Int32)].iter().cloned(),
    ));

    let mut second = NativeClass::new_with_properties(
        "second_class",
        [("second_prop".to_owned(), Type::Int32)].iter().cloned(),
    );
    second.parent = Some(first.clone());
    let second = Rc::new(second);

    let reduce_to_first = second
        .clone()
        .select_minimal_class_based_on_property_usage(["first_prop".to_owned()].iter());

    assert_eq!(reduce_to_first.class_name, first.class_name);

    let reduce_to_second = second
        .clone()
        .select_minimal_class_based_on_property_usage(["second_prop".to_owned()].iter());

    assert_eq!(reduce_to_second.class_name, second.class_name);

    let reduce_to_second = second.clone().select_minimal_class_based_on_property_usage(
        ["first_prop".to_owned(), "second_prop".to_owned()].iter(),
    );

    assert_eq!(reduce_to_second.class_name, second.class_name);
}

#[derive(PartialEq, Debug)]
pub struct PropertyLookupResult<'a> {
    pub resolved_name: std::borrow::Cow<'a, str>,
    pub property_type: Type,
}

#[derive(Debug, Clone)]
pub struct Enumeration {
    pub name: String,
    pub values: Vec<String>,
    pub default_value: usize, // index in values
}

impl PartialEq for Enumeration {
    fn eq(&self, other: &Self) -> bool {
        self.name.eq(&other.name)
    }
}

impl Enumeration {
    pub fn default_value(self: Rc<Self>) -> EnumerationValue {
        EnumerationValue { value: self.default_value, enumeration: self.clone() }
    }

    pub fn try_value_from_string(self: Rc<Self>, value: &str) -> Option<EnumerationValue> {
        self.values.iter().enumerate().find_map(|(idx, name)| {
            if name == value {
                Some(EnumerationValue { value: idx, enumeration: self.clone() })
            } else {
                None
            }
        })
    }
}

#[derive(Clone, Debug)]
pub struct EnumerationValue {
    pub value: usize, // index in enumeration.values
    pub enumeration: Rc<Enumeration>,
}

impl PartialEq for EnumerationValue {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.enumeration, &other.enumeration) && self.value == other.value
    }
}

impl std::fmt::Display for EnumerationValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.enumeration.values[self.value].fmt(f)
    }
}
