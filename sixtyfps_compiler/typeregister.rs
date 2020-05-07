use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug)]
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
    types: HashMap<String, Rc<BuiltinElement>>,
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
        r.types.insert("Rectangle".to_owned(), Rc::new(rectangle));
        let mut image = BuiltinElement::default();
        image.properties.insert("source".to_owned(), Type::Image);
        image.properties.insert("x".to_owned(), Type::Number);
        image.properties.insert("y".to_owned(), Type::Number);
        image.properties.insert("width".to_owned(), Type::Number);
        image.properties.insert("height".to_owned(), Type::Number);
        r.types.insert("Image".to_owned(), Rc::new(image));

        r
    }

    pub fn lookup(&self, name: &str) -> Option<Rc<BuiltinElement>> {
        self.types.get(name).cloned()
    }
}
