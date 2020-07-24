use crate::object_tree::*;
use crate::parser::SyntaxNodeWithSourceFile;
use crate::typeregister::BuiltinElement;
use crate::{
    diagnostics::{BuildDiagnostics, SpannedWithSourceFile},
    typeregister::Type,
};
use core::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::{Rc, Weak};

/// Reference to a property or signal of a given name within an element.
#[derive(Debug, Clone)]
pub struct NamedReference {
    pub element: Weak<RefCell<Element>>,
    pub name: String,
}

impl Eq for NamedReference {}

impl PartialEq for NamedReference {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && Weak::ptr_eq(&self.element, &other.element)
    }
}

impl Hash for NamedReference {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.element.as_ptr().hash(state);
    }
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

macro_rules! declare_units {
    ($( $(#[$m:meta])* $ident:ident = $string:literal -> $ty:ident $(* $factor:expr)? ,)*) => {
        /// The units that can be used after numbers in the language
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Unit {
            $($(#[$m])* $ident,)*
        }

        impl std::fmt::Display for Unit {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$ident => write!(f, $string), )*
                }
            }
        }

        impl std::str::FromStr for Unit {
            type Err = ();
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($string => Ok(Self::$ident), )*
                    _ => Err(())
                }
            }
        }

        impl Unit {
            pub fn ty(self) -> Type {
                match self {
                    $(Self::$ident => Type::$ty, )*
                }
            }

            pub fn normalize(self, x: f64) -> f64 {
                match self {
                    $(Self::$ident => x $(* $factor as f64)?, )*
                }
            }

        }
    };
}

declare_units! {
    /// No unit was given
    None = "" -> Float32,
    ///
    Percent = "%" -> Float32 * 0.01,

    // Lenghts or Coord

    /// Physical pixels
    Px = "px" -> Length,
    /// Logical pixels
    Lx = "lx" -> LogicalLength,
    /// Centimeters
    Cm = "cm" -> LogicalLength * 37.8,
    /// Milimeters
    Mm = "mm" -> LogicalLength * 3.78,
    /// inches
    In = "in" -> LogicalLength * 96,
    /// Points
    Pt = "pt" -> LogicalLength * 96/72,

    // durations

    /// Seconds
    S = "s" -> Duration * 1000,
    /// Milliseconds
    Ms = "ms" -> Duration,
}

impl Default for Unit {
    fn default() -> Self {
        Self::None
    }
}

/// The Expression is hold by properties, so it should not hold any strong references to node from the object_tree
#[derive(Debug, Clone)]
pub enum Expression {
    /// Something went wrong (and an error will be reported)
    Invalid,
    /// We haven't done the lookup yet
    Uncompiled(SyntaxNodeWithSourceFile),
    /// A string literal. The .0 is the content of the string, without the quotes
    StringLiteral(String),
    /// Number
    NumberLiteral(f64, Unit),
    ///
    BoolLiteral(bool),

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

    /// Should be directly within a CodeBlock expression, and store the value of the expression in a local variable
    StoreLocalVariable {
        name: String,
        value: Box<Expression>,
    },

    /// a reference to the local variable with the given name. The type system should ensure that a variable has been stored
    /// with this name and this type before in one of the statement of an enclosing codeblock
    ReadLocalVariable {
        name: String,
        ty: Type,
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

    /// A SelfAssignment or an Assignment.  When op is '=' this is a signel assignment.
    SelfAssignment {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
        /// '+', '-', '/', '*', or '='
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

    PathElements {
        elements: Path,
    },
}

impl Default for Expression {
    fn default() -> Self {
        Expression::Invalid
    }
}

impl Expression {
    /// Return the type of this property
    pub fn ty(&self) -> Type {
        match self {
            Expression::Invalid => Type::Invalid,
            Expression::Uncompiled(_) => Type::Invalid,
            Expression::StringLiteral(_) => Type::String,
            Expression::NumberLiteral(_, unit) => unit.ty(),
            Expression::BoolLiteral(_) => Type::Bool,
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
            Expression::CodeBlock(sub) => sub.last().map_or(Type::Void, |e| e.ty()),
            Expression::FunctionCall { function } => function.ty(),
            Expression::SelfAssignment { .. } => Type::Void,
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
            Expression::BinaryExpression { op, lhs, rhs } => {
                if operator_class(*op) == OperatorClass::ArithmeticOp {
                    macro_rules! unit_operations {
                        ($($unit:ident)*) => {
                            match (*op, lhs.ty(), rhs.ty()) {
                                $(
                                    ('+', Type::$unit, Type::$unit) => Type::$unit,
                                    ('-', Type::$unit, Type::$unit) => Type::$unit,
                                    ('*', Type::$unit, _) => Type::$unit,
                                    ('*', _, Type::$unit) => Type::$unit,
                                    ('/', Type::$unit, Type::$unit) => Type::Float32,
                                    ('/', Type::$unit, _) => Type::$unit,
                                )*
                                _ => Type::Float32,
                            }
                        }
                    }
                    unit_operations!(Duration Length LogicalLength)
                } else {
                    Type::Bool
                }
            }
            Expression::UnaryOp { sub, .. } => sub.ty(),
            Expression::Array { element_ty, .. } => Type::Array(Box::new(element_ty.clone())),
            Expression::Object { ty, .. } => ty.clone(),
            Expression::PathElements { .. } => Type::PathElements,
            Expression::StoreLocalVariable { .. } => Type::Void,
            Expression::ReadLocalVariable { ty, .. } => ty.clone(),
        }
    }

    /// Call the visitor for each sub-expression.  (note: this function does not recurse)
    pub fn visit(&self, mut visitor: impl FnMut(&Self)) {
        match self {
            Expression::Invalid => {}
            Expression::Uncompiled(_) => {}
            Expression::StringLiteral(_) => {}
            Expression::NumberLiteral(_, _) => {}
            Expression::BoolLiteral(_) => {}
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
            Expression::PathElements { elements } => {
                if let Path::Elements(elements) = elements {
                    for element in elements {
                        element.bindings.values().for_each(|binding| visitor(binding))
                    }
                }
            }
            Expression::StoreLocalVariable { value, .. } => visitor(&**value),
            Expression::ReadLocalVariable { .. } => {}
        }
    }

    pub fn visit_mut(&mut self, mut visitor: impl FnMut(&mut Self)) {
        match self {
            Expression::Invalid => {}
            Expression::Uncompiled(_) => {}
            Expression::StringLiteral(_) => {}
            Expression::NumberLiteral(_, _) => {}
            Expression::BoolLiteral(_) => {}
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
            Expression::PathElements { elements } => {
                if let Path::Elements(elements) = elements {
                    for element in elements {
                        element.bindings.values_mut().for_each(|binding| visitor(binding))
                    }
                }
            }
            Expression::StoreLocalVariable { value, .. } => visitor(&mut **value),
            Expression::ReadLocalVariable { .. } => {}
        }
    }

    pub fn is_constant(&self) -> bool {
        match self {
            Expression::Invalid => true,
            Expression::Uncompiled(_) => false,
            Expression::StringLiteral(_) => true,
            Expression::NumberLiteral(_, _) => true,
            Expression::BoolLiteral(_) => true,
            Expression::SignalReference { .. } => false,
            Expression::PropertyReference { .. } => false,
            Expression::RepeaterIndexReference { .. } => false,
            Expression::RepeaterModelReference { .. } => false,
            Expression::ObjectAccess { base, .. } => base.is_constant(),
            Expression::Cast { from, to } => {
                from.is_constant() && !matches!(to, Type::Length | Type::LogicalLength)
            }
            Expression::CodeBlock(sub) => sub.len() == 1 && sub.first().unwrap().is_constant(),
            Expression::FunctionCall { .. } => false,
            Expression::SelfAssignment { .. } => false,
            Expression::ResourceReference { .. } => true,
            Expression::Condition { .. } => false,
            Expression::BinaryExpression { lhs, rhs, .. } => lhs.is_constant() && rhs.is_constant(),
            Expression::UnaryOp { sub, .. } => sub.is_constant(),
            Expression::Array { values, .. } => values.iter().all(Expression::is_constant),
            Expression::Object { values, .. } => values.iter().all(|(_, v)| v.is_constant()),
            Expression::PathElements { elements } => {
                if let Path::Elements(elements) = elements {
                    elements
                        .iter()
                        .all(|element| element.bindings.values().all(|v| v.is_constant()))
                } else {
                    true
                }
            }
            Expression::StoreLocalVariable { .. } => false,
            Expression::ReadLocalVariable { .. } => false,
        }
    }

    /// Create a conversion node if needed, or throw an error if the type is not matching
    pub fn maybe_convert_to(
        self,
        target_type: Type,
        node: &impl SpannedWithSourceFile,
        diag: &mut BuildDiagnostics,
    ) -> Expression {
        let ty = self.ty();
        if ty == target_type {
            self
        } else if ty.can_convert(&target_type) {
            Expression::Cast { from: Box::new(self), to: target_type }
        } else if ty == Type::Invalid || target_type == Type::Invalid {
            self
        } else {
            diag.push_error(format!("Cannot convert {} to {}", ty, target_type), node);
            self
        }
    }

    /// Return the default value for the given type
    pub fn default_value_for_type(ty: &Type) -> Expression {
        match ty {
            Type::Invalid
            | Type::Component(_)
            | Type::Builtin(_)
            | Type::Native(_)
            | Type::Signal
            | Type::Void => Expression::Invalid,
            Type::Float32 => Expression::NumberLiteral(0., Unit::None),
            Type::Int32 => Expression::NumberLiteral(0., Unit::None),
            Type::String => Expression::StringLiteral(String::new()),
            Type::Color => Expression::Cast {
                from: Box::new(Expression::NumberLiteral(0., Unit::None)),
                to: Type::Color,
            },
            Type::Duration => Expression::NumberLiteral(0., Unit::Ms),
            Type::Length => Expression::NumberLiteral(0., Unit::Px),
            Type::LogicalLength => Expression::NumberLiteral(0., Unit::Lx),
            // FIXME: Is that correct?
            Type::Resource => Expression::ResourceReference { absolute_source_path: String::new() },
            Type::Bool => Expression::BoolLiteral(false),
            Type::Model => Expression::Invalid,
            Type::PathElements => Expression::PathElements { elements: Path::Elements(vec![]) },
            Type::Array(element_ty) => {
                Expression::Array { element_ty: (**element_ty).clone(), values: vec![] }
            }
            Type::Object(map) => Expression::Object {
                ty: ty.clone(),
                values: map
                    .into_iter()
                    .map(|(k, v)| (k.clone(), Expression::default_value_for_type(v)))
                    .collect(),
            },
        }
    }
}

pub type PathEvents = Vec<lyon::path::Event<lyon::math::Point, lyon::math::Point>>;

#[derive(Debug, Clone)]
pub enum Path {
    Elements(Vec<PathElement>),
    Events(PathEvents),
}

#[derive(Debug, Clone)]
pub struct PathElement {
    pub element_type: Rc<BuiltinElement>,
    pub bindings: HashMap<String, Expression>,
}
