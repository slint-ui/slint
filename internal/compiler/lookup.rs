// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Helper to do lookup in expressions

use std::rc::Rc;

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{
    BuiltinFunction, BuiltinMacroFunction, Callable, EasingCurve, Expression, Unit,
};
use crate::langtype::{ElementType, Enumeration, EnumerationValue, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::{ElementRc, PropertyVisibility};
use crate::parser::NodeOrToken;
use crate::typeregister::TypeRegister;
use smol_str::{SmolStr, ToSmolStr};
use std::cell::RefCell;

mod named_colors;

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
    pub arguments: Vec<SmolStr>,

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
            Type::Callback(f) | Type::Function(f) => &f.return_type,
            _ => &self.property_type,
        }
    }

    pub fn is_legacy_component(&self) -> bool {
        self.component_scope.first().is_some_and(|e| e.borrow().is_legacy_syntax)
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
    Callable(LookupResultCallable),
}

#[derive(Debug)]
pub enum LookupResultCallable {
    Callable(Callable),
    Macro(BuiltinMacroFunction),
    /// for example for `item.focus`, where `item` is the base
    MemberFunction {
        /// This becomes the first argument of the function call
        base: Expression,
        base_node: Option<NodeOrToken>,
        member: Box<LookupResultCallable>,
    },
}

#[derive(Debug, derive_more::Display)]
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
impl From<Callable> for LookupResult {
    fn from(callable: Callable) -> Self {
        Self::Callable(LookupResultCallable::Callable(callable))
    }
}
impl From<BuiltinMacroFunction> for LookupResult {
    fn from(macro_function: BuiltinMacroFunction) -> Self {
        Self::Callable(LookupResultCallable::Macro(macro_function))
    }
}
impl From<BuiltinFunction> for LookupResult {
    fn from(function: BuiltinFunction) -> Self {
        Self::Callable(LookupResultCallable::Callable(Callable::Builtin(function)))
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
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R>;

    /// Perform a lookup of a given identifier.
    /// One does not have to re-implement unless we can make it faster
    fn lookup(&self, ctx: &LookupCtx, name: &SmolStr) -> Option<LookupResult> {
        self.for_each_entry(ctx, &mut |prop, expr| (prop == name).then_some(expr))
    }
}

impl<T1: LookupObject, T2: LookupObject> LookupObject for (T1, T2) {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        self.0.for_each_entry(ctx, f).or_else(|| self.1.for_each_entry(ctx, f))
    }

    fn lookup(&self, ctx: &LookupCtx, name: &SmolStr) -> Option<LookupResult> {
        self.0.lookup(ctx, name).or_else(|| self.1.lookup(ctx, name))
    }
}

impl LookupObject for LookupResult {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
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
            LookupResult::Callable(..) => None,
        }
    }

    fn lookup(&self, ctx: &LookupCtx, name: &SmolStr) -> Option<LookupResult> {
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
            LookupResult::Callable(..) => None,
        }
    }
}

struct ArgumentsLookup;
impl LookupObject for ArgumentsLookup {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let args = match &ctx.property_type {
            Type::Callback(f) | Type::Function(f) => &f.args,
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
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let last = ctx.component_scope.last();
        let mut f = |n, e: Expression| f(&SmolStr::new_static(n), e.into());
        None.or_else(|| f("self", Expression::ElementReference(Rc::downgrade(last?))))
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
            .or_else(|| f("true", Expression::BoolLiteral(true)))
            .or_else(|| f("false", Expression::BoolLiteral(false)))
        // "root" is just a normal id
    }
}

struct IdLookup;
impl LookupObject for IdLookup {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        fn visit<R>(
            root: &ElementRc,
            f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
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
        mut visit_entry: impl FnMut(&SmolStr, LookupResult) -> Option<R>,
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
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let f = RefCell::new(f);
        Self::visit_scope(
            ctx,
            |str, r| f.borrow_mut()(str, r),
            |elem| elem.for_each_entry(ctx, *f.borrow_mut()),
            |elem| {
                for (name, prop) in &elem.borrow().property_declarations {
                    let e = expression_from_reference(
                        NamedReference::new(elem, name.clone()),
                        &prop.property_type,
                        None,
                    );
                    if let Some(r) = f.borrow_mut()(name, e) {
                        return Some(r);
                    }
                }
                None
            },
        )
    }

    fn lookup(&self, ctx: &LookupCtx, name: &SmolStr) -> Option<LookupResult> {
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
                        NamedReference::new(elem, name.clone()),
                        &prop.property_type,
                        None,
                    )
                })
            },
        )
    }
}

impl LookupObject for ElementRc {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        for (name, prop) in &self.borrow().property_declarations {
            let deprecated = match check_deprecated_stylemetrics(self, ctx, name) {
                StyleMetricsPropertyUse::Acceptable => None,
                StyleMetricsPropertyUse::Deprecated(msg) => Some(msg),
                StyleMetricsPropertyUse::Unacceptable => continue, // Skip from lookup
            };

            let r = expression_from_reference(
                NamedReference::new(self, name.clone()),
                &prop.property_type,
                deprecated,
            );
            if let Some(r) = f(name, r) {
                return Some(r);
            }
        }
        let list = self.borrow().base_type.property_list();
        for (name, ty) in list {
            let e = expression_from_reference(NamedReference::new(self, name.clone()), &ty, None);
            if let Some(r) = f(&name, e) {
                return Some(r);
            }
        }
        if !(matches!(self.borrow().base_type, ElementType::Global)) {
            for (name, ty, _) in crate::typeregister::reserved_properties() {
                let name = SmolStr::new_static(name);
                let e =
                    expression_from_reference(NamedReference::new(self, name.clone()), &ty, None);
                if let Some(r) = f(&name, e) {
                    return Some(r);
                }
            }
        }
        None
    }

    fn lookup(&self, ctx: &LookupCtx, name: &SmolStr) -> Option<LookupResult> {
        let lookup_result = self.borrow().lookup_property(name);
        if lookup_result.property_type != Type::Invalid
            && (lookup_result.is_local_to_component
                || lookup_result.property_visibility != PropertyVisibility::Private)
        {
            let deprecated = (lookup_result.resolved_name != name.as_str())
                .then(|| Some(lookup_result.resolved_name.to_string()))
                .or_else(|| match check_deprecated_stylemetrics(self, ctx, name) {
                    StyleMetricsPropertyUse::Acceptable => Some(None),
                    StyleMetricsPropertyUse::Deprecated(msg) => Some(Some(msg)),
                    StyleMetricsPropertyUse::Unacceptable => None,
                })?;
            Some(expression_from_reference(
                NamedReference::new(self, lookup_result.resolved_name.to_smolstr()),
                &lookup_result.property_type,
                deprecated,
            ))
        } else {
            None
        }
    }
}

/// This enum describes the result of checking the use of a property of the StyleMetrics object.
pub enum StyleMetricsPropertyUse {
    /// The property is acceptable for use.
    Acceptable,
    /// The property is acceptable fo use, but it is deprecated. The string provides the name of the
    /// property that should be used instead.
    Deprecated(String),
    /// The property is not acceptable for use, it is internal.
    Unacceptable,
}

pub fn check_deprecated_stylemetrics(
    elem: &ElementRc,
    ctx: &LookupCtx<'_>,
    name: &SmolStr,
) -> StyleMetricsPropertyUse {
    let borrow = elem.borrow();

    let is_style_metrics_prop = matches!(
        borrow.enclosing_component.upgrade().unwrap().id.as_str(),
        "StyleMetrics" | "NativeStyleMetrics"
    );

    (!ctx.type_register.expose_internal_types
        && is_style_metrics_prop
        && borrow
            .debug
            .first()
            .and_then(|x| x.node.source_file())
            .map_or(true, |x| x.path().starts_with("builtin:"))
        && !name.starts_with("layout-"))
    .then(|| format!("Palette.{name}"))
    .map_or(StyleMetricsPropertyUse::Acceptable, |msg| {
        if is_style_metrics_prop && name == "style-name" {
            StyleMetricsPropertyUse::Unacceptable
        } else {
            StyleMetricsPropertyUse::Deprecated(msg)
        }
    })
}

fn expression_from_reference(
    n: NamedReference,
    ty: &Type,
    deprecated: Option<String>,
) -> LookupResult {
    match ty {
        Type::Callback { .. } => Callable::Callback(n).into(),
        Type::InferredCallback => Callable::Callback(n).into(),
        Type::Function { .. } => Callable::Function(n).into(),
        _ => LookupResult::Expression { expression: Expression::PropertyReference(n), deprecated },
    }
}

/// Lookup for Globals and Enum.
struct LookupType;
impl LookupObject for LookupType {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
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

    fn lookup(&self, ctx: &LookupCtx, name: &SmolStr) -> Option<LookupResult> {
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
                    .is_some_and(|x| x.is_internal && x.name == name)
                    && !ctx.type_register.expose_internal_types
                {
                    None
                } else {
                    Some(Expression::ElementReference(Rc::downgrade(&c.root_element)).into())
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
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        match ctx.return_type() {
            Type::Color => ColorSpecific.for_each_entry(ctx, f),
            Type::Brush => ColorSpecific.for_each_entry(ctx, f),
            Type::Easing => EasingSpecific.for_each_entry(ctx, f),
            Type::Enumeration(enumeration) => enumeration.clone().for_each_entry(ctx, f),
            _ => None,
        }
    }

    fn lookup(&self, ctx: &LookupCtx, name: &SmolStr) -> Option<LookupResult> {
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
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        for (name, c) in named_colors::named_colors().iter() {
            if let Some(r) = f(&SmolStr::new_static(name), Self::as_result(*c)) {
                return Some(r);
            }
        }
        None
    }
    fn lookup(&self, _ctx: &LookupCtx, name: &SmolStr) -> Option<LookupResult> {
        named_colors::named_colors().get(name.as_str()).map(|c| Self::as_result(*c))
    }
}
impl ColorSpecific {
    fn as_result(value: u32) -> LookupResult {
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
                f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
            ) -> Option<R> {
                None
                $(.or_else(|| {
                    let mut tmp = [0; 4];
                    f(&SmolStr::new_static(stringify!($name)), Expression::StringLiteral(SmolStr::new_inline($char.encode_utf8(&mut tmp))).into())
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
        _ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        use EasingCurve::CubicBezier;
        let mut curve = |n, e| f(&SmolStr::new_static(n), Expression::EasingCurve(e).into());
        let r = None
            .or_else(|| curve("linear", EasingCurve::Linear))
            .or_else(|| curve("ease-in-quad", CubicBezier(0.11, 0.0, 0.5, 0.0)))
            .or_else(|| curve("ease-out-quad", CubicBezier(0.5, 1.0, 0.89, 1.0)))
            .or_else(|| curve("ease-in-out-quad", CubicBezier(0.45, 0.0, 0.55, 1.0)))
            .or_else(|| curve("ease", CubicBezier(0.25, 0.1, 0.25, 1.0)))
            .or_else(|| curve("ease-in", CubicBezier(0.42, 0.0, 1.0, 1.0)))
            .or_else(|| curve("ease-in-out", CubicBezier(0.42, 0.0, 0.58, 1.0)))
            .or_else(|| curve("ease-out", CubicBezier(0.0, 0.0, 0.58, 1.0)))
            .or_else(|| curve("ease-in-quart", CubicBezier(0.5, 0.0, 0.75, 0.0)))
            .or_else(|| curve("ease-out-quart", CubicBezier(0.25, 1.0, 0.5, 1.0)))
            .or_else(|| curve("ease-in-out-quart", CubicBezier(0.76, 0.0, 0.24, 1.0)))
            .or_else(|| curve("ease-in-quint", CubicBezier(0.64, 0.0, 0.78, 0.0)))
            .or_else(|| curve("ease-out-quint", CubicBezier(0.22, 1.0, 0.36, 1.0)))
            .or_else(|| curve("ease-in-out-quint", CubicBezier(0.83, 0.0, 0.17, 1.0)))
            .or_else(|| curve("ease-in-expo", CubicBezier(0.7, 0.0, 0.84, 0.0)))
            .or_else(|| curve("ease-out-expo", CubicBezier(0.16, 1.0, 0.3, 1.0)))
            .or_else(|| curve("ease-in-out-expo", CubicBezier(0.87, 0.0, 0.13, 1.0)))
            .or_else(|| curve("ease-in-back", CubicBezier(0.36, 0.0, 0.66, -0.56)))
            .or_else(|| curve("ease-out-back", CubicBezier(0.34, 1.56, 0.64, 1.0)))
            .or_else(|| curve("ease-in-out-back", CubicBezier(0.68, -0.6, 0.32, 1.6)))
            .or_else(|| curve("ease-in-sine", CubicBezier(0.12, 0.0, 0.39, 0.0)))
            .or_else(|| curve("ease-out-sine", CubicBezier(0.61, 1.0, 0.88, 1.0)))
            .or_else(|| curve("ease-in-out-sine", CubicBezier(0.37, 0.0, 0.63, 1.0)))
            .or_else(|| curve("ease-in-circ", CubicBezier(0.55, 0.0, 1.0, 0.45)))
            .or_else(|| curve("ease-out-circ", CubicBezier(0.0, 0.55, 0.45, 1.0)))
            .or_else(|| curve("ease-in-out-circ", CubicBezier(0.85, 0.0, 0.15, 1.0)))
            .or_else(|| curve("ease-in-elastic", EasingCurve::EaseInElastic))
            .or_else(|| curve("ease-out-elastic", EasingCurve::EaseOutElastic))
            .or_else(|| curve("ease-in-out-elastic", EasingCurve::EaseInOutElastic))
            .or_else(|| curve("ease-in-bounce", EasingCurve::EaseInBounce))
            .or_else(|| curve("ease-out-bounce", EasingCurve::EaseOutBounce))
            .or_else(|| curve("ease-in-out-bounce", EasingCurve::EaseInOutBounce));
        r.or_else(|| {
            f(&SmolStr::new_static("cubic-bezier"), BuiltinMacroFunction::CubicBezier.into())
        })
    }
}

impl LookupObject for Rc<Enumeration> {
    fn for_each_entry<R>(
        &self,
        _ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
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
        _ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let mut f = |n, e| f(&SmolStr::new_static(n), e);
        let b = |b| LookupResult::from(Callable::Builtin(b));
        None.or_else(|| f("mod", BuiltinMacroFunction::Mod.into()))
            .or_else(|| f("round", b(BuiltinFunction::Round)))
            .or_else(|| f("ceil", b(BuiltinFunction::Ceil)))
            .or_else(|| f("floor", b(BuiltinFunction::Floor)))
            .or_else(|| f("clamp", BuiltinMacroFunction::Clamp.into()))
            .or_else(|| f("abs", BuiltinMacroFunction::Abs.into()))
            .or_else(|| f("sqrt", b(BuiltinFunction::Sqrt)))
            .or_else(|| f("max", BuiltinMacroFunction::Max.into()))
            .or_else(|| f("min", BuiltinMacroFunction::Min.into()))
            .or_else(|| f("sin", b(BuiltinFunction::Sin)))
            .or_else(|| f("cos", b(BuiltinFunction::Cos)))
            .or_else(|| f("tan", b(BuiltinFunction::Tan)))
            .or_else(|| f("asin", b(BuiltinFunction::ASin)))
            .or_else(|| f("acos", b(BuiltinFunction::ACos)))
            .or_else(|| f("atan", b(BuiltinFunction::ATan)))
            .or_else(|| f("atan2", b(BuiltinFunction::ATan2)))
            .or_else(|| f("log", b(BuiltinFunction::Log)))
            .or_else(|| f("ln", b(BuiltinFunction::Ln)))
            .or_else(|| f("pow", b(BuiltinFunction::Pow)))
            .or_else(|| f("exp", b(BuiltinFunction::Exp)))
    }
}

struct SlintInternal;
impl LookupObject for SlintInternal {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let sl = || ctx.current_token.as_ref().map(|t| t.to_source_location());
        let mut f = |n, e: LookupResult| f(&SmolStr::new_static(n), e);
        let b = |b| LookupResult::from(Callable::Builtin(b));
        None.or_else(|| {
            let style = ctx.type_loader.and_then(|tl| tl.compiler_config.style.as_ref());
            f(
                "color-scheme",
                if style.is_some_and(|s| s.ends_with("-light")) {
                    let e = crate::typeregister::BUILTIN.with(|e| e.enums.ColorScheme.clone());
                    Expression::EnumerationValue(e.try_value_from_string("light").unwrap())
                } else if style.is_some_and(|s| s.ends_with("-dark")) {
                    let e = crate::typeregister::BUILTIN.with(|e| e.enums.ColorScheme.clone());
                    Expression::EnumerationValue(e.try_value_from_string("dark").unwrap())
                } else {
                    Expression::FunctionCall {
                        function: BuiltinFunction::ColorScheme.into(),
                        arguments: vec![],
                        source_location: sl(),
                    }
                }
                .into(),
            )
        })
        .or_else(|| {
            f(
                "use-24-hour-format",
                Expression::FunctionCall {
                    function: BuiltinFunction::Use24HourFormat.into(),
                    arguments: vec![],
                    source_location: sl(),
                }
                .into(),
            )
        })
        .or_else(|| f("month-day-count", b(BuiltinFunction::MonthDayCount)))
        .or_else(|| f("month-offset", b(BuiltinFunction::MonthOffset)))
        .or_else(|| f("format-date", b(BuiltinFunction::FormatDate)))
        .or_else(|| f("date-now", b(BuiltinFunction::DateNow)))
        .or_else(|| f("valid-date", b(BuiltinFunction::ValidDate)))
        .or_else(|| f("parse-date", b(BuiltinFunction::ParseDate)))
    }
}

struct ColorFunctions;
impl LookupObject for ColorFunctions {
    fn for_each_entry<R>(
        &self,
        _ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let mut f = |n, m| f(&SmolStr::new_static(n), LookupResult::from(m));
        None.or_else(|| f("rgb", BuiltinMacroFunction::Rgb))
            .or_else(|| f("rgba", BuiltinMacroFunction::Rgb))
            .or_else(|| f("hsv", BuiltinMacroFunction::Hsv))
    }
}

struct BuiltinFunctionLookup;
impl LookupObject for BuiltinFunctionLookup {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        (MathFunctions, ColorFunctions)
            .for_each_entry(ctx, f)
            .or_else(|| f(&SmolStr::new_static("debug"), BuiltinMacroFunction::Debug.into()))
            .or_else(|| {
                f(&SmolStr::new_static("animation-tick"), BuiltinFunction::AnimationTick.into())
            })
    }
}

struct BuiltinNamespaceLookup;
impl LookupObject for BuiltinNamespaceLookup {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let mut f = |s, res| f(&SmolStr::new_static(s), res);
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
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        match self {
            Expression::ElementReference(e) => e.upgrade().unwrap().for_each_entry(ctx, f),
            _ => match self.ty() {
                Type::Struct(s) => {
                    for name in s.fields.keys() {
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
                Type::Float32 | Type::Int32 | Type::Percent => {
                    NumberExpression(self).for_each_entry(ctx, f)
                }
                ty if ty.as_unit_product().is_some() => {
                    NumberWithUnitExpression(self).for_each_entry(ctx, f)
                }
                _ => None,
            },
        }
    }

    fn lookup(&self, ctx: &LookupCtx, name: &SmolStr) -> Option<LookupResult> {
        match self {
            Expression::ElementReference(e) => e.upgrade().unwrap().lookup(ctx, name),
            _ => match self.ty() {
                Type::Struct(s) => s.fields.contains_key(name).then(|| {
                    LookupResult::from(Expression::StructFieldAccess {
                        base: Box::new(self.clone()),
                        name: name.clone(),
                    })
                }),
                Type::String => StringExpression(self).lookup(ctx, name),
                Type::Brush | Type::Color => ColorExpression(self).lookup(ctx, name),
                Type::Image => ImageExpression(self).lookup(ctx, name),
                Type::Array(_) => ArrayExpression(self).lookup(ctx, name),
                Type::Float32 | Type::Int32 | Type::Percent => {
                    NumberExpression(self).lookup(ctx, name)
                }
                ty if ty.as_unit_product().is_some() => {
                    NumberWithUnitExpression(self).lookup(ctx, name)
                }
                _ => None,
            },
        }
    }
}

struct StringExpression<'a>(&'a Expression);
impl LookupObject for StringExpression<'_> {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let member_function = |f: BuiltinFunction| {
            LookupResult::Callable(LookupResultCallable::MemberFunction {
                base: self.0.clone(),
                base_node: ctx.current_token.clone(), // Note that this is not the base_node, but the function's node
                member: LookupResultCallable::Callable(Callable::Builtin(f)).into(),
            })
        };
        let function_call = |f: BuiltinFunction| {
            LookupResult::from(Expression::FunctionCall {
                function: Callable::Builtin(f),
                source_location: ctx.current_token.as_ref().map(|t| t.to_source_location()),
                arguments: vec![self.0.clone()],
            })
        };

        let mut f = |s, res| f(&SmolStr::new_static(s), res);
        None.or_else(|| f("is-float", member_function(BuiltinFunction::StringIsFloat)))
            .or_else(|| f("to-float", member_function(BuiltinFunction::StringToFloat)))
            .or_else(|| f("is-empty", function_call(BuiltinFunction::StringIsEmpty)))
            .or_else(|| f("character-count", function_call(BuiltinFunction::StringCharacterCount)))
            .or_else(|| f("to-lowercase", member_function(BuiltinFunction::StringToLowercase)))
            .or_else(|| f("to-uppercase", member_function(BuiltinFunction::StringToUppercase)))
    }
}
struct ColorExpression<'a>(&'a Expression);
impl LookupObject for ColorExpression<'_> {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let member_function = |f: BuiltinFunction| {
            let base = if f == BuiltinFunction::ColorHsvaStruct && self.0.ty() == Type::Brush {
                Expression::Cast { from: Box::new(self.0.clone()), to: Type::Color }
            } else {
                self.0.clone()
            };
            LookupResult::Callable(LookupResultCallable::MemberFunction {
                base,
                base_node: ctx.current_token.clone(), // Note that this is not the base_node, but the function's node
                member: Box::new(LookupResultCallable::Callable(Callable::Builtin(f))),
            })
        };
        let field_access = |f: &'static str| {
            let base = if self.0.ty() == Type::Brush {
                Expression::Cast { from: Box::new(self.0.clone()), to: Type::Color }
            } else {
                self.0.clone()
            };
            LookupResult::from(Expression::StructFieldAccess {
                base: Box::new(Expression::FunctionCall {
                    function: BuiltinFunction::ColorRgbaStruct.into(),
                    source_location: ctx.current_token.as_ref().map(|t| t.to_source_location()),
                    arguments: vec![base],
                }),
                name: SmolStr::new_static(f),
            })
        };

        let mut f = |s, res| f(&SmolStr::new_static(s), res);
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
impl LookupObject for ImageExpression<'_> {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let field_access = |f: &str| {
            LookupResult::from(Expression::StructFieldAccess {
                base: Box::new(Expression::FunctionCall {
                    function: BuiltinFunction::ImageSize.into(),
                    source_location: ctx.current_token.as_ref().map(|t| t.to_source_location()),
                    arguments: vec![self.0.clone()],
                }),
                name: f.into(),
            })
        };
        let mut f = |s, res| f(&SmolStr::new_static(s), res);
        None.or_else(|| f("width", field_access("width")))
            .or_else(|| f("height", field_access("height")))
    }
}

struct ArrayExpression<'a>(&'a Expression);
impl LookupObject for ArrayExpression<'_> {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let member_function = |f: BuiltinFunction| {
            LookupResult::from(Expression::FunctionCall {
                function: Callable::Builtin(f),
                source_location: ctx.current_token.as_ref().map(|t| t.to_source_location()),
                arguments: vec![self.0.clone()],
            })
        };
        None.or_else(|| {
            f(&SmolStr::new_static("length"), member_function(BuiltinFunction::ArrayLength))
        })
    }
}

/// An expression of type int or float
struct NumberExpression<'a>(&'a Expression);
impl LookupObject for NumberExpression<'_> {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let member_function = |f: BuiltinFunction| {
            LookupResult::Callable(LookupResultCallable::MemberFunction {
                base: self.0.clone(),
                base_node: ctx.current_token.clone(), // Note that this is not the base_node, but the function's node
                member: LookupResultCallable::Callable(Callable::Builtin(f)).into(),
            })
        };

        let mut f2 = |s, res| f(&SmolStr::new_static(s), res);
        None.or_else(|| f2("round", member_function(BuiltinFunction::Round)))
            .or_else(|| f2("ceil", member_function(BuiltinFunction::Ceil)))
            .or_else(|| f2("floor", member_function(BuiltinFunction::Floor)))
            .or_else(|| f2("sqrt", member_function(BuiltinFunction::Sqrt)))
            .or_else(|| f2("asin", member_function(BuiltinFunction::ASin)))
            .or_else(|| f2("acos", member_function(BuiltinFunction::ACos)))
            .or_else(|| f2("atan", member_function(BuiltinFunction::ATan)))
            .or_else(|| f2("log", member_function(BuiltinFunction::Log)))
            .or_else(|| f2("pow", member_function(BuiltinFunction::Pow)))
            .or_else(|| f2("to-fixed", member_function(BuiltinFunction::ToFixed)))
            .or_else(|| f2("to-precision", member_function(BuiltinFunction::ToPrecision)))
            .or_else(|| NumberWithUnitExpression(self.0).for_each_entry(ctx, f))
    }
}

/// An expression of any numerical value with an unit
struct NumberWithUnitExpression<'a>(&'a Expression);
impl LookupObject for NumberWithUnitExpression<'_> {
    fn for_each_entry<R>(
        &self,
        ctx: &LookupCtx,
        f: &mut impl FnMut(&SmolStr, LookupResult) -> Option<R>,
    ) -> Option<R> {
        let member_macro = |f: BuiltinMacroFunction| {
            LookupResult::Callable(LookupResultCallable::MemberFunction {
                base: self.0.clone(),
                base_node: ctx.current_token.clone(), // Note that this is not the base_node, but the function's node
                member: Box::new(LookupResultCallable::Macro(f)),
            })
        };

        let mut f = |s, res| f(&SmolStr::new_static(s), res);
        None.or_else(|| f("mod", member_macro(BuiltinMacroFunction::Mod)))
            .or_else(|| f("clamp", member_macro(BuiltinMacroFunction::Clamp)))
            .or_else(|| f("abs", member_macro(BuiltinMacroFunction::Abs)))
            .or_else(|| f("max", member_macro(BuiltinMacroFunction::Max)))
            .or_else(|| f("min", member_macro(BuiltinMacroFunction::Min)))
            .or_else(|| {
                if self.0.ty() != Type::Angle {
                    return None;
                }
                let member_function = |f: BuiltinFunction| {
                    LookupResult::Callable(LookupResultCallable::MemberFunction {
                        base: self.0.clone(),
                        base_node: ctx.current_token.clone(), // Note that this is not the base_node, but the function's node
                        member: Box::new(LookupResultCallable::Callable(Callable::Builtin(f))),
                    })
                };
                None.or_else(|| f("sin", member_function(BuiltinFunction::Sin)))
                    .or_else(|| f("cos", member_function(BuiltinFunction::Cos)))
                    .or_else(|| f("tan", member_function(BuiltinFunction::Tan)))
            })
    }
}
