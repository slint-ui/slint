// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore imum

use smol_str::{format_smolstr, SmolStr, StrExt, ToSmolStr};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use crate::expression_tree::BuiltinFunction;
use crate::langtype::{
    BuiltinElement, BuiltinPropertyDefault, BuiltinPropertyInfo, ElementType, Enumeration,
    Function, PropertyLookupResult, Struct, Type,
};
use crate::object_tree::{Component, PropertyVisibility};
use crate::typeloader;

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
];

pub const RESERVED_GRIDLAYOUT_PROPERTIES: &[(&str, Type)] = &[
    ("col", Type::Int32),
    ("row", Type::Int32),
    ("colspan", Type::Int32),
    ("rowspan", Type::Int32),
];

macro_rules! declare_enums {
    ($( $(#[$enum_doc:meta])* enum $Name:ident { $( $(#[$value_doc:meta])* $Value:ident,)* })*) => {
        #[allow(non_snake_case)]
        pub struct BuiltinEnums {
            $(pub $Name : Rc<Enumeration>),*
        }
        impl BuiltinEnums {
            fn new() -> Self {
                Self {
                    $($Name : Rc::new(Enumeration {
                        name: stringify!($Name).replace_smolstr("_", "-"),
                        values: vec![$(crate::generator::to_kebab_case(stringify!($Value).trim_start_matches("r#")).into()),*],
                        default_value: 0,
                        node: None,
                    })),*
                }
            }
            fn fill_register(&self, register: &mut TypeRegister) {
                $(if stringify!($Name) != "PathEvent" {
                    register.insert_type_with_name(
                        Type::Enumeration(self.$Name.clone()),
                        stringify!($Name).replace_smolstr("_", "-")
                    );
                })*
            }
        }
    };
}

i_slint_common::for_each_enums!(declare_enums);

pub struct BuiltinTypes {
    pub enums: BuiltinEnums,
    pub noarg_callback_type: Type,
    pub strarg_callback_type: Type,
    pub logical_point_type: Type,
    pub font_metrics_type: Type,
    pub layout_info_type: Rc<Struct>,
    pub path_element_type: Type,
    pub box_layout_cell_data_type: Type,
}

impl BuiltinTypes {
    fn new() -> Self {
        let layout_info_type = Rc::new(Struct {
            fields: ["min", "max", "preferred"]
                .iter()
                .map(|s| (SmolStr::new_static(s), Type::LogicalLength))
                .chain(
                    ["min_percent", "max_percent", "stretch"]
                        .iter()
                        .map(|s| (SmolStr::new_static(s), Type::Float32)),
                )
                .collect(),
            name: Some("slint::private_api::LayoutInfo".into()),
            node: None,
            rust_attributes: None,
        });
        Self {
            enums: BuiltinEnums::new(),
            logical_point_type: Type::Struct(Rc::new(Struct {
                fields: IntoIterator::into_iter([
                    (SmolStr::new_static("x"), Type::LogicalLength),
                    (SmolStr::new_static("y"), Type::LogicalLength),
                ])
                .collect(),
                name: Some("slint::LogicalPosition".into()),
                node: None,
                rust_attributes: None,
            })),
            font_metrics_type: Type::Struct(Rc::new(Struct {
                fields: IntoIterator::into_iter([
                    (SmolStr::new_static("ascent"), Type::LogicalLength),
                    (SmolStr::new_static("descent"), Type::LogicalLength),
                    (SmolStr::new_static("x-height"), Type::LogicalLength),
                    (SmolStr::new_static("cap-height"), Type::LogicalLength),
                ])
                .collect(),
                name: Some("slint::private_api::FontMetrics".into()),
                node: None,
                rust_attributes: None,
            })),
            noarg_callback_type: Type::Callback(Rc::new(Function {
                return_type: Type::Void,
                args: vec![],
                arg_names: vec![],
            })),
            strarg_callback_type: Type::Callback(Rc::new(Function {
                return_type: Type::Void,
                args: vec![Type::String],
                arg_names: vec![],
            })),
            layout_info_type: layout_info_type.clone(),
            path_element_type: Type::Struct(Rc::new(Struct {
                fields: Default::default(),
                name: Some("PathElement".into()),
                node: None,
                rust_attributes: None,
            })),
            box_layout_cell_data_type: Type::Struct(Rc::new(Struct {
                fields: IntoIterator::into_iter([("constraint".into(), layout_info_type.into())])
                    .collect(),
                name: Some("BoxLayoutCellData".into()),
                node: None,
                rust_attributes: None,
            })),
        }
    }
}

thread_local! {
    pub static BUILTIN: BuiltinTypes = BuiltinTypes::new();
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

pub const RESERVED_TRANSFORM_PROPERTIES: &[(&str, Type)] = &[
    ("rotation-angle", Type::Angle),
    ("scale-x", Type::Float32),
    ("scale-y", Type::Float32),
    ("rotation-origin-x", Type::LogicalLength),
    ("rotation-origin-y", Type::LogicalLength),
];

pub fn noarg_callback_type() -> Type {
    BUILTIN.with(|types| types.noarg_callback_type.clone())
}

fn strarg_callback_type() -> Type {
    BUILTIN.with(|types| types.strarg_callback_type.clone())
}

pub fn reserved_accessibility_properties() -> impl Iterator<Item = (&'static str, Type)> {
    [
        //("accessible-role", ...)
        ("accessible-checkable", Type::Bool),
        ("accessible-checked", Type::Bool),
        ("accessible-delegate-focus", Type::Int32),
        ("accessible-description", Type::String),
        ("accessible-enabled", Type::Bool),
        ("accessible-expandable", Type::Bool),
        ("accessible-expanded", Type::Bool),
        ("accessible-label", Type::String),
        ("accessible-value", Type::String),
        ("accessible-value-maximum", Type::Float32),
        ("accessible-value-minimum", Type::Float32),
        ("accessible-value-step", Type::Float32),
        ("accessible-placeholder-text", Type::String),
        ("accessible-action-default", noarg_callback_type()),
        ("accessible-action-increment", noarg_callback_type()),
        ("accessible-action-decrement", noarg_callback_type()),
        ("accessible-action-set-value", strarg_callback_type()),
        ("accessible-action-expand", noarg_callback_type()),
        ("accessible-item-selectable", Type::Bool),
        ("accessible-item-selected", Type::Bool),
        ("accessible-item-index", Type::Int32),
        ("accessible-item-count", Type::Int32),
        ("accessible-read-only", Type::Bool),
    ]
    .into_iter()
}

/// list of reserved property injected in every item
pub fn reserved_properties() -> impl Iterator<Item = (&'static str, Type, PropertyVisibility)> {
    RESERVED_GEOMETRY_PROPERTIES
        .iter()
        .chain(RESERVED_LAYOUT_PROPERTIES.iter())
        .chain(RESERVED_OTHER_PROPERTIES.iter())
        .chain(RESERVED_DROP_SHADOW_PROPERTIES.iter())
        .chain(RESERVED_TRANSFORM_PROPERTIES.iter())
        .map(|(k, v)| (*k, v.clone(), PropertyVisibility::Input))
        .chain(reserved_accessibility_properties().map(|(k, v)| (k, v, PropertyVisibility::Input)))
        .chain(
            RESERVED_GRIDLAYOUT_PROPERTIES
                .iter()
                .map(|(k, v)| (*k, v.clone(), PropertyVisibility::Constexpr)),
        )
        .chain(IntoIterator::into_iter([
            ("absolute-position", logical_point_type(), PropertyVisibility::Output),
            ("forward-focus", Type::ElementReference, PropertyVisibility::Constexpr),
            (
                "focus",
                Type::Function(BuiltinFunction::SetFocusItem.ty()),
                PropertyVisibility::Public,
            ),
            (
                "clear-focus",
                Type::Function(BuiltinFunction::ClearFocusItem.ty()),
                PropertyVisibility::Public,
            ),
            (
                "dialog-button-role",
                Type::Enumeration(BUILTIN.with(|e| e.enums.DialogButtonRole.clone())),
                PropertyVisibility::Constexpr,
            ),
            (
                "accessible-role",
                Type::Enumeration(BUILTIN.with(|e| e.enums.AccessibleRole.clone())),
                PropertyVisibility::Constexpr,
            ),
        ]))
        .chain(std::iter::once(("init", noarg_callback_type(), PropertyVisibility::Private)))
}

/// lookup reserved property injected in every item
pub fn reserved_property(name: &str) -> PropertyLookupResult<'_> {
    thread_local! {
        static RESERVED_PROPERTIES: HashMap<&'static str, (Type, PropertyVisibility, Option<BuiltinFunction>)>
            = reserved_properties().map(|(name, ty, visibility)| (name, (ty, visibility, reserved_member_function(name)))).collect();
    }
    if let Some(result) = RESERVED_PROPERTIES.with(|reserved| {
        reserved.get(name).map(|(ty, visibility, builtin_function)| PropertyLookupResult {
            property_type: ty.clone(),
            resolved_name: name.into(),
            is_local_to_component: false,
            is_in_direct_base: false,
            property_visibility: *visibility,
            declared_pure: None,
            builtin_function: builtin_function.clone(),
        })
    }) {
        return result;
    }

    // Report deprecated known reserved properties (maximum_width, minimum_height, ...)
    for pre in &["min", "max"] {
        if let Some(a) = name.strip_prefix(pre) {
            for suf in &["width", "height"] {
                if let Some(b) = a.strip_suffix(suf) {
                    if b == "imum-" {
                        return PropertyLookupResult {
                            property_type: Type::LogicalLength,
                            resolved_name: format!("{pre}-{suf}").into(),
                            is_local_to_component: false,
                            is_in_direct_base: false,
                            property_visibility: crate::object_tree::PropertyVisibility::InOut,
                            declared_pure: None,
                            builtin_function: None,
                        };
                    }
                }
            }
        }
    }
    PropertyLookupResult::invalid(name.into())
}

/// These member functions are injected in every time
pub fn reserved_member_function(name: &str) -> Option<BuiltinFunction> {
    for (m, e) in [
        ("focus", BuiltinFunction::SetFocusItem), // match for callable "focus" property
        ("clear-focus", BuiltinFunction::ClearFocusItem), // match for callable "clear-focus" property
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
    types: HashMap<SmolStr, Type>,
    /// The set of element types
    elements: HashMap<SmolStr, ElementType>,
    supported_property_animation_types: HashSet<String>,
    pub(crate) property_animation_type: ElementType,
    pub(crate) empty_type: ElementType,
    /// Map from a context restricted type to the list of contexts (parent type) it is allowed in. This is
    /// used to construct helpful error messages, such as "Row can only be within a GridLayout element".
    context_restricted_types: HashMap<SmolStr, HashSet<SmolStr>>,
    parent_registry: Option<Rc<RefCell<TypeRegister>>>,
    /// If the lookup function should return types that are marked as internal
    pub(crate) expose_internal_types: bool,
}

impl TypeRegister {
    pub(crate) fn snapshot(&self, snapshotter: &mut typeloader::Snapshotter) -> Self {
        Self {
            types: self.types.clone(),
            elements: self
                .elements
                .iter()
                .map(|(k, v)| (k.clone(), snapshotter.snapshot_element_type(v)))
                .collect(),
            supported_property_animation_types: self.supported_property_animation_types.clone(),
            property_animation_type: snapshotter
                .snapshot_element_type(&self.property_animation_type),
            empty_type: snapshotter.snapshot_element_type(&self.empty_type),
            context_restricted_types: self.context_restricted_types.clone(),
            parent_registry: self
                .parent_registry
                .as_ref()
                .map(|tr| snapshotter.snapshot_type_register(tr)),
            expose_internal_types: self.expose_internal_types,
        }
    }

    /// Insert a type into the type register with its builtin type name.
    ///
    /// Returns false if it replaced an existing type.
    pub fn insert_type(&mut self, t: Type) -> bool {
        self.types.insert(t.to_smolstr(), t).is_none()
    }
    /// Insert a type into the type register with a specified name.
    ///
    /// Returns false if it replaced an existing type.
    pub fn insert_type_with_name(&mut self, t: Type, name: SmolStr) -> bool {
        self.types.insert(name, t).is_none()
    }

    fn builtin_internal() -> Self {
        let mut register = TypeRegister::default();

        register.insert_type(Type::Float32);
        register.insert_type(Type::Int32);
        register.insert_type(Type::String);
        register.insert_type(Type::PhysicalLength);
        register.insert_type(Type::LogicalLength);
        register.insert_type(Type::Color);
        register.insert_type(Type::ComponentFactory);
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

        BUILTIN.with(|e| e.enums.fill_register(&mut register));

        register.supported_property_animation_types.insert(Type::Float32.to_string());
        register.supported_property_animation_types.insert(Type::Int32.to_string());
        register.supported_property_animation_types.insert(Type::Color.to_string());
        register.supported_property_animation_types.insert(Type::PhysicalLength.to_string());
        register.supported_property_animation_types.insert(Type::LogicalLength.to_string());
        register.supported_property_animation_types.insert(Type::Brush.to_string());
        register.supported_property_animation_types.insert(Type::Angle.to_string());

        #[rustfmt::skip]
        macro_rules! map_type {
            ($pub_type:ident, bool) => { Type::Bool };
            ($pub_type:ident, i32) => { Type::Int32 };
            ($pub_type:ident, f32) => { Type::Float32 };
            ($pub_type:ident, SharedString) => { Type::String };
            ($pub_type:ident, Image) => { Type::Image };
            ($pub_type:ident, Coord) => { Type::LogicalLength };
            ($pub_type:ident, LogicalPosition) => { logical_point_type() };
            ($pub_type:ident, KeyboardModifiers) => { $pub_type.clone() };
            ($pub_type:ident, $_:ident) => {
                BUILTIN.with(|e| Type::Enumeration(e.enums.$pub_type.clone()))
            };
        }
        #[rustfmt::skip]
        macro_rules! maybe_clone {
            ($pub_type:ident, KeyboardModifiers) => { $pub_type.clone() };
            ($pub_type:ident, $_:ident) => { $pub_type };
        }
        macro_rules! register_builtin_structs {
            ($(
                $(#[$attr:meta])*
                struct $Name:ident {
                    @name = $inner_name:literal
                    export {
                        $( $(#[$pub_attr:meta])* $pub_field:ident : $pub_type:ident, )*
                    }
                    private {
                        $( $(#[$pri_attr:meta])* $pri_field:ident : $pri_type:ty, )*
                    }
                }
            )*) => { $(
                #[allow(non_snake_case)]
                let $Name = Type::Struct(Rc::new(Struct{
                    fields: BTreeMap::from([
                        $((stringify!($pub_field).replace_smolstr("_", "-"), map_type!($pub_type, $pub_type))),*
                    ]),
                    name: Some(format_smolstr!("{}", $inner_name)),
                    node: None,
                    rust_attributes: None,
                }));
                register.insert_type_with_name(maybe_clone!($Name, $Name), SmolStr::new(stringify!($Name)));
            )* };
        }
        i_slint_common::for_each_builtin_structs!(register_builtin_structs);

        crate::load_builtins::load_builtins(&mut register);

        for e in register.elements.values() {
            if let ElementType::Builtin(b) = e {
                for accepted_child_type_name in b.additional_accepted_child_types.keys() {
                    register
                        .context_restricted_types
                        .entry(accepted_child_type_name.clone())
                        .or_default()
                        .insert(b.native_class.class_name.clone());
                }
                if b.additional_accept_self {
                    register
                        .context_restricted_types
                        .entry(b.native_class.class_name.clone())
                        .or_default()
                        .insert(b.native_class.class_name.clone());
                }
            }
        }

        match &mut register.elements.get_mut("PopupWindow").unwrap() {
            ElementType::Builtin(ref mut b) => {
                let popup = Rc::get_mut(b).unwrap();
                popup.properties.insert(
                    "show".into(),
                    BuiltinPropertyInfo::from(BuiltinFunction::ShowPopupWindow),
                );

                popup.properties.insert(
                    "close".into(),
                    BuiltinPropertyInfo::from(BuiltinFunction::ClosePopupWindow),
                );

                popup.properties.get_mut("close-on-click").unwrap().property_visibility =
                    PropertyVisibility::Constexpr;

                popup.properties.get_mut("close-policy").unwrap().property_visibility =
                    PropertyVisibility::Constexpr;
            }
            _ => unreachable!(),
        };

        match &mut register.elements.get_mut("Timer").unwrap() {
            ElementType::Builtin(ref mut b) => {
                let timer = Rc::get_mut(b).unwrap();
                timer
                    .properties
                    .insert("start".into(), BuiltinPropertyInfo::from(BuiltinFunction::StartTimer));
                timer
                    .properties
                    .insert("stop".into(), BuiltinPropertyInfo::from(BuiltinFunction::StopTimer));
                timer.properties.insert(
                    "restart".into(),
                    BuiltinPropertyInfo::from(BuiltinFunction::RestartTimer),
                );
            }
            _ => unreachable!(),
        }

        let font_metrics_prop = crate::langtype::BuiltinPropertyInfo {
            ty: font_metrics_type(),
            property_visibility: PropertyVisibility::Output,
            default_value: BuiltinPropertyDefault::WithElement(|elem| {
                crate::expression_tree::Expression::FunctionCall {
                    function: BuiltinFunction::ItemFontMetrics.into(),
                    arguments: vec![crate::expression_tree::Expression::ElementReference(
                        Rc::downgrade(elem),
                    )],
                    source_location: None,
                }
            }),
        };

        match &mut register.elements.get_mut("TextInput").unwrap() {
            ElementType::Builtin(ref mut b) => {
                let text_input = Rc::get_mut(b).unwrap();
                text_input.properties.insert(
                    "set-selection-offsets".into(),
                    BuiltinPropertyInfo::from(BuiltinFunction::SetSelectionOffsets),
                );
                text_input.properties.insert("font-metrics".into(), font_metrics_prop.clone());
            }

            _ => unreachable!(),
        };

        match &mut register.elements.get_mut("Text").unwrap() {
            ElementType::Builtin(ref mut b) => {
                let text = Rc::get_mut(b).unwrap();
                text.properties.insert("font-metrics".into(), font_metrics_prop);
            }

            _ => unreachable!(),
        };

        match &mut register.elements.get_mut("Path").unwrap() {
            ElementType::Builtin(ref mut b) => {
                let path = Rc::get_mut(b).unwrap();
                path.properties.get_mut("commands").unwrap().property_visibility =
                    PropertyVisibility::Fake;
            }

            _ => unreachable!(),
        };

        register
    }

    #[doc(hidden)]
    /// All builtins incl. experimental ones! Do not use in production code!
    pub fn builtin_experimental() -> Rc<RefCell<Self>> {
        let register = Self::builtin_internal();
        Rc::new(RefCell::new(register))
    }

    pub fn builtin() -> Rc<RefCell<Self>> {
        let mut register = Self::builtin_internal();

        register.elements.remove("ComponentContainer").unwrap();
        register.types.remove("component-factory").unwrap();

        register.elements.remove("DragArea").unwrap();
        register.elements.remove("DropArea").unwrap();
        register.types.remove("DropEvent").unwrap(); // Also removed in xtask/src/slintdocs.rs

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
    ) -> Result<ElementType, HashMap<SmolStr, HashSet<SmolStr>>> {
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
                format!("'{ty}' cannot be used as an element")
            } else {
                format!("Unknown element '{name}'")
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

    /// Add the component with its defined name
    ///
    /// Returns false if there was already an element with the same name
    pub fn add(&mut self, comp: Rc<Component>) -> bool {
        self.add_with_name(comp.id.clone(), comp)
    }

    /// Add the component with a specified name
    ///
    /// Returns false if there was already an element with the same name
    pub fn add_with_name(&mut self, name: SmolStr, comp: Rc<Component>) -> bool {
        self.elements.insert(name, ElementType::Component(comp)).is_none()
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
    pub fn all_types(&self) -> HashMap<SmolStr, Type> {
        let mut all =
            self.parent_registry.as_ref().map(|r| r.borrow().all_types()).unwrap_or_default();
        for (k, v) in &self.types {
            all.insert(k.clone(), v.clone());
        }
        all
    }

    /// Return a hashmap with all the registered element type
    pub fn all_elements(&self) -> HashMap<SmolStr, ElementType> {
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
    BUILTIN.with(|types| types.logical_point_type.clone())
}

pub fn font_metrics_type() -> Type {
    BUILTIN.with(|types| types.font_metrics_type.clone())
}

/// The [`Type`] for a runtime LayoutInfo structure
pub fn layout_info_type() -> Rc<Struct> {
    BUILTIN.with(|types| types.layout_info_type.clone())
}

/// The [`Type`] for a runtime PathElement structure
pub fn path_element_type() -> Type {
    BUILTIN.with(|types| types.path_element_type.clone())
}

/// The [`Type`] for a runtime BoxLayoutCellData structure
pub fn box_layout_cell_data_type() -> Type {
    BUILTIN.with(|types| types.box_layout_cell_data_type.clone())
}
