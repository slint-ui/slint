// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{
    GlobalIdx, LocalMemberIndex, LocalMemberReference, MemberReference, RepeatedElementIdx,
    SubComponentIdx, SubComponentInstanceIdx,
};
use crate::expression_tree::{BuiltinFunction, MinMaxOp, OperatorClass};
use crate::langtype::Type;
use crate::layout::Orientation;
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

    MouseCursor(crate::expression_tree::MouseCursor),

    LinearGradient {
        angle: Box<Expression>,
        /// First expression in the tuple is a color, second expression is the stop position
        stops: Vec<(Expression, Expression)>,
    },

    RadialGradient {
        /// First expression in the tuple is a color, second expression is the stop position
        stops: Vec<(Expression, Expression)>,
    },

    ConicGradient {
        /// The starting angle (rotation) of the gradient, corresponding to CSS `from <angle>`
        from_angle: Box<Expression>,
        /// First expression in the tuple is a color, second expression is the stop position (normalized angle 0-1)
        stops: Vec<(Expression, Expression)>,
    },

    EnumerationValue(crate::langtype::EnumerationValue),

    LayoutCacheAccess {
        layout_cache_prop: MemberReference,
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
            Type::Cursor => Expression::MouseCursor(crate::expression_tree::MouseCursor::default()),
            Type::Brush => Expression::Cast {
                from: Box::new(Expression::default_value_for_type(&Type::Color)?),
                to: Type::Brush,
            },
            Type::Enumeration(enumeration) => {
                Expression::EnumerationValue(enumeration.clone().default_value())
            }
            Type::ComponentFactory => Expression::EmptyComponentFactory,
            Type::StyledText => return None,
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
            Self::MouseCursor(_) => Type::Cursor,
            Self::LinearGradient { .. } => Type::Brush,
            Self::RadialGradient { .. } => Type::Brush,
            Self::ConicGradient { .. } => Type::Brush,
            Self::EnumerationValue(e) => Type::Enumeration(e.enumeration.clone()),
            Self::LayoutCacheAccess { .. } => Type::LogicalLength,
            Self::BoxLayoutFunction { sub_expression, .. } => sub_expression.ty(ctx),
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
            Expression::MouseCursor(_) => {}
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
            Expression::ConicGradient { from_angle, stops } => {
                $visitor(from_angle);
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
            let binding = g.init_values.get(r).map(|b| (b, map));
            match r {
                LocalMemberIndex::Property(index) => {
                    let property_decl = &g.properties[*index];
                    PropertyInfoResult {
                        analysis: Some(&g.prop_analysis[*index]),
                        binding,
                        animation: None,
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
                            let p = parent.unwrap();
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
                }
            }
            MemberReference::Global { global_index, member } => {
                let g = &self.compilation_unit.globals[*global_index];
                in_global(g, member, ContextMap::InGlobal(*global_index))
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
                LocalMemberIndex::Native { .. } => unreachable!(),
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
            LocalMemberIndex::Native { item_index, prop_name } => {
                if prop_name == "elements" {
                    // The `Path::elements` property is not in the NativeClass
                    return &Type::PathData;
                }
                sc.items[*item_index].ty.lookup_property(prop_name).unwrap()
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
                    LocalMemberIndex::Native { .. } => unreachable!(),
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
/// This allows to go from the current subcomponent's context, to the context
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
