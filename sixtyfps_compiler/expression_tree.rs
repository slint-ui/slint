use crate::object_tree::*;
use crate::{parser::SyntaxNode, typeregister::Type};
use core::cell::RefCell;
use std::rc::Weak;

/// The Expression is hold by properties, so it should not hold any strong references to node from the object_tree
#[derive(Debug, Clone)]
pub enum Expression {
    /// Something went wrong (and an error will be reported)
    Invalid,
    /// We haven't done the lookup yet
    Uncompiled(SyntaxNode),
    /// A string literal. The .0 is the content of the string, without the quotes
    StringLiteral(String),
    /// Number
    NumberLiteral(f64),

    /// Reference to the signal <name> in the <element> within the <Component>
    ///
    /// Note: if we are to separate expression and statement, we probably do not need to have signal reference within expressions
    SignalReference { component: Weak<Component>, element: Weak<RefCell<Element>>, name: String },

    /// Reference to the signal <name> in the <element> within the <Component>
    ///
    /// Note: if we are to separate expression and statement, we probably do not need to have signal reference within expressions
    PropertyReference { component: Weak<Component>, element: Weak<RefCell<Element>>, name: String },

    /// Cast an expression to the given type
    Cast { from: Box<Expression>, to: Type },
}

impl Expression {
    /// Return the type of this property
    pub fn ty(&self) -> Type {
        match self {
            Expression::Invalid => Type::Invalid,
            Expression::Uncompiled(_) => Type::Invalid,
            Expression::StringLiteral(_) => Type::String,
            Expression::NumberLiteral(_) => Type::Float32,
            Expression::SignalReference { .. } => Type::Signal,
            Expression::PropertyReference { element, name, .. } => {
                element.upgrade().unwrap().borrow().lookup_property(name)
            }
            Expression::Cast { to, .. } => to.clone(),
        }
    }
}
