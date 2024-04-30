// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

//! Helper to do lookup in expressions

use std::rc::Rc;

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{
    BuiltinFunction, BuiltinMacroFunction, EasingCurve, Expression, Unit,
};
use crate::langtype::{ElementType, Enumeration, EnumerationValue, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::{ElementRc, PropertyVisibility};
use crate::parser::NodeOrToken;
use crate::typeregister::TypeRegister;
use std::cell::RefCell;

/// Contains information which allow to lookup identifier in expressions
pub struct LookupCtx<'a> {
    /// the name of the property for which this expression refers.
    pub property_name: Option<&'a str>,

    /// the type of the property for which this expression refers.
    /// (some property come in the scope)
    pub property_type: Type,

    /// Here is the stack in which id applies. (the last element in the scope is looked up first)
    pub component_scope: &'a [ElementRc],

    /// Somewhere to report diagnostics
    pub diag: &'a mut BuildDiagnostics,

    /// The name of the arguments of the callback or function
    pub arguments: Vec<String>,

    /// The type register in which to look for Globals
    pub type_register: &'a TypeRegister,

    /// The type loader instance, which may be used to resolve relative path references
    /// for example for img!
    pub type_loader: Option<&'a crate::typeloader::TypeLoader>,

    /// The token currently processed
    pub current_token: Option<NodeOrToken>,
}

impl<'a> LookupCtx<'a> {
    /// Return a context that is just suitable to build simple const expression
    pub fn empty_context(type_register: &'a TypeRegister, diag: &'a mut BuildDiagnostics) -> Self {
        Self {
            property_name: Default::default(),
            property_type: Default::default(),
            component_scope: Default::default(),
            diag,
            arguments: Default::default(),
            type_register,
            type_loader: None,
            current_token: None,
        }
    }

    pub fn return_type(&self) -> &Type {
        match &self.property_type {
            Type::Callback { return_type, .. } => {
                return_type.as_ref().map_or(&Type::Void, |b| &(**b))
            }
            Type::Function { return_type, .. } => return_type,
            _ => &self.property_type,
        }
    }

    pub fn is_legacy_component(&self) -> bool {
        self.component_scope.first().map_or(false, |e| e.borrow().is_legacy_syntax)
    }

    /// True if the element is in the same component as the scope
    pub fn is_local_element(&self, elem: &ElementRc) -> bool {
        Option::zip(
            elem.borrow().enclosing_component.upgrade(),
            self.component_scope.first().and_then(|x| x.borrow().enclosing_component.upgrade()),
        )
        .map_or(true, |(x, y)| Rc::ptr_eq(&x, &y))
    }
}

#[derive(Debug)]
pub enum LookupResult {
    Expression {
        expression: Expression,
        /// When set, this is deprecated, and the string is the new name
        deprecated: Option<String>,
    },
    Enumeration(Rc<Enumeration>),
    Namespace(BuiltinNamespace),
}

#[derive(Debug)]
pub enum BuiltinNamespace {
    Colors,
    Math,
    Key,
    SlintInternal,
}

impl From<Expression> for LookupResult {
    fn from(expression: Expression) -> Self {
        Self::Expression { expression, deprecated: None }
    }
}

impl LookupResult {
    pub fn deprecated(&self) -> Option<&str> {
        match self {
            Self::Expression { deprecated: Some(x), .. } => Some(x.as_str()),
            _ => None,
        }
    }
}

/// Represent an object which has properties which can be accessible
pub trait LookupObject {
    /// Will call the function for each entry (useful for completion)
    /// If the function return Some, it will immediately be returned and not called further
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R>;

    /// Perform a lookup of a given identifier.
    /// One does not have to re-implement unless we can make it faster
    fn lookup(&self, ctx: &LookupCtx, name: &str) -> Option<LookupResult> {
        self.for_each_entry(ctx, &mut |prop, expr| (prop == name).then_some(expr))
    }
}

impl<T1: LookupObject, T2: LookupObject> LookupObject for (T1, T2) {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        self.0.for_each_entry(ctx, f).or_else(|| self.1.for_each_entry(ctx, f))
    }

    fn lookup(&self, ctx: &LookupCtx, name: &str) -> Option<LookupResult> {
        self.0.lookup(ctx, name).or_else(|| self.1.lookup(ctx, name))
    }
}

impl LookupObject for LookupResult {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        match self {
            LookupResult::Expression { expression, .. } => expression.for_each_entry(ctx, f),
            LookupResult::Enumeration(e) => e.for_each_entry(ctx, f),
            LookupResult::Namespace(BuiltinNamespace::Colors) => {
                (ColorSpecific, ColorFunctions).for_each_entry(ctx, f)
            }
            LookupResult::Namespace(BuiltinNamespace::Math) => MathFunctions.for_each_entry(ctx, f),
            LookupResult::Namespace(BuiltinNamespace::Key) => KeysLookup.for_each_entry(ctx, f),
            LookupResult::Namespace(BuiltinNamespace::SlintInternal) => {
                SlintInternal.for_each_entry(ctx, f)
            }
        }
    }

    fn lookup(&self, ctx: &LookupCtx, name: &str) -> Option<LookupResult> {
        match self {
            LookupResult::Expression { expression, .. } => expression.lookup(ctx, name),
            LookupResult::Enumeration(e) => e.lookup(ctx, name),
            LookupResult::Namespace(BuiltinNamespace::Colors) => {
                (ColorSpecific, ColorFunctions).lookup(ctx, name)
            }
            LookupResult::Namespace(BuiltinNamespace::Math) => MathFunctions.lookup(ctx, name),
            LookupResult::Namespace(BuiltinNamespace::Key) => KeysLookup.lookup(ctx, name),
            LookupResult::Namespace(BuiltinNamespace::SlintInternal) => {
                SlintInternal.lookup(ctx, name)
            }
        }
    }
}

struct ArgumentsLookup;
impl LookupObject for ArgumentsLookup {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let args = match &ctx.property_type {
            Type::Callback { args, .. } | Type::Function { args, .. } => args,
            _ => return None,
        };
        for (index, (name, ty)) in ctx.arguments.iter().zip(args.iter()).enumerate() {
            if let Some(r) =
                f(name, Expression::FunctionParameterReference { index, ty: ty.clone() }.into())
            {
                return Some(r);
            }
        }
        None
    }
}

struct SpecialIdLookup;
impl LookupObject for SpecialIdLookup {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let last = ctx.component_scope.last();
        None.or_else(|| f("self", Expression::ElementReference(Rc::downgrade(last?)).into()))
            .or_else(|| {
                let len = ctx.component_scope.len();
                if len >= 2 {
                    f(
                        "parent",
                        Expression::ElementReference(Rc::downgrade(&ctx.component_scope[len - 2]))
                            .into(),
                    )
                } else {
                    None
                }
            })
            .or_else(|| f("true", Expression::BoolLiteral(true).into()))
            .or_else(|| f("false", Expression::BoolLiteral(false).into()))
        // "root" is just a normal id
    }
}

struct IdLookup;
impl LookupObject for IdLookup {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        fn visit<R>(
            root: &ElementRc,
            f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
        ) -> Option<R> {
            if !root.borrow().id.is_empty() {
                if let Some(r) =
                    f(&root.borrow().id, Expression::ElementReference(Rc::downgrade(root)).into())
                {
                    return Some(r);
                }
            }
            for x in &root.borrow().children {
                if x.borrow().repeated.is_some() {
                    continue;
                }
                if let Some(r) = visit(x, f) {
                    return Some(r);
                }
            }
            None
        }
        for e in ctx.component_scope.iter().rev() {
            if e.borrow().repeated.is_some() {
                if let Some(r) = visit(e, f) {
                    return Some(r);
                }
            }
        }
        if let Some(root) = ctx.component_scope.first() {
            if let Some(r) = visit(root, f) {
                return Some(r);
            }
        }
        None
    }
    // TODO: hash based lookup
}

/// In-scope properties, or model
pub struct InScopeLookup;
impl InScopeLookup {
    fn visit_scope<R>(
        ctx: &LookupCtx,
        mut visit_entry: impl FnMut(&str, LookupResult) -> Option<R>,
        mut visit_legacy_scope: impl FnMut(&ElementRc) -> Option<R>,
        mut visit_scope: impl FnMut(&ElementRc) -> Option<R>,
    ) -> Option<R> {
        let is_legacy = ctx.is_legacy_component();
        for (idx, elem) in ctx.component_scope.iter().rev().enumerate() {
            if let Some(repeated) = &elem.borrow().repeated {
                if !repeated.index_id.is_empty() {
                    if let Some(r) = visit_entry(
                        &repeated.index_id,
                        Expression::RepeaterIndexReference { element: Rc::downgrade(elem) }.into(),
                    ) {
                        return Some(r);
                    }
                }
                if !repeated.model_data_id.is_empty() {
                    if let Some(r) = visit_entry(
                        &repeated.model_data_id,
                        Expression::RepeaterModelReference { element: Rc::downgrade(elem) }.into(),
                    ) {
                        return Some(r);
                    }
                }
            }

            if is_legacy {
                if elem.borrow().repeated.is_some()
                    || idx == 0
                    || idx == ctx.component_scope.len() - 1
                {
                    if let Some(r) = visit_legacy_scope(elem) {
                        return Some(r);
                    }
                }
            } else if let Some(r) = visit_scope(elem) {
                return Some(r);
            }
        }
        None
    }
}
impl LookupObject for InScopeLookup {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let f = RefCell::new(f);
        Self::visit_scope(
            ctx,
            |str, r| f.borrow_mut()(str, r),
            |elem| elem.for_each_entry(ctx, *f.borrow_mut()),
            |elem| {
                for (name, prop) in &elem.borrow().property_declarations {
                    let e = expression_from_reference(
                        NamedReference::new(elem, name),
                        &prop.property_type,
                        &ctx.current_token,
                    );
                    if let Some(r) = f.borrow_mut()(name, e.into()) {
                        return Some(r);
                    }
                }
                None
            },
        )
    }

    fn lookup(&self, ctx: &LookupCtx, name: &str) -> Option<LookupResult> {
        if name.is_empty() {
            return None;
        }
        Self::visit_scope(
            ctx,
            |str, r| (str == name).then_some(r),
            |elem| elem.lookup(ctx, name),
            |elem| {
                elem.borrow().property_declarations.get(name).map(|prop| {
                    expression_from_reference(
                        NamedReference::new(elem, name),
                        &prop.property_type,
                        &ctx.current_token,
                    )
                    .into()
                })
            },
        )
    }
}

impl LookupObject for ElementRc {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        for (name, prop) in &self.borrow().property_declarations {
            let e = expression_from_reference(
                NamedReference::new(self, name),
                &prop.property_type,
                &ctx.current_token,
            );
            if let Some(r) = f(name, e.into()) {
                return Some(r);
            }
        }
        let list = self.borrow().base_type.property_list();
        for (name, ty) in list {
            let e = expression_from_reference(
                NamedReference::new(self, &name),
                &ty,
                &ctx.current_token,
            );
            if let Some(r) = f(&name, e.into()) {
                return Some(r);
            }
        }
        if !matches!(self.borrow().base_type, ElementType::Global) {
            for (name, ty, _) in crate::typeregister::reserved_properties() {
                let e = expression_from_reference(
                    NamedReference::new(self, name),
                    &ty,
                    &ctx.current_token,
                );
                if let Some(r) = f(name, e.into()) {
                    return Some(r);
                }
            }
        }
        None
    }

    fn lookup(&self, ctx: &LookupCtx, name: &str) -> Option<LookupResult> {
        let lookup_result = self.borrow().lookup_property(name);
        if lookup_result.property_type != Type::Invalid
            && (lookup_result.is_local_to_component
                || lookup_result.property_visibility != PropertyVisibility::Private)
        {
            Some(LookupResult::Expression {
                expression: expression_from_reference(
                    NamedReference::new(self, &lookup_result.resolved_name),
                    &lookup_result.property_type,
                    &ctx.current_token,
                ),
                deprecated: (lookup_result.resolved_name != name)
                    .then(|| lookup_result.resolved_name.to_string()),
            })
        } else {
            None
        }
    }
}

fn expression_from_reference(
    n: NamedReference,
    ty: &Type,
    node: &Option<NodeOrToken>,
) -> Expression {
    match ty {
        Type::Callback { .. } => Expression::CallbackReference(n, node.clone()),
        Type::InferredCallback => Expression::CallbackReference(n, node.clone()),
        Type::Function { .. } => Expression::FunctionReference(n, node.clone()),
        _ => Expression::PropertyReference(n),
    }
}

/// Lookup for Globals and Enum.
struct LookupType;
impl LookupObject for LookupType {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        for (name, ty) in ctx.type_register.all_types() {
            if let Some(r) = Self::from_type(ty).and_then(|e| f(&name, e)) {
                return Some(r);
            }
        }
        for (name, ty) in ctx.type_register.all_elements() {
            if let Some(r) = Self::from_element(ty, ctx, &name).and_then(|e| f(&name, e)) {
                return Some(r);
            }
        }
        None
    }

    fn lookup(&self, ctx: &LookupCtx, name: &str) -> Option<LookupResult> {
        Self::from_type(ctx.type_register.lookup(name))
            .or_else(|| Self::from_element(ctx.type_register.lookup_element(name).ok()?, ctx, name))
    }
}
impl LookupType {
    fn from_type(ty: Type) -> Option<LookupResult> {
        match ty {
            Type::Enumeration(e) => Some(LookupResult::Enumeration(e)),
            _ => None,
        }
    }

    fn from_element(el: ElementType, ctx: &LookupCtx, name: &str) -> Option<LookupResult> {
        match el {
            ElementType::Component(c) if c.is_global() => {
                // Check if it is internal, but allow re-export (different name) eg: NativeStyleMetrics re-exported as StyleMetrics
                if c.root_element
                    .borrow()
                    .builtin_type()
                    .map_or(false, |x| x.is_internal && x.name == name)
                    && !ctx.type_register.expose_internal_types
                {
                    None
                } else {
                    return Some(LookupResult::Expression {
                        expression: Expression::ElementReference(Rc::downgrade(&c.root_element)),
                        deprecated: (name == "StyleMetrics"
                            && !ctx.type_register.expose_internal_types)
                            .then(|| "Palette".to_string()),
                    });
                }
            }
            _ => None,
        }
    }
}

/// Lookup for things specific to the return type (eg: colors or enums)
pub struct ReturnTypeSpecificLookup;
impl LookupObject for ReturnTypeSpecificLookup {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        match ctx.return_type() {
            Type::Color => ColorSpecific.for_each_entry(ctx, f),
            Type::Brush => ColorSpecific.for_each_entry(ctx, f),
            Type::Easing => EasingSpecific.for_each_entry(ctx, f),
            Type::Enumeration(enumeration) => enumeration.clone().for_each_entry(ctx, f),
            _ => None,
        }
    }

    fn lookup(&self, ctx: &LookupCtx, name: &str) -> Option<LookupResult> {
        match ctx.return_type() {
            Type::Color => ColorSpecific.lookup(ctx, name),
            Type::Brush => ColorSpecific.lookup(ctx, name),
            Type::Easing => EasingSpecific.lookup(ctx, name),
            Type::Enumeration(enumeration) => enumeration.clone().lookup(ctx, name),
            _ => None,
        }
    }
}

struct ColorSpecific;
impl LookupObject for ColorSpecific {
    fn for_each_entry<R>(
        &self,
        _ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        for (name, c) in css_color_parser2::NAMED_COLORS.iter() {
            if let Some(r) = f(name, Self::as_result(*c)) {
                return Some(r);
            }
        }
        None
    }
    fn lookup(&self, _ctx: &LookupCtx, name: &str) -> Option<LookupResult> {
        css_color_parser2::NAMED_COLORS.get(name).map(|c| Self::as_result(*c))
    }
}
impl ColorSpecific {
    fn as_result(c: css_color_parser2::Color) -> LookupResult {
        let value =
            ((c.a as u32 * 255) << 24) | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
        Expression::Cast {
            from: Box::new(Expression::NumberLiteral(value as f64, Unit::None)),
            to: Type::Color,
        }
        .into()
    }
}

struct KeysLookup;

macro_rules! special_keys_lookup {
    ($($char:literal # $name:ident # $($qt:ident)|* # $($winit:ident $(($_pos:ident))?)|* # $($_xkb:ident)|*;)*) => {
        impl LookupObject for KeysLookup {
            fn for_each_entry<R>(
                &self,
                _ctx: &LookupCtx,
                f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
            ) -> Option<R> {
                None
                $(.or_else(|| {
                    f(stringify!($name), Expression::StringLiteral($char.into()).into())
                }))*
            }
        }
    };
}

i_slint_common::for_each_special_keys!(special_keys_lookup);

struct EasingSpecific;
impl LookupObject for EasingSpecific {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        use EasingCurve::CubicBezier;
        None.or_else(|| f("linear", Expression::EasingCurve(EasingCurve::Linear).into()))
            .or_else(|| {
                f("ease-in-quad", Expression::EasingCurve(CubicBezier(0.11, 0.0, 0.5, 0.0)).into())
            })
            .or_else(|| {
                f("ease-out-quad", Expression::EasingCurve(CubicBezier(0.5, 1.0, 0.89, 1.0)).into())
            })
            .or_else(|| {
                f(
                    "ease-in-out-quad",
                    Expression::EasingCurve(CubicBezier(0.45, 0.0, 0.55, 1.0)).into(),
                )
            })
            .or_else(|| {
                f("ease", Expression::EasingCurve(CubicBezier(0.25, 0.1, 0.25, 1.0)).into())
            })
            .or_else(|| {
                f("ease-in", Expression::EasingCurve(CubicBezier(0.42, 0.0, 1.0, 1.0)).into())
            })
            .or_else(|| {
                f("ease-in-out", Expression::EasingCurve(CubicBezier(0.42, 0.0, 0.58, 1.0)).into())
            })
            .or_else(|| {
                f("ease-out", Expression::EasingCurve(CubicBezier(0.0, 0.0, 0.58, 1.0)).into())
            })
            .or_else(|| {
                f("ease-in-quart", Expression::EasingCurve(CubicBezier(0.5, 0.0, 0.75, 0.0)).into())
            })
            .or_else(|| {
                f(
                    "ease-out-quart",
                    Expression::EasingCurve(CubicBezier(0.25, 1.0, 0.5, 1.0)).into(),
                )
            })
            .or_else(|| {
                f(
                    "ease-in-out-quart",
                    Expression::EasingCurve(CubicBezier(0.76, 0.0, 0.24, 1.0)).into(),
                )
            })
            .or_else(|| {
                f(
                    "ease-in-quint",
                    Expression::EasingCurve(CubicBezier(0.64, 0.0, 0.78, 0.0)).into(),
                )
            })
            .or_else(|| {
                f(
                    "ease-out-quint",
                    Expression::EasingCurve(CubicBezier(0.22, 1.0, 0.36, 1.0)).into(),
                )
            })
            .or_else(|| {
                f(
                    "ease-in-out-quint",
                    Expression::EasingCurve(CubicBezier(0.83, 0.0, 0.17, 1.0)).into(),
                )
            })
            .or_else(|| {
                f("ease-in-expo", Expression::EasingCurve(CubicBezier(0.7, 0.0, 0.84, 0.0)).into())
            })
            .or_else(|| {
                f("ease-out-expo", Expression::EasingCurve(CubicBezier(0.16, 1.0, 0.3, 1.0)).into())
            })
            .or_else(|| {
                f(
                    "ease-in-out-expo",
                    Expression::EasingCurve(CubicBezier(0.87, 0.0, 0.13, 1.0)).into(),
                )
            })
            .or_else(|| {
                f(
                    "ease-in-back",
                    Expression::EasingCurve(CubicBezier(0.36, 0.0, 0.66, -0.56)).into(),
                )
            })
            .or_else(|| {
                f(
                    "ease-out-back",
                    Expression::EasingCurve(CubicBezier(0.34, 1.56, 0.64, 1.0)).into(),
                )
            })
            .or_else(|| {
                f(
                    "ease-in-out-back",
                    Expression::EasingCurve(CubicBezier(0.68, -0.6, 0.32, 1.6)).into(),
                )
            })
            .or_else(|| {
                f("ease-in-sine", Expression::EasingCurve(CubicBezier(0.12, 0.0, 0.39, 0.0)).into())
            })
            .or_else(|| {
                f(
                    "ease-out-sine",
                    Expression::EasingCurve(CubicBezier(0.61, 1.0, 0.88, 1.0)).into(),
                )
            })
            .or_else(|| {
                f(
                    "ease-in-out-sine",
                    Expression::EasingCurve(CubicBezier(0.37, 0.0, 0.63, 1.0)).into(),
                )
            })
            .or_else(|| {
                f("ease-in-circ", Expression::EasingCurve(CubicBezier(0.55, 0.0, 1.0, 0.45)).into())
            })
            .or_else(|| {
                f(
                    "ease-out-circ",
                    Expression::EasingCurve(CubicBezier(0.0, 0.55, 0.45, 1.0)).into(),
                )
            })
            .or_else(|| {
                f(
                    "ease-in-out-circ",
                    Expression::EasingCurve(CubicBezier(0.85, 0.0, 0.15, 1.0)).into(),
                )
            })
            .or_else(|| {
                f(
                    "cubic-bezier",
                    Expression::BuiltinMacroReference(
                        BuiltinMacroFunction::CubicBezier,
                        ctx.current_token.clone(),
                    )
                    .into(),
                )
            })
            .or_else(|| {
                f("ease-in-elastic", Expression::EasingCurve(EasingCurve::EaseInElastic).into())
            })
            .or_else(|| {
                f("ease-out-elastic", Expression::EasingCurve(EasingCurve::EaseOutElastic).into())
            })
            .or_else(|| {
                f(
                    "ease-in-out-elastic",
                    Expression::EasingCurve(EasingCurve::EaseInOutElastic).into(),
                )
            })
            .or_else(|| {
                f("ease-in-bounce", Expression::EasingCurve(EasingCurve::EaseInBounce).into())
            })
            .or_else(|| {
                f("ease-out-bounce", Expression::EasingCurve(EasingCurve::EaseOutBounce).into())
            })
            .or_else(|| {
                f(
                    "ease-in-out-bounce",
                    Expression::EasingCurve(EasingCurve::EaseInOutBounce).into(),
                )
            })
    }
}

impl LookupObject for Rc<Enumeration> {
    fn for_each_entry<R>(
        &self,
        _ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        for (value, name) in self.values.iter().enumerate() {
            if let Some(r) = f(
                name,
                Expression::EnumerationValue(EnumerationValue { value, enumeration: self.clone() })
                    .into(),
            ) {
                return Some(r);
            }
        }
        None
    }
}

struct MathFunctions;
impl LookupObject for MathFunctions {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        use Expression::{BuiltinFunctionReference, BuiltinMacroReference};
        let t = &ctx.current_token;
        let sl = || t.as_ref().map(|t| t.to_source_location());
        let mut f = |n, e: Expression| f(n, e.into());
        None.or_else(|| f("mod", BuiltinMacroReference(BuiltinMacroFunction::Mod, t.clone())))
            .or_else(|| f("round", BuiltinFunctionReference(BuiltinFunction::Round, sl())))
            .or_else(|| f("ceil", BuiltinFunctionReference(BuiltinFunction::Ceil, sl())))
            .or_else(|| f("floor", BuiltinFunctionReference(BuiltinFunction::Floor, sl())))
            .or_else(|| f("clamp", BuiltinMacroReference(BuiltinMacroFunction::Clamp, t.clone())))
            .or_else(|| f("abs", BuiltinFunctionReference(BuiltinFunction::Abs, sl())))
            .or_else(|| f("sqrt", BuiltinFunctionReference(BuiltinFunction::Sqrt, sl())))
            .or_else(|| f("max", BuiltinMacroReference(BuiltinMacroFunction::Max, t.clone())))
            .or_else(|| f("min", BuiltinMacroReference(BuiltinMacroFunction::Min, t.clone())))
            .or_else(|| f("sin", BuiltinFunctionReference(BuiltinFunction::Sin, sl())))
            .or_else(|| f("cos", BuiltinFunctionReference(BuiltinFunction::Cos, sl())))
            .or_else(|| f("tan", BuiltinFunctionReference(BuiltinFunction::Tan, sl())))
            .or_else(|| f("asin", BuiltinFunctionReference(BuiltinFunction::ASin, sl())))
            .or_else(|| f("acos", BuiltinFunctionReference(BuiltinFunction::ACos, sl())))
            .or_else(|| f("atan", BuiltinFunctionReference(BuiltinFunction::ATan, sl())))
            .or_else(|| f("log", BuiltinFunctionReference(BuiltinFunction::Log, sl())))
            .or_else(|| f("pow", BuiltinFunctionReference(BuiltinFunction::Pow, sl())))
    }
}

struct SlintInternal;
impl LookupObject for SlintInternal {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        f(
            "color-scheme",
            Expression::FunctionCall {
                function: Expression::BuiltinFunctionReference(BuiltinFunction::ColorScheme, None)
                    .into(),
                arguments: vec![],
                source_location: ctx.current_token.as_ref().map(|t| t.to_source_location()),
            }
            .into(),
        )
    }
}

struct ColorFunctions;
impl LookupObject for ColorFunctions {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        use Expression::BuiltinMacroReference;
        let t = &ctx.current_token;
        let mut f = |n, e: Expression| f(n, e.into());
        None.or_else(|| f("rgb", BuiltinMacroReference(BuiltinMacroFunction::Rgb, t.clone())))
            .or_else(|| f("rgba", BuiltinMacroReference(BuiltinMacroFunction::Rgb, t.clone())))
            .or_else(|| f("hsv", BuiltinMacroReference(BuiltinMacroFunction::Hsv, t.clone())))
    }
}

struct BuiltinFunctionLookup;
impl LookupObject for BuiltinFunctionLookup {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        (MathFunctions, ColorFunctions)
            .for_each_entry(ctx, f)
            .or_else(|| {
                f(
                    "debug",
                    Expression::BuiltinMacroReference(
                        BuiltinMacroFunction::Debug,
                        ctx.current_token.clone(),
                    )
                    .into(),
                )
            })
            .or_else(|| {
                f(
                    "animation-tick",
                    Expression::BuiltinFunctionReference(
                        BuiltinFunction::AnimationTick,
                        ctx.current_token.as_ref().map(|t| t.to_source_location()),
                    )
                    .into(),
                )
            })
    }
}

struct BuiltinNamespaceLookup;
impl LookupObject for BuiltinNamespaceLookup {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        None.or_else(|| f("Colors", LookupResult::Namespace(BuiltinNamespace::Colors)))
            .or_else(|| f("Math", LookupResult::Namespace(BuiltinNamespace::Math)))
            .or_else(|| f("Key", LookupResult::Namespace(BuiltinNamespace::Key)))
            .or_else(|| {
                if ctx.type_register.expose_internal_types {
                    f("SlintInternal", LookupResult::Namespace(BuiltinNamespace::SlintInternal))
                } else {
                    None
                }
            })
    }
}

pub fn global_lookup() -> impl LookupObject {
    (
        ArgumentsLookup,
        (
            SpecialIdLookup,
            (
                IdLookup,
                (
                    InScopeLookup,
                    (
                        LookupType,
                        (BuiltinNamespaceLookup, (ReturnTypeSpecificLookup, BuiltinFunctionLookup)),
                    ),
                ),
            ),
        ),
    )
}

impl LookupObject for Expression {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        match self {
            Expression::ElementReference(e) => e.upgrade().unwrap().for_each_entry(ctx, f),
            _ => match self.ty() {
                Type::Struct { fields, .. } => {
                    for name in fields.keys() {
                        if let Some(r) = f(
                            name,
                            Expression::StructFieldAccess {
                                base: Box::new(self.clone()),
                                name: name.clone(),
                            }
                            .into(),
                        ) {
                            return Some(r);
                        }
                    }
                    None
                }
                Type::String => StringExpression(self).for_each_entry(ctx, f),
                Type::Brush | Type::Color => ColorExpression(self).for_each_entry(ctx, f),
                Type::Image => ImageExpression(self).for_each_entry(ctx, f),
                Type::Array(_) => ArrayExpression(self).for_each_entry(ctx, f),
                _ => None,
            },
        }
    }

    fn lookup(&self, ctx: &LookupCtx, name: &str) -> Option<LookupResult> {
        match self {
            Expression::ElementReference(e) => e.upgrade().unwrap().lookup(ctx, name),
            _ => match self.ty() {
                Type::Struct { fields, .. } => fields.contains_key(name).then(|| {
                    LookupResult::from(Expression::StructFieldAccess {
                        base: Box::new(self.clone()),
                        name: name.to_string(),
                    })
                }),
                Type::String => StringExpression(self).lookup(ctx, name),
                Type::Brush | Type::Color => ColorExpression(self).lookup(ctx, name),
                Type::Image => ImageExpression(self).lookup(ctx, name),
                Type::Array(_) => ArrayExpression(self).lookup(ctx, name),
                _ => None,
            },
        }
    }
}

struct StringExpression<'a>(&'a Expression);
impl<'a> LookupObject for StringExpression<'a> {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let member_function = |f: BuiltinFunction| {
            LookupResult::from(Expression::MemberFunction {
                base: Box::new(self.0.clone()),
                base_node: ctx.current_token.clone(), // Note that this is not the base_node, but the function's node
                member: Box::new(Expression::BuiltinFunctionReference(
                    f,
                    ctx.current_token.as_ref().map(|t| t.to_source_location()),
                )),
            })
        };
        None.or_else(|| f("is-float", member_function(BuiltinFunction::StringIsFloat)))
            .or_else(|| f("to-float", member_function(BuiltinFunction::StringToFloat)))
    }
}
struct ColorExpression<'a>(&'a Expression);
impl<'a> LookupObject for ColorExpression<'a> {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let member_function = |f: BuiltinFunction| {
            let base = if f == BuiltinFunction::ColorHsvaStruct && self.0.ty() == Type::Brush {
                Expression::Cast { from: Box::new(self.0.clone()), to: Type::Color }
            } else {
                self.0.clone()
            };
            LookupResult::from(Expression::MemberFunction {
                base: Box::new(base),
                base_node: ctx.current_token.clone(), // Note that this is not the base_node, but the function's node
                member: Box::new(Expression::BuiltinFunctionReference(
                    f,
                    ctx.current_token.as_ref().map(|t| t.to_source_location()),
                )),
            })
        };
        let field_access = |f: &str| {
            let base = if self.0.ty() == Type::Brush {
                Expression::Cast { from: Box::new(self.0.clone()), to: Type::Color }
            } else {
                self.0.clone()
            };
            LookupResult::from(Expression::StructFieldAccess {
                base: Box::new(Expression::FunctionCall {
                    function: Box::new(Expression::BuiltinFunctionReference(
                        BuiltinFunction::ColorRgbaStruct,
                        ctx.current_token.as_ref().map(|t| t.to_source_location()),
                    )),
                    source_location: ctx.current_token.as_ref().map(|t| t.to_source_location()),
                    arguments: vec![base],
                }),
                name: f.into(),
            })
        };
        None.or_else(|| f("red", field_access("red")))
            .or_else(|| f("green", field_access("green")))
            .or_else(|| f("blue", field_access("blue")))
            .or_else(|| f("alpha", field_access("alpha")))
            .or_else(|| f("to-hsv", member_function(BuiltinFunction::ColorHsvaStruct)))
            .or_else(|| f("brighter", member_function(BuiltinFunction::ColorBrighter)))
            .or_else(|| f("darker", member_function(BuiltinFunction::ColorDarker)))
            .or_else(|| f("transparentize", member_function(BuiltinFunction::ColorTransparentize)))
            .or_else(|| f("with-alpha", member_function(BuiltinFunction::ColorWithAlpha)))
            .or_else(|| f("mix", member_function(BuiltinFunction::ColorMix)))
    }
}

struct ImageExpression<'a>(&'a Expression);
impl<'a> LookupObject for ImageExpression<'a> {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let field_access = |f: &str| {
            LookupResult::from(Expression::StructFieldAccess {
                base: Box::new(Expression::FunctionCall {
                    function: Box::new(Expression::BuiltinFunctionReference(
                        BuiltinFunction::ImageSize,
                        ctx.current_token.as_ref().map(|t| t.to_source_location()),
                    )),
                    source_location: ctx.current_token.as_ref().map(|t| t.to_source_location()),
                    arguments: vec![self.0.clone()],
                }),
                name: f.into(),
            })
        };
        None.or_else(|| f("width", field_access("width")))
            .or_else(|| f("height", field_access("height")))
    }
}

struct ArrayExpression<'a>(&'a Expression);
impl<'a> LookupObject for ArrayExpression<'a> {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&str, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let member_function = |f: BuiltinFunction| {
            LookupResult::from(Expression::FunctionCall {
                function: Box::new(Expression::BuiltinFunctionReference(
                    f,
                    ctx.current_token.as_ref().map(|t| t.to_source_location()),
                )),
                source_location: ctx.current_token.as_ref().map(|t| t.to_source_location()),
                arguments: vec![self.0.clone()],
            })
        };
        None.or_else(|| f("length", member_function(BuiltinFunction::ArrayLength)))
    }
}
