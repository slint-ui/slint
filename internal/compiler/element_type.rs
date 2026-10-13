// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The base type of an element, and the lookup of its properties

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::rc::Rc;
use std::sync::Arc;

use smol_str::SmolStr;

use crate::builtin_functions::BuiltinFunction;
use crate::langtype::Type;
use crate::native_class::{
    BuiltinPropertyDefault, BuiltinPropertyInfo, NativeClass, PropertyVisibility,
};
use crate::object_tree::{Component, DEFAULT_SLOT_NAME};
use crate::parser::SyntaxNode;
use crate::typeregister::TypeRegister;

/// The base of an element
#[derive(Clone, Debug, derive_more::From, Default)]
pub enum ElementType {
    /// The element is based of a component
    Component(Rc<Component>),
    /// The element is a builtin element
    Builtin(Arc<BuiltinElement>),
    /// The base element couldn't be looked up
    #[default]
    Error,
    /// This should be the base type of the root element of a global component
    Global,
    /// This should be the base type of the root element of an interface.
    /// The payload is the interface this one inherits, if any.
    Interface(Option<Rc<Component>>),
}

impl PartialEq for ElementType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Component(a), Self::Component(b)) => Rc::ptr_eq(a, b),
            (Self::Builtin(a), Self::Builtin(b)) => Arc::ptr_eq(a, b),
            (Self::Interface(a), Self::Interface(b)) => {
                a.as_ref().map(Rc::as_ptr) == b.as_ref().map(Rc::as_ptr)
            }
            (Self::Error, Self::Error) | (Self::Global, Self::Global) => true,
            _ => false,
        }
    }
}

impl ElementType {
    /// Resolve a name written in `.slint` source.
    /// Resolve `name` in the given [`PropertyLookupMode`]. See
    /// [`crate::object_tree::Element::lookup_property`]. Only a component or an interface can have
    /// shadowed members, so the other bases ignore the mode.
    pub fn lookup_property<'a>(
        &self,
        name: &'a str,
        mode: PropertyLookupMode,
    ) -> PropertyLookupResult<'a> {
        match self {
            Self::Component(c) | Self::Interface(Some(c)) => {
                c.root_element.borrow().lookup_property(name, mode)
            }
            Self::Builtin(b) => {
                let resolved_name =
                    if let Some(alias_name) = b.native_class.lookup_alias(name.as_ref()) {
                        Cow::Owned(alias_name.to_string())
                    } else {
                        Cow::Borrowed(name)
                    };
                match b.properties.get(resolved_name.as_ref()) {
                    None => {
                        if b.is_non_item_type || b.is_global {
                            PropertyLookupResult::invalid(resolved_name)
                        } else {
                            crate::typeregister::reserved_property(resolved_name)
                        }
                    }
                    Some(p) => PropertyLookupResult {
                        resolved_name,
                        property_type: p.ty.clone(),
                        property_visibility: p.property_visibility,
                        declared_pure: p.declared_pure(),
                        is_local_to_component: false,
                        is_in_direct_base: false,
                        is_shadowable: p.shadowable,
                        builtin_function: match &p.default_value {
                            BuiltinPropertyDefault::BuiltinFunction(f) => Some(f.clone()),
                            _ => None,
                        },
                        is_slint_sc: p.slint_sc,
                        internal_name: None,
                        deprecated: None,
                    },
                }
            }
            Self::Interface(None) => crate::typeregister::reserved_property(Cow::Borrowed(name)),
            _ => PropertyLookupResult::invalid(Cow::Borrowed(name)),
        }
    }

    /// Return the node declaring `name` in this type or one of its bases, if there is one.
    pub fn property_declaration_node(&self, name: &str) -> Option<SyntaxNode> {
        match self {
            Self::Component(c) | Self::Interface(Some(c)) => {
                c.root_element.borrow().property_declaration_node(name)
            }
            _ => None,
        }
    }

    /// List of sub properties valid for the auto completion
    pub fn property_list(&self) -> Vec<(SmolStr, Type)> {
        match self {
            Self::Component(c) | Self::Interface(Some(c)) => {
                let root = c.root_element.borrow();
                let mut r = root.base_type.property_list();
                // A visible shadowing declaration replaces the inherited entry of the same name.
                if !root.shadowing_members.is_empty() {
                    let hidden: std::collections::HashSet<_> =
                        root.visible_shadowing_members().collect();
                    r.retain(|(name, _)| !hidden.contains(name));
                }
                r.extend(
                    root.property_declarations
                        .iter()
                        .filter(|(_, d)| d.visibility != PropertyVisibility::Private)
                        .map(|(k, d)| (d.declared_name(k).clone(), d.property_type.clone())),
                );
                r
            }
            Self::Builtin(b) => {
                b.properties.iter().map(|(k, t)| (k.clone(), t.ty.clone())).collect()
            }
            _ => Vec::new(),
        }
    }

    /// This function looks at the element and checks whether it can have Elements of type `name` as children.
    /// In addition to what `accepts_child_element` does, this method also probes the type of `name`.
    /// It returns an Error if that is not possible or an `ElementType` if it is.
    pub fn lookup_type_for_child_element(
        &self,
        name: &str,
        tr: &TypeRegister,
    ) -> Result<ElementType, String> {
        match self {
            Self::Component(component) => {
                let base_type = match component.child_insertion_points.borrow().get(DEFAULT_SLOT_NAME) {
                    Some(insert_in) => insert_in.parent.borrow().base_type.clone(),
                    None => {
                        let base_type = component.root_element.borrow().base_type.clone();
                        if base_type == tr.empty_type() {
                            let element = tr.lookup_element(name)?;
                            if matches!(&element, ElementType::Builtin(b) if b.can_be_declared_without_children_slot) {
                                return Ok(element);
                            }
                            return Err(format!("'{}' cannot have children. Only components with @children can have children", component.id));
                        }
                        base_type
                    }
                };
                base_type.lookup_type_for_child_element(name, tr)
            }
            Self::Builtin(builtin) => {
                let looked_up = tr.lookup_element(name);
                if let Ok(ElementType::Builtin(b)) = &looked_up
                    && b.can_be_declared_without_children_slot
                {
                    return Ok(ElementType::Builtin(b.clone()));
                }
                if builtin.disallow_global_types_as_child_elements {
                    if let Some(child_type) = builtin.additional_accepted_child_types.get(name) {
                        return Ok(child_type.clone().into());
                    } else if builtin.additional_accept_self && name == builtin.native_class.class_name {
                        return Ok(builtin.clone().into());
                    }
                    let mut valid_children: Vec<_> =
                        builtin.additional_accepted_child_types.keys().cloned().collect();
                    if builtin.additional_accept_self {
                        valid_children.push(builtin.native_class.class_name.clone());
                    }
                    valid_children.sort();

                    let err = if valid_children.is_empty() {
                        // No whitelist to suggest from; prefer "Unknown element" for typos.
                        looked_up?;
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
                let err = match looked_up {
                    Err(e) => e,
                    Ok(t) => {
                        if !tr.expose_internal_types
                            && matches!(&t, Self::Builtin(e) if e.is_internal)
                        {
                            format!("Unknown element '{name}'. (The type exists as an internal type, but cannot be accessed in this scope)")
                        } else {
                            return Ok(t);
                        }
                    }
                };
                if let Some(child_type) = builtin.additional_accepted_child_types.get(name) {
                    return Ok(child_type.clone().into());
                } else if builtin.additional_accept_self && name == builtin.native_class.class_name {
                    return Ok(builtin.clone().into());
                }
                match tr.lookup(name) {
                    Type::Invalid => Err(err),
                    ty => Err(format!("'{ty}' cannot be used as an element")),
                }
            }
            _ => tr.lookup_element(name).and_then(|t| {
                if !tr.expose_internal_types && matches!(&t, Self::Builtin(e) if e.is_internal) {
                    Err(format!("Unknown element '{name}'. (The type exists as an internal type, but cannot be accessed in this scope)"))
                } else {
                    Ok(t)
                }
            })
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
            ElementType::Error => None,
            ElementType::Global => None,
            ElementType::Interface(_) => None,
        }
    }
}

impl Display for ElementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Component(c) => c.id.fmt(f),
            Self::Builtin(b) => b.name.fmt(f),
            Self::Error => write!(f, "<error>"),
            Self::Global => Ok(()),
            Self::Interface(_) => Ok(()),
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

/// One entry in the documentation of a builtin element, in declaration order.
#[derive(Debug, Clone)]
pub enum ElementDocEntry {
    /// Free-form documentation text (from `///` or `//!` comments).
    Text(String),
    /// Reference to a property, callback, or function by name.
    Member(SmolStr),
}

#[derive(Debug, Clone, Default)]
pub struct BuiltinElement {
    pub name: SmolStr,
    pub native_class: Arc<NativeClass>,
    pub properties: BTreeMap<SmolStr, BuiltinPropertyInfo>,
    /// Additional builtin element that can be accepted as child of this element
    /// (example `Tab` in `TabWidget`, `Row` in `GridLayout` and the path elements in `Path`)
    pub additional_accepted_child_types: BTreeMap<SmolStr, Arc<BuiltinElement>>,
    /// `Self` is conceptually in `additional_accepted_child_types` (which it can't otherwise that'd make a Rc loop)
    pub additional_accept_self: bool,
    pub disallow_global_types_as_child_elements: bool,
    /// Non-item type do not have reserved properties (x/width/rowspan/...) added to them  (eg: PropertyAnimation)
    pub is_non_item_type: bool,
    pub accepts_focus: bool,
    pub is_global: bool,
    pub default_size_binding: DefaultSizeBinding,
    /// When true this is an internal type not shown in the auto-completion
    pub is_internal: bool,
    /// Documentation sections of the builtin element declaration, preserving source order.
    /// `Text` entries come from `///` (element-level) and `//!` (section) comments;
    /// `Member` entries reference a property, callback, or function by name.
    pub docs: Vec<ElementDocEntry>,
    /// When true this builtin can be declared as a child even if the parent element
    /// does not expose an explicit @children insertion slot.
    pub can_be_declared_without_children_slot: bool,
    /// When true this element is part of the Slint SC (safety-critical) subset.
    pub slint_sc: bool,
}

/// How [`crate::object_tree::Element::lookup_property`] resolves a name.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum PropertyLookupMode {
    /// A source name resolved from within the declaring component: a private shadow is visible.
    ComponentLocal,
    /// A source name resolved from outside the component: a private shadow is invisible, so the name
    /// resolves to the member it shadows.
    FromOutside,
    /// A storage key, as a `NamedReference` carries: no shadow resolution.
    InternalName,
}

#[derive(PartialEq, Debug)]
pub struct PropertyLookupResult<'a> {
    pub resolved_name: std::borrow::Cow<'a, str>,
    pub property_type: Type,
    pub property_visibility: PropertyVisibility,
    pub declared_pure: Option<bool>,
    /// True if the property is part of the current component (for visibility purposes)
    pub is_local_to_component: bool,
    /// True if the property in the direct base of the component (for protected visibility purposes)
    pub is_in_direct_base: bool,
    /// True if a local declaration may shadow this member: it is marked `@shadowable`.
    pub is_shadowable: bool,

    /// Set when the lookup went through a shadow: the member is declared under this
    /// name in `Element::property_declarations`, `bindings`, etc, while `resolved_name`
    /// keeps the name as it is written in the source. Only ever set by
    /// [`crate::object_tree::Element::lookup_property`].
    pub internal_name: Option<SmolStr>,

    /// If the property is a builtin function
    pub builtin_function: Option<BuiltinFunction>,

    /// Whether the property is part of the Slint SC subset (`@sc` in the builtin element
    /// declaration).
    pub is_slint_sc: bool,

    /// Some if the property was declared with `@deprecated`: the hint message shown after
    /// "The property 'xxx' has been deprecated:" in the warning.
    /// (Only set for properties declared in a component; builtin aliases use `resolved_name` instead.)
    pub deprecated: Option<SmolStr>,
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

    /// Report the property as outside the Slint SC subset, unless it's one the subset has.
    /// A name that resolves to nothing is left to the diagnostic that says so.
    #[cfg(feature = "slint-sc")]
    pub fn check_slint_sc(
        &self,
        name: &dyn Display,
        source: &dyn crate::diagnostics::Spanned,
        diag: &mut crate::diagnostics::BuildDiagnostics,
    ) {
        if self.is_valid() && !self.is_slint_sc {
            diag.slint_sc_error(&format!("The property '{name}' is"), source);
        }
    }

    /// The name the member is stored under in `Element::property_declarations`, `bindings`,
    /// `change_callbacks` and `property_analysis`, and the name a `NamedReference` to it carries.
    pub fn internal_or_resolved_name(&self) -> SmolStr {
        self.internal_name.clone().unwrap_or_else(|| self.resolved_name.as_ref().into())
    }

    pub fn invalid(resolved_name: Cow<'a, str>) -> Self {
        Self {
            resolved_name,
            property_type: Type::Invalid,
            property_visibility: PropertyVisibility::Private,
            declared_pure: None,
            is_local_to_component: false,
            is_in_direct_base: false,
            is_shadowable: false,
            builtin_function: None,
            is_slint_sc: false,
            internal_name: None,
            deprecated: None,
        }
    }
}
