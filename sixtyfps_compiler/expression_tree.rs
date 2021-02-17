/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use crate::diagnostics::{BuildDiagnostics, SourceLocation, Spanned, SpannedWithSourceFile};
use crate::langtype::{BuiltinElement, EnumerationValue, Type};
use crate::object_tree::*;
use crate::parser::{NodeOrTokenWithSourceFile, SyntaxNodeWithSourceFile};
use core::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::{Rc, Weak};

/// Reference to a property or callback of a given name within an element.
#[derive(Clone)]
pub struct NamedReference {
    pub element: Weak<RefCell<Element>>,
    pub name: String,
}

fn pretty_print_element_ref(
    f: &mut dyn std::fmt::Write,
    element: &Weak<RefCell<Element>>,
) -> std::fmt::Result {
    match element.upgrade() {
        Some(e) => match e.try_borrow() {
            Ok(el) => write!(f, "{}", el.id),
            Err(_) => write!(f, "<borrowed>"),
        },
        None => write!(f, "<null>"),
    }
}

impl std::fmt::Debug for NamedReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        pretty_print_element_ref(f, &self.element)?;
        write!(f, ".{}", self.name)
    }
}

impl NamedReference {
    pub fn new(element: &ElementRc, name: &str) -> Self {
        Self { element: Rc::downgrade(element), name: name.to_owned() }
    }
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

#[derive(Debug, Clone)]
/// A function built into the run-time
pub enum BuiltinFunction {
    GetWindowScaleFactor,
    Debug,
    Mod,
    Round,
    Ceil,
    Floor,
    Cos,
    Sin,
    Tan,
    ACos,
    ASin,
    ATan,
    SetFocusItem,
    ShowPopupWindow,
    /// the "42".to_float()
    StringToFloat,
    /// the "42".is_float()
    StringIsFloat,
    ImplicitItemSize,
}

#[derive(Debug, Clone)]
/// A builtin function which is handled by the compiler pass
pub enum BuiltinMacroFunction {
    Min,
    Max,
    CubicBezier,
}

impl BuiltinFunction {
    pub fn ty(&self) -> Type {
        match self {
            BuiltinFunction::GetWindowScaleFactor => {
                Type::Function { return_type: Box::new(Type::Float32), args: vec![] }
            }
            BuiltinFunction::Debug => {
                Type::Function { return_type: Box::new(Type::Void), args: vec![Type::String] }
            }
            BuiltinFunction::Mod => Type::Function {
                return_type: Box::new(Type::Int32),
                args: vec![Type::Int32, Type::Int32],
            },
            BuiltinFunction::Round | BuiltinFunction::Ceil | BuiltinFunction::Floor => {
                Type::Function { return_type: Box::new(Type::Int32), args: vec![Type::Float32] }
            }
            BuiltinFunction::Cos | BuiltinFunction::Sin | BuiltinFunction::Tan => {
                Type::Function { return_type: Box::new(Type::Float32), args: vec![Type::Angle] }
            }
            BuiltinFunction::ACos | BuiltinFunction::ASin | BuiltinFunction::ATan => {
                Type::Function { return_type: Box::new(Type::Angle), args: vec![Type::Float32] }
            }
            BuiltinFunction::SetFocusItem => Type::Function {
                return_type: Box::new(Type::Void),
                args: vec![Type::ElementReference],
            },
            BuiltinFunction::ShowPopupWindow => Type::Function {
                return_type: Box::new(Type::Void),
                args: vec![Type::ElementReference],
            },
            BuiltinFunction::StringToFloat => {
                Type::Function { return_type: Box::new(Type::Float32), args: vec![Type::String] }
            }
            BuiltinFunction::StringIsFloat => {
                Type::Function { return_type: Box::new(Type::Bool), args: vec![Type::String] }
            }
            BuiltinFunction::ImplicitItemSize => Type::Function {
                return_type: Box::new(Type::Object {
                    fields: [
                        ("width".to_string(), Type::Length),
                        ("height".to_string(), Type::Length),
                    ]
                    .iter()
                    .cloned()
                    .collect(),
                    name: Some("Size".to_string()),
                }),
                args: vec![Type::ElementReference],
            },
        }
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
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    Percent = "%" -> Percent,

    // Lengths or Coord

    /// Physical pixels
    Phx = "phx" -> Length,
    /// Logical pixels
    Px = "px" -> LogicalLength,
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

    // angles

    /// Degree
    Deg = "deg" -> Angle,
    /// Gradians
    Grad = "grad" -> Angle * 400/360,
    /// Turns
    Turn = "turn" -> Angle * 1/360,
    /// Radians
    Rad = "rad" -> Angle * std::f32::consts::TAU/360.,
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

    /// Special expression that can be the value of a two way binding
    ///
    /// The named reference is what it is aliased to, and the optional Expression is
    /// the initialization expression, if any.  That expression can be a TwoWayBinding as well
    TwoWayBinding(NamedReference, Option<Box<Expression>>),

    /// A string literal. The .0 is the content of the string, without the quotes
    StringLiteral(String),
    /// Number
    NumberLiteral(f64, Unit),
    ///
    BoolLiteral(bool),

    /// Reference to the callback <name> in the <element>
    ///
    /// Note: if we are to separate expression and statement, we probably do not need to have callback reference within expressions
    CallbackReference(NamedReference),

    /// Reference to the callback <name> in the <element>
    PropertyReference(NamedReference),

    /// Reference to a function built into the run-time, implemented natively
    BuiltinFunctionReference(BuiltinFunction),

    /// A MemberFunction expression exists only for a short time, for example for `item.focus()` to be translated to
    /// a regular FunctionCall expression where the base becomes the first argument.
    MemberFunction {
        base: Box<Expression>,
        base_node: NodeOrTokenWithSourceFile,
        member: Box<Expression>,
    },

    /// Reference to a macro understood by the compiler.
    /// These should be transformed to other expression before reaching generation
    BuiltinMacroReference(BuiltinMacroFunction, NodeOrTokenWithSourceFile),

    /// A reference to a specific element. This isn't possible to create in .60 syntax itself, but intermediate passes may generate this
    /// type of expression.
    ElementReference(Weak<RefCell<Element>>),

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

    /// Reference the parameter at the given index of the current function.
    FunctionParameterReference {
        index: usize,
        ty: Type,
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
        arguments: Vec<Expression>,
        source_location: Option<SourceLocation>,
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

    ResourceReference(ResourceReference),

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

    EasingCurve(EasingCurve),

    LinearGradient {
        angle: Box<Expression>,
        /// First expression in the tuple is a color, second expression is the stop position
        stops: Vec<(Expression, Expression)>,
    },

    EnumerationValue(EnumerationValue),

    ReturnStatement(Option<Box<Expression>>),
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
            Expression::TwoWayBinding(NamedReference { element, name }, _) => {
                element.upgrade().unwrap().borrow().lookup_property(name).property_type
            }
            Expression::CallbackReference(NamedReference { element, name }) => {
                element.upgrade().unwrap().borrow().lookup_property(name).property_type
            }
            Expression::PropertyReference(NamedReference { element, name }) => {
                element.upgrade().unwrap().borrow().lookup_property(name).property_type
            }
            Expression::BuiltinFunctionReference(funcref) => funcref.ty(),
            Expression::MemberFunction { member, .. } => member.ty(),
            Expression::BuiltinMacroReference { .. } => Type::Invalid, // We don't know the type
            Expression::ElementReference(_) => Type::ElementReference,
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
            Expression::FunctionParameterReference { ty, .. } => ty.clone(),
            Expression::ObjectAccess { base, name } => match base.ty() {
                Type::Object { fields, .. } => {
                    fields.get(name.as_str()).unwrap_or(&Type::Invalid).clone()
                }
                Type::Component(c) => c.root_element.borrow().lookup_property(name).property_type,
                _ => Type::Invalid,
            },
            Expression::Cast { to, .. } => to.clone(),
            Expression::CodeBlock(sub) => sub.last().map_or(Type::Void, |e| e.ty()),
            Expression::FunctionCall { function, .. } => match function.ty() {
                Type::Function { return_type, .. } => *return_type,
                Type::Callback { return_type, .. } => return_type.map_or(Type::Void, |x| *x),
                _ => Type::Invalid,
            },
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
                if operator_class(*op) != OperatorClass::ArithmeticOp {
                    Type::Bool
                } else if *op == '+' || *op == '-' {
                    let (rhs_ty, lhs_ty) = (rhs.ty(), lhs.ty());
                    if rhs_ty == lhs_ty {
                        rhs_ty
                    } else {
                        Type::Invalid
                    }
                } else {
                    debug_assert!(*op == '*' || *op == '/');
                    let unit_vec = |ty| {
                        if let Type::UnitProduct(v) = ty {
                            v.clone()
                        } else if let Some(u) = ty.default_unit() {
                            vec![(u, 1)]
                        } else {
                            vec![]
                        }
                    };
                    let mut l_units = unit_vec(lhs.ty());
                    let mut r_units = unit_vec(rhs.ty());
                    if *op == '/' {
                        for (_, power) in &mut r_units {
                            *power = -*power;
                        }
                    }
                    for (unit, power) in r_units {
                        if let Some((_, p)) = l_units.iter_mut().find(|(u, _)| *u == unit) {
                            *p += power;
                        } else {
                            l_units.push((unit, power));
                        }
                    }

                    // normalize the vector by removing empty and sorting
                    l_units.retain(|(_, p)| *p != 0);
                    l_units.sort_unstable_by(|(u1, p1), (u2, p2)| match p2.cmp(p1) {
                        std::cmp::Ordering::Equal => u1.cmp(u2),
                        x => x,
                    });

                    if l_units.is_empty() {
                        Type::Float32
                    } else if l_units.len() == 1 && l_units[0].1 == 1 {
                        l_units[0].0.ty()
                    } else {
                        Type::UnitProduct(l_units)
                    }
                }
            }
            Expression::UnaryOp { sub, .. } => sub.ty(),
            Expression::Array { element_ty, .. } => Type::Array(Box::new(element_ty.clone())),
            Expression::Object { ty, .. } => ty.clone(),
            Expression::PathElements { .. } => Type::PathElements,
            Expression::StoreLocalVariable { .. } => Type::Void,
            Expression::ReadLocalVariable { ty, .. } => ty.clone(),
            Expression::EasingCurve(_) => Type::Easing,
            Expression::LinearGradient { .. } => Type::Brush,
            Expression::EnumerationValue(value) => Type::Enumeration(value.enumeration.clone()),
            // invalid because the expression is unreachable
            Expression::ReturnStatement(_) => Type::Invalid,
        }
    }

    /// Call the visitor for each sub-expression.  (note: this function does not recurse)
    pub fn visit(&self, mut visitor: impl FnMut(&Self)) {
        match self {
            Expression::Invalid => {}
            Expression::Uncompiled(_) => {}
            Expression::TwoWayBinding(_, sub) => {
                if let Some(e) = sub.as_deref() {
                    visitor(e)
                }
            }
            Expression::StringLiteral(_) => {}
            Expression::NumberLiteral(_, _) => {}
            Expression::BoolLiteral(_) => {}
            Expression::CallbackReference { .. } => {}
            Expression::PropertyReference { .. } => {}
            Expression::FunctionParameterReference { .. } => {}
            Expression::BuiltinFunctionReference { .. } => {}
            Expression::MemberFunction { base, member, .. } => {
                visitor(&**base);
                visitor(&**member);
            }
            Expression::BuiltinMacroReference { .. } => {}
            Expression::ElementReference(_) => {}
            Expression::ObjectAccess { base, .. } => visitor(&**base),
            Expression::RepeaterIndexReference { .. } => {}
            Expression::RepeaterModelReference { .. } => {}
            Expression::Cast { from, .. } => visitor(&**from),
            Expression::CodeBlock(sub) => {
                sub.iter().for_each(visitor);
            }
            Expression::FunctionCall { function, arguments, source_location: _ } => {
                visitor(&**function);
                arguments.iter().for_each(visitor);
            }
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
                for x in values.values() {
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
            Expression::EasingCurve(_) => {}
            Expression::LinearGradient { angle, stops } => {
                visitor(&angle);
                for (c, s) in stops {
                    visitor(c);
                    visitor(s);
                }
            }
            Expression::EnumerationValue(_) => {}
            Expression::ReturnStatement(expr) => {
                expr.as_deref().map(|expr| visitor(expr));
            }
        }
    }

    pub fn visit_mut(&mut self, mut visitor: impl FnMut(&mut Self)) {
        match self {
            Expression::Invalid => {}
            Expression::Uncompiled(_) => {}
            Expression::TwoWayBinding(_, sub) => {
                if let Some(e) = sub.as_deref_mut() {
                    visitor(e)
                }
            }
            Expression::StringLiteral(_) => {}
            Expression::NumberLiteral(_, _) => {}
            Expression::BoolLiteral(_) => {}
            Expression::CallbackReference { .. } => {}
            Expression::PropertyReference { .. } => {}
            Expression::FunctionParameterReference { .. } => {}
            Expression::BuiltinFunctionReference { .. } => {}
            Expression::MemberFunction { base, member, .. } => {
                visitor(&mut **base);
                visitor(&mut **member);
            }
            Expression::BuiltinMacroReference { .. } => {}
            Expression::ElementReference(_) => {}
            Expression::ObjectAccess { base, .. } => visitor(&mut **base),
            Expression::RepeaterIndexReference { .. } => {}
            Expression::RepeaterModelReference { .. } => {}
            Expression::Cast { from, .. } => visitor(&mut **from),
            Expression::CodeBlock(sub) => {
                sub.iter_mut().for_each(visitor);
            }
            Expression::FunctionCall { function, arguments, source_location: _ } => {
                visitor(&mut **function);
                arguments.iter_mut().for_each(visitor);
            }
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
                for x in values.values_mut() {
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
            Expression::EasingCurve(_) => {}
            Expression::LinearGradient { angle, stops } => {
                visitor(&mut *angle);
                for (c, s) in stops {
                    visitor(c);
                    visitor(s);
                }
            }
            Expression::EnumerationValue(_) => {}
            Expression::ReturnStatement(expr) => {
                expr.as_deref_mut().map(|expr| visitor(expr));
            }
        }
    }

    /// Visit itself and each sub expression recursively
    pub fn visit_recursive(&self, visitor: &mut dyn FnMut(&Self)) {
        visitor(self);
        self.visit(|e| e.visit_recursive(visitor));
    }

    pub fn is_constant(&self) -> bool {
        match self {
            Expression::Invalid => true,
            Expression::Uncompiled(_) => false,
            Expression::TwoWayBinding(..) => false,
            Expression::StringLiteral(_) => true,
            Expression::NumberLiteral(_, _) => true,
            Expression::BoolLiteral(_) => true,
            Expression::CallbackReference { .. } => false,
            Expression::PropertyReference { .. } => false,
            Expression::BuiltinFunctionReference { .. } => false,
            Expression::MemberFunction { .. } => false,
            Expression::ElementReference(_) => false,
            Expression::RepeaterIndexReference { .. } => false,
            Expression::RepeaterModelReference { .. } => false,
            Expression::FunctionParameterReference { .. } => false,
            Expression::BuiltinMacroReference { .. } => false,
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
            Expression::EasingCurve(_) => true,
            Expression::LinearGradient { angle, stops } => {
                angle.is_constant() && stops.iter().all(|(c, s)| c.is_constant() && s.is_constant())
            }
            Expression::EnumerationValue(_) => true,
            Expression::ReturnStatement(expr) => {
                expr.as_ref().map_or(true, |expr| expr.is_constant())
            }
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
        if ty == target_type
            || target_type == Type::Void
            || target_type == Type::Invalid
            || ty == Type::Invalid
        {
            self
        } else if ty.can_convert(&target_type) {
            let from = match (ty, &target_type) {
                (Type::Length, Type::LogicalLength) => Expression::BinaryExpression {
                    lhs: Box::new(self),
                    rhs: Box::new(Expression::FunctionCall {
                        function: Box::new(Expression::BuiltinFunctionReference(
                            BuiltinFunction::GetWindowScaleFactor,
                        )),
                        arguments: vec![],
                        source_location: Some(node.to_source_location()),
                    }),
                    op: '/',
                },
                (Type::LogicalLength, Type::Length) => Expression::BinaryExpression {
                    lhs: Box::new(self),
                    rhs: Box::new(Expression::FunctionCall {
                        function: Box::new(Expression::BuiltinFunctionReference(
                            BuiltinFunction::GetWindowScaleFactor,
                        )),
                        arguments: vec![],
                        source_location: Some(node.to_source_location()),
                    }),
                    op: '*',
                },
                (Type::Percent, Type::Float32) => Expression::BinaryExpression {
                    lhs: Box::new(self),
                    rhs: Box::new(Expression::NumberLiteral(0.01, Unit::None)),
                    op: '*',
                },
                (Type::Object { fields: ref a, .. }, Type::Object { fields: b, name })
                    if a != b =>
                {
                    if let Expression::Object { mut values, .. } = self {
                        let mut new_values = HashMap::new();
                        for (k, ty) in b {
                            let (k, e) = values.remove_entry(k).map_or_else(
                                || (k.clone(), Expression::default_value_for_type(ty)),
                                |(k, e)| (k, e.maybe_convert_to(ty.clone(), node, diag)),
                            );
                            new_values.insert(k, e);
                        }
                        return Expression::Object { values: new_values, ty: target_type };
                    }
                    let var_name = "tmpobj";
                    let mut new_values = HashMap::new();
                    for (k, ty) in b {
                        let e = if a.contains_key(k) {
                            Expression::ObjectAccess {
                                base: Box::new(Expression::ReadLocalVariable {
                                    name: var_name.into(),
                                    ty: Type::Object { fields: a.clone(), name: name.clone() },
                                }),
                                name: k.clone(),
                            }
                            .maybe_convert_to(ty.clone(), node, diag)
                        } else {
                            Expression::default_value_for_type(ty)
                        };
                        new_values.insert(k.clone(), e);
                    }
                    return Expression::CodeBlock(vec![
                        Expression::StoreLocalVariable {
                            name: var_name.into(),
                            value: Box::new(self),
                        },
                        Expression::Object { values: new_values, ty: target_type },
                    ]);
                }
                (Type::Object { .. }, Type::Component(c)) => {
                    let object_type_for_component = Type::Object {
                        fields: c
                            .root_element
                            .borrow()
                            .property_declarations
                            .iter()
                            .map(|(name, prop_decl)| {
                                (name.clone(), prop_decl.property_type.clone())
                            })
                            .collect(),
                        name: None,
                    };
                    self.maybe_convert_to(object_type_for_component, node, diag)
                }
                _ => self,
            };
            Expression::Cast { from: Box::new(from), to: target_type }
        } else if matches!((&ty, &target_type, &self), (Type::Array(a), Type::Array(b), Expression::Array{..})
            if a.can_convert(b) || **a == Type::Invalid)
        {
            // Special case for converting array literals
            match (self, target_type) {
                (Expression::Array { values, .. }, Type::Array(target_type)) => Expression::Array {
                    values: values
                        .into_iter()
                        .map(|e| e.maybe_convert_to((*target_type).clone(), node, diag))
                        .collect(),
                    element_ty: *target_type,
                },
                _ => unreachable!(),
            }
        } else {
            let mut message = format!("Cannot convert {} to {}", ty, target_type);
            // Explicit error message for unit cnversion
            if let Some(from_unit) = ty.default_unit() {
                if matches!(&target_type, Type::Int32 | Type::Float32 | Type::String) {
                    message = format!(
                        "{}. Divide by 1{} to convert to a plain number.",
                        message, from_unit
                    );
                }
            } else if let Some(to_unit) = target_type.default_unit() {
                if matches!(ty, Type::Int32 | Type::Float32) {
                    if let Expression::NumberLiteral(value, Unit::None) = self {
                        if value == 0. {
                            // Allow conversion from literal 0 to any unit
                            return Expression::NumberLiteral(0., to_unit);
                        }
                    }
                    message = format!(
                        "{}. Use an unit, or multiply by 1{} to convert explicitly.",
                        message, to_unit
                    );
                }
            }
            diag.push_error(message, node);
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
            | Type::Callback { .. }
            | Type::Function { .. }
            | Type::Void
            | Type::ElementReference => Expression::Invalid,
            Type::Float32 => Expression::NumberLiteral(0., Unit::None),
            Type::Int32 => Expression::NumberLiteral(0., Unit::None),
            Type::String => Expression::StringLiteral(String::new()),
            Type::Color => Expression::Cast {
                from: Box::new(Expression::NumberLiteral(0., Unit::None)),
                to: Type::Color,
            },
            Type::Duration => Expression::NumberLiteral(0., Unit::Ms),
            Type::Angle => Expression::NumberLiteral(0., Unit::Deg),
            Type::Length => Expression::NumberLiteral(0., Unit::Phx),
            Type::LogicalLength => Expression::NumberLiteral(0., Unit::Px),
            Type::Percent => Expression::NumberLiteral(100., Unit::Percent),
            // FIXME: Is that correct?
            Type::Resource => {
                Expression::ResourceReference(ResourceReference::AbsolutePath(String::new()))
            }
            Type::Bool => Expression::BoolLiteral(false),
            Type::Model => Expression::Invalid,
            Type::PathElements => Expression::PathElements { elements: Path::Elements(vec![]) },
            Type::Array(element_ty) => {
                Expression::Array { element_ty: (**element_ty).clone(), values: vec![] }
            }
            Type::Object { fields, .. } => Expression::Object {
                ty: ty.clone(),
                values: fields
                    .iter()
                    .map(|(k, v)| (k.clone(), Expression::default_value_for_type(v)))
                    .collect(),
            },
            Type::Easing => Expression::EasingCurve(EasingCurve::default()),
            Type::Brush => Expression::Cast {
                from: Box::new(Expression::default_value_for_type(&Type::Color)),
                to: Type::Brush,
            },
            Type::Enumeration(enumeration) => {
                Expression::EnumerationValue(enumeration.clone().default_value())
            }
            Type::UnitProduct(_) => Expression::Cast {
                from: Box::new(Expression::NumberLiteral(0., Unit::None)),
                to: ty.clone(),
            },
        }
    }

    /// Return true if the expression is a "lvalue" that can be used as the left hand side of a `=` or `+=` or similar
    pub fn is_rw(&self) -> bool {
        match self {
            Expression::PropertyReference(_) => true,
            Expression::ObjectAccess { base, .. } => base.is_rw(),
            Expression::RepeaterModelReference { .. } => true,
            _ => false,
        }
    }
}

/// The expression in the Element::binding hash table
#[derive(Default, Debug, Clone, derive_more::Deref, derive_more::DerefMut)]
pub struct BindingExpression {
    #[deref]
    #[deref_mut]
    pub expression: Expression,
    /// The location of this expression in the source code
    pub span: Option<SourceLocation>,
    /// How deep is this binding declared in the hierarchy. When two binding are conflicting
    /// for the same priority (because of two way binding), the lower priority wins.
    pub priority: i32,
}

impl std::convert::From<Expression> for BindingExpression {
    fn from(expression: Expression) -> Self {
        Self { expression, span: None, priority: 0 }
    }
}

impl BindingExpression {
    pub fn new_uncompiled(node: SyntaxNodeWithSourceFile) -> Self {
        Self {
            expression: Expression::Uncompiled(node.clone()),
            span: Some(node.into()),
            priority: 0,
        }
    }
}

impl SpannedWithSourceFile for BindingExpression {
    fn source_file(&self) -> Option<&crate::diagnostics::SourceFile> {
        self.span.as_ref().and_then(|x| x.source_file())
    }
}

impl Spanned for BindingExpression {
    fn span(&self) -> crate::diagnostics::Span {
        self.span.as_ref().map(|x| x.span()).unwrap_or_default()
    }
}

pub type PathEvents = Vec<lyon_path::Event<lyon_path::math::Point, lyon_path::math::Point>>;

#[derive(Debug, Clone)]
pub enum Path {
    Elements(Vec<PathElement>),
    Events(PathEvents),
}

#[derive(Debug, Clone)]
pub struct PathElement {
    pub element_type: Rc<BuiltinElement>,
    pub bindings: HashMap<String, BindingExpression>,
}

#[derive(Clone, Debug)]
pub enum EasingCurve {
    Linear,
    CubicBezier(f32, f32, f32, f32),
    // CubicBesizerNonConst([Box<Expression>; 4]),
    // Custom(Box<dyn Fn(f32)->f32>),
}

impl Default for EasingCurve {
    fn default() -> Self {
        Self::Linear
    }
}

// The compiler generates ResourceReference::AbsolutePath for all references likg @image-url("foo.png")
// and the resource lowering path may change this to EmbeddedData if configured.
#[derive(Clone, Debug)]
pub enum ResourceReference {
    None,
    AbsolutePath(String),
    EmbeddedData(usize),
}

/// Print the expression as a .60 code (not nessecarily valid .60)
pub fn pretty_print(f: &mut dyn std::fmt::Write, expression: &Expression) -> std::fmt::Result {
    match expression {
        Expression::Invalid => write!(f, "<invalid>"),
        Expression::Uncompiled(u) => write!(f, "{:?}", u),
        Expression::TwoWayBinding(a, b) => {
            write!(f, "<=>{:?}", a)?;
            if let Some(b) = b {
                write!(f, ":")?;
                pretty_print(f, b)?;
            }
            Ok(())
        }
        Expression::StringLiteral(s) => write!(f, "{:?}", s),
        Expression::NumberLiteral(vl, unit) => write!(f, "{}{}", vl, unit),
        Expression::BoolLiteral(b) => write!(f, "{:?}", b),
        Expression::CallbackReference(a) => write!(f, "{:?}", a),
        Expression::PropertyReference(a) => write!(f, "{:?}", a),
        Expression::BuiltinFunctionReference(a) => write!(f, "{:?}", a),
        Expression::MemberFunction { base, base_node: _, member } => {
            pretty_print(f, base)?;
            write!(f, ".")?;
            pretty_print(f, member)
        }
        Expression::BuiltinMacroReference(a, _) => write!(f, "{:?}", a),
        Expression::ElementReference(a) => write!(f, "{:?}", a),
        Expression::RepeaterIndexReference { element } => pretty_print_element_ref(f, element),
        Expression::RepeaterModelReference { element } => {
            pretty_print_element_ref(f, element)?;
            write!(f, ".@model")
        }
        Expression::FunctionParameterReference { index, ty: _ } => write!(f, "_arg_{}", index),
        Expression::StoreLocalVariable { name, value } => {
            write!(f, "{} = ", name)?;
            pretty_print(f, value)
        }
        Expression::ReadLocalVariable { name, ty: _ } => write!(f, "{}", name),
        Expression::ObjectAccess { base, name } => {
            pretty_print(f, base)?;
            write!(f, ".{}", name)
        }
        Expression::Cast { from, to } => {
            write!(f, "(")?;
            pretty_print(f, from)?;
            write!(f, "/* as {} */)", to)
        }
        Expression::CodeBlock(c) => {
            write!(f, "{{ ")?;
            for e in c {
                pretty_print(f, e)?;
                write!(f, "; ")?;
            }
            write!(f, "}}")
        }
        Expression::FunctionCall { function, arguments, source_location: _ } => {
            pretty_print(f, function)?;
            write!(f, "(")?;
            for e in arguments {
                pretty_print(f, e)?;
                write!(f, ", ")?;
            }
            write!(f, ")")
        }
        Expression::SelfAssignment { lhs, rhs, op } => {
            pretty_print(f, lhs)?;
            write!(f, " {}= ", if *op == '=' { ' ' } else { *op })?;
            pretty_print(f, rhs)
        }
        Expression::BinaryExpression { lhs, rhs, op } => {
            write!(f, "(")?;
            pretty_print(f, lhs)?;
            match *op {
                '=' | '!' => write!(f, " {}= ", op)?,
                _ => write!(f, " {} ", op)?,
            };
            pretty_print(f, rhs)?;
            write!(f, ")")
        }
        Expression::UnaryOp { sub, op } => {
            write!(f, "{}", op)?;
            pretty_print(f, sub)
        }
        Expression::ResourceReference(a) => write!(f, "{:?}", a),
        Expression::Condition { condition, true_expr, false_expr } => {
            write!(f, "if (")?;
            pretty_print(f, condition)?;
            write!(f, ") {{ ")?;
            pretty_print(f, true_expr)?;
            write!(f, " }} else {{ ")?;
            pretty_print(f, false_expr)?;
            write!(f, " }}")
        }
        Expression::Array { element_ty: _, values } => {
            write!(f, "[")?;
            for e in values {
                pretty_print(f, e)?;
                write!(f, ", ")?;
            }
            write!(f, "]")
        }
        Expression::Object { ty: _, values } => {
            write!(f, "{{ ")?;
            for (name, e) in values {
                write!(f, "{}: ", name)?;
                pretty_print(f, e)?;
                write!(f, ", ")?;
            }
            write!(f, " }}")
        }
        Expression::PathElements { elements } => write!(f, "{:?}", elements),
        Expression::EasingCurve(e) => write!(f, "{:?}", e),
        Expression::LinearGradient { angle, stops } => {
            write!(f, "@linear-gradient(")?;
            pretty_print(f, &angle)?;
            for (c, s) in stops {
                write!(f, ", ")?;
                pretty_print(f, &c)?;
                write!(f, "  ")?;
                pretty_print(f, &s)?;
            }
            write!(f, ")")
        }
        Expression::EnumerationValue(e) => match e.enumeration.values.get(e.value as usize) {
            Some(val) => write!(f, "{}.{}", e.enumeration.name, val),
            None => write!(f, "{}.{}", e.enumeration.name, e.value),
        },
        Expression::ReturnStatement(e) => {
            write!(f, "return ")?;
            e.as_ref().map(|e| pretty_print(f, e)).unwrap_or(Ok(()))
        }
    }
}
