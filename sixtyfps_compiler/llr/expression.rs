// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use super::PropertyReference;
use crate::expression_tree::{BuiltinFunction, OperatorClass};
use crate::langtype::Type;
use itertools::Either;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Expression {
    /// A string literal. The .0 is the content of the string, without the quotes
    StringLiteral(String),
    /// Number
    NumberLiteral(f64),
    /// Bool
    BoolLiteral(bool),

    /// Reference to the callback <name> in the <element>
    PropertyReference(PropertyReference),

    // TODO
    // CallbackReference(P::PropertyReference),
    /// Reference to a function built into the run-time, implemented natively
    //BuiltinFunctionReference(BuiltinFunction, Option<SourceLocation>),

    /// A reference to a specific element. This isn't possible to create in .60 syntax itself, but intermediate passes may generate this
    /// type of expression.
    //ElementReference(P::ItemReference),

    /// Reference the parameter at the given index of the current function.
    FunctionParameterReference {
        index: usize,
        //ty: Type,
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

    /// Access to a field of the given name within a struct.
    StructFieldAccess {
        /// This expression should have [`Type::Struct`] type
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
    BuiltinFunctionCall {
        function: BuiltinFunction,
        arguments: Vec<Expression>,
    },
    CallBackCall {
        callback: PropertyReference,
        arguments: Vec<Expression>,
    },

    /// A BuiltinFunctionCall, but the function is not yet in the `BuiltinFunction` enum
    /// TODO: merge in BuiltinFunctionCall
    ExtraBuiltinFunctionCall {
        function: String,
        arguments: Vec<Expression>,
    },

    /// An assignment of a value to a property
    PropertyAssignment {
        property: PropertyReference,
        value: Box<Expression>,
    },
    /// an assignment of a value to the model data
    ModelDataAssignment {
        // how deep in the parent hierarchy we go
        level: usize,
        value: Box<Expression>,
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

    ImageReference {
        resource_ref: crate::expression_tree::ImageReference,
    },

    Condition {
        condition: Box<Expression>,
        true_expr: Box<Expression>,
        false_expr: Option<Box<Expression>>,
    },

    Array {
        element_ty: Type,
        values: Vec<Expression>,
    },
    Struct {
        ty: Type,
        values: HashMap<String, Expression>,
    },

    PathEvents(crate::expression_tree::PathEvents),

    EasingCurve(crate::expression_tree::EasingCurve),

    LinearGradient {
        angle: Box<Expression>,
        /// First expression in the tuple is a color, second expression is the stop position
        stops: Vec<(Expression, Expression)>,
    },

    EnumerationValue(crate::langtype::EnumerationValue),

    ReturnStatement(Option<Box<Expression>>),

    LayoutCacheAccess {
        layout_cache_prop: PropertyReference,
        index: usize,
        /// When set, this is the index within a repeater, and the index is then the location of another offset.
        /// So this looks like `layout_cache_prop[layout_cache_prop[index] + repeater_index]`
        repeater_index: Option<Box<Expression>>,
    },
    /// Generate the array of BoxLayoutCellData form elements
    BoxLayoutCellDataArray {
        /// Either an expression of type BoxLayoutCellData, or an index to the repeater
        elements: Vec<Either<Expression, usize>>,
        /// The name for the local variable that stores the repeater indices
        /// In other word, this expression has side effect and change that
        repeater_indices: Option<String>,
    },
}

impl Expression {
    pub fn default_value_for_type(ty: &Type) -> Option<Self> {
        Some(match ty {
            Type::Invalid
            | Type::Component(_)
            | Type::Builtin(_)
            | Type::Native(_)
            | Type::Callback { .. }
            | Type::Function { .. }
            | Type::Void
            | Type::InferredProperty
            | Type::InferredCallback
            | Type::ElementReference
            | Type::LayoutCache => return None,
            Type::Float32
            | Type::Duration
            | Type::Int32
            | Type::Angle
            | Type::PhysicalLength
            | Type::LogicalLength
            | Type::UnitProduct(_) => Expression::NumberLiteral(0.),
            Type::Percent => Expression::NumberLiteral(1.),
            Type::String => Expression::StringLiteral(String::new()),
            Type::Color => {
                Expression::Cast { from: Box::new(Expression::NumberLiteral(0.)), to: ty.clone() }
            }
            Type::Image => Expression::ImageReference {
                resource_ref: crate::expression_tree::ImageReference::None,
            },
            Type::Bool => Expression::BoolLiteral(false),
            Type::Model => return None,
            Type::PathElements => Expression::PathEvents(Default::default()),
            Type::Array(element_ty) => {
                Expression::Array { element_ty: (**element_ty).clone(), values: vec![] }
            }
            Type::Struct { fields, .. } => Expression::Struct {
                ty: ty.clone(),
                values: fields
                    .iter()
                    .map(|(k, v)| Some((k.clone(), Expression::default_value_for_type(v)?)))
                    .collect::<Option<_>>()?,
            },
            Type::Easing => Expression::EasingCurve(crate::expression_tree::EasingCurve::default()),
            Type::Brush => Expression::Cast {
                from: Box::new(Expression::default_value_for_type(&Type::Color)?),
                to: Type::Brush,
            },
            Type::Enumeration(enumeration) => {
                Expression::EnumerationValue(enumeration.clone().default_value())
            }
        })
    }

    pub fn ty(&self) -> Type {
        match self {
            Self::StringLiteral(_) => Type::String,
            Self::NumberLiteral(_) => Type::Float32,
            Self::BoolLiteral(_) => Type::Bool,
            Self::PropertyReference(_) => todo!(),
            Self::FunctionParameterReference { .. } => todo!(),
            Self::StoreLocalVariable { .. } => Type::Void,
            Self::ReadLocalVariable { ty, .. } => ty.clone(),
            Self::StructFieldAccess { base, name } => match base.ty() {
                Type::Struct { fields, .. } => fields[name].clone(),
                _ => unreachable!(),
            },
            Self::Cast { to, .. } => to.clone(),
            Self::CodeBlock(sub) => sub.last().map_or(Type::Void, |e| e.ty()),
            Self::BuiltinFunctionCall { function, .. } => match function.ty() {
                Type::Function { return_type, .. } => *return_type,
                _ => unreachable!(),
            },
            Self::CallBackCall { .. } => todo!(),
            Self::ExtraBuiltinFunctionCall { .. } => todo!(),
            Self::PropertyAssignment { .. } => Type::Void,
            Self::ModelDataAssignment { .. } => Type::Void,
            Self::BinaryExpression { lhs, rhs: _, op } => {
                if crate::expression_tree::operator_class(*op) != OperatorClass::ArithmeticOp {
                    Type::Bool
                } else {
                    lhs.ty()
                }
            }
            Self::UnaryOp { sub, .. } => sub.ty(),
            Self::ImageReference { .. } => Type::Image,
            Self::Condition { true_expr, .. } => true_expr.ty(),
            Self::Array { element_ty, .. } => Type::Array(element_ty.clone().into()),
            Self::Struct { ty, .. } => ty.clone(),
            Self::PathEvents { .. } => todo!(),
            Self::EasingCurve(_) => Type::Easing,
            Self::LinearGradient { .. } => Type::Brush,
            Self::EnumerationValue(e) => Type::Enumeration(e.enumeration.clone()),
            Self::ReturnStatement(_) => Type::Invalid,
            Self::LayoutCacheAccess { .. } => crate::layout::layout_info_type(),
            Self::BoxLayoutCellDataArray { .. } => {
                Type::Array(Box::new(crate::layout::layout_info_type()))
            }
        }
    }
}
