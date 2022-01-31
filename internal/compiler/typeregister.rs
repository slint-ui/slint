// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::expression_tree::{BuiltinFunction, Expression};
use crate::langtype::{BuiltinPropertyInfo, Enumeration, PropertyLookupResult, Type};
use crate::object_tree::Component;

pub(crate) const RESERVED_GEOMETRY_PROPERTIES: &[(&str, Type)] = &[
    ("x", Type::LogicalLength),
    ("y", Type::LogicalLength),
    ("width", Type::LogicalLength),
    ("height", Type::LogicalLength),
    ("z", Type::Float32),
];

const RESERVED_LAYOUT_PROPERTIES: &[(&str, Type)] = &[
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

thread_local! {
    pub static DIALOG_BUTTON_ROLE_ENUM: Rc<Enumeration> =
        Rc::new(Enumeration {
            name: "DialogButtonRole".into(),
            values: IntoIterator::into_iter([
                "none".to_owned(),
                "accept".to_owned(),
                "reject".to_owned(),
                "apply".to_owned(),
                "reset".to_owned(),
                "action".to_owned(),
                "help".to_owned(),
            ])
            .collect(),
            default_value: 0,
        });

    pub static LAYOUT_ALIGNMENT_ENUM: Rc<Enumeration> =
        Rc::new(Enumeration {
            name: "LayoutAlignment".into(),
            values: IntoIterator::into_iter(
                ["stretch", "center", "start", "end", "space-between", "space-around"]
            ).map(String::from).collect(),
            default_value: 0,
        });

    pub static PATH_EVENT_ENUM: Rc<Enumeration> =
    Rc::new(Enumeration {
        name: "PathEvent".into(),
        values: IntoIterator::into_iter(
            ["begin", "line", "quadratic", "cubic", "end_open", "end_closed"]
        ).map(String::from).collect(),
        default_value: 0,
    });
}

const RESERVED_OTHER_PROPERTIES: &[(&str, Type)] = &[
    ("clip", Type::Bool),
    ("opacity", Type::Float32),
    ("visible", Type::Bool), // ("enabled", Type::Bool),
];

pub(crate) const RESERVED_DROP_SHADOW_PROPERTIES: &[(&str, Type)] = &[
    ("drop-shadow-offset-x", Type::LogicalLength),
    ("drop-shadow-offset-y", Type::LogicalLength),
    ("drop-shadow-blur", Type::LogicalLength),
    ("drop-shadow-color", Type::Color),
];

/// list of reserved property injected in every item
pub fn reserved_properties() -> impl Iterator<Item = (&'static str, Type)> {
    RESERVED_GEOMETRY_PROPERTIES
        .iter()
        .chain(RESERVED_LAYOUT_PROPERTIES.iter())
        .chain(RESERVED_OTHER_PROPERTIES.iter())
        .chain(RESERVED_DROP_SHADOW_PROPERTIES.iter())
        .map(|(k, v)| (*k, v.clone()))
        .chain(IntoIterator::into_iter([
            ("forward-focus", Type::ElementReference),
            ("focus", BuiltinFunction::SetFocusItem.ty()),
            ("dialog-button-role", Type::Enumeration(DIALOG_BUTTON_ROLE_ENUM.with(|e| e.clone()))),
        ]))
}

/// lookup reserved property injected in every item
pub fn reserved_property(name: &str) -> PropertyLookupResult {
    for (p, t) in reserved_properties() {
        if p == name {
            return PropertyLookupResult { property_type: t, resolved_name: name.into() };
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
                        };
                    }
                }
            }
        }
    }
    PropertyLookupResult { resolved_name: name.into(), property_type: Type::Invalid }
}

/// These member functions are injected in every time
pub fn reserved_member_function(name: &str) -> Expression {
    for (m, e) in [
        ("focus", Expression::BuiltinFunctionReference(BuiltinFunction::SetFocusItem, None)), // match for callable "focus" property
    ]
    .iter()
    {
        if *m == name {
            return e.clone();
        }
    }
    Expression::Invalid
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

        let mut declare_enum = |name: &str, values: &[&str]| {
            register.insert_type_with_name(
                Type::Enumeration(Rc::new(Enumeration {
                    name: name.to_owned(),
                    values: values.iter().cloned().map(String::from).collect(),
                    default_value: 0,
                })),
                name.to_owned(),
            );
        };

        declare_enum("TextHorizontalAlignment", &["left", "center", "right"]);
        declare_enum("TextVerticalAlignment", &["top", "center", "bottom"]);
        declare_enum("TextWrap", &["no-wrap", "word-wrap"]);
        declare_enum("TextOverflow", &["clip", "elide"]);
        declare_enum("ImageFit", &["fill", "contain", "cover"]);
        declare_enum("ImageRendering", &["smooth", "pixelated"]);
        declare_enum("EventResult", &["reject", "accept"]);
        declare_enum("FillRule", &["nonzero", "evenodd"]);
        declare_enum(
            "MouseCursor",
            &[
                "default",
                "none",
                "help",
                "pointer",
                "progress",
                "wait",
                "crosshair",
                "text",
                "alias",
                "copy",
                "no-drop",
                "not-allowed",
                "grab",
                "grabbing",
                "col-resize",
                "row-resize",
                "n-resize",
                "e-resize",
                "s-resize",
                "w-resize",
                "ne-resize",
                "nw-resize",
                "se-resize",
                "sw-resize",
                "ew-resize",
                "ns-resize",
                "nesw-resize",
                "nwse-resize",
            ],
        );
        declare_enum(
            "StandardButtonKind",
            &[
                "ok", "cancel", "apply", "close", "reset", "help", "yes", "no", "abort", "retry",
                "ignore",
            ],
        );
        declare_enum("PointerEventKind", &["cancel", "down", "up"]);
        declare_enum("PointerEventButton", &["none", "left", "right", "middle"]);
        DIALOG_BUTTON_ROLE_ENUM
            .with(|e| register.insert_type_with_name(Type::Enumeration(e.clone()), e.name.clone()));
        LAYOUT_ALIGNMENT_ENUM
            .with(|e| register.insert_type_with_name(Type::Enumeration(e.clone()), e.name.clone()));

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
            .types
            .values()
            .for_each(|ty| ty.collect_contextual_types(&mut context_restricted_types));
        register.context_restricted_types = context_restricted_types;

        match &mut register.types.get_mut("PopupWindow").unwrap() {
            Type::Builtin(ref mut b) => {
                Rc::get_mut(b).unwrap().properties.insert(
                    "show".into(),
                    BuiltinPropertyInfo::new(BuiltinFunction::ShowPopupWindow.ty()),
                );
                Rc::get_mut(b).unwrap().member_functions.insert(
                    "show".into(),
                    Expression::BuiltinFunctionReference(BuiltinFunction::ShowPopupWindow, None),
                );
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
                } else {
                    let mut elements = permitted_parent_types.iter().cloned().collect::<Vec<_>>();
                    elements.sort();
                    format!(
                        "{} can only be within the following elements: {}",
                        name,
                        elements.join(", ")
                    )
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

    /// Return a hashmap with all the registered type
    pub fn all_types(&self) -> HashMap<String, Type> {
        let mut all =
            self.parent_registry.as_ref().map(|r| r.borrow().all_types()).unwrap_or_default();
        for (k, v) in &self.types {
            all.insert(k.clone(), v.clone());
        }
        all
    }
}
