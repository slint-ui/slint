use crate::object_tree::*;
use crate::parser::{Spanned, SyntaxNode};
use crate::{diagnostics::Diagnostics, typeregister::Type};
use core::cell::RefCell;
use std::{collections::HashMap, rc::Weak};

/// Reference to a property or signal of a given name within an element.
#[derive(Debug, Clone)]
pub struct NamedReference {
    pub element: Weak<RefCell<Element>>,
    pub name: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum OperatorClass {
    ComparisonOp,
    LogicalOp,
    ArithmeticOp,
}

/// the class of for this (binary) operation
pub fn operator_class(op: char) -> OperatorClass {
    match op {
        '=' | '!' | '<' | '>' | '≤' | '≥' => OperatorClass::ComparisonOp,
        '&' | '|' => OperatorClass::LogicalOp,
        '+' | '-' | '/' | '*' => OperatorClass::ArithmeticOp,
        _ => panic!("Invalid operator {:?}", op),
    }
}

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

    /// Reference to the signal <name> in the <element>
    ///
    /// Note: if we are to separate expression and statement, we probably do not need to have signal reference within expressions
    SignalReference(NamedReference),

    /// Reference to the signal <name> in the <element>
    PropertyReference(NamedReference),

    /// Reference to the index variable of a repeater
    ///
    /// Example: `idx`  in `for xxx[idx] in ...`.   The element is the reference to the
    /// element that is repeated
    RepeaterIndexReference {
        element: Weak<RefCell<Element>>,
    },

    /// Reference to the model variable of a repeater
    ///
    /// Example: `xxx`  in `for xxx[idx] in ...`.   The element is the reference to the
    /// element that is repeated
    RepeaterModelReference {
        element: Weak<RefCell<Element>>,
    },

    /// Access to a field of the given name within a object.
    ObjectAccess {
        /// This expression should have Type::Object type
        base: Box<Expression>,
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

    SelfAssignment {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
        /// '+', '-', '/', or '*'
        op: char,
    },

    BinaryExpression {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
        /// '+', '-', '/', '*', '=', '!', '<', '>', '≤', '≥', '&', '|'
        op: char,
    },

    UnaryOp {
        sub: Box<Expression>,
        /// '+', '-', '!'
        op: char,
    },

    ResourceReference {
        absolute_source_path: String,
    },

    Condition {
        condition: Box<Expression>,
        true_expr: Box<Expression>,
        false_expr: Box<Expression>,
    },

    Array {
        element_ty: Type,
        values: Vec<Expression>,
    },
    Object {
        ty: Type,
        values: HashMap<String, Expression>,
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
            Expression::PropertyReference(NamedReference { element, name }) => {
                element.upgrade().unwrap().borrow().lookup_property(name)
            }
            Expression::RepeaterIndexReference { .. } => Type::Int32,
            Expression::RepeaterModelReference { element } => {
                if let Expression::Cast { from, .. } = element
                    .upgrade()
                    .unwrap()
                    .borrow()
                    .repeated
                    .as_ref()
                    .map_or(&Expression::Invalid, |e| &e.model)
                {
                    match from.ty() {
                        Type::Float32 | Type::Int32 => Type::Int32,
                        Type::Array(elem) => *elem,
                        _ => Type::Invalid,
                    }
                } else {
                    Type::Invalid
                }
            }
            Expression::ObjectAccess { base, name } => {
                if let Type::Object(o) = base.ty() {
                    o.get(name.as_str()).unwrap_or(&Type::Invalid).clone()
                } else {
                    Type::Invalid
                }
            }
            Expression::Cast { to, .. } => to.clone(),
            Expression::CodeBlock(sub) => sub.last().map_or(Type::Invalid, |e| e.ty()),
            Expression::FunctionCall { function } => function.ty(),
            Expression::SelfAssignment { .. } => Type::Invalid,
            Expression::ResourceReference { .. } => Type::Resource,
            Expression::Condition { condition: _, true_expr, false_expr } => {
                let true_type = true_expr.ty();
                let false_type = false_expr.ty();
                if true_type == false_type {
                    true_type
                } else {
                    Type::Invalid
                }
            }
            Expression::BinaryExpression { op, .. } => {
                if operator_class(*op) == OperatorClass::ArithmeticOp {
                    Type::Float32
                } else {
                    Type::Bool
                }
            }
            Expression::UnaryOp { sub, .. } => sub.ty(),
            Expression::Array { element_ty, .. } => Type::Array(Box::new(element_ty.clone())),
            Expression::Object { ty, .. } => ty.clone(),
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
            Expression::ObjectAccess { base, .. } => visitor(&**base),
            Expression::RepeaterIndexReference { .. } => {}
            Expression::RepeaterModelReference { .. } => {}
            Expression::Cast { from, .. } => visitor(&**from),
            Expression::CodeBlock(sub) => {
                for e in sub {
                    visitor(e)
                }
            }
            Expression::FunctionCall { function } => visitor(&**function),
            Expression::SelfAssignment { lhs, rhs, .. } => {
                visitor(&**lhs);
                visitor(&**rhs);
            }
            Expression::ResourceReference { .. } => {}
            Expression::Condition { condition, true_expr, false_expr } => {
                visitor(&**condition);
                visitor(&**true_expr);
                visitor(&**false_expr);
            }
            Expression::BinaryExpression { lhs, rhs, .. } => {
                visitor(&**lhs);
                visitor(&**rhs);
            }
            Expression::UnaryOp { sub, .. } => visitor(&**sub),
            Expression::Array { values, .. } => {
                for x in values {
                    visitor(x);
                }
            }
            Expression::Object { values, .. } => {
                for (_, x) in values {
                    visitor(x);
                }
            }
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
            Expression::ObjectAccess { base, .. } => visitor(&mut **base),
            Expression::RepeaterIndexReference { .. } => {}
            Expression::RepeaterModelReference { .. } => {}
            Expression::Cast { from, .. } => visitor(&mut **from),
            Expression::CodeBlock(sub) => {
                for e in sub {
                    visitor(e)
                }
            }
            Expression::FunctionCall { function } => visitor(&mut **function),
            Expression::SelfAssignment { lhs, rhs, .. } => {
                visitor(&mut **lhs);
                visitor(&mut **rhs);
            }
            Expression::ResourceReference { .. } => {}
            Expression::Condition { condition, true_expr, false_expr } => {
                visitor(&mut **condition);
                visitor(&mut **true_expr);
                visitor(&mut **false_expr);
            }
            Expression::BinaryExpression { lhs, rhs, .. } => {
                visitor(&mut **lhs);
                visitor(&mut **rhs);
            }
            Expression::UnaryOp { sub, .. } => visitor(&mut **sub),
            Expression::Array { values, .. } => {
                for x in values {
                    visitor(x);
                }
            }
            Expression::Object { values, .. } => {
                for (_, x) in values {
                    visitor(x);
                }
            }
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
            Expression::RepeaterIndexReference { .. } => false,
            Expression::RepeaterModelReference { .. } => false,
            Expression::ObjectAccess { base, .. } => base.is_constant(),
            Expression::Cast { from, .. } => from.is_constant(),
            Expression::CodeBlock(sub) => sub.len() == 1 && sub.first().unwrap().is_constant(),
            Expression::FunctionCall { .. } => false,
            Expression::SelfAssignment { .. } => false,
            Expression::ResourceReference { .. } => true,
            Expression::Condition { .. } => false,
            Expression::BinaryExpression { lhs, rhs, .. } => lhs.is_constant() && rhs.is_constant(),
            Expression::UnaryOp { sub, .. } => sub.is_constant(),
            Expression::Array { values, .. } => values.iter().all(Expression::is_constant),
            Expression::Object { values, .. } => values.iter().all(|(_, v)| v.is_constant()),
        }
    }

    /// Create a conversion node if needed, or throw an error if the type is not matching
    pub fn maybe_convert_to(
        self,
        target_type: Type,
        node: &SyntaxNode,
        diag: &mut Diagnostics,
    ) -> Expression {
        let ty = self.ty();
        if ty == target_type {
            self
        } else if ty.can_convert(&target_type) {
            Expression::Cast { from: Box::new(self), to: target_type }
        } else if ty == Type::Invalid {
            self
        } else {
            diag.push_error(format!("Cannot convert {} to {}", ty, target_type), node.span());
            self
        }
    }
}
