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
    SignalReference {
        component: Weak<Component>,
        element: Weak<RefCell<Element>>,
        name: String,
    },

    /// Reference to the signal <name> in the <element> within the <Component>
    ///
    /// Note: if we are to separate expression and statement, we probably do not need to have signal reference within expressions
    PropertyReference {
        component: Weak<Component>,
        element: Weak<RefCell<Element>>,
        name: String,
    },

    /// Cast an expression to the given type
    Cast {
        from: Box<Expression>,
        to: Type,
    },

    /// a code block with different expression
    CodeBlock(Vec<Expression>),

    /// A function call
    FunctionCall {
        function: Box<Expression>,
    },

    SelfAssignement {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
        /// '+', '-', '/', or '*'
        op: char,
    },

    ResourceReference {
        absolute_source_path: String,
    },
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
            Expression::CodeBlock(sub) => sub.last().map_or(Type::Invalid, |e| e.ty()),
            Expression::FunctionCall { function } => function.ty(),
            Expression::SelfAssignement { .. } => Type::Invalid,
            Expression::ResourceReference { .. } => Type::Resource,
        }
    }

    /// Call the visitor for each sub-expression.  (note: this function does not recurse)
    pub fn visit(&self, mut visitor: impl FnMut(&Self)) {
        match self {
            Expression::Invalid => {}
            Expression::Uncompiled(_) => {}
            Expression::StringLiteral(_) => {}
            Expression::NumberLiteral(_) => {}
            Expression::SignalReference { .. } => {}
            Expression::PropertyReference { .. } => {}
            Expression::Cast { from, .. } => visitor(&**from),
            Expression::CodeBlock(sub) => {
                for e in sub {
                    visitor(e)
                }
            }
            Expression::FunctionCall { function } => visitor(&**function),
            Expression::SelfAssignement { lhs, rhs, .. } => {
                visitor(&**lhs);
                visitor(&**rhs);
            }
            Expression::ResourceReference { .. } => {}
        }
    }

    pub fn visit_mut(&mut self, mut visitor: impl FnMut(&mut Self)) {
        match self {
            Expression::Invalid => {}
            Expression::Uncompiled(_) => {}
            Expression::StringLiteral(_) => {}
            Expression::NumberLiteral(_) => {}
            Expression::SignalReference { .. } => {}
            Expression::PropertyReference { .. } => {}
            Expression::Cast { from, .. } => visitor(&mut **from),
            Expression::CodeBlock(sub) => {
                for e in sub {
                    visitor(e)
                }
            }
            Expression::FunctionCall { function } => visitor(&mut **function),
            Expression::SelfAssignement { lhs, rhs, .. } => {
                visitor(&mut **lhs);
                visitor(&mut **rhs);
            }
            Expression::ResourceReference { .. } => {}
        }
    }

    pub fn is_constant(&self) -> bool {
        match self {
            Expression::Invalid => true,
            Expression::Uncompiled(_) => false,
            Expression::StringLiteral(_) => true,
            Expression::NumberLiteral(_) => true,
            Expression::SignalReference { .. } => false,
            Expression::PropertyReference { .. } => false,
            Expression::Cast { from, .. } => from.is_constant(),
            Expression::CodeBlock(sub) => sub.len() == 1 && sub.first().unwrap().is_constant(),
            Expression::FunctionCall { .. } => false,
            Expression::SelfAssignement { .. } => false,
            Expression::ResourceReference { .. } => true,
        }
    }
}
