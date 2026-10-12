// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore imum noarg strarg

use smol_str::{SmolStr, StrExt, ToSmolStr};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use crate::expression_tree::BuiltinFunction;
use crate::langtype::{
    BuiltinElement, BuiltinStruct, ElementType, Enumeration, Function, PropertyLookupResult,
    Struct, Type,
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

// Per-item properties of a FlexboxLayout, HorizontalLayout or VerticalLayout cell.
// Note: cross-axis-self-alignment is added in reserved_properties() instead,
// because Type::Enumeration requires a runtime Arc allocation.
pub const RESERVED_LAYOUT_CELL_PROPERTIES: &[(&str, Type)] = &[("layout-order", Type::Int32)];

macro_rules! declare_enums {
    ($( $(#[$enum_doc:meta])* $vis:vis enum $Name:ident { $( $(#[$value_doc:meta])* $Value:ident,)* })*) => {
        #[allow(non_snake_case)]
        pub struct BuiltinEnums {
            $(pub $Name : Arc<Enumeration>),*
        }
        impl BuiltinEnums {
            fn new() -> Self {
                Self { $($Name: enumeration(
                    stringify!($Name),
                    &[$(stringify!($Value)),*],
                    stringify!($vis) == "pub",
                )),* }
            }
            fn all(&self) -> impl Iterator<Item = &Arc<Enumeration>> {
                [$(&self.$Name),*].into_iter()
            }
            fn fill_register(&self, register: &mut TypeRegister) {
                for e in self.all() {
                    if !matches!(e.name.as_str(), "PathEvent" | "BuiltInMouseCursor") {
                        register.insert_type_with_name(Type::Enumeration(e.clone()), e.name.clone());
                    }
                }
            }
        }
    };
}

i_slint_common::for_each_enums!(declare_enums);

fn enumeration(name: &str, values: &[&str], public: bool) -> Arc<Enumeration> {
    Arc::new(Enumeration {
        name: name.into(),
        public,
        values: values
            .iter()
            .map(|v| crate::generator::to_kebab_case(v.trim_start_matches("r#")).into())
            .collect(),
        default_value: 0,
        node: None,
        rust_attributes: Vec::new(),
    })
}

pub struct BuiltinTypes {
    pub enums: BuiltinEnums,
    pub noarg_callback_type: Type,
    pub strarg_callback_type: Type,
    pub set_selection_offsets_callback_type: Type,
    /// The builtin structs that aren't in `for_each_builtin_structs` nor path elements,
    /// by name. See [`builtin_structs::get`].
    structs: HashMap<BuiltinStruct, Arc<Struct>>,
}

impl BuiltinTypes {
    fn new() -> Self {
        let enums = BuiltinEnums::new();
        let mut structs = HashMap::new();
        let mut add = |name: BuiltinStruct, fields: &[(&str, Type)]| {
            let fields = fields.iter().map(|(field, ty)| (SmolStr::new(field), ty.clone()));
            let s = Arc::new(Struct::new(fields.collect(), name.clone()));
            structs.insert(name, s.clone());
            Type::Struct(s)
        };
        let enumeration = |e: &Arc<Enumeration>| Type::Enumeration(e.clone());

        let layout_info = add(
            BuiltinStruct::LayoutInfo,
            &[
                ("min", Type::LogicalLength),
                ("max", Type::LogicalLength),
                ("preferred", Type::LogicalLength),
                ("min_percent", Type::Float32),
                ("max_percent", Type::Float32),
                ("stretch", Type::Float32),
            ],
        );
        let align_self = enumeration(&enums.CrossAxisAlignment);
        let flex_item_props = add(
            BuiltinStruct::FlexItemProps,
            &[("cross-axis-self-alignment", align_self.clone()), ("layout-order", Type::Int32)],
        );
        let layout_item_info = add(
            BuiltinStruct::LayoutItemInfo,
            &[
                ("constraint", layout_info.clone()),
                ("cross-axis-self-alignment", align_self.clone()),
                ("layout-order", Type::Int32),
            ],
        );
        add(
            BuiltinStruct::FlexboxLayoutItemInfo,
            &[("constraint", layout_info), ("props", flex_item_props.clone())],
        );
        let padding =
            add(BuiltinStruct::Padding, &[("begin", Type::Float32), ("end", Type::Float32)]);
        let cells = Type::Array(Arc::new(layout_item_info));
        let layout_alignment = enumeration(&enums.LayoutAlignment);
        add(
            BuiltinStruct::BoxLayoutData,
            &[
                ("size", Type::Float32),
                ("spacing", Type::Float32),
                ("padding", padding.clone()),
                ("alignment", layout_alignment.clone()),
                ("cells", cells.clone()),
            ],
        );
        add(
            BuiltinStruct::BoxLayoutOrthoData,
            &[
                ("size", Type::Float32),
                ("padding", padding.clone()),
                ("cross_axis_alignment", align_self.clone()),
                ("cells", cells.clone()),
            ],
        );
        add(
            BuiltinStruct::GridLayoutData,
            &[
                ("size", Type::Float32),
                ("spacing", Type::Float32),
                ("padding", padding.clone()),
                ("organized_data", Type::ArrayOfU16),
            ],
        );
        add(
            BuiltinStruct::GridLayoutInputData,
            &[
                ("new_row", Type::Bool),
                ("row", Type::Float32),
                ("col", Type::Float32),
                ("rowspan", Type::Float32),
                ("colspan", Type::Float32),
            ],
        );
        add(
            BuiltinStruct::FlexboxLayoutData,
            &[
                ("width", Type::Float32),
                ("height", Type::Float32),
                ("spacing_h", Type::Float32),
                ("spacing_v", Type::Float32),
                ("padding_h", padding.clone()),
                ("padding_v", padding),
                ("alignment", layout_alignment.clone()),
                ("direction", enumeration(&enums.FlexboxLayoutDirection)),
                ("cross_axis_alignment", align_self),
                ("cross_axis_line_alignment", layout_alignment),
                ("flex_wrap", enumeration(&enums.FlexboxLayoutWrap)),
                ("cells_h", cells.clone()),
                ("cells_v", cells),
                ("flex_props", Type::Array(Arc::new(flex_item_props))),
            ],
        );
        add(
            BuiltinStruct::StateInfo,
            &[
                ("current-state", Type::Int32),
                ("previous-state", Type::Int32),
                ("change-time", Type::Duration),
            ],
        );
        add(
            BuiltinStruct::PropertyAnimation,
            &[
                ("duration", Type::Int32),
                ("iteration-count", Type::Float32),
                ("direction", enumeration(&enums.AnimationDirection)),
                ("easing", Type::Easing),
                ("delay", Type::Int32),
                ("enabled", Type::Bool),
            ],
        );
        add(
            BuiltinStruct::LogicalPosition,
            &[("x", Type::LogicalLength), ("y", Type::LogicalLength)],
        );
        add(
            BuiltinStruct::LogicalSize,
            &[("width", Type::LogicalLength), ("height", Type::LogicalLength)],
        );
        add(BuiltinStruct::Point, &[("x", Type::Float32), ("y", Type::Float32)]);
        add(BuiltinStruct::Size, &[("width", Type::Int32), ("height", Type::Int32)]);
        add(BuiltinStruct::PathElement, &[]);
        add(
            BuiltinStruct::ColorRgba,
            &[
                ("red", Type::Int32),
                ("green", Type::Int32),
                ("blue", Type::Int32),
                ("alpha", Type::Int32),
            ],
        );
        add(
            BuiltinStruct::ColorHsva,
            &[
                ("hue", Type::Float32),
                ("saturation", Type::Float32),
                ("value", Type::Float32),
                ("alpha", Type::Float32),
            ],
        );
        add(
            BuiltinStruct::ColorOklch,
            &[
                ("lightness", Type::Float32),
                ("chroma", Type::Float32),
                ("hue", Type::Float32),
                ("alpha", Type::Float32),
            ],
        );

        Self {
            noarg_callback_type: Type::Callback(Arc::new(Function {
                return_type: Type::Void,
                args: Vec::new(),
                arg_names: Vec::new(),
            })),
            strarg_callback_type: Type::Callback(Arc::new(Function {
                return_type: Type::Void,
                args: vec![Type::String],
                arg_names: Vec::new(),
            })),
            set_selection_offsets_callback_type: Type::Callback(Arc::new(Function {
                return_type: Type::Void,
                args: vec![Type::Int32, Type::Int32],
                arg_names: vec![SmolStr::new_static("anchor"), SmolStr::new_static("focus")],
            })),
            structs,
            enums,
        }
    }
}

pub static BUILTIN: std::sync::LazyLock<BuiltinTypes> = std::sync::LazyLock::new(BuiltinTypes::new);

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
    ("drop-shadow-spread", Type::LogicalLength),
    ("drop-shadow-color", Type::Color),
];

pub const RESERVED_INNER_SHADOW_PROPERTIES: &[(&str, Type)] = &[
    ("inner-shadow-offset-x", Type::LogicalLength),
    ("inner-shadow-offset-y", Type::LogicalLength),
    ("inner-shadow-blur", Type::LogicalLength),
    ("inner-shadow-spread", Type::LogicalLength),
    ("inner-shadow-color", Type::Color),
];

pub const RESERVED_BACKDROP_BLUR_PROPERTIES: &[(&str, Type)] =
    &[("backdrop-blur", Type::LogicalLength)];

pub const RESERVED_TRANSFORM_PROPERTIES: &[(&str, Type)] = &[
    ("transform-rotation", Type::Angle),
    ("transform-scale-x", Type::Float32),
    ("transform-scale-y", Type::Float32),
    ("transform-scale", Type::Float32),
];

pub fn transform_origin_property() -> (&'static str, Arc<Struct>) {
    ("transform-origin", logical_point_type())
}

pub const DEPRECATED_ROTATION_ORIGIN_PROPERTIES: [(&str, Type); 2] =
    [("rotation-origin-x", Type::LogicalLength), ("rotation-origin-y", Type::LogicalLength)];

pub fn noarg_callback_type() -> Type {
    BUILTIN.noarg_callback_type.clone()
}

fn strarg_callback_type() -> Type {
    BUILTIN.strarg_callback_type.clone()
}

fn set_selection_offsets_callback_type() -> Type {
    BUILTIN.set_selection_offsets_callback_type.clone()
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
        ("accessible-id", Type::String),
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
        ("accessible-action-set-selection-offsets", set_selection_offsets_callback_type()),
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
        .chain(RESERVED_INNER_SHADOW_PROPERTIES.iter())
        .chain(RESERVED_BACKDROP_BLUR_PROPERTIES.iter())
        .chain(RESERVED_TRANSFORM_PROPERTIES.iter())
        .chain(DEPRECATED_ROTATION_ORIGIN_PROPERTIES.iter())
        .map(|(k, v)| (*k, v.clone(), PropertyVisibility::Input))
        .chain(
            std::iter::once(transform_origin_property())
                .map(|(k, v)| (k, v.into(), PropertyVisibility::Input)),
        )
        .chain(reserved_accessibility_properties().map(|(k, v)| (k, v, PropertyVisibility::Input)))
        .chain(
            RESERVED_GRIDLAYOUT_PROPERTIES
                .iter()
                .map(|(k, v)| (*k, v.clone(), PropertyVisibility::Input)),
        )
        .chain(
            RESERVED_LAYOUT_CELL_PROPERTIES
                .iter()
                .map(|(k, v)| (*k, v.clone(), PropertyVisibility::Input)),
        )
        // The per-item cross-axis-self-alignment (flexbox and box layouts) can't be in a
        // const array because Type::Enumeration requires a runtime Arc allocation.
        .chain(std::iter::once((
            "cross-axis-self-alignment",
            Type::Enumeration(BUILTIN.enums.CrossAxisAlignment.clone()),
            PropertyVisibility::Input,
        )))
        .chain(IntoIterator::into_iter([
            ("absolute-position", logical_point_type().into(), PropertyVisibility::Output),
            ("forward-focus", Type::ElementReference, PropertyVisibility::Constexpr),
            (
                "dialog-button-role",
                Type::Enumeration(BUILTIN.enums.DialogButtonRole.clone()),
                PropertyVisibility::Constexpr,
            ),
            (
                "accessible-role",
                Type::Enumeration(BUILTIN.enums.AccessibleRole.clone()),
                PropertyVisibility::Constexpr,
            ),
            (
                "accessible-orientation",
                Type::Enumeration(BUILTIN.enums.Orientation.clone()),
                PropertyVisibility::Input,
            ),
            (
                "accessible-live-region",
                Type::Enumeration(BUILTIN.enums.AccessibleLiveness.clone()),
                PropertyVisibility::Input,
            ),
        ]))
        .chain(std::iter::once(("init", noarg_callback_type(), PropertyVisibility::Private)))
        .chain(reserved_member_functions().map(|(name, f, v)| (name, Type::Function(f.ty()), v)))
}

/// lookup reserved property injected in every item
pub fn reserved_property(name: std::borrow::Cow<'_, str>) -> PropertyLookupResult<'_> {
    static RESERVED_PROPERTIES: std::sync::LazyLock<
        HashMap<&'static str, (Type, PropertyVisibility, Option<BuiltinFunction>)>,
    > = std::sync::LazyLock::new(|| {
        reserved_properties()
            .map(|(name, ty, visibility)| (name, (ty, visibility, reserved_member_function(name))))
            .collect()
    });
    if let Some((ty, visibility, builtin_function)) =
        RESERVED_PROPERTIES.get(name.as_ref()).cloned()
    {
        return PropertyLookupResult {
            property_type: ty,
            is_slint_sc: matches!(name.as_ref(), "x" | "y" | "width" | "height"),
            resolved_name: name,
            is_local_to_component: false,
            is_in_direct_base: false,
            is_shadowable: false,
            property_visibility: visibility,
            declared_pure: None,
            builtin_function,
            internal_name: None,
            deprecated: None,
        };
    }

    // Report deprecated known reserved properties (maximum_width, minimum_height, ...)
    for pre in &["min", "max"] {
        if let Some(a) = name.strip_prefix(pre) {
            for suf in &["width", "height"] {
                if let Some(b) = a.strip_suffix(suf)
                    && b == "imum-"
                {
                    return PropertyLookupResult {
                        property_type: Type::LogicalLength,
                        resolved_name: format!("{pre}-{suf}").into(),
                        is_local_to_component: false,
                        is_in_direct_base: false,
                        is_shadowable: false,
                        property_visibility: crate::object_tree::PropertyVisibility::InOut,
                        declared_pure: None,
                        builtin_function: None,
                        is_slint_sc: false,
                        internal_name: None,
                        deprecated: None,
                    };
                }
            }
        }
    }
    PropertyLookupResult::invalid(name)
}

pub fn reserved_member_functions()
-> impl Iterator<Item = (&'static str, BuiltinFunction, PropertyVisibility)> {
    IntoIterator::into_iter([
        ("focus", BuiltinFunction::SetFocusItem, PropertyVisibility::Public), // match for callable "focus" property
        ("clear-focus", BuiltinFunction::ClearFocusItem, PropertyVisibility::Public), // match for callable "clear-focus" property
    ])
}

/// These member functions are injected in every time
pub fn reserved_member_function(name: &str) -> Option<BuiltinFunction> {
    for (m, e, _) in reserved_member_functions() {
        if m == name {
            return Some(e);
        }
    }
    None
}

/// All types (datatypes, internal elements, properties, ...) are stored in this type
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
    pub(crate) context_restricted_types: HashMap<SmolStr, HashSet<SmolStr>>,
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
        let mut register = Self::with_builtin_types();
        crate::builtin_elements::load(&mut register);
        register
    }

    /// A register with the basic types, the builtin structs and enums, but no elements.
    pub(crate) fn with_builtin_types() -> Self {
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
        register.insert_type(Type::StyledText);
        register.insert_type(Type::Keys);
        register.insert_type(Type::DataTransfer);
        register.insert_type(Type::MouseCursor);
        register.types.insert("Point".into(), logical_point_type().into());
        register.types.insert("Size".into(), logical_size_type().into());

        BUILTIN.enums.fill_register(&mut register);

        register.supported_property_animation_types.insert(Type::Float32.to_string());
        register.supported_property_animation_types.insert(Type::Int32.to_string());
        register.supported_property_animation_types.insert(Type::Color.to_string());
        register.supported_property_animation_types.insert(Type::PhysicalLength.to_string());
        register.supported_property_animation_types.insert(Type::LogicalLength.to_string());
        register.supported_property_animation_types.insert(Type::Brush.to_string());
        register.supported_property_animation_types.insert(Type::Angle.to_string());

        macro_rules! register_builtin_structs {
            ($(
                $(#[$attr:meta])*
                $vis:vis struct $Name:ident {
                    $( $(#[$field_attr:meta])* $field:ident : $field_type:ident $(= $field_default:expr)?, )*
                }
            )*) => { $(
                register.insert_type_with_name(Type::Struct(builtin_structs::$Name()), SmolStr::new(stringify!($Name)));
            )* };
        }
        i_slint_common::for_each_builtin_structs!(register_builtin_structs);

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

    pub fn add_builtin(&mut self, builtin: Arc<BuiltinElement>) {
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

/// Type definitions for each builtin struct
pub mod builtin_structs {
    use super::*;
    use crate::langtype::ConstantExpression;

    pub static BUILTIN_STRUCTS: std::sync::LazyLock<BuiltinStructs> =
        std::sync::LazyLock::new(BuiltinStructs::new);

    #[rustfmt::skip]
    macro_rules! map_type {
        ($pub_type:ident, bool) => { Type::Bool };
        ($pub_type:ident, i32) => { Type::Int32 };
        ($pub_type:ident, f32) => { Type::Float32 };
        ($pub_type:ident, SharedString) => { Type::String };
        ($pub_type:ident, Image) => { Type::Image };
        ($pub_type:ident, Coord) => { Type::LogicalLength };
        ($pub_type:ident, Keys) => { Type::Keys };
        ($pub_type:ident, DataTransfer) => { Type::DataTransfer };
        ($pub_type:ident, LogicalPosition) => { Type::Struct(logical_point_type()) };
        // A builtin struct declared earlier: `$pub_type` names the local of `BuiltinStructs::new`
        ($pub_type:ident, KeyboardModifiers) => { Type::Struct($pub_type.clone()) };
        ($pub_type:ident, $enum:ident) => { Type::Enumeration(BUILTIN.enums.$enum.clone()) };
    }

    #[rustfmt::skip]
    macro_rules! field_default {
        () => { None };
        (true) => { Some(ConstantExpression::BoolLiteral(true)) };
        (false) => { Some(ConstantExpression::BoolLiteral(false)) };
        ($enum:ident :: $value:ident) => {
            Some(ConstantExpression::EnumerationValue(
                BUILTIN.enums.$enum.clone()
                    .try_value_from_string(&crate::generator::to_kebab_case(stringify!($value)))
                    .expect(concat!("unknown enum variant in field default ", stringify!($enum), "::", stringify!($value))),
            ))
        };
        (($($tt:tt)*)) => { field_default!($($tt)*) };
    }

    macro_rules! declare_builtin_structs {
        ($(
            $(#[$attr:meta])*
            $vis:vis struct $Name:ident {
                $( $(#[$field_attr:meta])* $field:ident : $field_type:ident $(= $field_default:tt)?, )*
            }
        )*) => {
            pub struct BuiltinStructs {
                $(
                #[allow(non_snake_case)]
                $Name: Arc<Struct>
                ),*
            }
            impl BuiltinStructs {
                pub fn new() -> Self {
                    $(
                        #[allow(non_snake_case)]
                        let $Name = build_struct(BuiltinStruct::$Name, &[$(
                            (stringify!($field), map_type!($field_type, $field_type), field_default!($($field_default)?)),
                        )*]);
                    )*
                    Self { $($Name),* }
                }
            }

            impl Default for BuiltinStructs {
                fn default() -> Self {
                    Self::new()
                }
            }

            fn from_macro(name: &BuiltinStruct) -> Option<Arc<Struct>> {
                match name {
                    $(BuiltinStruct::$Name => Some($Name()),)*
                    _ => None,
                }
            }

            $(
            #[allow(non_snake_case)]
            pub fn $Name() -> Arc<Struct> {
                BUILTIN_STRUCTS.$Name.clone()
            }
            )*
        };
    }
    i_slint_common::for_each_builtin_structs!(declare_builtin_structs);

    /// The path elements' structs, made of the properties of their native class.
    static PATH_ELEMENTS: std::sync::LazyLock<HashMap<BuiltinStruct, Arc<Struct>>> =
        std::sync::LazyLock::new(|| {
            crate::builtin_elements::BUILTIN_ELEMENTS
                .elements()
                .filter_map(|element| {
                    let class = &element.native_class;
                    let name = class.builtin_struct.clone()?;
                    let fields = class.properties.iter().map(|(k, v)| (k.clone(), v.ty.clone()));
                    Some((name.clone(), Arc::new(Struct::new(fields.collect(), name))))
                })
                .collect()
        });

    /// The definition of the builtin struct `name`.
    /// Code that makes a struct with a builtin name takes its type from here.
    pub fn get(name: &BuiltinStruct) -> Arc<Struct> {
        // Each lookup touches only the static that holds `name`: the macro structs ask for
        // `LogicalPosition` while they're built, and the path elements need a builtin register.
        if let Some(s) = from_macro(name) {
            return s;
        }
        BUILTIN
            .structs
            .get(name)
            .or_else(|| PATH_ELEMENTS.get(name))
            .unwrap_or_else(|| panic!("no definition for {name:?}"))
            .clone()
    }

    fn build_struct(
        name: BuiltinStruct,
        fields: &[(&str, Type, Option<ConstantExpression>)],
    ) -> Arc<Struct> {
        let mut s =
            Struct { fields: BTreeMap::new(), field_defaults: BTreeMap::new(), name: name.into() };
        for (field, ty, default) in fields {
            let field = field.replace_smolstr("_", "-");
            if let Some(default) = default {
                s.field_defaults.insert(field.clone(), default.clone());
            }
            s.fields.insert(field, ty.clone());
        }
        Arc::new(s)
    }
}

pub fn logical_point_type() -> Arc<Struct> {
    builtin_structs::get(&BuiltinStruct::LogicalPosition)
}

pub fn logical_size_type() -> Arc<Struct> {
    builtin_structs::get(&BuiltinStruct::LogicalSize)
}

pub fn font_metrics_type() -> Type {
    Type::Struct(builtin_structs::FontMetrics())
}

/// The [`Type`] for a runtime LayoutInfo structure
pub fn layout_info_type() -> Arc<Struct> {
    builtin_structs::get(&BuiltinStruct::LayoutInfo)
}

/// The [`Type`] for a runtime PathElement structure
pub fn path_element_type() -> Type {
    builtin_structs::get(&BuiltinStruct::PathElement).into()
}

/// The [`Type`] for a runtime LayoutItemInfo structure
pub fn layout_item_info_type() -> Type {
    builtin_structs::get(&BuiltinStruct::LayoutItemInfo).into()
}

/// The [`Type`] for a runtime FlexItemProps structure
pub fn flex_item_props_type() -> Type {
    builtin_structs::get(&BuiltinStruct::FlexItemProps).into()
}
