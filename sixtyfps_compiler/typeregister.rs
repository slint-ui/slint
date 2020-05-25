use std::collections::HashMap;
use std::rc::Rc;

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
    Image,
    Bool,
}

impl Type {
    pub fn is_object_type(&self) -> bool {
        matches!(self, Self::Component(_) | Self::Builtin(_))
    }

    /// valid type for properties
    pub fn is_property_type(&self) -> bool {
        matches!(
            self,
            Self::Float32 | Self::Int32 | Self::String | Self::Color | Self::Image | Self::Bool
        )
    }

    pub fn lookup_property(&self, name: &str) -> Type {
        match self {
            Type::Component(c) => {
                if c.root_element.borrow().signals_declaration.iter().any(|x| x == name) {
                    Type::Signal
                } else {
                    c.root_element.borrow().base_type.lookup_property(name)
                }
            }
            Type::Builtin(b) => b.properties.get(name).cloned().unwrap_or_default(),
            _ => Type::Invalid,
        }
    }
}

impl Default for Type {
    fn default() -> Self {
        Self::Invalid
    }
}

#[derive(Debug, Default)]
pub struct BuiltinElement {
    pub properties: HashMap<String, Type>,
}

#[derive(Debug, Default)]
pub struct TypeRegister {
    /// The set of types.
    types: HashMap<String, Type>,
}

impl TypeRegister {
    pub fn builtin() -> Self {
        let mut r = TypeRegister::default();

        r.types.insert("float32".into(), Type::Float32);
        r.types.insert("int32".into(), Type::Int32);
        r.types.insert("string".into(), Type::String);
        r.types.insert("color".into(), Type::Color);
        r.types.insert("image".into(), Type::Image);
        r.types.insert("bool".into(), Type::Bool);

        let mut rectangle = BuiltinElement::default();
        rectangle.properties.insert("color".to_owned(), Type::Color);
        rectangle.properties.insert("x".to_owned(), Type::Float32);
        rectangle.properties.insert("y".to_owned(), Type::Float32);
        rectangle.properties.insert("width".to_owned(), Type::Float32);
        rectangle.properties.insert("height".to_owned(), Type::Float32);
        r.types.insert("Rectangle".to_owned(), Type::Builtin(Rc::new(rectangle)));

        let mut image = BuiltinElement::default();
        image.properties.insert("source".to_owned(), Type::Image);
        image.properties.insert("x".to_owned(), Type::Float32);
        image.properties.insert("y".to_owned(), Type::Float32);
        image.properties.insert("width".to_owned(), Type::Float32);
        image.properties.insert("height".to_owned(), Type::Float32);
        r.types.insert("Image".to_owned(), Type::Builtin(Rc::new(image)));

        let mut text = BuiltinElement::default();
        text.properties.insert("text".to_owned(), Type::String);
        text.properties.insert("font_family".to_owned(), Type::String);
        text.properties.insert("font_pixel_size".to_owned(), Type::Float32);
        text.properties.insert("color".to_owned(), Type::Color);
        text.properties.insert("x".to_owned(), Type::Float32);
        text.properties.insert("y".to_owned(), Type::Float32);
        r.types.insert("Text".to_owned(), Type::Builtin(Rc::new(text)));

        let mut touch_area = BuiltinElement::default();
        touch_area.properties.insert("x".to_owned(), Type::Float32);
        touch_area.properties.insert("y".to_owned(), Type::Float32);
        touch_area.properties.insert("width".to_owned(), Type::Float32);
        touch_area.properties.insert("height".to_owned(), Type::Float32);
        touch_area.properties.insert("pressed".to_owned(), Type::Bool);
        touch_area.properties.insert("clicked".to_owned(), Type::Signal);
        r.types.insert("TouchArea".to_owned(), Type::Builtin(Rc::new(touch_area)));

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
