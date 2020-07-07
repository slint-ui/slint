use std::collections::{BTreeMap, HashMap, HashSet};
use std::{fmt::Display, rc::Rc};

#[derive(Debug, Clone)]
pub enum Type {
    Invalid,
    Component(Rc<crate::object_tree::Component>),
    Builtin(Rc<BuiltinElement>),

    Signal,

    // Other property types:
    Float32,
    Int32,
    String,
    Color,
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
            (Type::Component(a), Type::Component(b)) => Rc::ptr_eq(a, b),
            (Type::Builtin(a), Type::Builtin(b)) => Rc::ptr_eq(a, b),
            (Type::Signal, Type::Signal) => true,
            (Type::Float32, Type::Float32) => true,
            (Type::Int32, Type::Int32) => true,
            (Type::String, Type::String) => true,
            (Type::Color, Type::Color) => true,
            (Type::Resource, Type::Resource) => true,
            (Type::Bool, Type::Bool) => true,
            (Type::Array(a), Type::Array(b)) => a == b,
            (Type::Object(a), Type::Object(b)) => a == b,
            (Type::PathElements, Type::PathElements) => true,
            _ => false,
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Invalid => write!(f, "<error>"),
            Type::Component(c) => c.id.fmt(f),
            Type::Builtin(b) => b.class_name.fmt(f),
            Type::Signal => write!(f, "signal"),
            Type::Float32 => write!(f, "float32"),
            Type::Int32 => write!(f, "int32"),
            Type::String => write!(f, "string"),
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
                        builtin.class_name,
                        valid_children.join(" ")
                    ));
                }
            }
            _ => {}
        };
        tr.lookup_element(name)
    }

    pub fn as_builtin(&self) -> &BuiltinElement {
        match &self {
            Type::Builtin(b) => &b,
            Type::Component(_) => panic!("This should not happen because of inlining"),
            _ => panic!("invalid type"),
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
                .insert(builtin.class_name.clone());

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
pub struct BuiltinElement {
    pub class_name: String,
    pub vtable_symbol: String,
    pub properties: HashMap<String, Type>,
    pub additional_accepted_child_types: HashMap<String, Type>,
    pub disallow_global_types_as_child_elements: bool,
    pub cpp_type: Option<String>,
    pub rust_type_constructor: Option<String>,
}

impl BuiltinElement {
    pub fn new(class_name: &str) -> Self {
        let vtable_symbol = format!("{}VTable", class_name);
        Self {
            class_name: class_name.into(),
            vtable_symbol,
            properties: Default::default(),
            ..Default::default()
        }
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
}

impl TypeRegister {
    pub fn builtin() -> Self {
        let mut r = TypeRegister::default();

        let mut instert_type = |t: Type| r.types.insert(t.to_string(), t);
        instert_type(Type::Float32);
        instert_type(Type::Int32);
        instert_type(Type::String);
        instert_type(Type::Color);
        instert_type(Type::Resource);
        instert_type(Type::Bool);
        instert_type(Type::Model);

        let mut rectangle = BuiltinElement::new("Rectangle");
        rectangle.properties.insert("color".to_owned(), Type::Color);
        rectangle.properties.insert("x".to_owned(), Type::Float32);
        rectangle.properties.insert("y".to_owned(), Type::Float32);
        rectangle.properties.insert("width".to_owned(), Type::Float32);
        rectangle.properties.insert("height".to_owned(), Type::Float32);
        r.types.insert("Rectangle".to_owned(), Type::Builtin(Rc::new(rectangle)));

        let mut image = BuiltinElement::new("Image");
        image.properties.insert("source".to_owned(), Type::Resource);
        image.properties.insert("x".to_owned(), Type::Float32);
        image.properties.insert("y".to_owned(), Type::Float32);
        image.properties.insert("width".to_owned(), Type::Float32);
        image.properties.insert("height".to_owned(), Type::Float32);
        r.types.insert("Image".to_owned(), Type::Builtin(Rc::new(image)));

        let mut text = BuiltinElement::new("Text");
        text.properties.insert("text".to_owned(), Type::String);
        text.properties.insert("font_family".to_owned(), Type::String);
        text.properties.insert("font_pixel_size".to_owned(), Type::Float32);
        text.properties.insert("color".to_owned(), Type::Color);
        text.properties.insert("x".to_owned(), Type::Float32);
        text.properties.insert("y".to_owned(), Type::Float32);
        r.types.insert("Text".to_owned(), Type::Builtin(Rc::new(text)));

        let mut touch_area = BuiltinElement::new("TouchArea");
        touch_area.properties.insert("x".to_owned(), Type::Float32);
        touch_area.properties.insert("y".to_owned(), Type::Float32);
        touch_area.properties.insert("width".to_owned(), Type::Float32);
        touch_area.properties.insert("height".to_owned(), Type::Float32);
        touch_area.properties.insert("pressed".to_owned(), Type::Bool);
        touch_area.properties.insert("clicked".to_owned(), Type::Signal);
        r.types.insert("TouchArea".to_owned(), Type::Builtin(Rc::new(touch_area)));

        let mut grid_layout = BuiltinElement::new("GridLayout");

        // Row can only be in a GridLayout
        let row = BuiltinElement::new("Row");
        grid_layout
            .additional_accepted_child_types
            .insert("Row".to_owned(), Type::Builtin(Rc::new(row)));

        r.types.insert("GridLayout".to_owned(), Type::Builtin(Rc::new(grid_layout)));

        let mut path = BuiltinElement::new("Path");
        path.properties.insert("x".to_owned(), Type::Float32);
        path.properties.insert("y".to_owned(), Type::Float32);
        path.properties.insert("fill_color".to_owned(), Type::Color);
        path.disallow_global_types_as_child_elements = true;

        let mut line_to = BuiltinElement::new("LineTo");
        line_to.properties.insert("x".to_owned(), Type::Float32);
        line_to.properties.insert("y".to_owned(), Type::Float32);
        line_to.rust_type_constructor =
            Some("sixtyfps::re_exports::PathElement::LineTo(PathLineTo{{}})".into());
        line_to.cpp_type = Some("sixtyfps::PathLineTo".into());
        path.additional_accepted_child_types
            .insert("LineTo".to_owned(), Type::Builtin(Rc::new(line_to)));

        let mut arc_to = BuiltinElement::new("ArcTo");
        arc_to.properties.insert("x".to_owned(), Type::Float32);
        arc_to.properties.insert("y".to_owned(), Type::Float32);
        arc_to.properties.insert("radius_x".to_owned(), Type::Float32);
        arc_to.properties.insert("radius_y".to_owned(), Type::Float32);
        arc_to.properties.insert("x_rotation".to_owned(), Type::Float32);
        arc_to.properties.insert("large_arc".to_owned(), Type::Bool);
        arc_to.properties.insert("sweep".to_owned(), Type::Bool);
        arc_to.rust_type_constructor =
            Some("sixtyfps::re_exports::PathElement::ArcTo(PathArcTo{{}})".into());
        arc_to.cpp_type = Some("sixtyfps::PathArcTo".into());
        path.additional_accepted_child_types
            .insert("ArcTo".to_owned(), Type::Builtin(Rc::new(arc_to)));

        r.types.insert("Path".to_owned(), Type::Builtin(Rc::new(path)));

        let mut property_animation =
            BuiltinElement { class_name: "PropertyAnimation".into(), ..Default::default() };
        property_animation.properties.insert("duration".to_owned(), Type::Int32);
        r.property_animation_type = Type::Builtin(Rc::new(property_animation));
        r.supported_property_animation_types.insert(Type::Float32.to_string());
        r.supported_property_animation_types.insert(Type::Int32.to_string());
        r.supported_property_animation_types.insert(Type::Color.to_string());

        let mut context_restricted_types = HashMap::new();
        r.types.values().for_each(|ty| ty.collect_contextual_types(&mut context_restricted_types));
        r.context_restricted_types = context_restricted_types;

        r
    }

    pub fn lookup(&self, name: &str) -> Type {
        self.types.get(name).cloned().unwrap_or_default()
    }

    pub fn lookup_element(&self, name: &str) -> Result<Type, String> {
        self.types.get(name).cloned().ok_or_else(|| {
            if let Some(permitted_parent_types) = self.context_restricted_types.get(name) {
                if permitted_parent_types.len() == 1 {
                    format!(
                        "{} can only be within a {} element",
                        name,
                        permitted_parent_types.iter().next().unwrap()
                    )
                    .to_owned()
                } else {
                    let mut elements = permitted_parent_types.iter().fold(
                        String::new(),
                        |mut elements, typename| {
                            elements.push_str(typename);
                            elements.push_str(" ,");
                            elements
                        },
                    );
                    elements.pop();
                    elements.pop();

                    format!("{} can only be within the following elements: {}", name, elements)
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
        self.types.insert(comp.id.clone(), Type::Component(comp));
    }

    pub fn property_animation_type_for_property(&self, property_type: Type) -> Type {
        if self.supported_property_animation_types.contains(&property_type.to_string()) {
            self.property_animation_type.clone()
        } else {
            Type::Invalid
        }
    }
}
