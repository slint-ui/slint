use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Type {
    Invalid,
    Component(Rc<crate::object_tree::Component>),
    Builtin(Rc<BuiltinElement>),

    // other property type:
    Number,
    String,
    Color,
    Image,
}

impl Type {
    pub fn is_object_type(&self) -> bool {
        matches!(self, Self::Component(_) | Self::Builtin(_))
    }

    pub fn lookup_property(&self, name: &str) -> Type {
        match self {
            Type::Component(c) => c.root_element.base_type.lookup_property(name),
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
    /// FIXME: could also be a component
    types: HashMap<String, Type>,
}

impl TypeRegister {
    pub fn builtin() -> Self {
        let mut r = TypeRegister::default();
        let mut rectangle = BuiltinElement::default();
        rectangle.properties.insert("color".to_owned(), Type::Color);
        rectangle.properties.insert("x".to_owned(), Type::Number);
        rectangle.properties.insert("y".to_owned(), Type::Number);
        rectangle.properties.insert("width".to_owned(), Type::Number);
        rectangle.properties.insert("height".to_owned(), Type::Number);
        r.types.insert("Rectangle".to_owned(), Type::Builtin(Rc::new(rectangle)));
        let mut image = BuiltinElement::default();
        image.properties.insert("source".to_owned(), Type::Image);
        image.properties.insert("x".to_owned(), Type::Number);
        image.properties.insert("y".to_owned(), Type::Number);
        image.properties.insert("width".to_owned(), Type::Number);
        image.properties.insert("height".to_owned(), Type::Number);
        r.types.insert("Image".to_owned(), Type::Builtin(Rc::new(image)));
        let mut text = BuiltinElement::default();
        text.properties.insert("text".to_owned(), Type::String);
        text.properties.insert("color".to_owned(), Type::Color);
        text.properties.insert("x".to_owned(), Type::Number);
        text.properties.insert("y".to_owned(), Type::Number);
        r.types.insert("Text".to_owned(), Type::Builtin(Rc::new(text)));

        r
    }

    pub fn lookup(&self, name: &str) -> Type {
        self.types.get(name).cloned().unwrap_or_default()
    }

    pub fn add(&mut self, comp: Rc<crate::object_tree::Component>) {
        self.types.insert(comp.id.clone(), Type::Component(comp));
    }
}
