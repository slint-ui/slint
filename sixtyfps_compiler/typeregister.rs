/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use std::collections::{HashMap, HashSet};
use std::{cell::RefCell, rc::Rc};

use crate::expression_tree::{BuiltinFunction, Expression, Unit};
use crate::langtype::{BuiltinElement, Enumeration, NativeClass, Type};
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
    property_animation_type: Type,
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

        let declare_enum = |name: &str, values: &[&str]| {
            Rc::new(Enumeration {
                name: name.to_owned(),
                values: values.into_iter().cloned().map(String::from).collect(),
                default_value: 0,
            })
        };

        let text_horizontal_alignment =
            declare_enum("TextHorizontalAlignment", &["align_left", "align_center", "align_right"]);
        let text_vertical_alignment =
            declare_enum("TextVerticalAlignment", &["align_top", "align_center", "align_bottom"]);
        let layout_alignment = declare_enum(
            "LayoutAlignment",
            &["stretch", "center", "start", "end", "space_between", "space_around"],
        );

        let native_class_with_member_functions =
            |tr: &mut TypeRegister,
             name: &str,
             properties: &[(&str, Type)],
             default_bindings: &[(&str, Expression)],
             member_functions: &[(&str, Type, Expression)]| {
                let native = Rc::new(NativeClass::new_with_properties(
                    name,
                    properties.iter().map(|(n, t)| (n.to_string(), t.clone())),
                ));
                let mut builtin = BuiltinElement::new(native);
                for (prop, expr) in default_bindings {
                    builtin.default_bindings.insert(prop.to_string(), expr.clone());
                }
                for (name, funtype, fun) in member_functions {
                    builtin.properties.insert(name.to_string(), funtype.clone());
                    builtin.member_functions.insert(name.to_string(), fun.clone());
                }
                tr.insert_type(Type::Builtin(Rc::new(builtin)));
            };

        let native_class = |tr: &mut TypeRegister,
                            name: &str,
                            properties: &[(&str, Type)],
                            default_bindings: &[(&str, Expression)]| {
            native_class_with_member_functions(tr, name, properties, default_bindings, &[])
        };

        let mut rectangle = NativeClass::new("Rectangle");
        rectangle.properties.insert("color".to_owned(), Type::Color);
        rectangle.properties.insert("x".to_owned(), Type::Length);
        rectangle.properties.insert("y".to_owned(), Type::Length);
        rectangle.properties.insert("width".to_owned(), Type::Length);
        rectangle.properties.insert("height".to_owned(), Type::Length);
        let rectangle = Rc::new(rectangle);

        let mut border_rectangle = NativeClass::new("BorderRectangle");
        border_rectangle.parent = Some(rectangle.clone());
        border_rectangle.properties.insert("border_width".to_owned(), Type::Length);
        border_rectangle.properties.insert("border_radius".to_owned(), Type::Length);
        border_rectangle.properties.insert("border_color".to_owned(), Type::Color);
        let border_rectangle = Rc::new(border_rectangle);

        register.types.insert(
            "Rectangle".to_owned(),
            Type::Builtin(Rc::new(BuiltinElement::new(border_rectangle))),
        );

        native_class(
            &mut register,
            "Image",
            &[
                ("source", Type::Resource),
                ("x", Type::Length),
                ("y", Type::Length),
                ("width", Type::Length),
                ("height", Type::Length),
            ],
            &[],
        );

        native_class(
            &mut register,
            "Text",
            &[
                ("text", Type::String),
                ("font_family", Type::String),
                ("font_size", Type::Length),
                ("color", Type::Color),
                ("horizontal_alignment", Type::Enumeration(text_horizontal_alignment.clone())),
                ("vertical_alignment", Type::Enumeration(text_vertical_alignment.clone())),
                ("x", Type::Length),
                ("y", Type::Length),
                ("width", Type::Length),
                ("height", Type::Length),
            ],
            &[(
                "color",
                Expression::Cast {
                    from: Box::new(Expression::NumberLiteral(0xff000000u32 as _, Unit::None)),
                    to: Type::Color,
                },
            )],
        );

        native_class(
            &mut register,
            "TouchArea",
            &[
                ("x", Type::Length),
                ("y", Type::Length),
                ("width", Type::Length),
                ("height", Type::Length),
                ("pressed", Type::Bool),
                ("mouse_x", Type::Length),
                ("mouse_y", Type::Length),
                ("pressed_x", Type::Length),
                ("pressed_y", Type::Length),
                ("clicked", Type::Signal { args: vec![] }),
            ],
            &[],
        );

        native_class(
            &mut register,
            "Flickable",
            &[
                ("x", Type::Length),
                ("y", Type::Length),
                ("width", Type::Length),
                ("height", Type::Length),
                // These properties are actually going to be forwarded to the viewport by the
                // code generator
                ("viewport_height", Type::Length),
                ("viewport_width", Type::Length),
                ("viewport_x", Type::Length),
                ("viewport_y", Type::Length),
                ("interactive", Type::Bool),
            ],
            &[("interactive", Expression::BoolLiteral(true))],
        );

        native_class(
            &mut register,
            "Window",
            &[("width", Type::Length), ("height", Type::Length)],
            &[],
        );

        native_class_with_member_functions(
            &mut register,
            "TextInput",
            &[
                ("text", Type::String),
                ("font_family", Type::String),
                ("font_size", Type::Length),
                ("color", Type::Color),
                ("selection_foreground_color", Type::Color),
                ("selection_background_color", Type::Color),
                ("horizontal_alignment", Type::Enumeration(text_horizontal_alignment)),
                ("vertical_alignment", Type::Enumeration(text_vertical_alignment)),
                ("x", Type::Length),
                ("y", Type::Length),
                ("width", Type::Length),
                ("height", Type::Length),
                ("text_cursor_width", Type::Length),
                ("cursor_position", Type::Int32),
                ("anchor_position", Type::Int32),
                ("has_focus", Type::Bool),
                ("accepted", Type::Signal { args: vec![] }),
                ("edited", Type::Signal { args: vec![] }),
                ("enabled", Type::Bool),
            ],
            &[
                (
                    "color",
                    Expression::Cast {
                        from: Box::new(Expression::NumberLiteral(0xff000000u32 as _, Unit::None)),
                        to: Type::Color,
                    },
                ),
                (
                    "selection_foreground_color",
                    Expression::Cast {
                        from: Box::new(Expression::NumberLiteral(0xff000000u32 as _, Unit::None)),
                        to: Type::Color,
                    },
                ),
                (
                    "selection_background_color",
                    Expression::Cast {
                        from: Box::new(Expression::NumberLiteral(0xff808080u32 as _, Unit::None)),
                        to: Type::Color,
                    },
                ),
                ("text_cursor_width", Expression::NumberLiteral(2., Unit::Px)),
                ("enabled", Expression::BoolLiteral(true)),
            ],
            &[(
                "focus",
                BuiltinFunction::SetFocusItem.ty(),
                Expression::BuiltinFunctionReference(BuiltinFunction::SetFocusItem),
            )],
        );

        let mut grid_layout = BuiltinElement::new(Rc::new(NativeClass::new("GridLayout")));
        grid_layout.properties.insert("spacing".to_owned(), Type::Length);

        // Row can only be in a GridLayout
        let mut row = BuiltinElement::new(Rc::new(NativeClass::new("Row")));
        row.is_non_item_type = true;
        grid_layout
            .additional_accepted_child_types
            .insert("Row".to_owned(), Type::Builtin(Rc::new(row)));

        register.insert_type(Type::Builtin(Rc::new(grid_layout)));

        let mut horizontal_layout =
            BuiltinElement::new(Rc::new(NativeClass::new("HorizontalLayout")));
        horizontal_layout.properties.insert("spacing".to_owned(), Type::Length);
        horizontal_layout
            .properties
            .insert("alignment".to_owned(), Type::Enumeration(layout_alignment.clone()));
        register.insert_type(Type::Builtin(Rc::new(horizontal_layout)));
        let mut vertical_layout = BuiltinElement::new(Rc::new(NativeClass::new("VerticalLayout")));
        vertical_layout.properties.insert("spacing".to_owned(), Type::Length);
        vertical_layout
            .properties
            .insert("alignment".to_owned(), Type::Enumeration(layout_alignment));
        register.insert_type(Type::Builtin(Rc::new(vertical_layout)));

        let mut path_class = NativeClass::new("Path");
        path_class.properties.insert("x".to_owned(), Type::Length);
        path_class.properties.insert("y".to_owned(), Type::Length);
        path_class.properties.insert("width".to_owned(), Type::Length);
        path_class.properties.insert("height".to_owned(), Type::Length);
        path_class.properties.insert("fill_color".to_owned(), Type::Color);
        path_class.properties.insert("stroke_color".to_owned(), Type::Color);
        path_class.properties.insert("stroke_width".to_owned(), Type::Float32);
        let path = Rc::new(path_class);
        let mut path_elem = BuiltinElement::new(path);
        path_elem.properties.insert("commands".to_owned(), Type::String);
        path_elem.disallow_global_types_as_child_elements = true;

        let path_elements = {
            let mut line_to_class = NativeClass::new("LineTo");
            line_to_class.properties.insert("x".to_owned(), Type::Float32);
            line_to_class.properties.insert("y".to_owned(), Type::Float32);
            line_to_class.rust_type_constructor =
                Some("sixtyfps::re_exports::PathElement::LineTo(PathLineTo{{}})".into());
            line_to_class.cpp_type = Some("sixtyfps::PathLineTo".into());
            let line_to_class = Rc::new(line_to_class);
            let mut line_to = BuiltinElement::new(line_to_class);
            line_to.is_non_item_type = true;

            let mut arc_to_class = NativeClass::new("ArcTo");
            arc_to_class.properties.insert("x".to_owned(), Type::Float32);
            arc_to_class.properties.insert("y".to_owned(), Type::Float32);
            arc_to_class.properties.insert("radius_x".to_owned(), Type::Float32);
            arc_to_class.properties.insert("radius_y".to_owned(), Type::Float32);
            arc_to_class.properties.insert("x_rotation".to_owned(), Type::Float32);
            arc_to_class.properties.insert("large_arc".to_owned(), Type::Bool);
            arc_to_class.properties.insert("sweep".to_owned(), Type::Bool);
            arc_to_class.rust_type_constructor =
                Some("sixtyfps::re_exports::PathElement::ArcTo(PathArcTo{{}})".into());
            arc_to_class.cpp_type = Some("sixtyfps::PathArcTo".into());
            let arc_to_class = Rc::new(arc_to_class);
            let mut arc_to = BuiltinElement::new(arc_to_class);
            arc_to.is_non_item_type = true;

            let mut close_class = NativeClass::new("Close");
            close_class.rust_type_constructor =
                Some("sixtyfps::re_exports::PathElement::Close".into());
            let close_class = Rc::new(close_class);
            let mut close = BuiltinElement::new(close_class);
            close.is_non_item_type = true;

            [Rc::new(line_to), Rc::new(arc_to), Rc::new(close)]
        };

        path_elements.iter().for_each(|elem| {
            path_elem
                .additional_accepted_child_types
                .insert(elem.native_class.class_name.clone(), Type::Builtin(elem.clone()));
        });

        register.insert_type(Type::Builtin(Rc::new(path_elem)));

        let mut path_layout = BuiltinElement::new(Rc::new(NativeClass::new("PathLayout")));
        path_layout.properties.insert("x".to_owned(), Type::Length);
        path_layout.properties.insert("y".to_owned(), Type::Length);
        path_layout.properties.insert("width".to_owned(), Type::Length);
        path_layout.properties.insert("height".to_owned(), Type::Length);
        path_layout.properties.insert("commands".to_owned(), Type::String);
        path_layout.properties.insert("offset".to_owned(), Type::Float32);
        path_elements.iter().for_each(|elem| {
            path_layout
                .additional_accepted_child_types
                .insert(elem.native_class.class_name.clone(), Type::Builtin(elem.clone()));
        });
        register.insert_type(Type::Builtin(Rc::new(path_layout)));

        let mut property_animation = NativeClass::new("PropertyAnimation");
        property_animation.properties.insert("duration".to_owned(), Type::Duration);
        property_animation.properties.insert("easing".to_owned(), Type::Easing);
        property_animation.properties.insert("loop_count".to_owned(), Type::Int32);
        let mut property_animation = BuiltinElement::new(Rc::new(property_animation));
        property_animation.is_non_item_type = true;
        register.property_animation_type = Type::Builtin(Rc::new(property_animation));
        register.supported_property_animation_types.insert(Type::Float32.to_string());
        register.supported_property_animation_types.insert(Type::Int32.to_string());
        register.supported_property_animation_types.insert(Type::Color.to_string());
        register.supported_property_animation_types.insert(Type::Length.to_string());
        register.supported_property_animation_types.insert(Type::LogicalLength.to_string());

        let mut context_restricted_types = HashMap::new();
        register
            .types
            .values()
            .for_each(|ty| ty.collect_contextual_types(&mut context_restricted_types));
        register.context_restricted_types = context_restricted_types;

        let standard_listview_item = Type::Object {
            name: Some("sixtyfps::StandardListViewItem".into()),
            fields: [("text".to_owned(), Type::String.into())].iter().cloned().collect(),
        };
        register.types.insert("StandardListViewItem".into(), standard_listview_item.clone());

        // FIXME: should this be auto generated or placed somewhere else
        native_class(
            &mut register,
            "NativeButton",
            &[
                ("x", Type::Length),
                ("y", Type::Length),
                ("width", Type::Length),
                ("height", Type::Length),
                ("text", Type::String),
                ("pressed", Type::Bool),
                ("clicked", Type::Signal { args: vec![] }),
                ("enabled", Type::Bool),
            ],
            &[],
        );
        native_class(
            &mut register,
            "NativeCheckBox",
            &[
                ("x", Type::Length),
                ("y", Type::Length),
                ("width", Type::Length),
                ("height", Type::Length),
                ("text", Type::String),
                ("checked", Type::Bool),
                ("toggled", Type::Signal { args: vec![] }),
            ],
            &[],
        );
        native_class(
            &mut register,
            "NativeSpinBox",
            &[
                ("x", Type::Length),
                ("y", Type::Length),
                ("width", Type::Length),
                ("height", Type::Length),
                ("value", Type::Int32),
            ],
            &[],
        );
        native_class(
            &mut register,
            "NativeSlider",
            &[
                ("x", Type::Length),
                ("y", Type::Length),
                ("width", Type::Length),
                ("height", Type::Length),
                ("value", Type::Float32),
                ("min", Type::Float32),
                ("max", Type::Float32),
            ],
            &[],
        );
        native_class(
            &mut register,
            "NativeGroupBox",
            &[
                ("x", Type::Length),
                ("y", Type::Length),
                ("width", Type::Length),
                ("height", Type::Length),
                ("title", Type::String),
                ("native_padding_left", Type::Length),
                ("native_padding_right", Type::Length),
                ("native_padding_top", Type::Length),
                ("native_padding_bottom", Type::Length),
            ],
            &[],
        );
        native_class(
            &mut register,
            "NativeLineEdit",
            &[
                ("x", Type::Length),
                ("y", Type::Length),
                ("width", Type::Length),
                ("height", Type::Length),
                ("native_padding_left", Type::Length),
                ("native_padding_right", Type::Length),
                ("native_padding_top", Type::Length),
                ("native_padding_bottom", Type::Length),
                ("focused", Type::Bool),
                ("enabled", Type::Bool),
            ],
            &[],
        );
        native_class(
            &mut register,
            "NativeScrollView",
            &[
                ("x", Type::Length),
                ("y", Type::Length),
                ("width", Type::Length),
                ("height", Type::Length),
                ("horizontal_max", Type::Length),
                ("horizontal_page_size", Type::Length),
                ("horizontal_value", Type::Length),
                ("vertical_max", Type::Length),
                ("vertical_page_size", Type::Length),
                ("vertical_value", Type::Length),
                ("native_padding_left", Type::Length),
                ("native_padding_right", Type::Length),
                ("native_padding_top", Type::Length),
                ("native_padding_bottom", Type::Length),
            ],
            &[],
        );
        native_class(
            &mut register,
            "NativeStandardListViewItem",
            &[
                ("x", Type::Length),
                ("y", Type::Length),
                ("width", Type::Length),
                ("height", Type::Length),
                ("index", Type::Int32),
                ("item", standard_listview_item),
                ("is_selected", Type::Bool),
            ],
            &[],
        );
        native_class(
            &mut register,
            "NativeComboBox",
            &[
                ("x", Type::Length),
                ("y", Type::Length),
                ("width", Type::Length),
                ("height", Type::Length),
                ("current_value", Type::String),
                ("is_open", Type::Bool),
                ("enabled", Type::Bool),
            ],
            &[],
        );

        let mut native_style_metrics =
            BuiltinElement::new(Rc::new(NativeClass::new_with_properties(
                "NativeStyleMetrics",
                [("layout_spacing", Type::LogicalLength), ("layout_padding", Type::LogicalLength)]
                    .iter()
                    .map(|(n, t)| (n.to_string(), t.clone())),
            )));
        native_style_metrics.is_global = true;
        native_style_metrics.is_non_item_type = true;
        let native_style_metrics = Rc::new(Component {
            id: "NativeStyleMetrics".into(),
            root_element: Rc::new(RefCell::new(crate::object_tree::Element {
                base_type: Type::Builtin(Rc::new(native_style_metrics)),
                ..Default::default()
            })),
            ..Default::default()
        });
        native_style_metrics.root_element.borrow_mut().enclosing_component =
            Rc::downgrade(&native_style_metrics);
        register.insert_type(Type::Component(native_style_metrics));

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
