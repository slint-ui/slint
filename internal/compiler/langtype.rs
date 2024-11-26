// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Display;
use std::rc::Rc;

use itertools::Itertools;

use smol_str::SmolStr;

use crate::expression_tree::{BuiltinFunction, Expression, Unit};
use crate::object_tree::{Component, PropertyVisibility};
use crate::parser::syntax_nodes;
use crate::typeregister::TypeRegister;

#[derive(Debug, Clone, Default)]
pub enum Type {
    /// Correspond to an uninitialized type, or an error
    #[default]
    Invalid,
    /// The type of an expression that return nothing
    Void,
    /// The type of a property two way binding whose type was not yet inferred
    InferredProperty,
    /// The type of a callback alias whose type was not yet inferred
    InferredCallback,

    Callback(Rc<Function>),
    Function(Rc<Function>),

    ComponentFactory,

    // Other property types:
    Float32,
    Int32,
    String,
    Color,
    Duration,
    PhysicalLength,
    LogicalLength,
    Rem,
    Angle,
    Percent,
    Image,
    Bool,
    /// Fake type that can represent anything that can be converted into a model.
    Model,
    PathData, // Either a vector of path elements or a two vectors of events and coordinates
    Easing,
    Brush,
    /// This is usually a model
    Array(Rc<Type>),
    Struct(Rc<Struct>),
    Enumeration(Rc<Enumeration>),

    /// A type made up of the product of several "unit" types.
    /// The first parameter is the unit, and the second parameter is the power.
    /// The vector should be sorted by 1) the power, 2) the unit.
    UnitProduct(Vec<(Unit, i8)>),

    ElementReference,

    /// This is a `SharedArray<f32>`
    LayoutCache,
}

impl core::cmp::PartialEq for Type {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Type::Invalid => matches!(other, Type::Invalid),
            Type::Void => matches!(other, Type::Void),
            Type::InferredProperty => matches!(other, Type::InferredProperty),
            Type::InferredCallback => matches!(other, Type::InferredCallback),
            Type::Callback(lhs) => {
                matches!(other, Type::Callback(rhs) if lhs == rhs)
            }
            Type::Function(lhs) => {
                matches!(other, Type::Function(rhs) if lhs == rhs)
            }
            Type::ComponentFactory => matches!(other, Type::ComponentFactory),
            Type::Float32 => matches!(other, Type::Float32),
            Type::Int32 => matches!(other, Type::Int32),
            Type::String => matches!(other, Type::String),
            Type::Color => matches!(other, Type::Color),
            Type::Duration => matches!(other, Type::Duration),
            Type::Angle => matches!(other, Type::Angle),
            Type::PhysicalLength => matches!(other, Type::PhysicalLength),
            Type::LogicalLength => matches!(other, Type::LogicalLength),
            Type::Rem => matches!(other, Type::Rem),
            Type::Percent => matches!(other, Type::Percent),
            Type::Image => matches!(other, Type::Image),
            Type::Bool => matches!(other, Type::Bool),
            Type::Model => matches!(other, Type::Model),
            Type::PathData => matches!(other, Type::PathData),
            Type::Easing => matches!(other, Type::Easing),
            Type::Brush => matches!(other, Type::Brush),
            Type::Array(a) => matches!(other, Type::Array(b) if a == b),
            Type::Struct(lhs) => {
                matches!(other, Type::Struct(rhs) if lhs.fields == rhs.fields && lhs.name == rhs.name)
            }
            Type::Enumeration(lhs) => matches!(other, Type::Enumeration(rhs) if lhs == rhs),
            Type::UnitProduct(a) => matches!(other, Type::UnitProduct(b) if a == b),
            Type::ElementReference => matches!(other, Type::ElementReference),
            Type::LayoutCache => matches!(other, Type::LayoutCache),
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Invalid => write!(f, "<error>"),
            Type::Void => write!(f, "void"),
            Type::InferredProperty => write!(f, "?"),
            Type::InferredCallback => write!(f, "callback"),
            Type::Callback(callback) => {
                write!(f, "callback")?;
                if !callback.args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in callback.args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ",")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?
                }
                write!(f, "-> {}", callback.return_type)?;
                Ok(())
            }
            Type::ComponentFactory => write!(f, "component-factory"),
            Type::Function(function) => {
                write!(f, "function(")?;
                for (i, arg) in function.args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ") -> {}", function.return_type)
            }
            Type::Float32 => write!(f, "float"),
            Type::Int32 => write!(f, "int"),
            Type::String => write!(f, "string"),
            Type::Duration => write!(f, "duration"),
            Type::Angle => write!(f, "angle"),
            Type::PhysicalLength => write!(f, "physical-length"),
            Type::LogicalLength => write!(f, "length"),
            Type::Rem => write!(f, "relative-font-size"),
            Type::Percent => write!(f, "percent"),
            Type::Color => write!(f, "color"),
            Type::Image => write!(f, "image"),
            Type::Bool => write!(f, "bool"),
            Type::Model => write!(f, "model"),
            Type::Array(t) => write!(f, "[{}]", t),
            Type::Struct(t) => {
                if let Some(name) = &t.name {
                    if let Some(separator_pos) = name.rfind("::") {
                        // write the slint type and not the native type
                        write!(f, "{}", &name[separator_pos + 2..])
                    } else {
                        write!(f, "{}", name)
                    }
                } else {
                    write!(f, "{{ ")?;
                    for (k, v) in &t.fields {
                        write!(f, "{}: {},", k, v)?;
                    }
                    write!(f, "}}")
                }
            }
            Type::PathData => write!(f, "pathdata"),
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
            Type::LayoutCache => write!(f, "layout cache"),
        }
    }
}

impl Type {
    /// valid type for properties
    pub fn is_property_type(&self) -> bool {
        matches!(
            self,
            Self::Float32
                | Self::Int32
                | Self::String
                | Self::Color
                | Self::ComponentFactory
                | Self::Duration
                | Self::Angle
                | Self::PhysicalLength
                | Self::LogicalLength
                | Self::Rem
                | Self::Percent
                | Self::Image
                | Self::Bool
                | Self::Easing
                | Self::Enumeration(_)
                | Self::ElementReference
                | Self::Struct { .. }
                | Self::Array(_)
                | Self::Brush
                | Self::InferredProperty
        )
    }

    pub fn ok_for_public_api(&self) -> bool {
        !matches!(self, Self::Easing)
    }

    /// Assume it is an enumeration, panic if it isn't
    pub fn as_enum(&self) -> &Rc<Enumeration> {
        match self {
            Type::Enumeration(e) => e,
            _ => panic!("should be an enumeration, bug in compiler pass"),
        }
    }

    /// Return true if the type can be converted to the other type
    pub fn can_convert(&self, other: &Self) -> bool {
        let can_convert_struct = |a: &BTreeMap<SmolStr, Type>, b: &BTreeMap<SmolStr, Type>| {
            // the struct `b` has property that the struct `a` doesn't
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
            | (Type::Float32, Type::Model)
            | (Type::Int32, Type::Model)
            | (Type::PhysicalLength, Type::LogicalLength)
            | (Type::LogicalLength, Type::PhysicalLength)
            | (Type::Rem, Type::LogicalLength)
            | (Type::Rem, Type::PhysicalLength)
            | (Type::LogicalLength, Type::Rem)
            | (Type::PhysicalLength, Type::Rem)
            | (Type::Percent, Type::Float32)
            | (Type::Brush, Type::Color)
            | (Type::Color, Type::Brush) => true,
            (Type::Array(a), Type::Model) if a.is_property_type() => true,
            (Type::Struct(a), Type::Struct(b)) => can_convert_struct(&a.fields, &b.fields),
            (Type::UnitProduct(u), o) => match o.as_unit_product() {
                Some(o) => unit_product_length_conversion(u.as_slice(), o.as_slice()).is_some(),
                None => false,
            },
            (o, Type::UnitProduct(u)) => match o.as_unit_product() {
                Some(o) => unit_product_length_conversion(u.as_slice(), o.as_slice()).is_some(),
                None => false,
            },
            _ => false,
        }
    }

    /// If this is a number type which should be used with an unit, this returns the default unit
    /// otherwise, returns None
    pub fn default_unit(&self) -> Option<Unit> {
        match self {
            Type::Duration => Some(Unit::Ms),
            Type::PhysicalLength => Some(Unit::Phx),
            Type::LogicalLength => Some(Unit::Px),
            Type::Rem => Some(Unit::Rem),
            // Unit::Percent is special that it does not combine with other units like
            Type::Percent => None,
            Type::Angle => Some(Unit::Deg),
            Type::Invalid => None,
            Type::Void => None,
            Type::InferredProperty | Type::InferredCallback => None,
            Type::Callback { .. } => None,
            Type::ComponentFactory => None,
            Type::Function { .. } => None,
            Type::Float32 => None,
            Type::Int32 => None,
            Type::String => None,
            Type::Color => None,
            Type::Image => None,
            Type::Bool => None,
            Type::Model => None,
            Type::PathData => None,
            Type::Easing => None,
            Type::Brush => None,
            Type::Array(_) => None,
            Type::Struct { .. } => None,
            Type::Enumeration(_) => None,
            Type::UnitProduct(_) => None,
            Type::ElementReference => None,
            Type::LayoutCache => None,
        }
    }

    /// Return a unit product vector even for single scalar
    pub fn as_unit_product(&self) -> Option<Vec<(Unit, i8)>> {
        match self {
            Type::UnitProduct(u) => Some(u.clone()),
            Type::Float32 | Type::Int32 => Some(Vec::new()),
            Type::Percent => Some(Vec::new()),
            _ => self.default_unit().map(|u| vec![(u, 1)]),
        }
    }
}

#[derive(Debug, Clone)]

pub enum BuiltinPropertyDefault {
    None,
    Expr(Expression),
    Fn(fn(&crate::object_tree::ElementRc) -> Expression),
}

impl BuiltinPropertyDefault {
    pub fn expr(&self, elem: &crate::object_tree::ElementRc) -> Option<Expression> {
        match self {
            BuiltinPropertyDefault::None => None,
            BuiltinPropertyDefault::Expr(expression) => Some(expression.clone()),
            BuiltinPropertyDefault::Fn(init_expr) => Some(init_expr(elem)),
        }
    }
}

/// Information about properties in NativeClass
#[derive(Debug, Clone)]
pub struct BuiltinPropertyInfo {
    /// The property type
    pub ty: Type,
    /// When != None, this is the initial value that we will have to set if no other binding were specified
    pub default_value: BuiltinPropertyDefault,
    pub property_visibility: PropertyVisibility,
}

impl BuiltinPropertyInfo {
    pub fn new(ty: Type) -> Self {
        Self {
            ty,
            default_value: BuiltinPropertyDefault::None,
            property_visibility: PropertyVisibility::InOut,
        }
    }

    pub fn is_native_output(&self) -> bool {
        matches!(self.property_visibility, PropertyVisibility::InOut | PropertyVisibility::Output)
    }
}

/// The base of an element
#[derive(Clone, Debug, derive_more::From)]
pub enum ElementType {
    /// The element is based of a component
    Component(Rc<Component>),
    /// The element is a builtin element
    Builtin(Rc<BuiltinElement>),
    /// The native type was resolved by the resolve_native_class pass.
    Native(Rc<NativeClass>),
    /// The base element couldn't be looked up
    Error,
    /// This should be the base type of the root element of a global component
    Global,
}

impl PartialEq for ElementType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Component(a), Self::Component(b)) => Rc::ptr_eq(a, b),
            (Self::Builtin(a), Self::Builtin(b)) => Rc::ptr_eq(a, b),
            (Self::Native(a), Self::Native(b)) => Rc::ptr_eq(a, b),
            (Self::Error, Self::Error) | (Self::Global, Self::Global) => true,
            _ => false,
        }
    }
}

impl ElementType {
    pub fn lookup_property<'a>(&self, name: &'a str) -> PropertyLookupResult<'a> {
        match self {
            Self::Component(c) => c.root_element.borrow().lookup_property(name),
            Self::Builtin(b) => {
                let resolved_name =
                    if let Some(alias_name) = b.native_class.lookup_alias(name.as_ref()) {
                        Cow::Owned(alias_name.to_string())
                    } else {
                        Cow::Borrowed(name)
                    };
                match b.properties.get(resolved_name.as_ref()) {
                    None => {
                        if b.is_non_item_type {
                            PropertyLookupResult {
                                resolved_name,
                                property_type: Type::Invalid,
                                property_visibility: PropertyVisibility::Private,
                                declared_pure: None,
                                is_local_to_component: false,
                                is_in_direct_base: false,
                            }
                        } else {
                            crate::typeregister::reserved_property(name)
                        }
                    }
                    Some(p) => PropertyLookupResult {
                        resolved_name,
                        property_type: p.ty.clone(),
                        property_visibility: p.property_visibility,
                        declared_pure: None,
                        is_local_to_component: false,
                        is_in_direct_base: false,
                    },
                }
            }
            Self::Native(n) => {
                let resolved_name = if let Some(alias_name) = n.lookup_alias(name.as_ref()) {
                    Cow::Owned(alias_name.to_string())
                } else {
                    Cow::Borrowed(name)
                };
                let property_type =
                    n.lookup_property(resolved_name.as_ref()).cloned().unwrap_or_default();
                PropertyLookupResult {
                    resolved_name,
                    property_type,
                    property_visibility: PropertyVisibility::InOut,
                    declared_pure: None,
                    is_local_to_component: false,
                    is_in_direct_base: false,
                }
            }
            _ => PropertyLookupResult {
                resolved_name: Cow::Borrowed(name),
                property_type: Type::Invalid,
                property_visibility: PropertyVisibility::Private,
                declared_pure: None,
                is_local_to_component: false,
                is_in_direct_base: false,
            },
        }
    }

    /// List of sub properties valid for the auto completion
    pub fn property_list(&self) -> Vec<(SmolStr, Type)> {
        match self {
            Self::Component(c) => {
                let mut r = c.root_element.borrow().base_type.property_list();
                r.extend(
                    c.root_element
                        .borrow()
                        .property_declarations
                        .iter()
                        .map(|(k, d)| (k.clone(), d.property_type.clone())),
                );
                r
            }
            Self::Builtin(b) => {
                b.properties.iter().map(|(k, t)| (k.clone(), t.ty.clone())).collect()
            }
            Self::Native(n) => {
                n.properties.iter().map(|(k, t)| (k.clone(), t.ty.clone())).collect()
            }
            _ => Vec::new(),
        }
    }

    /// This function looks at the element and checks whether it can have Elements of type `name` as children.
    /// It returns an Error if that is not possible or an Option of the ElementType if it is.
    /// The option is unset when the compiler does not know the type well enough to avoid further
    /// probing.
    pub fn accepts_child_element(
        &self,
        name: &str,
        tr: &TypeRegister,
    ) -> Result<Option<ElementType>, String> {
        match self {
            Self::Component(component) if component.child_insertion_point.borrow().is_none() => {
                let base_type = component.root_element.borrow().base_type.clone();
                if base_type == tr.empty_type() {
                    return Err(format!("'{}' cannot have children. Only components with @children can have children", component.id));
                }
                return base_type.accepts_child_element(name, tr);
            }
            Self::Builtin(builtin) => {
                if let Some(child_type) = builtin.additional_accepted_child_types.get(name) {
                    return Ok(Some(child_type.clone()));
                }
                if builtin.disallow_global_types_as_child_elements {
                    let mut valid_children: Vec<_> =
                        builtin.additional_accepted_child_types.keys().cloned().collect();
                    valid_children.sort();

                    let err = if valid_children.is_empty() {
                        format!("{} cannot have children elements", builtin.native_class.class_name,)
                    } else {
                        format!(
                            "{} is not allowed within {}. Only {} are valid children",
                            name,
                            builtin.native_class.class_name,
                            valid_children.join(" ")
                        )
                    };

                    return Err(err);
                }
            }
            _ => {}
        };
        Ok(None)
    }

    /// This function looks at the element and checks whether it can have Elements of type `name` as children.
    /// In addition to what `accepts_child_element` does, this method also probes the type of `name`.
    /// It returns an Error if that is not possible or an `ElementType` if it is.
    pub fn lookup_type_for_child_element(
        &self,
        name: &str,
        tr: &TypeRegister,
    ) -> Result<ElementType, String> {
        if let Some(ct) = self.accepts_child_element(name, tr)? {
            return Ok(ct);
        }

        tr.lookup_element(name).and_then(|t| {
            if !tr.expose_internal_types && matches!(&t, Self::Builtin(e) if e.is_internal) {
                Err(format!("Unknown element '{}'. (The type exist as an internal type, but cannot be accessed in this scope)", name))
            } else {
                Ok(t)
            }
        }).map_err(|s| {
            match tr.lookup(name)  {
                Type::Invalid => s,
                ty => format!("'{ty}' cannot be used as an element")
            }
        })
    }

    pub fn lookup_member_function(&self, name: &str) -> Option<BuiltinFunction> {
        match self {
            Self::Builtin(builtin) => builtin
                .member_functions
                .get(name)
                .cloned()
                .or_else(|| crate::typeregister::reserved_member_function(name)),
            Self::Component(component) => {
                component.root_element.borrow().base_type.lookup_member_function(name)
            }
            _ => None,
        }
    }

    pub fn collect_contextual_types(
        &self,
        context_restricted_types: &mut HashMap<SmolStr, HashSet<SmolStr>>,
    ) {
        let builtin = match self {
            Self::Builtin(ty) => ty,
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

    /// Assume this is a builtin type, panic if it isn't
    pub fn as_builtin(&self) -> &BuiltinElement {
        match self {
            Self::Builtin(b) => b,
            Self::Component(_) => panic!("This should not happen because of inlining"),
            _ => panic!("invalid type"),
        }
    }

    /// Assume this is a builtin type, panic if it isn't
    pub fn as_native(&self) -> &NativeClass {
        match self {
            Self::Native(b) => b,
            Self::Component(_) => {
                panic!("This should not happen because of native class resolution")
            }
            _ => panic!("invalid type"),
        }
    }

    /// Assume it is a Component, panic if it isn't
    pub fn as_component(&self) -> &Rc<Component> {
        match self {
            Self::Component(c) => c,
            _ => panic!("should be a component because of the repeater_component pass"),
        }
    }

    /// Returns the Slint type name if applicable (for example `Rectangle` or `MyButton` when `component MyButton {}` is used as `MyButton` element)
    pub fn type_name(&self) -> Option<&str> {
        match self {
            ElementType::Component(component) => Some(&component.id),
            ElementType::Builtin(b) => Some(&b.name),
            ElementType::Native(_) => None, // Too late, caller should call this function before the native class lowering
            ElementType::Error => None,
            ElementType::Global => None,
        }
    }
}

impl Display for ElementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Component(c) => c.id.fmt(f),
            Self::Builtin(b) => b.name.fmt(f),
            Self::Native(b) => b.class_name.fmt(f),
            Self::Error => write!(f, "<error>"),
            Self::Global => Ok(()),
        }
    }
}

impl Default for ElementType {
    fn default() -> Self {
        Self::Error
    }
}

#[derive(Debug, Clone, Default)]
pub struct NativeClass {
    pub parent: Option<Rc<NativeClass>>,
    pub class_name: SmolStr,
    pub cpp_vtable_getter: String,
    pub properties: HashMap<SmolStr, BuiltinPropertyInfo>,
    pub deprecated_aliases: HashMap<SmolStr, SmolStr>,
    pub cpp_type: Option<SmolStr>,
    pub rust_type_constructor: Option<SmolStr>,
}

impl NativeClass {
    pub fn new(class_name: &str) -> Self {
        let cpp_vtable_getter = format!("SLINT_GET_ITEM_VTABLE({}VTable)", class_name);
        Self {
            class_name: class_name.into(),
            cpp_vtable_getter,
            properties: Default::default(),
            ..Default::default()
        }
    }

    pub fn new_with_properties(
        class_name: &str,
        properties: impl IntoIterator<Item = (SmolStr, BuiltinPropertyInfo)>,
    ) -> Self {
        let mut class = Self::new(class_name);
        class.properties = properties.into_iter().collect();
        class
    }

    pub fn property_count(&self) -> usize {
        self.properties.len() + self.parent.clone().map(|p| p.property_count()).unwrap_or_default()
    }

    pub fn lookup_property(&self, name: &str) -> Option<&Type> {
        if let Some(bty) = self.properties.get(name) {
            Some(&bty.ty)
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
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DefaultSizeBinding {
    /// There should not be a default binding for the size
    #[default]
    None,
    /// The size should default to `width:100%; height:100%`
    ExpandsToParentGeometry,
    /// The size should default to the item's implicit size
    ImplicitSize,
}

#[derive(Debug, Clone, Default)]
pub struct BuiltinElement {
    pub name: SmolStr,
    pub native_class: Rc<NativeClass>,
    pub properties: BTreeMap<SmolStr, BuiltinPropertyInfo>,
    pub additional_accepted_child_types: HashMap<SmolStr, ElementType>,
    pub disallow_global_types_as_child_elements: bool,
    /// Non-item type do not have reserved properties (x/width/rowspan/...) added to them  (eg: PropertyAnimation)
    pub is_non_item_type: bool,
    pub accepts_focus: bool,
    pub member_functions: HashMap<SmolStr, BuiltinFunction>,
    pub is_global: bool,
    pub default_size_binding: DefaultSizeBinding,
    /// When true this is an internal type not shown in the auto-completion
    pub is_internal: bool,
}

impl BuiltinElement {
    pub fn new(native_class: Rc<NativeClass>) -> Self {
        Self { name: native_class.class_name.clone(), native_class, ..Default::default() }
    }
}

#[derive(PartialEq, Debug)]
pub struct PropertyLookupResult<'a> {
    pub resolved_name: std::borrow::Cow<'a, str>,
    pub property_type: Type,
    pub property_visibility: PropertyVisibility,
    pub declared_pure: Option<bool>,
    /// True if the property is part of the the current component (for visibility purposes)
    pub is_local_to_component: bool,
    /// True if the property in the direct base of the component (for visibility purposes)
    pub is_in_direct_base: bool,
}

impl<'a> PropertyLookupResult<'a> {
    pub fn is_valid(&self) -> bool {
        self.property_type != Type::Invalid
    }

    /// Can this property be used in an assignment
    pub fn is_valid_for_assignment(&self) -> bool {
        !matches!(
            (self.property_visibility, self.is_local_to_component),
            (PropertyVisibility::Private, false)
                | (PropertyVisibility::Input, true)
                | (PropertyVisibility::Output, false)
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub return_type: Type,
    pub args: Vec<Type>,
    /// The optional names of the arguments (empty string means not set).
    /// The names are not technically part of the type, but it is good to have them available for auto-completion
    pub arg_names: Vec<SmolStr>,
}

#[derive(Debug, Clone)]
pub struct Struct {
    pub fields: BTreeMap<SmolStr, Type>,
    /// When declared in .slint as  `struct Foo := { }`, then the name is "Foo"
    /// When there is no node, but there is a name, then it is a builtin type
    pub name: Option<SmolStr>,
    /// When declared in .slint, this is the node of the declaration.
    pub node: Option<syntax_nodes::ObjectType>,
    /// derived
    pub rust_attributes: Option<Vec<SmolStr>>,
}

#[derive(Debug, Clone)]
pub struct Enumeration {
    pub name: SmolStr,
    pub values: Vec<SmolStr>,
    pub default_value: usize, // index in values
    // For non-builtins enums, this is the declaration node
    pub node: Option<syntax_nodes::EnumDeclaration>,
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

impl EnumerationValue {
    pub fn to_pascal_case(&self) -> String {
        crate::generator::to_pascal_case(&self.enumeration.values[self.value])
    }
}

#[derive(Debug, PartialEq)]
pub struct LengthConversionPowers {
    pub rem_to_px_power: i8,
    pub px_to_phx_power: i8,
}

/// If the `Type::UnitProduct(a)` can be converted to `Type::UnitProduct(a)` by multiplying
/// by the scale factor, return that scale factor, otherwise, return None
pub fn unit_product_length_conversion(
    a: &[(Unit, i8)],
    b: &[(Unit, i8)],
) -> Option<LengthConversionPowers> {
    let mut units = [0i8; 16];
    for (u, count) in a {
        units[*u as usize] += count;
    }
    for (u, count) in b {
        units[*u as usize] -= count;
    }

    if units[Unit::Px as usize] + units[Unit::Phx as usize] + units[Unit::Rem as usize] != 0 {
        return None;
    }

    if units[Unit::Rem as usize] != 0
        && units[Unit::Phx as usize] == -units[Unit::Rem as usize]
        && units[Unit::Px as usize] == 0
    {
        units[Unit::Px as usize] = -units[Unit::Rem as usize];
        units[Unit::Phx as usize] = -units[Unit::Rem as usize];
    }

    let result = LengthConversionPowers {
        rem_to_px_power: if units[Unit::Rem as usize] != 0 { units[Unit::Px as usize] } else { 0 },
        px_to_phx_power: if units[Unit::Px as usize] != 0 { units[Unit::Phx as usize] } else { 0 },
    };

    units[Unit::Px as usize] = 0;
    units[Unit::Phx as usize] = 0;
    units[Unit::Rem as usize] = 0;
    units.into_iter().all(|x| x == 0).then_some(result)
}

#[test]
fn unit_product_length_conversion_test() {
    use Option::None;
    use Unit::*;
    assert_eq!(
        unit_product_length_conversion(&[(Px, 1)], &[(Phx, 1)]),
        Some(LengthConversionPowers { rem_to_px_power: 0, px_to_phx_power: -1 })
    );
    assert_eq!(
        unit_product_length_conversion(&[(Phx, -2)], &[(Px, -2)]),
        Some(LengthConversionPowers { rem_to_px_power: 0, px_to_phx_power: -2 })
    );
    assert_eq!(
        unit_product_length_conversion(&[(Px, 1), (Phx, -2)], &[(Phx, -1)]),
        Some(LengthConversionPowers { rem_to_px_power: 0, px_to_phx_power: -1 })
    );
    assert_eq!(
        unit_product_length_conversion(
            &[(Deg, 3), (Phx, 2), (Ms, -1)],
            &[(Phx, 4), (Deg, 3), (Ms, -1), (Px, -2)]
        ),
        Some(LengthConversionPowers { rem_to_px_power: 0, px_to_phx_power: -2 })
    );
    assert_eq!(unit_product_length_conversion(&[(Px, 1)], &[(Phx, -1)]), None);
    assert_eq!(unit_product_length_conversion(&[(Deg, 1), (Phx, -2)], &[(Px, -2)]), None);
    assert_eq!(unit_product_length_conversion(&[(Px, 1)], &[(Phx, -1)]), None);

    assert_eq!(
        unit_product_length_conversion(&[(Rem, 1)], &[(Px, 1)]),
        Some(LengthConversionPowers { rem_to_px_power: -1, px_to_phx_power: 0 })
    );
    assert_eq!(
        unit_product_length_conversion(&[(Rem, 1)], &[(Phx, 1)]),
        Some(LengthConversionPowers { rem_to_px_power: -1, px_to_phx_power: -1 })
    );
    assert_eq!(
        unit_product_length_conversion(&[(Rem, 2)], &[(Phx, 2)]),
        Some(LengthConversionPowers { rem_to_px_power: -2, px_to_phx_power: -2 })
    );
}
