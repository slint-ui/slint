use std::collections::{BTreeMap, HashMap, HashSet};
use std::{cell::RefCell, fmt::Display, rc::Rc};

#[derive(Debug, Clone)]
pub enum Type {
    /// Correspond to an uninitialized type, or an error
    Invalid,
    /// The type of an expression that return nothing
    Void,
    Component(Rc<crate::object_tree::Component>),
    Builtin(Rc<BuiltinElement>),
    Native(Rc<NativeClass>),

    Signal,

    // Other property types:
    Float32,
    Int32,
    String,
    Color,
    Duration,
    Length,
    LogicalLength,
    Resource,
    Bool,
    Model,
    PathElements,

    Array(Box<Type>),
    Object(BTreeMap<String, Type>),
}

impl core::cmp::PartialEq for Type {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Type::Invalid, Type::Invalid) => true,
            (Type::Void, Type::Void) => true,
            (Type::Component(a), Type::Component(b)) => Rc::ptr_eq(a, b),
            (Type::Builtin(a), Type::Builtin(b)) => Rc::ptr_eq(a, b),
            (Type::Native(a), Type::Native(b)) => Rc::ptr_eq(a, b),
            (Type::Signal, Type::Signal) => true,
            (Type::Float32, Type::Float32) => true,
            (Type::Int32, Type::Int32) => true,
            (Type::String, Type::String) => true,
            (Type::Color, Type::Color) => true,
            (Type::Duration, Type::Duration) => true,
            (Type::Length, Type::Length) => true,
            (Type::LogicalLength, Type::LogicalLength) => true,
            (Type::Resource, Type::Resource) => true,
            (Type::Bool, Type::Bool) => true,
            (Type::Array(a), Type::Array(b)) => a == b,
            (Type::Object(a), Type::Object(b)) => a == b,
            (Type::Model, Type::Model) => true,
            (Type::PathElements, Type::PathElements) => true,
            _ => false,
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Invalid => write!(f, "<error>"),
            Type::Void => write!(f, "void"),
            Type::Component(c) => c.id.fmt(f),
            Type::Builtin(b) => b.native_class.class_name.fmt(f),
            Type::Native(b) => b.class_name.fmt(f),
            Type::Signal => write!(f, "signal"),
            Type::Float32 => write!(f, "float32"),
            Type::Int32 => write!(f, "int32"),
            Type::String => write!(f, "string"),
            Type::Duration => write!(f, "duration"),
            Type::Length => write!(f, "length"),
            Type::LogicalLength => write!(f, "logical_length"),
            Type::Color => write!(f, "color"),
            Type::Resource => write!(f, "resource"),
            Type::Bool => write!(f, "bool"),
            Type::Model => write!(f, "model"),
            Type::Array(t) => write!(f, "[{}]", t),
            Type::Object(t) => {
                write!(f, "{{ ")?;
                for (k, v) in t {
                    write!(f, "{}: {},", k, v)?;
                }
                write!(f, "}}")
            }
            Type::PathElements => write!(f, "pathelements"),
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
                | Self::Length
                | Self::LogicalLength
                | Self::Resource
                | Self::Bool
                | Self::Model
                | Self::Object(_)
        )
    }

    pub fn lookup_property(&self, name: &str) -> Type {
        match self {
            Type::Component(c) => c.root_element.borrow().lookup_property(name),
            Type::Builtin(b) => b.properties.get(name).cloned().unwrap_or_default(),
            Type::Native(n) => n.lookup_property(name).unwrap_or_default(),
            _ => Type::Invalid,
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
    pub fn as_component(&self) -> &Rc<crate::object_tree::Component> {
        match self {
            Type::Component(c) => c,
            _ => panic!("should be a component because of the repeater_component pass"),
        }
    }

    /// Return true if the type can be converted to the other type
    pub fn can_convert(&self, other: &Self) -> bool {
        self == other
            || matches!(
                (self, other),
                (Type::Float32, Type::Int32)
                    | (Type::Float32, Type::String)
                    | (Type::Int32, Type::Float32)
                    | (Type::Int32, Type::String)
                    | (Type::Array(_), Type::Model)
                    | (Type::Float32, Type::Model)
                    | (Type::Int32, Type::Model)
                    | (Type::Length, Type::LogicalLength)
                    | (Type::LogicalLength, Type::Length)
            )
    }

    fn collect_contextual_types(
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

    fn lookup_property_distance(self: Rc<Self>, name: &str) -> (usize, Rc<Self>) {
        let mut distance = 0;
        let mut class = self.clone();
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
        let (_min_distance, minimal_class) = properties_used.fold(
            (std::usize::MAX, self.clone()),
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

#[derive(Debug, Clone, Default)]
pub struct BuiltinElement {
    pub native_class: Rc<NativeClass>,
    pub properties: HashMap<String, Type>,
    pub additional_accepted_child_types: HashMap<String, Type>,
    pub disallow_global_types_as_child_elements: bool,
}

impl BuiltinElement {
    pub fn new(native_class: Rc<NativeClass>) -> Self {
        let mut properties = HashMap::new();
        native_class.clone().visit_class_hierarchy(|class| {
            for (prop_name, prop_type) in class.local_property_iter() {
                properties.insert(prop_name.clone(), prop_type.clone());
            }
        });
        Self { native_class, properties, ..Default::default() }
    }
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
    pub fn builtin() -> Rc<RefCell<Self>> {
        let mut r = TypeRegister::default();

        let mut insert_type = |t: Type| r.types.insert(t.to_string(), t);
        insert_type(Type::Float32);
        insert_type(Type::Int32);
        insert_type(Type::String);
        insert_type(Type::Length);
        insert_type(Type::LogicalLength);
        insert_type(Type::Color);
        insert_type(Type::Duration);
        insert_type(Type::Resource);
        insert_type(Type::Bool);
        insert_type(Type::Model);

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

        r.types.insert(
            "Rectangle".to_owned(),
            Type::Builtin(Rc::new(BuiltinElement::new(border_rectangle))),
        );

        let mut image = NativeClass::new("Image");
        image.properties.insert("source".to_owned(), Type::Resource);
        image.properties.insert("x".to_owned(), Type::Length);
        image.properties.insert("y".to_owned(), Type::Length);
        image.properties.insert("width".to_owned(), Type::Length);
        image.properties.insert("height".to_owned(), Type::Length);
        let image = Rc::new(image);
        r.types.insert("Image".to_owned(), Type::Builtin(Rc::new(BuiltinElement::new(image))));

        let mut text = NativeClass::new("Text");
        text.properties.insert("text".to_owned(), Type::String);
        text.properties.insert("font_family".to_owned(), Type::String);
        text.properties.insert("font_pixel_size".to_owned(), Type::Length);
        text.properties.insert("color".to_owned(), Type::Color);
        text.properties.insert("x".to_owned(), Type::Length);
        text.properties.insert("y".to_owned(), Type::Length);
        let text = Rc::new(text);
        r.types.insert("Text".to_owned(), Type::Builtin(Rc::new(BuiltinElement::new(text))));

        let mut touch_area = NativeClass::new("TouchArea");
        touch_area.properties.insert("x".to_owned(), Type::Length);
        touch_area.properties.insert("y".to_owned(), Type::Length);
        touch_area.properties.insert("width".to_owned(), Type::Length);
        touch_area.properties.insert("height".to_owned(), Type::Length);
        touch_area.properties.insert("pressed".to_owned(), Type::Bool);
        touch_area.properties.insert("clicked".to_owned(), Type::Signal);
        let touch_area = Rc::new(touch_area);
        r.types.insert(
            "TouchArea".to_owned(),
            Type::Builtin(Rc::new(BuiltinElement::new(touch_area))),
        );

        let mut grid_layout = BuiltinElement::new(Rc::new(NativeClass::new("GridLayout")));
        grid_layout.properties.insert("x".to_owned(), Type::Length);
        grid_layout.properties.insert("y".to_owned(), Type::Length);

        // Row can only be in a GridLayout
        let row = BuiltinElement::new(Rc::new(NativeClass::new("Row")));
        grid_layout
            .additional_accepted_child_types
            .insert("Row".to_owned(), Type::Builtin(Rc::new(row)));

        r.types.insert("GridLayout".to_owned(), Type::Builtin(Rc::new(grid_layout)));

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
            let line_to = BuiltinElement::new(line_to_class);

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
            let arc_to = BuiltinElement::new(arc_to_class);

            let mut close_class = NativeClass::new("Close");
            close_class.rust_type_constructor =
                Some("sixtyfps::re_exports::PathElement::Close".into());
            let close_class = Rc::new(close_class);
            let close = BuiltinElement::new(close_class);

            [Rc::new(line_to), Rc::new(arc_to), Rc::new(close)]
        };

        path_elements.iter().for_each(|elem| {
            path_elem
                .additional_accepted_child_types
                .insert(elem.native_class.class_name.clone(), Type::Builtin(elem.clone()));
        });

        r.types.insert("Path".to_owned(), Type::Builtin(Rc::new(path_elem)));

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
        r.types.insert("PathLayout".to_owned(), Type::Builtin(Rc::new(path_layout)));

        let mut property_animation = NativeClass::new("PropertyAnimation");
        property_animation.properties.insert("duration".to_owned(), Type::Duration);
        property_animation.properties.insert("loop_count".to_owned(), Type::Int32);
        let property_animation = Rc::new(property_animation);
        r.property_animation_type = Type::Builtin(Rc::new(BuiltinElement::new(property_animation)));
        r.supported_property_animation_types.insert(Type::Float32.to_string());
        r.supported_property_animation_types.insert(Type::Int32.to_string());
        r.supported_property_animation_types.insert(Type::Color.to_string());
        r.supported_property_animation_types.insert(Type::Length.to_string());
        r.supported_property_animation_types.insert(Type::LogicalLength.to_string());

        let mut context_restricted_types = HashMap::new();
        r.types.values().for_each(|ty| ty.collect_contextual_types(&mut context_restricted_types));
        r.context_restricted_types = context_restricted_types;

        Rc::new(RefCell::new(r))
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

    pub fn add(&mut self, comp: Rc<crate::object_tree::Component>) {
        self.add_with_name(comp.id.clone(), comp);
    }

    pub fn add_with_name(&mut self, name: String, comp: Rc<crate::object_tree::Component>) {
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
