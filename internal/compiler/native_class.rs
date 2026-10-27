// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The classes of the native items of the run-time, and the properties they declare

use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;
use std::sync::Arc;

use smol_str::SmolStr;

use crate::builtin_functions::BuiltinFunction;
use crate::langtype::{BuiltinStruct, ConstantExpression, Type};

#[derive(Debug, Clone)]
pub enum BuiltinPropertyDefault {
    /// The default of the property's type
    None,
    /// When unset, the style or a compiler pass gives the property its value
    Computed,
    Expr(ConstantExpression),
    /// The property is computed per element by this function, which takes the element.
    ElementFunction(BuiltinFunction),
    /// A value only the runtime knows, which this function returns. It takes no argument, so
    /// unlike [`Self::ElementFunction`] the property still belongs to the native item.
    RuntimeValue(BuiltinFunction),
    /// The property is actually not a property but a builtin function
    BuiltinFunction(BuiltinFunction),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PropertyVisibility {
    #[default]
    Private,
    Input,
    Output,
    InOut,
    /// for builtin properties that must be known at compile time and cannot be changed at runtime
    Constexpr,
    /// For builtin properties that are meant to just be bindings but cannot be read or written
    /// (eg, Path's `commands`)
    Fake,
    /// For functions, not properties
    Public,
    Protected,
}

impl Display for PropertyVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropertyVisibility::Private => f.write_str("private"),
            PropertyVisibility::Input => f.write_str("in"),
            PropertyVisibility::Output => f.write_str("out"),
            PropertyVisibility::InOut => f.write_str("in-out"),
            PropertyVisibility::Constexpr => f.write_str("constexpr"),
            PropertyVisibility::Public => f.write_str("public"),
            PropertyVisibility::Protected => f.write_str("protected"),
            PropertyVisibility::Fake => f.write_str("fake"),
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
    /// Raw `///` doc comment from the builtin element declaration, if any.
    pub docs: Option<String>,
    /// Whether the property is part of the Slint SC subset
    /// (`@sc` modifier in the builtin element declaration).
    pub slint_sc: bool,
    /// True when a component may declare a member of the same name, shadowing this one
    /// (`@shadowable` attribute in the builtin element declaration).
    /// Members added to a builtin element after its initial release should be marked
    /// shadowable so that older code that already declares the name keeps compiling —
    /// unless a compiler pass accesses the member by name, in which case shadowing
    /// would generate wrong code and the member must not be marked.
    pub shadowable: bool,
    /// For a function or callback: whether it is pure.
    /// A member implemented natively has no body the compiler could inspect, so this comes from
    /// the `pure` qualifier of the builtin element declaration, or from [`BuiltinFunction::is_pure`] when the
    /// declaration names one.
    pub pure: bool,
}

impl BuiltinPropertyInfo {
    pub fn new(ty: Type) -> Self {
        Self {
            ty,
            default_value: BuiltinPropertyDefault::None,
            property_visibility: PropertyVisibility::InOut,
            docs: None,
            shadowable: false,
            pure: false,
            slint_sc: false,
        }
    }

    pub fn is_native_output(&self) -> bool {
        matches!(self.property_visibility, PropertyVisibility::InOut | PropertyVisibility::Output)
    }

    /// The `pure` declaration of a function or callback, `None` for a property.
    pub fn declared_pure(&self) -> Option<bool> {
        matches!(self.ty, Type::Function(_) | Type::Callback(_)).then_some(self.pure)
    }
}

impl From<BuiltinFunction> for BuiltinPropertyInfo {
    fn from(function: BuiltinFunction) -> Self {
        Self {
            ty: Type::Function(function.ty()),
            property_visibility: PropertyVisibility::Public,
            docs: None,
            shadowable: false,
            pure: function.is_pure(),
            default_value: BuiltinPropertyDefault::BuiltinFunction(function),
            slint_sc: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NativeClass {
    pub parent: Option<Arc<NativeClass>>,
    pub class_name: SmolStr,
    pub cpp_vtable_getter: String,
    pub properties: BTreeMap<SmolStr, BuiltinPropertyInfo>,
    pub deprecated_aliases: HashMap<SmolStr, SmolStr>,
    /// Type override if class_name is not equal to the name to be used in the
    /// target language API.
    pub builtin_struct: Option<BuiltinStruct>,
}

impl NativeClass {
    pub fn new(class_name: &str) -> Self {
        let cpp_vtable_getter = format!("SLINT_GET_ITEM_VTABLE({class_name}VTable)");
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
        self.lookup_property_info(name).map(|info| &info.ty)
    }

    /// The declaration of `name` on this class or the closest parent that has it.
    pub fn lookup_property_info(&self, name: &str) -> Option<&BuiltinPropertyInfo> {
        self.properties
            .get(name)
            .or_else(|| self.parent.as_ref().and_then(|parent| parent.lookup_property_info(name)))
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
