// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{
    GlobalIdx, PropertyReference, RepeatedElementIdx, SubComponentIdx, SubComponentInstanceIdx,
};
use crate::expression_tree::{BuiltinFunction, MinMaxOp, OperatorClass};
use crate::langtype::Type;
use crate::layout::Orientation;
use core::num::NonZeroUsize;
use itertools::Either;
use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum Expression {
    /// A string literal. The .0 is the content of the string, without the quotes
    StringLiteral(SmolStr),
    /// Number
    NumberLiteral(f64),
    /// Bool
    BoolLiteral(bool),

    /// Reference to a property (which can also be a callback) or an element (property name is empty then).
    PropertyReference(PropertyReference),

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
        callback: PropertyReference,
        arguments: Vec<Expression>,
    },
    FunctionCall {
        function: PropertyReference,
        arguments: Vec<Expression>,
    },
    ItemMemberFunctionCall {
        function: PropertyReference,
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
        property: PropertyReference,
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
        /// When true, this should be converted to a model. When false, this should stay as a slice
        as_model: bool,
    },
    Struct {
        ty: Rc<crate::langtype::Struct>,
        values: BTreeMap<SmolStr, Expression>,
    },

    EasingCurve(crate::expression_tree::EasingCurve),

    LinearGradient {
        angle: Box<Expression>,
        /// First expression in the tuple is a color, second expression is the stop position
        stops: Vec<(Expression, Expression)>,
    },

    RadialGradient {
        /// First expression in the tuple is a color, second expression is the stop position
        stops: Vec<(Expression, Expression)>,
    },

    EnumerationValue(crate::langtype::EnumerationValue),

    LayoutCacheAccess {
        layout_cache_prop: PropertyReference,
        index: usize,
        /// When set, this is the index within a repeater, and the index is then the location of another offset.
        /// So this looks like `layout_cache_prop[layout_cache_prop[index] + repeater_index]`
        repeater_index: Option<Box<Expression>>,
    },
    /// Will call the sub_expression, with the cell variable set to the
    /// array of BoxLayoutCellData from the elements
    BoxLayoutFunction {
        /// The local variable (as read with [`Self::ReadLocalVariable`]) that contains the sell
        cells_variable: String,
        /// The name for the local variable that contains the repeater indices
        repeater_indices: Option<SmolStr>,
        /// Either an expression of type BoxLayoutCellData, or an index to the repeater
        elements: Vec<Either<Expression, RepeatedElementIdx>>,
        orientation: Orientation,
        sub_expression: Box<Expression>,
    },

    ComputeDialogLayoutCells {
        /// The local variable where the slice of cells is going to be stored
        cells_variable: String,
        roles: Box<Expression>,
        /// This is an Expression::Array
        unsorted_cells: Box<Expression>,
    },

    MinMax {
        ty: Type,
        op: MinMaxOp,
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },

    EmptyComponentFactory,

    /// A reference to bundled translated string
    TranslationReference {
        /// An expression of type array of strings
        format_args: Box<Expression>,
        string_index: usize,
        /// The `n` value to use for the plural form if it is a plural form
        plural: Option<Box<Expression>>,
    },
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
            | Type::LayoutCache => return None,
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
                values: vec![],
                as_model: true,
            },
            Type::Struct(s) => Expression::Struct {
                ty: s.clone(),
                values: s
                    .fields
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
            Type::ComponentFactory => Expression::EmptyComponentFactory,
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
            Self::BinaryExpression { lhs, rhs: _, op } => {
                if crate::expression_tree::operator_class(*op) != OperatorClass::ArithmeticOp {
                    Type::Bool
                } else {
                    lhs.ty(ctx)
                }
            }
            Self::UnaryOp { sub, .. } => sub.ty(ctx),
            Self::ImageReference { .. } => Type::Image,
            Self::Condition { false_expr, .. } => false_expr.ty(ctx),
            Self::Array { element_ty, .. } => Type::Array(element_ty.clone().into()),
            Self::Struct { ty, .. } => ty.clone().into(),
            Self::EasingCurve(_) => Type::Easing,
            Self::LinearGradient { .. } => Type::Brush,
            Self::RadialGradient { .. } => Type::Brush,
            Self::EnumerationValue(e) => Type::Enumeration(e.enumeration.clone()),
            Self::LayoutCacheAccess { .. } => Type::LogicalLength,
            Self::BoxLayoutFunction { sub_expression, .. } => sub_expression.ty(ctx),
            Self::ComputeDialogLayoutCells { .. } => {
                Type::Array(super::lower_expression::grid_layout_cell_data_ty().into())
            }
            Self::MinMax { ty, .. } => ty.clone(),
            Self::EmptyComponentFactory => Type::ComponentFactory,
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
            Expression::LinearGradient { angle, stops } => {
                $visitor(angle);
                for (a, b) in stops {
                    $visitor(a);
                    $visitor(b);
                }
            }
            Expression::RadialGradient { stops } => {
                for (a, b) in stops {
                    $visitor(a);
                    $visitor(b);
                }
            }
            Expression::EnumerationValue(_) => {}
            Expression::LayoutCacheAccess { repeater_index, .. } => {
                if let Some(repeater_index) = repeater_index {
                    $visitor(repeater_index);
                }
            }
            Expression::BoxLayoutFunction { elements, sub_expression, .. } => {
                $visitor(sub_expression);
                elements.$iter().filter_map(|x| x.$as_ref().left()).for_each($visitor);
            }
            Expression::ComputeDialogLayoutCells { roles, unsorted_cells, .. } => {
                $visitor(roles);
                $visitor(unsorted_cells);
            }
            Expression::MinMax { ty: _, op: _, lhs, rhs } => {
                $visitor(lhs);
                $visitor(rhs);
            }
            Expression::EmptyComponentFactory => {}
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

    pub fn visit_property_references(
        &self,
        ctx: &EvaluationContext,
        visitor: &mut dyn FnMut(&PropertyReference, &EvaluationContext),
    ) {
        self.visit_recursive(&mut |expr| {
            let p = match expr {
                Expression::PropertyReference(p) => p,
                Expression::CallBackCall { callback, .. } => callback,
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
    fn property_ty(&self, _: &PropertyReference) -> &Type;
    // The type of the specified argument when evaluating a callback
    fn arg_type(&self, _index: usize) -> &Type {
        unimplemented!()
    }
}

pub struct ParentCtx<'a, T = ()> {
    pub ctx: &'a EvaluationContext<'a, T>,
    // Index of the repeater within the ctx.current_sub_component
    pub repeater_index: Option<RepeatedElementIdx>,
}

impl<T> Clone for ParentCtx<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for ParentCtx<'_, T> {}

impl<'a, T> ParentCtx<'a, T> {
    pub fn new(
        ctx: &'a EvaluationContext<'a, T>,
        repeater_index: Option<RepeatedElementIdx>,
    ) -> Self {
        Self { ctx, repeater_index }
    }
}

#[derive(Clone)]
pub struct EvaluationContext<'a, T = ()> {
    pub compilation_unit: &'a super::CompilationUnit,
    pub current_sub_component: Option<SubComponentIdx>,
    pub current_global: Option<GlobalIdx>,
    pub generator_state: T,
    /// The repeater parent
    pub parent: Option<ParentCtx<'a, T>>,

    /// The callback argument types
    pub argument_types: &'a [Type],
}

impl<'a, T> EvaluationContext<'a, T> {
    pub fn new_sub_component(
        compilation_unit: &'a super::CompilationUnit,
        sub_component: SubComponentIdx,
        generator_state: T,
        parent: Option<ParentCtx<'a, T>>,
    ) -> Self {
        Self {
            compilation_unit,
            current_sub_component: Some(sub_component),
            current_global: None,
            generator_state,
            parent,
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
            current_sub_component: None,
            current_global: Some(global),
            generator_state,
            parent: None,
            argument_types: &[],
        }
    }

    pub(crate) fn property_info<'b>(&'b self, prop: &PropertyReference) -> PropertyInfoResult<'b> {
        fn match_in_sub_component<'b>(
            cu: &'b super::CompilationUnit,
            sc: &'b super::SubComponent,
            prop: &PropertyReference,
            map: ContextMap,
        ) -> PropertyInfoResult<'b> {
            let property_decl = || {
                if let PropertyReference::Local { property_index, sub_component_path } = &prop {
                    let mut sc = sc;
                    for i in sub_component_path {
                        sc = &cu.sub_components[sc.sub_components[*i].ty];
                    }
                    Some(&sc.properties[*property_index])
                } else {
                    None
                }
            };
            let animation = sc.animations.get(prop).map(|a| (a, map.clone()));
            let analysis = sc.prop_analysis.get(prop);
            if let Some(a) = &analysis {
                if let Some(init) = a.property_init {
                    return PropertyInfoResult {
                        analysis: Some(&a.analysis),
                        binding: Some((&sc.property_init[init].1, map)),
                        animation,
                        property_decl: property_decl(),
                    };
                }
            }
            let apply_analysis = |mut r: PropertyInfoResult<'b>| -> PropertyInfoResult<'b> {
                if animation.is_some() {
                    r.animation = animation
                };
                if let Some(a) = analysis {
                    r.analysis = Some(&a.analysis);
                }
                r
            };
            match prop {
                PropertyReference::Local { sub_component_path, property_index } => {
                    if !sub_component_path.is_empty() {
                        let prop2 = PropertyReference::Local {
                            sub_component_path: sub_component_path[1..].to_vec(),
                            property_index: *property_index,
                        };
                        let idx = sub_component_path[0];
                        return apply_analysis(match_in_sub_component(
                            cu,
                            &cu.sub_components[sc.sub_components[idx].ty],
                            &prop2,
                            map.deeper_in_sub_component(idx),
                        ));
                    }
                }
                PropertyReference::InNativeItem { item_index, sub_component_path, prop_name } => {
                    if !sub_component_path.is_empty() {
                        let prop2 = PropertyReference::InNativeItem {
                            sub_component_path: sub_component_path[1..].to_vec(),
                            prop_name: prop_name.clone(),
                            item_index: *item_index,
                        };
                        let idx = sub_component_path[0];
                        return apply_analysis(match_in_sub_component(
                            cu,
                            &cu.sub_components[sc.sub_components[idx].ty],
                            &prop2,
                            map.deeper_in_sub_component(idx),
                        ));
                    }
                }
                _ => unreachable!(),
            }
            apply_analysis(PropertyInfoResult {
                property_decl: property_decl(),
                ..Default::default()
            })
        }

        match prop {
            PropertyReference::Local { property_index, .. } => {
                if let Some(g) = self.current_global() {
                    PropertyInfoResult {
                        analysis: Some(&g.prop_analysis[*property_index]),
                        binding: g.init_values[*property_index]
                            .as_ref()
                            .map(|b| (b, ContextMap::Identity)),
                        animation: None,
                        property_decl: Some(&g.properties[*property_index]),
                    }
                } else if let Some(sc) = self.current_sub_component() {
                    match_in_sub_component(self.compilation_unit, sc, prop, ContextMap::Identity)
                } else {
                    unreachable!()
                }
            }
            PropertyReference::InNativeItem { .. } => match_in_sub_component(
                self.compilation_unit,
                self.current_sub_component().unwrap(),
                prop,
                ContextMap::Identity,
            ),
            PropertyReference::Global { global_index, property_index } => {
                let g = &self.compilation_unit.globals[*global_index];
                PropertyInfoResult {
                    analysis: Some(&g.prop_analysis[*property_index]),
                    animation: None,
                    binding: g
                        .init_values
                        .get(*property_index)
                        .and_then(Option::as_ref)
                        .map(|b| (b, ContextMap::InGlobal(*global_index))),
                    property_decl: Some(&g.properties[*property_index]),
                }
            }
            PropertyReference::InParent { level, parent_reference } => {
                let mut ctx = self;
                for _ in 0..level.get() {
                    ctx = ctx.parent.as_ref().unwrap().ctx;
                }
                let mut ret = ctx.property_info(parent_reference);
                match &mut ret.binding {
                    Some((_, m @ ContextMap::Identity)) => {
                        *m = ContextMap::InSubElement {
                            path: Default::default(),
                            parent: level.get(),
                        };
                    }
                    Some((_, ContextMap::InSubElement { parent, .. })) => {
                        *parent += level.get();
                    }
                    _ => {}
                }
                ret
            }
            PropertyReference::Function { .. } | PropertyReference::GlobalFunction { .. } => {
                unreachable!()
            }
        }
    }

    pub fn current_sub_component(&self) -> Option<&super::SubComponent> {
        self.current_sub_component.and_then(|i| self.compilation_unit.sub_components.get(i))
    }

    pub fn current_global(&self) -> Option<&super::GlobalComponent> {
        self.current_global.and_then(|i| self.compilation_unit.globals.get(i))
    }
}

impl<T> TypeResolutionContext for EvaluationContext<'_, T> {
    fn property_ty(&self, prop: &PropertyReference) -> &Type {
        match prop {
            PropertyReference::Local { sub_component_path, property_index } => {
                if let Some(mut sub_component) = self.current_sub_component() {
                    for i in sub_component_path {
                        sub_component = &self.compilation_unit.sub_components
                            [sub_component.sub_components[*i].ty];
                    }
                    &sub_component.properties[*property_index].ty
                } else if let Some(current_global) = self.current_global() {
                    &current_global.properties[*property_index].ty
                } else {
                    unreachable!()
                }
            }
            PropertyReference::InNativeItem { sub_component_path, item_index, prop_name } => {
                if prop_name == "elements" {
                    // The `Path::elements` property is not in the NativeClass
                    return &Type::PathData;
                }

                let mut sub_component = self.current_sub_component().unwrap();
                for i in sub_component_path {
                    sub_component =
                        &self.compilation_unit.sub_components[sub_component.sub_components[*i].ty];
                }

                sub_component.items[*item_index].ty.lookup_property(prop_name).unwrap()
            }
            PropertyReference::InParent { level, parent_reference } => {
                let mut ctx = self;
                for _ in 0..level.get() {
                    ctx = ctx.parent.as_ref().unwrap().ctx;
                }
                ctx.property_ty(parent_reference)
            }
            PropertyReference::Global { global_index, property_index } => {
                &self.compilation_unit.globals[*global_index].properties[*property_index].ty
            }
            PropertyReference::Function { sub_component_path, function_index } => {
                if let Some(mut sub_component) = self.current_sub_component() {
                    for i in sub_component_path {
                        sub_component = &self.compilation_unit.sub_components
                            [sub_component.sub_components[*i].ty];
                    }
                    &sub_component.functions[*function_index].ret_ty
                } else if let Some(current_global) = self.current_global() {
                    &current_global.functions[*function_index].ret_ty
                } else {
                    unreachable!()
                }
            }
            PropertyReference::GlobalFunction { global_index, function_index } => {
                &self.compilation_unit.globals[*global_index].functions[*function_index].ret_ty
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
    pub property_decl: Option<&'a super::Property>,
}

/// Maps between two evaluation context.
/// This allows to go from the current subcomponent's context, to the context
/// relative to the binding we want to inline
#[derive(Debug, Clone)]
pub(crate) enum ContextMap {
    Identity,
    InSubElement { path: Vec<SubComponentInstanceIdx>, parent: usize },
    InGlobal(GlobalIdx),
}

impl ContextMap {
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

    pub fn map_property_reference(&self, p: &PropertyReference) -> PropertyReference {
        match self {
            ContextMap::Identity => p.clone(),
            ContextMap::InSubElement { path, parent } => {
                let map_sub_path = |sub_component_path: &[SubComponentInstanceIdx]| -> Vec<SubComponentInstanceIdx> {
                    path.iter().chain(sub_component_path.iter()).copied().collect()
                };

                let p2 = match p {
                    PropertyReference::Local { sub_component_path, property_index } => {
                        PropertyReference::Local {
                            sub_component_path: map_sub_path(sub_component_path),
                            property_index: *property_index,
                        }
                    }
                    PropertyReference::Function { sub_component_path, function_index } => {
                        PropertyReference::Function {
                            sub_component_path: map_sub_path(sub_component_path),
                            function_index: *function_index,
                        }
                    }
                    PropertyReference::InNativeItem {
                        sub_component_path,
                        item_index,
                        prop_name,
                    } => PropertyReference::InNativeItem {
                        item_index: *item_index,
                        prop_name: prop_name.clone(),
                        sub_component_path: map_sub_path(sub_component_path),
                    },
                    PropertyReference::InParent { level, parent_reference } => {
                        return PropertyReference::InParent {
                            level: (parent + level.get()).try_into().unwrap(),
                            parent_reference: parent_reference.clone(),
                        }
                    }
                    PropertyReference::Global { .. } | PropertyReference::GlobalFunction { .. } => {
                        return p.clone()
                    }
                };
                if let Some(level) = NonZeroUsize::new(*parent) {
                    PropertyReference::InParent { level, parent_reference: p2.into() }
                } else {
                    p2
                }
            }
            ContextMap::InGlobal(global_index) => match p {
                PropertyReference::Local { sub_component_path, property_index } => {
                    assert!(sub_component_path.is_empty());
                    PropertyReference::Global {
                        global_index: *global_index,
                        property_index: *property_index,
                    }
                }
                g @ PropertyReference::Global { .. } => g.clone(),
                _ => unreachable!(),
            },
        }
    }

    pub fn map_expression(&self, e: &mut Expression) {
        match e {
            Expression::PropertyReference(p)
            | Expression::CallBackCall { callback: p, .. }
            | Expression::PropertyAssignment { property: p, .. }
            | Expression::LayoutCacheAccess { layout_cache_prop: p, .. } => {
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
                let mut ctx = ctx;
                for _ in 0..*parent {
                    ctx = ctx.parent.unwrap().ctx;
                }
                if path.is_empty() {
                    ctx.clone()
                } else {
                    let mut e = ctx.current_sub_component.unwrap();
                    for i in path {
                        e = ctx.compilation_unit.sub_components[e].sub_components[*i].ty;
                    }
                    EvaluationContext::new_sub_component(ctx.compilation_unit, e, (), None)
                }
            }
            ContextMap::InGlobal(g) => EvaluationContext::new_global(ctx.compilation_unit, *g, ()),
        }
    }
}
