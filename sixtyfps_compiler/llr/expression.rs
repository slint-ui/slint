// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use std::collections::HashMap;

use crate::langtype::Type;

use super::PropertyReference;

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
    FunctionCall {
        function: Box<Expression>,
        arguments: Vec<Expression>,
    },

    /// A SelfAssignment or an Assignment.  When op is '=' this is a signal assignment.
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

    PathElements {
        elements: crate::expression_tree::PathEvents,
    },

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
    //-/ Compute the LayoutInfo for the given layout.
    //TODO
    //ComputeLayoutInfo(Layout),
    //SolveLayout(Layout),
}

/*
#[derive(Debug, Default, Clone)]
pub struct LayoutConstraints {
    pub min: Option<PropertyReference>,
    pub max: Option<PropertyReference>,
    pub preferred: Option<PropertyReference>,
    pub stretch: Option<PropertyReference>,
    pub fixed: bool,
}

#[derive(Debug, Clone)]
pub struct LayoutItem {
    pos: Option<PropertyReference>,
    size: Option<PropertyReference>,
}

pub struct LayoutGeometry {}

/// An element in a GridLayout
#[derive(Debug, Clone)]
pub struct GridLayoutElement {
    pub col: u16,
    pub row: u16,
    pub colspan: u16,
    pub rowspan: u16,
    pub item_horiz: LayoutItem,
    pub item_vert: LayoutItem,
}

#[derive(Debug, Clone)]
pub enum Layout {
    Grid {
        /// All the elements will be layout within that element.
        elems: Vec<GridLayoutElement>,

        geometry: LayoutGeometry,

        /// When this GridLyout is actually the layout of a Dialog, then the cells start with all the buttons,
        /// and this variable contains their roles. The string is actually one of the values from the sixtyfps_corelib::layout::DialogButtonRole
        dialog_button_roles: Option<Vec<String>>,
    },
}
*/

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
            Type::PathElements => Expression::PathElements { elements: Default::default() },
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
}
