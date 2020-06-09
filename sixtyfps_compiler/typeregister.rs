use std::collections::HashMap;
use std::{fmt::Display, rc::Rc};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Type {
    Invalid,
    Component(Rc<crate::object_tree::Component>),
    Builtin(Rc<BuiltinElement>),

    Signal,

    // other property type:
    Float32,
    Int32,
    String,
    Color,
    Resource,
    Bool,
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
            Self::Float32 | Self::Int32 | Self::String | Self::Color | Self::Resource | Self::Bool
        )
    }

    pub fn lookup_property(&self, name: &str) -> Type {
        match self {
            Type::Component(c) => c.root_element.borrow().lookup_property(name),
            Type::Builtin(b) => b.properties.get(name).cloned().unwrap_or_default(),
            _ => Type::Invalid,
        }
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
                    // FIXME: REMOVE
                    | (Type::Float32, Type::Color)
                    | (Type::Int32, Type::Color)
            )
    }
}

impl Default for Type {
    fn default() -> Self {
        Self::Invalid
    }
}

#[derive(Debug, Default)]
pub struct BuiltinElement {
    pub class_name: String,
    pub vtable_symbol: String,
    pub properties: HashMap<String, Type>,
}

impl BuiltinElement {
    pub fn new(class_name: &str) -> Self {
        let vtable_symbol = format!("{}VTable", class_name);
        Self { class_name: class_name.into(), vtable_symbol, properties: Default::default() }
    }
}

#[derive(Debug, Default)]
pub struct TypeRegister {
    /// The set of types.
    types: HashMap<String, Type>,
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

        let grid_layout = BuiltinElement::new("GridLayout");
        r.types.insert("GridLayout".to_owned(), Type::Builtin(Rc::new(grid_layout)));

        // Row can only be in a GridLayout
        let row = BuiltinElement::new("Row");
        r.types.insert("Row".to_owned(), Type::Builtin(Rc::new(row)));

        r
    }

    pub fn lookup(&self, name: &str) -> Type {
        self.types.get(name).cloned().unwrap_or_default()
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
}
