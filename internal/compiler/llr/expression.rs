// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{
    GlobalIdx, GridLayoutRepeatedElement, LayoutRepeatedElement, LocalMemberIndex,
    LocalMemberReference, MemberReference, RepeatedElementIdx, SubComponentIdx,
    SubComponentInstanceIdx,
};
use crate::expression_tree::{BuiltinFunction, MinMaxOp, OperatorClass};
use crate::langtype::{Keys, Type};
use crate::layout::Orientation;
use itertools::Either;
use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum ArrayOutput {
    Slice,
    Model,
    Vector,
}

pub use crate::expression_tree::MouseCursorInner;

#[derive(Debug, Clone)]
pub enum Expression {
    /// A string literal. The .0 is the content of the string, without the quotes
    StringLiteral(SmolStr),
    /// Number
    NumberLiteral(f64),
    /// Bool
    BoolLiteral(bool),

    // Keys
    KeysLiteral(Keys),

    /// Reference to a property (which can also be a callback) or an element (property name is empty then).
    PropertyReference(MemberReference),

    /// Reference the parameter at the given index of the current function.
    FunctionParameterReference {
        index: usize,
        //ty: Type,
    },

    /// Should be directly within a CodeBlock expression, and store the value of the expression in a local variable
    StoreLocalVariable {
        name: SmolStr,
        value: Box<Expression>,
    },

    /// a reference to the local variable with the given name. The type system should ensure that a variable has been stored
    /// with this name and this type before in one of the statement of an enclosing codeblock
    ReadLocalVariable {
        name: SmolStr,
        ty: Type,
    },

    /// Access to a field of the given name within a struct.
    StructFieldAccess {
        /// This expression should have [`Type::Struct`] type
        base: Box<Expression>,
        name: SmolStr,
    },

    /// Access to a index within an array.
    ArrayIndex {
        /// This expression should have [`Type::Array`] type
        array: Box<Expression>,
        index: Box<Expression>,
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
        callback: MemberReference,
        arguments: Vec<Expression>,
    },
    FunctionCall {
        function: MemberReference,
        arguments: Vec<Expression>,
    },
    ItemMemberFunctionCall {
        function: MemberReference,
    },

    /// A BuiltinFunctionCall, but the function is not yet in the `BuiltinFunction` enum
    /// TODO: merge in BuiltinFunctionCall
    ExtraBuiltinFunctionCall {
        return_ty: Type,
        function: String,
        arguments: Vec<Expression>,
    },

    /// An assignment of a value to a property
    PropertyAssignment {
        property: MemberReference,
        value: Box<Expression>,
    },
    /// an assignment of a value to the model data
    ModelDataAssignment {
        // how deep in the parent hierarchy we go
        level: usize,
        value: Box<Expression>,
    },
    /// An assignment done with the `foo[idx] = ...`
    ArrayIndexAssignment {
        array: Box<Expression>,
        index: Box<Expression>,
        value: Box<Expression>,
    },
    /// An assignment to a mutable slice element: `slice[idx] = value`
    /// Unlike ArrayIndexAssignment, this writes directly to the slice without model semantics
    SliceIndexAssignment {
        /// Name of the slice variable (e.g., "result")
        slice_name: SmolStr,
        index: usize,
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
        nine_slice: Option<[u16; 4]>,
    },

    Condition {
        condition: Box<Expression>,
        true_expr: Box<Expression>,
        false_expr: Box<Expression>,
    },

    Array {
        element_ty: Type,
        values: Vec<Expression>,
        /// Choose what will be generated: a slice, a model, or a vector
        output: ArrayOutput,
    },
    Struct {
        ty: Rc<crate::langtype::Struct>,
        values: BTreeMap<SmolStr, Expression>,
    },

    EasingCurve(crate::expression_tree::EasingCurve),

    MouseCursor(MouseCursorInner<Expression>),

    LinearGradient {
        angle: Box<Expression>,
        /// First expression in the tuple is a color, second expression is the stop position
        stops: Vec<(Expression, Expression)>,
    },

    RadialGradient {
        /// Explicit gradient center in the element's local coordinate space (`at <x> <y>`).
        /// `None` means use the element's bbox centre.
        center: Option<(Box<Expression>, Box<Expression>)>,
        /// Explicit radius in the element's local coordinate space (`circle <radius>`).
        /// `None` means use the element's bbox half-diagonal.
        radius: Option<Box<Expression>>,
        /// First expression in the tuple is a color, second expression is the stop position
        stops: Vec<(Expression, Expression)>,
    },

    ConicGradient {
        /// The starting angle (rotation) of the gradient, corresponding to CSS `from <angle>`
        from_angle: Box<Expression>,
        /// Explicit gradient center in the element's local coordinate space (`at <x> <y>`).
        /// `None` means use the element's bbox centre.
        center: Option<(Box<Expression>, Box<Expression>)>,
        /// First expression in the tuple is a color, second expression is the stop position (normalized angle 0-1)
        stops: Vec<(Expression, Expression)>,
    },

    EnumerationValue(crate::langtype::EnumerationValue),

    /// Standard cache access (box layouts and static grid cells).
    /// See LayoutCacheAccess in expression_tree.rs
    LayoutCacheAccess {
        layout_cache_prop: MemberReference,
        index: usize,
        repeater_index: Option<Box<Expression>>,
        entries_per_item: usize,
    },
    /// Two-level indirection cache access for grid layouts with repeaters.
    /// See GridRepeaterCacheAccess in expression_tree.rs
    GridRepeaterCacheAccess {
        layout_cache_prop: MemberReference,
        index: usize,
        repeater_index: Box<Expression>,
        stride: Box<Expression>,
        child_offset: usize,
        inner_repeater_index: Option<Box<Expression>>,
        entries_per_item: usize,
    },
    /// Will call the sub_expression, with the cells variable set to the
    /// array of LayoutItemInfo from the elements
    WithLayoutItemInfo {
        /// The local variable (as read with [`Self::ReadLocalVariable`]) that contains the cells
        cells_variable: String,
        /// The name for the local variable that contains the repeater indices
        repeater_indices_var_name: Option<SmolStr>,
        /// The name for the local variable that contains the repeater steps
        repeater_steps_var_name: Option<SmolStr>,
        /// Either an expression of type LayoutItemInfo, or information about the repeater
        elements: Vec<Either<Expression, LayoutRepeatedElement>>,
        orientation: Orientation,
        sub_expression: Box<Expression>,
    },
    /// Will call the sub_expression, with two cells variables (horizontal and vertical)
    /// set to the arrays of LayoutItemInfo from the elements for FlexboxLayout
    WithFlexboxLayoutItemInfo {
        /// The local variable for horizontal cells
        cells_h_variable: String,
        /// The local variable for vertical cells
        cells_v_variable: String,
        /// The name for the local variable that contains the repeater indices
        repeater_indices_var_name: Option<SmolStr>,
        /// Either an expression pair of type (LayoutItemInfo, LayoutItemInfo), or information about the repeater
        elements: Vec<Either<(Expression, Expression), LayoutRepeatedElement>>,
        /// Container (cross-axis) width for a column flex: passed to each
        /// repeated cell's `flexbox_layout_item_info_at_cross_width` so a
        /// height-for-width instance wraps to the real width instead of its
        /// preferred width. `None` for a row flex (no cross-width to forward).
        repeated_cross_width: Option<Box<Expression>>,
        sub_expression: Box<Expression>,
    },
    /// Calls `solve_flexbox_layout_with_measure` with a generated measure
    /// callback so the cross-axis size of height-for-width cells is recomputed
    /// at the width/height taffy actually assigns (rather than the cell's
    /// preferred size). `data` is the `FlexboxLayoutData`. For each static cell,
    /// `measure_cells[i]` is `(h_info_given_known_h, v_info_given_known_w)`,
    /// each a `LayoutInfo`-typed expression that reads
    /// `ReadLocalVariable("measure_known_w" / "measure_known_h")` (a `Float32`)
    /// as its cross-axis constraint. `default_cells[i]` is the cell's
    /// `(h_info, v_info)` at the default constraint (matching `data`'s cells);
    /// it provides the preferred size returned when taffy asks for a dimension
    /// without a known cross-axis size (mirroring the plain `solve_flexbox_layout`
    /// measure). A repeater cell (the `Right` case) is measured by calling
    /// `flexbox_layout_item_info_at_cross_width` on the instance taffy asks for.
    SolveFlexboxLayoutWithMeasure {
        /// The `FlexboxLayoutData` (built inline with the cell arrays, so its
        /// temporaries live for the duration of the solve call).
        data: Box<Expression>,
        repeater_indices: Box<Expression>,
        measure_cells: Vec<Either<(Expression, Expression), LayoutRepeatedElement>>,
        /// Only used when `cells_variables` is `None`; empty otherwise.
        default_cells: Vec<Either<(Expression, Expression), LayoutRepeatedElement>>,
        /// Names of the flat `(cells_h, cells_v)` locals set up by the enclosing
        /// `WithFlexboxLayoutItemInfo`. `Some` exactly when the layout has a
        /// repeater: a repeater expands to a runtime number of cells, so the
        /// callback maps taffy's flat cell index to an element with a runtime
        /// cursor, and takes per-cell defaults from these arrays instead of the
        /// per-element `default_cells`.
        cells_variables: Option<(SmolStr, SmolStr)>,
    },
    /// Will call the sub_expression, with the cells variable set to the
    /// array of GridLayoutInputData from the elements
    WithGridInputData {
        /// The local variable (as read with [`Self::ReadLocalVariable`]) that contains the cells
        cells_variable: String,
        /// The name for the local variable that contains the repeater indices
        repeater_indices_var_name: SmolStr,
        /// The name for the local variable that contains the repeater steps
        repeater_steps_var_name: SmolStr,
        /// Either an expression of type GridLayoutInputData, or information about the repeated element
        elements: Vec<Either<Expression, GridLayoutRepeatedElement>>,
        sub_expression: Box<Expression>,
    },

    MinMax {
        ty: Type,
        op: MinMaxOp,
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },

    EmptyComponentFactory,

    EmptyDataTransfer,

    /// A reference to bundled translated string
    TranslationReference {
        /// An expression of type array of strings
        format_args: Box<Expression>,
        string_index: usize,
        /// The `n` value to use for the plural form if it is a plural form
        plural: Option<Box<Expression>>,
    },
}

/// The type of a binary expression with the given operator:
/// comparison and logic operators produce a bool,
/// while the arithmetic operators keep the type of the left operand
pub fn binary_expression_ty(op: char, lhs_ty: impl FnOnce() -> Type) -> Type {
    if crate::expression_tree::operator_class(op) != OperatorClass::ArithmeticOp {
        Type::Bool
    } else {
        lhs_ty()
    }
}

impl Expression {
    pub fn default_value_for_type(ty: &Type) -> Option<Self> {
        Some(match ty {
            Type::Invalid
            | Type::Callback { .. }
            | Type::Function { .. }
            | Type::Void
            | Type::InferredProperty
            | Type::InferredCallback
            | Type::ElementReference
            | Type::LayoutCache
            | Type::ArrayOfU16 => return None,
            Type::Float32
            | Type::Duration
            | Type::Int32
            | Type::Angle
            | Type::PhysicalLength
            | Type::LogicalLength
            | Type::Rem
            | Type::UnitProduct(_) => Expression::NumberLiteral(0.),
            Type::Percent => Expression::NumberLiteral(1.),
            Type::String => Expression::StringLiteral(SmolStr::default()),
            Type::Color => {
                Expression::Cast { from: Box::new(Expression::NumberLiteral(0.)), to: ty.clone() }
            }
            Type::Image => Expression::ImageReference {
                resource_ref: crate::expression_tree::ImageReference::None,
                nine_slice: None,
            },
            Type::Bool => Expression::BoolLiteral(false),
            Type::Model => return None,
            Type::PathData => return None,
            Type::Array(element_ty) => Expression::Array {
                element_ty: (**element_ty).clone(),
                values: Vec::new(),
                output: ArrayOutput::Model,
            },
            Type::Struct(s) => Expression::Struct {
                ty: s.clone(),
                values: s
                    .fields
                    .iter()
                    .map(|(k, v)| {
                        let value = match s.field_defaults.get(k) {
                            Some(default_value) => {
                                super::lower_expression::lower_constant_expression(default_value)
                            }
                            None => Expression::default_value_for_type(v)?,
                        };
                        Some((k.clone(), value))
                    })
                    .collect::<Option<_>>()?,
            },
            Type::Easing => Expression::EasingCurve(crate::expression_tree::EasingCurve::default()),
            Type::MouseCursor => {
                let e = crate::typeregister::BUILTIN.with(|e| e.enums.BuiltInMouseCursor.clone());
                Expression::MouseCursor(MouseCursorInner::BuiltIn(Box::new(
                    Expression::EnumerationValue(e.default_value()),
                )))
            }
            Type::Brush => Expression::Cast {
                from: Box::new(Expression::default_value_for_type(&Type::Color)?),
                to: Type::Brush,
            },
            Type::Enumeration(enumeration) => {
                Expression::EnumerationValue(enumeration.clone().default_value())
            }
            Type::Keys => Expression::KeysLiteral(Keys::default()),
            Type::DataTransfer => Expression::EmptyDataTransfer,
            Type::ComponentFactory => Expression::EmptyComponentFactory,
            Type::StyledText => Expression::BuiltinFunctionCall {
                function: BuiltinFunction::StringToStyledText,
                arguments: vec![Expression::StringLiteral(SmolStr::default())],
            },
        })
    }

    pub fn ty(&self, ctx: &dyn TypeResolutionContext) -> Type {
        match self {
            Self::StringLiteral(_) => Type::String,
            Self::NumberLiteral(_) => Type::Float32,
            Self::BoolLiteral(_) => Type::Bool,
            Self::PropertyReference(prop) => ctx.property_ty(prop).clone(),
            Self::FunctionParameterReference { index } => ctx.arg_type(*index).clone(),
            Self::StoreLocalVariable { .. } => Type::Void,
            Self::ReadLocalVariable { ty, .. } => ty.clone(),
            Self::StructFieldAccess { base, name } => match base.ty(ctx) {
                Type::Struct(s) => s.fields[name].clone(),
                _ => unreachable!(),
            },
            Self::ArrayIndex { array, .. } => match array.ty(ctx) {
                Type::Array(ty) => (*ty).clone(),
                _ => unreachable!(),
            },
            Self::Cast { to, .. } => to.clone(),
            Self::CodeBlock(sub) => sub.last().map_or(Type::Void, |e| e.ty(ctx)),
            Self::BuiltinFunctionCall { function, .. } => function.ty().return_type.clone(),
            Self::CallBackCall { callback, .. } => match ctx.property_ty(callback) {
                Type::Callback(callback) => callback.return_type.clone(),
                _ => Type::Invalid,
            },
            Self::FunctionCall { function, .. } => ctx.property_ty(function).clone(),
            Self::ItemMemberFunctionCall { function } => match ctx.property_ty(function) {
                Type::Function(function) => function.return_type.clone(),
                _ => Type::Invalid,
            },
            Self::ExtraBuiltinFunctionCall { return_ty, .. } => return_ty.clone(),
            Self::PropertyAssignment { .. } => Type::Void,
            Self::ModelDataAssignment { .. } => Type::Void,
            Self::ArrayIndexAssignment { .. } => Type::Void,
            Self::SliceIndexAssignment { .. } => Type::Void,
            Self::BinaryExpression { lhs, rhs: _, op } => binary_expression_ty(*op, || lhs.ty(ctx)),
            Self::UnaryOp { sub, .. } => sub.ty(ctx),
            Self::ImageReference { .. } => Type::Image,
            Self::Condition { false_expr, .. } => false_expr.ty(ctx),
            Self::Array { element_ty, .. } => Type::Array(element_ty.clone().into()),
            Self::Struct { ty, .. } => ty.clone().into(),
            Self::EasingCurve(_) => Type::Easing,
            Self::MouseCursor(_) => Type::MouseCursor,
            Self::LinearGradient { .. } => Type::Brush,
            Self::RadialGradient { .. } => Type::Brush,
            Self::ConicGradient { .. } => Type::Brush,
            Self::EnumerationValue(e) => Type::Enumeration(e.enumeration.clone()),
            Self::KeysLiteral(_) => Type::Keys,
            Self::LayoutCacheAccess { .. } => Type::LogicalLength,
            Self::GridRepeaterCacheAccess { .. } => Type::LogicalLength,
            Self::WithLayoutItemInfo { sub_expression, .. } => sub_expression.ty(ctx),
            Self::WithFlexboxLayoutItemInfo { sub_expression, .. } => sub_expression.ty(ctx),
            Self::SolveFlexboxLayoutWithMeasure { .. } => Type::LayoutCache,
            Self::WithGridInputData { sub_expression, .. } => sub_expression.ty(ctx),
            Self::MinMax { ty, .. } => ty.clone(),
            Self::EmptyComponentFactory => Type::ComponentFactory,
            Self::EmptyDataTransfer => Type::DataTransfer,
            Self::TranslationReference { .. } => Type::String,
        }
    }
}

macro_rules! visit_impl {
    ($self:ident, $visitor:ident, $as_ref:ident, $iter:ident, $values:ident) => {
        match $self {
            Expression::StringLiteral(_) => {}
            Expression::NumberLiteral(_) => {}
            Expression::BoolLiteral(_) => {}
            Expression::PropertyReference(_) => {}
            Expression::FunctionParameterReference { .. } => {}
            Expression::StoreLocalVariable { value, .. } => $visitor(value),
            Expression::ReadLocalVariable { .. } => {}
            Expression::StructFieldAccess { base, .. } => $visitor(base),
            Expression::ArrayIndex { array, index } => {
                $visitor(array);
                $visitor(index);
            }
            Expression::Cast { from, .. } => $visitor(from),
            Expression::CodeBlock(b) => b.$iter().for_each($visitor),
            Expression::BuiltinFunctionCall { arguments, .. }
            | Expression::CallBackCall { arguments, .. }
            | Expression::FunctionCall { arguments, .. } => arguments.$iter().for_each($visitor),
            Expression::ItemMemberFunctionCall { function: _ } => {}
            Expression::ExtraBuiltinFunctionCall { arguments, .. } => {
                arguments.$iter().for_each($visitor)
            }
            Expression::PropertyAssignment { value, .. } => $visitor(value),
            Expression::ModelDataAssignment { value, .. } => $visitor(value),
            Expression::ArrayIndexAssignment { array, index, value } => {
                $visitor(array);
                $visitor(index);
                $visitor(value);
            }
            Expression::SliceIndexAssignment { value, .. } => {
                $visitor(value);
            }
            Expression::BinaryExpression { lhs, rhs, .. } => {
                $visitor(lhs);
                $visitor(rhs);
            }
            Expression::UnaryOp { sub, .. } => {
                $visitor(sub);
            }
            Expression::ImageReference { .. } => {}
            Expression::Condition { condition, true_expr, false_expr } => {
                $visitor(condition);
                $visitor(true_expr);
                $visitor(false_expr);
            }
            Expression::Array { values, .. } => values.$iter().for_each($visitor),
            Expression::Struct { values, .. } => values.$values().for_each($visitor),
            Expression::EasingCurve(_) => {}
            Expression::MouseCursor(cursor) => match cursor {
                MouseCursorInner::CustomMouseCursor { image, hotspot_x, hotspot_y } => {
                    $visitor(image);
                    $visitor(hotspot_x);
                    $visitor(hotspot_y);
                }
                MouseCursorInner::BuiltIn(e) => {
                    $visitor(e);
                }
            },
            Expression::LinearGradient { angle, stops } => {
                $visitor(angle);
                for (a, b) in stops {
                    $visitor(a);
                    $visitor(b);
                }
            }
            Expression::RadialGradient { center, radius, stops } => {
                if let Some((cx, cy)) = center {
                    $visitor(cx);
                    $visitor(cy);
                }
                if let Some(r) = radius {
                    $visitor(r);
                }
                for (a, b) in stops {
                    $visitor(a);
                    $visitor(b);
                }
            }
            Expression::ConicGradient { from_angle, center, stops } => {
                $visitor(from_angle);
                if let Some((cx, cy)) = center {
                    $visitor(cx);
                    $visitor(cy);
                }
                for (a, b) in stops {
                    $visitor(a);
                    $visitor(b);
                }
            }
            Expression::EnumerationValue(_) => {}
            Expression::KeysLiteral(_) => {}
            Expression::LayoutCacheAccess { repeater_index, .. } => {
                if let Some(repeater_index) = repeater_index {
                    $visitor(repeater_index);
                }
            }
            Expression::GridRepeaterCacheAccess {
                repeater_index,
                stride,
                inner_repeater_index,
                ..
            } => {
                $visitor(repeater_index);
                $visitor(stride);
                if let Some(inner_repeater_index) = inner_repeater_index {
                    $visitor(inner_repeater_index);
                }
            }
            Expression::WithLayoutItemInfo { elements, sub_expression, .. } => {
                $visitor(sub_expression);
                elements.$iter().filter_map(|x| x.$as_ref().left()).for_each($visitor);
            }
            Expression::WithFlexboxLayoutItemInfo {
                elements,
                repeated_cross_width,
                sub_expression,
                ..
            } => {
                $visitor(sub_expression);
                if let Some(w) = repeated_cross_width {
                    $visitor(w);
                }
                elements.$iter().filter_map(|x| x.$as_ref().left()).for_each(|(h, v)| {
                    $visitor(h);
                    $visitor(v);
                });
            }
            Expression::SolveFlexboxLayoutWithMeasure {
                data,
                repeater_indices,
                measure_cells,
                default_cells,
                cells_variables: _,
            } => {
                $visitor(data);
                $visitor(repeater_indices);
                measure_cells.$iter().filter_map(|x| x.$as_ref().left()).for_each(|(h, v)| {
                    $visitor(h);
                    $visitor(v);
                });
                default_cells.$iter().filter_map(|x| x.$as_ref().left()).for_each(|(h, v)| {
                    $visitor(h);
                    $visitor(v);
                });
            }
            Expression::WithGridInputData { elements, sub_expression, .. } => {
                $visitor(sub_expression);
                elements.$iter().filter_map(|x| x.$as_ref().left()).for_each($visitor);
            }
            Expression::MinMax { ty: _, op: _, lhs, rhs } => {
                $visitor(lhs);
                $visitor(rhs);
            }
            Expression::EmptyComponentFactory => {}
            Expression::EmptyDataTransfer => {}
            Expression::TranslationReference { format_args, plural, string_index: _ } => {
                $visitor(format_args);
                if let Some(plural) = plural {
                    $visitor(plural);
                }
            }
        }
    };
}

impl Expression {
    /// Call the visitor for each sub-expression (not recursive)
    pub fn visit(&self, mut visitor: impl FnMut(&Self)) {
        visit_impl!(self, visitor, as_ref, iter, values)
    }

    /// Call the visitor for each sub-expression (not recursive)
    pub fn visit_mut(&mut self, mut visitor: impl FnMut(&mut Self)) {
        visit_impl!(self, visitor, as_mut, iter_mut, values_mut)
    }

    /// Visit itself and each sub expression recursively
    pub fn visit_recursive(&self, visitor: &mut dyn FnMut(&Self)) {
        visitor(self);
        self.visit(|e| e.visit_recursive(visitor));
    }

    /// Visit itself and each sub expression recursively
    pub fn visit_recursive_mut(&mut self, visitor: &mut dyn FnMut(&mut Self)) {
        visitor(self);
        self.visit_mut(|e| e.visit_recursive_mut(visitor));
    }

    pub fn visit_property_references(
        &self,
        ctx: &EvaluationContext,
        visitor: &mut dyn FnMut(&MemberReference, &EvaluationContext),
    ) {
        self.visit_recursive(&mut |expr| {
            let p = match expr {
                Expression::PropertyReference(p) => p,
                Expression::CallBackCall { callback, .. } => callback,
                // The `function` of a call is also a member reference. `property_info`
                // returns nothing for it, so callers that only care about properties
                // ignore it, while callers that track function use can act on it.
                Expression::FunctionCall { function, .. } => function,
                Expression::PropertyAssignment { property, .. } => {
                    if let Some((a, map)) = &ctx.property_info(property).animation {
                        let ctx2 = map.map_context(ctx);
                        a.visit_property_references(&ctx2, visitor);
                    }
                    property
                }
                // FIXME  (should be fine anyway because we mark these as not optimizable)
                Expression::ModelDataAssignment { .. } => return,
                Expression::LayoutCacheAccess { layout_cache_prop, .. } => layout_cache_prop,
                Expression::GridRepeaterCacheAccess { layout_cache_prop, .. } => layout_cache_prop,
                _ => return,
            };
            visitor(p, ctx)
        });
    }
}

pub trait TypeResolutionContext {
    /// The type of the property.
    ///
    /// For reference to function, this is the return type
    fn property_ty(&self, _: &MemberReference) -> &Type;

    // The type of the specified argument when evaluating a callback
    fn arg_type(&self, _index: usize) -> &Type {
        unimplemented!()
    }
}

/// The parent context of the current context when the current context is repeated
#[derive(Clone, Copy)]
pub struct ParentScope<'a> {
    /// The parent sub component
    pub sub_component: SubComponentIdx,
    /// Index of the repeater within the ctx.current_sub_component
    pub repeater_index: Option<RepeatedElementIdx>,
    /// A further parent context when the parent context is itself in a repeater
    pub parent: Option<&'a ParentScope<'a>>,
}

impl<'a> ParentScope<'a> {
    pub fn new<T>(
        ctx: &'a EvaluationContext<'a, T>,
        repeater_index: Option<RepeatedElementIdx>,
    ) -> Self {
        let EvaluationScope::SubComponent(sub_component, parent) = ctx.current_scope else {
            unreachable!()
        };
        Self { sub_component, repeater_index, parent }
    }
}

#[derive(Clone, Copy)]
pub enum EvaluationScope<'a> {
    /// The evaluation context is in a sub component, optionally with information about the repeater parent
    SubComponent(SubComponentIdx, Option<&'a ParentScope<'a>>),
    /// The evaluation context is in a global
    Global(GlobalIdx),
    /// The evaluation context is a constant expression that cannot reference any
    /// properties or elements, such as the default value of a struct field
    Const,
}

#[derive(Clone)]
pub struct EvaluationContext<'a, T = ()> {
    pub compilation_unit: &'a super::CompilationUnit,
    pub current_scope: EvaluationScope<'a>,
    pub generator_state: T,

    /// The callback argument types
    pub argument_types: &'a [Type],
}

impl<'a, T> EvaluationContext<'a, T> {
    pub fn new_sub_component(
        compilation_unit: &'a super::CompilationUnit,
        sub_component: SubComponentIdx,
        generator_state: T,
        parent: Option<&'a ParentScope<'a>>,
    ) -> Self {
        Self {
            compilation_unit,
            current_scope: EvaluationScope::SubComponent(sub_component, parent),
            generator_state,
            argument_types: &[],
        }
    }

    pub fn new_global(
        compilation_unit: &'a super::CompilationUnit,
        global: GlobalIdx,
        generator_state: T,
    ) -> Self {
        Self {
            compilation_unit,
            current_scope: EvaluationScope::Global(global),
            generator_state,
            argument_types: &[],
        }
    }

    /// A context for compiling a constant expression that cannot reference any
    /// properties or elements, such as the default value of a struct field
    /// (see [`crate::langtype::Struct::field_defaults`])
    pub fn new_const(compilation_unit: &'a super::CompilationUnit, generator_state: T) -> Self {
        Self {
            compilation_unit,
            current_scope: EvaluationScope::Const,
            generator_state,
            argument_types: &[],
        }
    }

    pub(crate) fn property_info<'b>(&'b self, prop: &MemberReference) -> PropertyInfoResult<'b> {
        fn match_in_sub_component<'b>(
            cu: &'b super::CompilationUnit,
            sc: &'b super::SubComponent,
            prop: &LocalMemberReference,
            map: ContextMap,
        ) -> PropertyInfoResult<'b> {
            let use_count_and_ty = || {
                let mut sc = sc;
                for i in &prop.sub_component_path {
                    sc = &cu.sub_components[sc.sub_components[*i].ty];
                }
                match &prop.reference {
                    LocalMemberIndex::Property(property_index) => {
                        sc.properties.get(*property_index).map(|x| (&x.use_count, &x.ty))
                    }
                    LocalMemberIndex::Callback(callback_index) => {
                        sc.callbacks.get(*callback_index).map(|x| (&x.use_count, &x.ty))
                    }
                    _ => None,
                }
            };

            let animation = sc.animations.get(prop).map(|a| (a, map.clone()));
            let analysis = sc.prop_analysis.get(&prop.clone().into());
            if let Some(a) = &analysis
                && let Some(init) = a.property_init
            {
                let u = use_count_and_ty();
                return PropertyInfoResult {
                    analysis: Some(&a.analysis),
                    binding: Some((&sc.property_init[init].1, map)),
                    animation,
                    ty: u.map_or(Type::Invalid, |x| x.1.clone()),
                    use_count: u.map(|x| x.0),
                };
            }
            let mut r = if let &[idx, ref rest @ ..] = prop.sub_component_path.as_slice() {
                let prop2 = LocalMemberReference {
                    sub_component_path: rest.to_vec(),
                    reference: prop.reference.clone(),
                };
                match_in_sub_component(
                    cu,
                    &cu.sub_components[sc.sub_components[idx].ty],
                    &prop2,
                    map.deeper_in_sub_component(idx),
                )
            } else {
                let u = use_count_and_ty();
                PropertyInfoResult {
                    ty: u.map_or(Type::Invalid, |x| x.1.clone()),
                    use_count: u.map(|x| x.0),
                    ..Default::default()
                }
            };

            if animation.is_some() {
                r.animation = animation
            };
            if let Some(a) = analysis {
                r.analysis = Some(&a.analysis);
            }
            r
        }

        fn in_global<'a>(
            g: &'a super::GlobalComponent,
            r: &'_ LocalMemberIndex,
            map: ContextMap,
        ) -> PropertyInfoResult<'a> {
            let binding = g.init_values.get(r).map(|b| (b, map.clone()));
            let animation = g.animations.get(r).map(|a| (a, map));
            match r {
                LocalMemberIndex::Property(index) => {
                    let property_decl = &g.properties[*index];
                    PropertyInfoResult {
                        analysis: Some(&g.prop_analysis[*index]),
                        binding,
                        animation,
                        ty: property_decl.ty.clone(),
                        use_count: Some(&property_decl.use_count),
                    }
                }
                LocalMemberIndex::Callback(index) => {
                    let callback_decl = &g.callbacks[*index];
                    PropertyInfoResult {
                        analysis: None,
                        binding,
                        animation: None,
                        ty: callback_decl.ty.clone(),
                        use_count: Some(&callback_decl.use_count),
                    }
                }
                _ => PropertyInfoResult::default(),
            }
        }

        match prop {
            MemberReference::Relative { parent_level, local_reference } => {
                match self.current_scope {
                    EvaluationScope::Global(g) => {
                        let g = &self.compilation_unit.globals[g];
                        in_global(g, &local_reference.reference, ContextMap::Identity)
                    }
                    EvaluationScope::SubComponent(mut sc, mut parent) => {
                        for _ in 0..*parent_level {
                            // The parent chain is severed for function bodies (see
                            // `for_each_expression`); the reference is then not
                            // resolvable, like `function_info` also reports.
                            let Some(p) = parent else {
                                return PropertyInfoResult::default();
                            };
                            sc = p.sub_component;
                            parent = p.parent;
                        }
                        match_in_sub_component(
                            self.compilation_unit,
                            &self.compilation_unit.sub_components[sc],
                            local_reference,
                            ContextMap::from_parent_level(*parent_level),
                        )
                    }
                    EvaluationScope::Const => {
                        panic!("property reference in a constant expression")
                    }
                }
            }
            MemberReference::Global { global_index, member } => {
                let g = &self.compilation_unit.globals[*global_index];
                in_global(g, member, ContextMap::InGlobal(*global_index))
            }
        }
    }

    /// Resolve a reference to a user function, returning the function and the
    /// [`ContextMap`] to evaluate its body in the current context.
    pub(crate) fn function_info<'b>(
        &'b self,
        reference: &MemberReference,
    ) -> Option<(&'b super::Function, ContextMap)> {
        let cu = self.compilation_unit;
        match reference {
            MemberReference::Relative { parent_level, local_reference } => {
                // Cheap check before walking the scope: most references are not functions.
                let LocalMemberIndex::Function(idx) = local_reference.reference else {
                    return None;
                };
                let mut scope = self.current_scope;
                for _ in 0..*parent_level {
                    let EvaluationScope::SubComponent(_, Some(p)) = scope else { return None };
                    scope = EvaluationScope::SubComponent(p.sub_component, p.parent);
                }
                let EvaluationScope::SubComponent(mut sc, _) = scope else { return None };
                for i in &local_reference.sub_component_path {
                    sc = cu.sub_components[sc].sub_components[*i].ty;
                }
                Some((
                    cu.sub_components[sc].functions.get(idx)?,
                    ContextMap::from_parent_level(*parent_level)
                        .deeper_by_path(&local_reference.sub_component_path),
                ))
            }
            MemberReference::Global { global_index, member } => {
                let LocalMemberIndex::Function(idx) = member else { return None };
                Some((
                    cu.globals[*global_index].functions.get(*idx)?,
                    ContextMap::InGlobal(*global_index),
                ))
            }
        }
    }

    pub fn current_sub_component(&self) -> Option<&super::SubComponent> {
        let EvaluationScope::SubComponent(i, _) = self.current_scope else { return None };
        self.compilation_unit.sub_components.get(i)
    }

    pub fn current_global(&self) -> Option<&super::GlobalComponent> {
        let EvaluationScope::Global(i) = self.current_scope else { return None };
        self.compilation_unit.globals.get(i)
    }

    pub fn parent_sub_component_idx(&self, parent: usize) -> Option<SubComponentIdx> {
        let EvaluationScope::SubComponent(mut sc, mut par) = self.current_scope else {
            return None;
        };
        for _ in 0..parent {
            let p = par?;
            sc = p.sub_component;
            par = p.parent;
        }
        Some(sc)
    }

    pub fn relative_property_ty(
        &self,
        local_reference: &LocalMemberReference,
        parent_level: usize,
    ) -> &Type {
        if let Some(g) = self.current_global() {
            return match &local_reference.reference {
                LocalMemberIndex::Property(property_idx) => &g.properties[*property_idx].ty,
                LocalMemberIndex::Function(function_idx) => &g.functions[*function_idx].ret_ty,
                LocalMemberIndex::Callback(callback_idx) => &g.callbacks[*callback_idx].ty,
                LocalMemberIndex::Native { .. } | LocalMemberIndex::Timer(_) => unreachable!(),
            };
        }

        let mut sc = &self.compilation_unit.sub_components
            [self.parent_sub_component_idx(parent_level).unwrap()];
        for i in &local_reference.sub_component_path {
            sc = &self.compilation_unit.sub_components[sc.sub_components[*i].ty];
        }
        match &local_reference.reference {
            LocalMemberIndex::Property(property_index) => &sc.properties[*property_index].ty,
            LocalMemberIndex::Function(function_index) => &sc.functions[*function_index].ret_ty,
            LocalMemberIndex::Callback(callback_index) => &sc.callbacks[*callback_index].ty,
            LocalMemberIndex::Timer(_) => unreachable!("a timer reference has no type"),
            LocalMemberIndex::Native { item_index, prop_name, .. } => {
                if prop_name == "elements" {
                    // The `Path::elements` property is not in the NativeClass
                    return &Type::PathData;
                }
                let item = &sc.items[*item_index];
                item.ty.lookup_property(prop_name).unwrap_or_else(|| {
                    panic!("Failed to lookup property {prop_name} for {}", item.name)
                })
            }
        }
    }
}

impl<T> TypeResolutionContext for EvaluationContext<'_, T> {
    fn property_ty(&self, prop: &MemberReference) -> &Type {
        match prop {
            MemberReference::Relative { parent_level, local_reference } => {
                self.relative_property_ty(local_reference, *parent_level)
            }
            MemberReference::Global { global_index, member } => {
                let g = &self.compilation_unit.globals[*global_index];
                match member {
                    LocalMemberIndex::Property(property_idx) => &g.properties[*property_idx].ty,
                    LocalMemberIndex::Function(function_idx) => &g.functions[*function_idx].ret_ty,
                    LocalMemberIndex::Callback(callback_idx) => &g.callbacks[*callback_idx].ty,
                    LocalMemberIndex::Native { .. } | LocalMemberIndex::Timer(_) => unreachable!(),
                }
            }
        }
    }

    fn arg_type(&self, index: usize) -> &Type {
        &self.argument_types[index]
    }
}

#[derive(Default, Debug)]
pub(crate) struct PropertyInfoResult<'a> {
    pub analysis: Option<&'a crate::object_tree::PropertyAnalysis>,
    pub binding: Option<(&'a super::BindingExpression, ContextMap)>,
    pub animation: Option<(&'a Expression, ContextMap)>,
    pub ty: Type,
    pub use_count: Option<&'a std::cell::Cell<usize>>,
}

/// Maps between two evaluation context.
/// This allows to go from the current subcomponents context, to the context
/// relative to the binding we want to inline
#[derive(Debug, Clone)]
pub(crate) enum ContextMap {
    Identity,
    InSubElement { path: Vec<SubComponentInstanceIdx>, parent: usize },
    InGlobal(GlobalIdx),
}

impl ContextMap {
    fn from_parent_level(parent_level: usize) -> Self {
        if parent_level == 0 {
            ContextMap::Identity
        } else {
            ContextMap::InSubElement { parent: parent_level, path: Vec::new() }
        }
    }

    fn deeper_in_sub_component(self, sub: SubComponentInstanceIdx) -> Self {
        match self {
            ContextMap::Identity => ContextMap::InSubElement { parent: 0, path: vec![sub] },
            ContextMap::InSubElement { mut path, parent } => {
                path.push(sub);
                ContextMap::InSubElement { path, parent }
            }
            ContextMap::InGlobal(_) => panic!(),
        }
    }

    fn deeper_by_path(self, path: &[SubComponentInstanceIdx]) -> Self {
        path.iter().fold(self, |m, sub| m.deeper_in_sub_component(*sub))
    }

    pub fn map_property_reference(&self, p: &MemberReference) -> MemberReference {
        match self {
            ContextMap::Identity => p.clone(),
            ContextMap::InSubElement { path, parent } => match p {
                MemberReference::Relative { parent_level, local_reference } => {
                    MemberReference::Relative {
                        parent_level: *parent_level + *parent,
                        local_reference: LocalMemberReference {
                            sub_component_path: path
                                .iter()
                                .chain(local_reference.sub_component_path.iter())
                                .copied()
                                .collect(),
                            reference: local_reference.reference.clone(),
                        },
                    }
                }
                MemberReference::Global { .. } => p.clone(),
            },
            ContextMap::InGlobal(global_index) => match p {
                MemberReference::Relative { parent_level, local_reference } => {
                    assert!(local_reference.sub_component_path.is_empty());
                    assert_eq!(*parent_level, 0);
                    MemberReference::Global {
                        global_index: *global_index,
                        member: local_reference.reference.clone(),
                    }
                }
                g @ MemberReference::Global { .. } => g.clone(),
            },
        }
    }

    pub fn map_expression(&self, e: &mut Expression) {
        match e {
            Expression::PropertyReference(p)
            | Expression::CallBackCall { callback: p, .. }
            | Expression::FunctionCall { function: p, .. }
            | Expression::ItemMemberFunctionCall { function: p, .. }
            | Expression::PropertyAssignment { property: p, .. }
            | Expression::LayoutCacheAccess { layout_cache_prop: p, .. }
            | Expression::GridRepeaterCacheAccess { layout_cache_prop: p, .. } => {
                *p = self.map_property_reference(p);
            }
            _ => (),
        }
        e.visit_mut(|e| self.map_expression(e))
    }

    pub fn map_context<'a>(&self, ctx: &EvaluationContext<'a>) -> EvaluationContext<'a> {
        match self {
            ContextMap::Identity => ctx.clone(),
            ContextMap::InSubElement { path, parent } => {
                let mut sc = ctx.parent_sub_component_idx(*parent).unwrap();
                for i in path {
                    sc = ctx.compilation_unit.sub_components[sc].sub_components[*i].ty;
                }
                EvaluationContext::new_sub_component(ctx.compilation_unit, sc, (), None)
            }
            ContextMap::InGlobal(g) => EvaluationContext::new_global(ctx.compilation_unit, *g, ()),
        }
    }
}
